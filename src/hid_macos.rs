//! macOS HID, straight to IOKit.
//!
//! There is no hidraw here, and pulling in a HID crate would end the
//! zero-dependency promise for one platform. IOKit's HID manager is a small,
//! stable C API, so this binds the handful of calls it needs directly.
//!
//! The report layout is identical to Linux: the first byte is the report ID,
//! and IOKit wants it passed separately, so it is peeled off on send and put
//! back on receive.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

type CFIndex = isize;
type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFSetRef = *const c_void;
type CFRunLoopRef = *const c_void;
type IOHIDManagerRef = *const c_void;
type IOHIDDeviceRef = *const c_void;
type IOReturn = i32;

const K_IORETURN_SUCCESS: IOReturn = 0;
const K_IOHID_REPORT_TYPE_OUTPUT: u32 = 1;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_IOHID_OPTIONS_TYPE_NONE: u32 = 0;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;
    fn IOHIDManagerScheduleWithRunLoop(
        manager: IOHIDManagerRef,
        run_loop: CFRunLoopRef,
        mode: CFStringRef,
    );
    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    #[allow(dead_code)] // Part of the IOKit surface this module declares.
    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
    fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        report_type: u32,
        report_id: CFIndex,
        report: *const u8,
        length: CFIndex,
    ) -> IOReturn;
    fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        length: CFIndex,
        callback: extern "C" fn(*mut c_void, IOReturn, *mut c_void, u32, u32, *mut u8, CFIndex),
        context: *mut c_void,
    );
    fn IOHIDDeviceScheduleWithRunLoop(
        device: IOHIDDeviceRef,
        run_loop: CFRunLoopRef,
        mode: CFStringRef,
    );
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFAllocatorDefault: CFAllocatorRef;
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        cstr: *const u8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFDictionaryCreateMutable(
        alloc: CFAllocatorRef,
        capacity: CFIndex,
        key_cb: *const c_void,
        value_cb: *const c_void,
    ) -> *mut c_void;
    fn CFDictionarySetValue(dict: *mut c_void, key: *const c_void, value: *const c_void);
    fn CFNumberCreate(alloc: CFAllocatorRef, the_type: CFIndex, value_ptr: *const c_void)
        -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, the_type: CFIndex, value_ptr: *mut c_void) -> bool;
    fn CFSetGetCount(set: CFSetRef) -> CFIndex;
    fn CFSetGetValues(set: CFSetRef, values: *mut CFTypeRef);
    fn CFRelease(cf: CFTypeRef);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source: bool) -> i32;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
}

const K_CF_NUMBER_SINT32: CFIndex = 3;

fn cfstr(s: &str) -> CFStringRef {
    let c = format!("{}\0", s);
    unsafe { CFStringCreateWithCString(kCFAllocatorDefault, c.as_ptr(), K_CF_STRING_ENCODING_UTF8) }
}

fn cfnum(v: i32) -> CFTypeRef {
    unsafe { CFNumberCreate(kCFAllocatorDefault, K_CF_NUMBER_SINT32, &v as *const i32 as *const c_void) }
}

fn device_int(dev: IOHIDDeviceRef, key: &str) -> Option<i32> {
    unsafe {
        let k = cfstr(key);
        let prop = IOHIDDeviceGetProperty(dev, k);
        CFRelease(k);
        if prop.is_null() {
            return None;
        }
        let mut out: i32 = 0;
        if CFNumberGetValue(prop, K_CF_NUMBER_SINT32, &mut out as *mut i32 as *mut c_void) {
            Some(out)
        } else {
            None
        }
    }
}

/// Everything the input callback needs, kept alive for the device's lifetime.
struct Inbox {
    tx: Sender<Vec<u8>>,
}

extern "C" fn on_input(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    _rtype: u32,
    report_id: u32,
    report: *mut u8,
    length: CFIndex,
) {
    if context.is_null() || report.is_null() || length <= 0 {
        return;
    }
    let inbox = unsafe { &*(context as *const Inbox) };
    let body = unsafe { std::slice::from_raw_parts(report, length as usize) };
    // put the report ID back so the shared framing code sees the same bytes
    // it would see on Linux
    let mut pkt = Vec::with_capacity(body.len() + 1);
    pkt.push(report_id as u8);
    pkt.extend_from_slice(body);
    let _ = inbox.tx.send(pkt);
}

pub struct MacHid {
    device: IOHIDDeviceRef,
    rx: Receiver<Vec<u8>>,
    _inbox: Box<Inbox>,
    _buf: Vec<u8>,
}

// IOKit objects are used only from the calling thread here.
unsafe impl Send for MacHid {}

pub struct Found {
    pub path: String,
    pub description: String,
}

fn matching_dict(vendor: u16, usage_page: u32, usage: u32) -> CFDictionaryRef {
    unsafe {
        let d = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            0,
            &kCFTypeDictionaryKeyCallBacks as *const _ as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const _ as *const c_void,
        );
        for (k, v) in [
            ("VendorID", vendor as i32),
            ("PrimaryUsagePage", usage_page as i32),
            ("PrimaryUsage", usage as i32),
        ] {
            let key = cfstr(k);
            let num = cfnum(v);
            CFDictionarySetValue(d, key, num);
            CFRelease(key);
            CFRelease(num);
        }
        d as CFDictionaryRef
    }
}

fn manager_with_devices(vendor: u16) -> (IOHIDManagerRef, Vec<IOHIDDeviceRef>) {
    unsafe {
        let mgr = IOHIDManagerCreate(kCFAllocatorDefault, K_IOHID_OPTIONS_TYPE_NONE);
        let dict = matching_dict(vendor, 0xFF00, 1);
        IOHIDManagerSetDeviceMatching(mgr, dict);
        CFRelease(dict);
        IOHIDManagerOpen(mgr, K_IOHID_OPTIONS_TYPE_NONE);
        IOHIDManagerScheduleWithRunLoop(mgr, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);
        // let matching settle
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.15, false);
        let set = IOHIDManagerCopyDevices(mgr);
        if set.is_null() {
            return (mgr, Vec::new());
        }
        let n = CFSetGetCount(set) as usize;
        let mut refs: Vec<CFTypeRef> = vec![std::ptr::null(); n];
        CFSetGetValues(set, refs.as_mut_ptr());
        let devices: Vec<IOHIDDeviceRef> = refs.into_iter().filter(|p| !p.is_null()).collect();
        (mgr, devices)
    }
}

pub fn find(vendor: u16) -> Vec<Found> {
    let (_mgr, devices) = manager_with_devices(vendor);
    devices
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let vid = device_int(*d, "VendorID").unwrap_or(0);
            let pid = device_int(*d, "ProductID").unwrap_or(0);
            Found {
                path: format!("iokit:{}", i),
                description: format!("{:04X}:{:04X}", vid, pid),
            }
        })
        .collect()
}

impl MacHid {
    /// `path` is `iokit:<n>` from [`find`], or empty for the first match.
    pub fn open(vendor: u16, path: &str) -> Result<MacHid, String> {
        let idx: usize = path
            .strip_prefix("iokit:")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let (_mgr, devices) = manager_with_devices(vendor);
        let device = *devices
            .get(idx)
            .ok_or_else(|| "no Clevetura configuration interface found".to_string())?;
        unsafe {
            if IOHIDDeviceOpen(device, K_IOHID_OPTIONS_TYPE_NONE) != K_IORETURN_SUCCESS {
                return Err(
                    "cannot open the keyboard — another app may hold it, or macOS \
                     needs Input Monitoring permission for this binary"
                        .into(),
                );
            }
            let (tx, rx) = channel();
            let inbox = Box::new(Inbox { tx });
            let mut buf = vec![0u8; super::transport::PACKET_SIZE];
            IOHIDDeviceRegisterInputReportCallback(
                device,
                buf.as_mut_ptr(),
                buf.len() as CFIndex,
                on_input,
                &*inbox as *const Inbox as *mut c_void,
            );
            IOHIDDeviceScheduleWithRunLoop(device, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode);
            Ok(MacHid {
                device,
                rx,
                _inbox: inbox,
                _buf: buf,
            })
        }
    }

    #[allow(dead_code)] // The CLI exits after one exchange, so the handle is released by the OS; kept for callers that hold a device open.
    pub fn close(&mut self) {
        unsafe {
            IOHIDDeviceClose(self.device, K_IOHID_OPTIONS_TYPE_NONE);
        }
    }

    pub fn write_report(&mut self, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        // IOKit takes the report ID separately from the body
        let id = data[0] as CFIndex;
        let body = &data[1..];
        let r = unsafe {
            IOHIDDeviceSetReport(
                self.device,
                K_IOHID_REPORT_TYPE_OUTPUT,
                id,
                body.as_ptr(),
                body.len() as CFIndex,
            )
        };
        if r == K_IORETURN_SUCCESS {
            Ok(())
        } else {
            Err(format!("IOHIDDeviceSetReport failed: 0x{:08X}", r))
        }
    }

    /// Input reports arrive on the run loop, so pump it while waiting.
    pub fn read_report(&mut self, timeout: Duration) -> Vec<u8> {
        if let Ok(p) = self.rx.try_recv() {
            return p;
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
            }
            if let Ok(p) = self.rx.try_recv() {
                return p;
            }
            if std::time::Instant::now() >= deadline {
                return Vec::new();
            }
        }
    }
}
