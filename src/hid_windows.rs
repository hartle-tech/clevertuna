//! Windows HID, straight to hid.dll and setupapi.dll.
//!
//! Same reasoning as macOS: one platform should not cost the whole project its
//! zero-dependency promise. Enumeration walks the HID device interface class,
//! opens each candidate, and keeps the one whose usage page is the vendor's.
//!
//! Windows reports are fixed length: writes must be padded to the output report
//! size and reads always return the full input report size, first byte the
//! report ID — which matches what the shared framing code already expects.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::time::Duration;

type Handle = *mut c_void;
type Bool = i32;
type Dword = u32;
type UShort = u16;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const GENERIC_READ: Dword = 0x8000_0000;
const GENERIC_WRITE: Dword = 0x4000_0000;
const FILE_SHARE_READ: Dword = 1;
const FILE_SHARE_WRITE: Dword = 2;
const OPEN_EXISTING: Dword = 3;
const FILE_FLAG_OVERLAPPED: Dword = 0x4000_0000;
const DIGCF_PRESENT: Dword = 0x02;
const DIGCF_DEVICEINTERFACE: Dword = 0x10;
const WAIT_OBJECT_0: Dword = 0;
const WAIT_TIMEOUT: Dword = 258;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct SpDeviceInterfaceData {
    cb_size: Dword,
    interface_class_guid: Guid,
    flags: Dword,
    reserved: usize,
}

#[repr(C)]
struct HiddAttributes {
    size: Dword,
    vendor_id: UShort,
    product_id: UShort,
    version_number: UShort,
}

#[repr(C)]
struct HidpCaps {
    usage: UShort,
    usage_page: UShort,
    input_report_byte_length: UShort,
    output_report_byte_length: UShort,
    feature_report_byte_length: UShort,
    reserved: [UShort; 17],
    number_link_collection_nodes: UShort,
    number_input_button_caps: UShort,
    number_input_value_caps: UShort,
    number_input_data_indices: UShort,
    number_output_button_caps: UShort,
    number_output_value_caps: UShort,
    number_output_data_indices: UShort,
    number_feature_button_caps: UShort,
    number_feature_value_caps: UShort,
    number_feature_data_indices: UShort,
}

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    h_event: Handle,
}

#[link(name = "hid")]
extern "system" {
    fn HidD_GetHidGuid(guid: *mut Guid);
    fn HidD_GetAttributes(device: Handle, attrs: *mut HiddAttributes) -> Bool;
    fn HidD_GetPreparsedData(device: Handle, data: *mut *mut c_void) -> Bool;
    fn HidD_FreePreparsedData(data: *mut c_void) -> Bool;
    fn HidP_GetCaps(preparsed: *mut c_void, caps: *mut HidpCaps) -> i32;
}

#[link(name = "setupapi")]
extern "system" {
    fn SetupDiGetClassDevsW(
        class_guid: *const Guid,
        enumerator: *const u16,
        hwnd: *mut c_void,
        flags: Dword,
    ) -> Handle;
    fn SetupDiEnumDeviceInterfaces(
        info_set: Handle,
        dev_info: *mut c_void,
        class_guid: *const Guid,
        index: Dword,
        data: *mut SpDeviceInterfaceData,
    ) -> Bool;
    fn SetupDiGetDeviceInterfaceDetailW(
        info_set: Handle,
        data: *mut SpDeviceInterfaceData,
        detail: *mut c_void,
        detail_size: Dword,
        required: *mut Dword,
        dev_info: *mut c_void,
    ) -> Bool;
    fn SetupDiDestroyDeviceInfoList(info_set: Handle) -> Bool;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: Dword,
        share: Dword,
        security: *mut c_void,
        creation: Dword,
        flags: Dword,
        template: Handle,
    ) -> Handle;
    fn CloseHandle(h: Handle) -> Bool;
    fn WriteFile(
        h: Handle,
        buf: *const u8,
        len: Dword,
        written: *mut Dword,
        overlapped: *mut Overlapped,
    ) -> Bool;
    fn ReadFile(
        h: Handle,
        buf: *mut u8,
        len: Dword,
        read: *mut Dword,
        overlapped: *mut Overlapped,
    ) -> Bool;
    fn CreateEventW(attrs: *mut c_void, manual: Bool, initial: Bool, name: *const u16) -> Handle;
    fn WaitForSingleObject(h: Handle, ms: Dword) -> Dword;
    fn GetOverlappedResult(h: Handle, ov: *mut Overlapped, count: *mut Dword, wait: Bool) -> Bool;
    fn CancelIo(h: Handle) -> Bool;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Found {
    pub path: String,
    pub description: String,
}

struct Caps {
    input_len: usize,
    output_len: usize,
    usage_page: u16,
    usage: u16,
    vendor: u16,
    product: u16,
}

fn caps_of(handle: Handle) -> Option<Caps> {
    unsafe {
        let mut attrs = HiddAttributes {
            size: std::mem::size_of::<HiddAttributes>() as Dword,
            vendor_id: 0,
            product_id: 0,
            version_number: 0,
        };
        if HidD_GetAttributes(handle, &mut attrs) == 0 {
            return None;
        }
        let mut pre: *mut c_void = std::ptr::null_mut();
        if HidD_GetPreparsedData(handle, &mut pre) == 0 {
            return None;
        }
        let mut caps: HidpCaps = std::mem::zeroed();
        let ok = HidP_GetCaps(pre, &mut caps);
        HidD_FreePreparsedData(pre);
        if ok < 0 {
            return None;
        }
        Some(Caps {
            input_len: caps.input_report_byte_length as usize,
            output_len: caps.output_report_byte_length as usize,
            usage_page: caps.usage_page,
            usage: caps.usage,
            vendor: attrs.vendor_id,
            product: attrs.product_id,
        })
    }
}

fn open_path(path: &str) -> Handle {
    unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            std::ptr::null_mut(),
        )
    }
}

/// Walk the HID interface class and keep the vendor's configuration interface.
pub fn find(vendor: u16) -> Vec<Found> {
    let mut out = Vec::new();
    unsafe {
        let mut guid = Guid { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
        HidD_GetHidGuid(&mut guid);
        let set = SetupDiGetClassDevsW(
            &guid,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut index = 0u32;
        loop {
            let mut data = SpDeviceInterfaceData {
                cb_size: std::mem::size_of::<SpDeviceInterfaceData>() as Dword,
                interface_class_guid: guid,
                flags: 0,
                reserved: 0,
            };
            if SetupDiEnumDeviceInterfaces(set, std::ptr::null_mut(), &guid, index, &mut data) == 0 {
                break;
            }
            index += 1;
            let mut needed: Dword = 0;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &mut data,
                std::ptr::null_mut(),
                0,
                &mut needed,
                std::ptr::null_mut(),
            );
            if needed == 0 {
                continue;
            }
            let mut buf = vec![0u8; needed as usize];
            // SP_DEVICE_INTERFACE_DETAIL_DATA_W: cbSize then WCHAR path[]
            let cb: Dword = if std::mem::size_of::<usize>() == 8 { 8 } else { 6 };
            std::ptr::write(buf.as_mut_ptr() as *mut Dword, cb);
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &mut data,
                buf.as_mut_ptr() as *mut c_void,
                needed,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                continue;
            }
            let wpath = buf.as_ptr().add(4) as *const u16;
            let mut len = 0usize;
            while *wpath.add(len) != 0 {
                len += 1;
            }
            let path = String::from_utf16_lossy(std::slice::from_raw_parts(wpath, len));
            let h = open_path(&path);
            if h == INVALID_HANDLE_VALUE {
                continue;
            }
            if let Some(c) = caps_of(h) {
                if c.vendor == vendor && c.usage_page == 0xFF00 && c.usage == 1 {
                    out.push(Found {
                        path: path.clone(),
                        description: format!("{:04X}:{:04X}", c.vendor, c.product),
                    });
                }
            }
            CloseHandle(h);
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    out
}

pub struct WinHid {
    handle: Handle,
    input_len: usize,
    output_len: usize,
    event: Handle,
}

unsafe impl Send for WinHid {}

/// Both handles are closed when the device goes out of scope.
///
/// This was a `close()` nothing ever called — which for a one-shot command
/// costs nothing, because the process exiting reclaims them, but leaks a handle
/// and an event per open in anything that runs on, such as the terminal
/// interface. `WinHid` is neither `Clone` nor `Copy` and is only ever
/// constructed after both handles are valid, so there is exactly one drop per
/// pair.
impl Drop for WinHid {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
            CloseHandle(self.handle);
        }
    }
}

impl WinHid {
    pub fn open(vendor: u16, path: &str) -> Result<WinHid, String> {
        let chosen = if path.is_empty() {
            find(vendor)
                .into_iter()
                .next()
                .map(|f| f.path)
                .ok_or_else(|| "no Clevetura configuration interface found".to_string())?
        } else {
            path.to_string()
        };
        let handle = open_path(&chosen);
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("cannot open {}", chosen));
        }
        let caps = caps_of(handle).ok_or_else(|| "cannot read HID capabilities".to_string())?;
        let event = unsafe {
            CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null())
        };
        Ok(WinHid {
            handle,
            input_len: caps.input_len.max(super::transport::PACKET_SIZE),
            output_len: caps.output_len.max(super::transport::PACKET_SIZE),
            event,
        })
    }

    /// Writes must be exactly the output report length, so pad.
    pub fn write_report(&mut self, data: &[u8]) -> Result<(), String> {
        let mut buf = data.to_vec();
        buf.resize(self.output_len, 0);
        unsafe {
            let mut ov: Overlapped = std::mem::zeroed();
            ov.h_event = self.event;
            let mut written: Dword = 0;
            let ok = WriteFile(
                self.handle,
                buf.as_ptr(),
                buf.len() as Dword,
                &mut written,
                &mut ov,
            );
            if ok == 0 {
                // overlapped writes complete asynchronously; wait for it
                if WaitForSingleObject(self.event, 1000) != WAIT_OBJECT_0 {
                    CancelIo(self.handle);
                    return Err("HID write timed out".into());
                }
                if GetOverlappedResult(self.handle, &mut ov, &mut written, 1) == 0 {
                    return Err("HID write failed".into());
                }
            }
            Ok(())
        }
    }

    pub fn read_report(&mut self, timeout: Duration) -> Vec<u8> {
        unsafe {
            let mut buf = vec![0u8; self.input_len];
            let mut ov: Overlapped = std::mem::zeroed();
            ov.h_event = self.event;
            let mut read: Dword = 0;
            let ok = ReadFile(
                self.handle,
                buf.as_mut_ptr(),
                buf.len() as Dword,
                &mut read,
                &mut ov,
            );
            if ok == 0 {
                let ms = timeout.as_millis().min(Dword::MAX as u128) as Dword;
                match WaitForSingleObject(self.event, ms) {
                    WAIT_OBJECT_0 => {
                        if GetOverlappedResult(self.handle, &mut ov, &mut read, 0) == 0 {
                            return Vec::new();
                        }
                    }
                    WAIT_TIMEOUT => {
                        CancelIo(self.handle);
                        return Vec::new();
                    }
                    _ => {
                        CancelIo(self.handle);
                        return Vec::new();
                    }
                }
            }
            buf.truncate(read as usize);
            buf
        }
    }
}
