//! Bluetooth on macOS, through CoreBluetooth.
//!
//! Over Bluetooth the keyboard does not publish its configuration interface as
//! HID. Measured on this Mac, it offers exactly two collections — keyboard
//! (usage page 1, usage 6) and mouse (1/2) — and nothing vendor-defined, which
//! is the same shape Linux reports. So the only route to the settings is the
//! vendor's GATT service, exactly as on Linux, where BlueZ plays the part
//! CoreBluetooth plays here.
//!
//! macOS has already connected and bonded the keyboard as a HID peripheral by
//! the time this runs. That does not lock the rest of it away:
//! `retrieveConnectedPeripheralsWithServices:` returns the same peripheral, and
//! its vendor service and characteristic are readable, writable and notifying.
//!
//! There is no Objective-C in this project and no crate wrapping CoreBluetooth,
//! so this binds the runtime directly — the same choice the D-Bus client makes
//! on Linux. Shipping a small Swift helper beside the binary would have been
//! easier and would have made Bluetooth on macOS depend on a second file being
//! present, which is precisely the failure that made the menu bar depend on
//! SwiftBar.

#![cfg(target_os = "macos")]

use std::collections::VecDeque;
use std::ffi::{c_void, CString};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const SERVICE_UUID: &str = "d0bf1500-c402-424a-80b0-bc7aeced077e";
pub const CHAR_UUID: &str = "d0bf0001-c402-424a-80b0-bc7aeced077e";

type Id = *mut c_void;
type Sel = *const c_void;
type Class = *mut c_void;
type CFStringRef = *const c_void;

const NIL: Id = std::ptr::null_mut();

#[link(name = "CoreBluetooth", kind = "framework")]
extern "C" {}
#[link(name = "Foundation", kind = "framework")]
extern "C" {}

extern "C" {
    fn objc_getClass(name: *const i8) -> Class;
    fn sel_registerName(name: *const i8) -> Sel;
    fn objc_allocateClassPair(superclass: Class, name: *const i8, extra: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_addMethod(cls: Class, name: Sel, imp: *const c_void, types: *const i8) -> bool;
    fn objc_msgSend();

    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source: bool) -> i32;
    static kCFRunLoopDefaultMode: CFStringRef;
}

// objc_msgSend must be called through a prototype that matches the selector
// exactly; on arm64 there is no variadic form to fall back on.
macro_rules! msg {
    ($obj:expr, $sel:expr $(, $arg:expr)*) => {{
        let f: extern "C" fn(Id, Sel $(, msg!(@ty $arg))*) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        f($obj, $sel $(, $arg)*)
    }};
    (@ty $a:expr) => { Id };
}

unsafe fn sel(name: &str) -> Sel {
    let c = CString::new(name).unwrap();
    sel_registerName(c.as_ptr())
}

unsafe fn class(name: &str) -> Class {
    let c = CString::new(name).unwrap();
    objc_getClass(c.as_ptr())
}

/// Read an `NSString` back out into Rust.
unsafe fn nsstring_to_string(s: Id) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let f: extern "C" fn(Id, Sel) -> *const std::os::raw::c_char =
        std::mem::transmute(objc_msgSend as *const c_void);
    let ptr = f(s, sel("UTF8String"));
    if ptr.is_null() {
        return None;
    }
    std::ffi::CStr::from_ptr(ptr).to_str().ok().map(|t| t.to_string())
}

unsafe fn nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap();
    let cls = class("NSString") as Id;
    let f: extern "C" fn(Id, Sel, *const i8) -> Id = std::mem::transmute(objc_msgSend as *const c_void);
    f(cls, sel("stringWithUTF8String:"), c.as_ptr())
}

unsafe fn cbuuid(s: &str) -> Id {
    let cls = class("CBUUID") as Id;
    msg!(cls, sel("UUIDWithString:"), nsstring(s))
}

unsafe fn nsarray_one(item: Id) -> Id {
    let cls = class("NSArray") as Id;
    msg!(cls, sel("arrayWithObject:"), item)
}

unsafe fn array_count(arr: Id) -> usize {
    if arr.is_null() {
        return 0;
    }
    let f: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const c_void);
    f(arr, sel("count"))
}

unsafe fn array_at(arr: Id, i: usize) -> Id {
    let f: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const c_void);
    f(arr, sel("objectAtIndex:"), i)
}

unsafe fn equal(a: Id, b: Id) -> bool {
    let f: extern "C" fn(Id, Sel, Id) -> bool = std::mem::transmute(objc_msgSend as *const c_void);
    f(a, sel("isEqual:"), b)
}

unsafe fn nsdata(bytes: &[u8]) -> Id {
    let cls = class("NSData") as Id;
    let f: extern "C" fn(Id, Sel, *const u8, usize) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    f(cls, sel("dataWithBytes:length:"), bytes.as_ptr(), bytes.len())
}

unsafe fn data_bytes(data: Id) -> Vec<u8> {
    if data.is_null() {
        return Vec::new();
    }
    let len: usize = {
        let f: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const c_void);
        f(data, sel("length"))
    };
    if len == 0 {
        return Vec::new();
    }
    let ptr: *const u8 = {
        let f: extern "C" fn(Id, Sel) -> *const u8 =
            std::mem::transmute(objc_msgSend as *const c_void);
        f(data, sel("bytes"))
    };
    if ptr.is_null() {
        return Vec::new();
    }
    std::slice::from_raw_parts(ptr, len).to_vec()
}

/// What the delegate callbacks report back.
///
/// One connection per process, so the delegate can find this without carrying
/// an instance variable around.
struct Shared {
    powered: bool,
    connected: bool,
    failed: Option<String>,
    services_done: bool,
    characteristic: usize, // the id, as an integer, so it can cross the Mutex
    notifying: bool,
    queue: VecDeque<Vec<u8>>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    powered: false,
    connected: false,
    failed: None,
    services_done: false,
    characteristic: 0,
    notifying: false,
    queue: VecDeque::new(),
});

fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

// MARK: delegate callbacks

extern "C" fn did_update_state(_self: Id, _cmd: Sel, central: Id) {
    unsafe {
        let f: extern "C" fn(Id, Sel) -> isize = std::mem::transmute(objc_msgSend as *const c_void);
        let state = f(central, sel("state"));
        with(|s| s.powered = state == 5); // CBManagerStatePoweredOn
    }
}

extern "C" fn did_connect(_self: Id, _cmd: Sel, _central: Id, _peripheral: Id) {
    with(|s| s.connected = true);
}

extern "C" fn did_fail(_self: Id, _cmd: Sel, _central: Id, _peripheral: Id, _error: Id) {
    with(|s| s.failed = Some("the keyboard refused the connection".into()));
}

extern "C" fn did_disconnect(_self: Id, _cmd: Sel, _central: Id, _peripheral: Id, _error: Id) {
    with(|s| {
        s.connected = false;
        s.failed = Some("the keyboard disconnected".into());
    });
}

extern "C" fn did_discover_services(_self: Id, _cmd: Sel, _peripheral: Id, _error: Id) {
    with(|s| s.services_done = true);
}

extern "C" fn did_discover_characteristics(
    _self: Id,
    _cmd: Sel,
    _peripheral: Id,
    service: Id,
    _error: Id,
) {
    unsafe {
        let want = cbuuid(CHAR_UUID);
        let chars = msg!(service, sel("characteristics"));
        for i in 0..array_count(chars) {
            let c = array_at(chars, i);
            let uuid = msg!(c, sel("UUID"));
            if equal(uuid, want) {
                with(|s| s.characteristic = c as usize);
                return;
            }
        }
    }
}

extern "C" fn did_update_notification_state(
    _self: Id,
    _cmd: Sel,
    _peripheral: Id,
    _characteristic: Id,
    _error: Id,
) {
    with(|s| s.notifying = true);
}

extern "C" fn did_update_value(_self: Id, _cmd: Sel, _peripheral: Id, characteristic: Id, _error: Id) {
    unsafe {
        let data = msg!(characteristic, sel("value"));
        let bytes = data_bytes(data);
        if !bytes.is_empty() {
            with(|s| s.queue.push_back(bytes));
        }
    }
}

unsafe fn delegate_class() -> Class {
    let name = CString::new("ClevertunaBleDelegate").unwrap();
    let existing = objc_getClass(name.as_ptr());
    if !existing.is_null() {
        return existing;
    }
    let cls = objc_allocateClassPair(class("NSObject"), name.as_ptr(), 0);
    let add = |selector: &str, imp: *const c_void, types: &str| {
        let t = CString::new(types).unwrap();
        class_addMethod(cls, sel(selector), imp, t.as_ptr());
    };
    add("centralManagerDidUpdateState:", did_update_state as *const c_void, "v@:@");
    add("centralManager:didConnectPeripheral:", did_connect as *const c_void, "v@:@@");
    add("centralManager:didFailToConnectPeripheral:error:", did_fail as *const c_void, "v@:@@@");
    add("centralManager:didDisconnectPeripheral:error:", did_disconnect as *const c_void, "v@:@@@");
    add("peripheral:didDiscoverServices:", did_discover_services as *const c_void, "v@:@@");
    add("peripheral:didDiscoverCharacteristicsForService:error:",
        did_discover_characteristics as *const c_void, "v@:@@@");
    add("peripheral:didUpdateNotificationStateForCharacteristic:error:",
        did_update_notification_state as *const c_void, "v@:@@@");
    add("peripheral:didUpdateValueForCharacteristic:error:", did_update_value as *const c_void, "v@:@@@");
    objc_registerClassPair(cls);
    cls
}

/// Let CoreBluetooth deliver its callbacks.
///
/// They arrive on the main queue, and this is a synchronous command-line tool,
/// so the run loop only turns when it is asked to.
fn pump(seconds: f64) {
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, seconds, false);
    }
}

fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        pump(0.05);
    }
    done()
}

pub struct MacBle {
    peripheral: Id,
    characteristic: Id,
}

/// The identifier macOS gives the peripheral this process is talking to.
///
/// This is how the keyboard's Bluetooth channels are told apart. Each channel
/// is a separate pairing, so each appears as a different peripheral with a
/// different identifier — which means "am I on the same slot as last time?" has
/// an answer, without the protocol needing a slot field it does not have.
pub fn peripheral_id(p: Id) -> Option<String> {
    unsafe {
        if p.is_null() {
            return None;
        }
        let uuid: Id = msg!(p, sel("identifier"));
        if uuid.is_null() {
            return None;
        }
        let s: Id = msg!(uuid, sel("UUIDString"));
        nsstring_to_string(s)
    }
}

// The peripheral is only ever touched from the thread that made it, which is
// the one running the command.
unsafe impl Send for MacBle {}

impl MacBle {
    /// Attach to the keyboard macOS already has connected.
    ///
    /// **One session per process, reused.** Every call used to build a fresh
    /// `CBCentralManager` and delegate, and nothing tore the previous pair
    /// down — so a command that read the keyboard and then wrote it left two
    /// delegates alive, both appending notifications to the one global queue.
    /// The second exchange then read a reply with another reply spliced into
    /// it, and the frame marker sitting in the middle of that surfaced as
    /// "response was not valid base64": an error that names the encoding and
    /// says nothing about the cause.
    ///
    /// The keyboard grants one connection at a time, so one session per
    /// process is also simply what is true.
    pub fn open() -> Result<MacBle, String> {
        // SAFETY-adjacent, but really a correctness note: this is only touched
        // from the thread running the command, the same one that owns the
        // peripheral.
        static mut LIVE: Option<(Id, Id)> = None;
        unsafe {
            let live = &raw mut LIVE;
            if let Some((peripheral, characteristic)) = *live {
                if with(|s| s.connected) {
                    // Anything still buffered belongs to the exchange that has
                    // already finished, and reading it as part of the next one
                    // is precisely the defect above.
                    with(|s| s.queue.clear());
                    return Ok(MacBle { peripheral, characteristic });
                }
                *live = None;
            }
            with(|s| {
                // The radio's power state is the manager's, not this session's,
                // so it is deliberately not cleared here — doing so would make
                // the shared manager look dead the moment a second command ran.
                s.connected = false;
                s.failed = None;
                s.services_done = false;
                s.characteristic = 0;
                s.notifying = false;
                s.queue = VecDeque::new();
            });

            let central = central_manager(Duration::from_secs(5)).ok_or(
                "Bluetooth is off, or this program has no permission to use it",
            )?;
            let delegate = DELEGATE;

            let service = cbuuid(SERVICE_UUID);
            let found = msg!(
                central,
                sel("retrieveConnectedPeripheralsWithServices:"),
                nsarray_one(service)
            );
            if array_count(found) == 0 {
                return Err("no Clevetura keyboard is connected over Bluetooth".into());
            }
            let peripheral = array_at(found, 0);
            let _: Id = msg!(peripheral, sel("setDelegate:"), delegate);

            // Already connected at the link layer, so this resolves quickly —
            // it is what gives this process a session on the peripheral.
            let _: Id = msg!(central, sel("connectPeripheral:options:"), peripheral, NIL);
            if !wait_until(Duration::from_secs(10), || with(|s| s.connected || s.failed.is_some())) {
                return Err("timed out connecting to the keyboard".into());
            }
            if let Some(e) = with(|s| s.failed.take()) {
                return Err(e);
            }

            let _: Id = msg!(peripheral, sel("discoverServices:"), nsarray_one(service));
            if !wait_until(Duration::from_secs(10), || with(|s| s.services_done)) {
                return Err("the keyboard did not report its services".into());
            }

            let services = msg!(peripheral, sel("services"));
            let mut vendor = NIL;
            for i in 0..array_count(services) {
                let s = array_at(services, i);
                if equal(msg!(s, sel("UUID")), service) {
                    vendor = s;
                    break;
                }
            }
            if vendor.is_null() {
                return Err("this keyboard does not expose the configuration service".into());
            }

            let _: Id = msg!(
                peripheral,
                sel("discoverCharacteristics:forService:"),
                nsarray_one(cbuuid(CHAR_UUID)),
                vendor
            );
            if !wait_until(Duration::from_secs(10), || with(|s| s.characteristic != 0)) {
                return Err("the configuration characteristic never appeared".into());
            }
            let characteristic = with(|s| s.characteristic) as Id;

            // Replies arrive as notifications, so subscribe before anything is
            // sent — the same reason the Linux path calls StartNotify first.
            let f: extern "C" fn(Id, Sel, bool, Id) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            f(peripheral, sel("setNotifyValue:forCharacteristic:"), true, characteristic);
            if !wait_until(Duration::from_secs(5), || with(|s| s.notifying)) {
                return Err("the keyboard would not enable notifications".into());
            }

            *live = Some((peripheral, characteristic));
            Ok(MacBle { peripheral, characteristic })
        }
    }

    /// Which pairing this is — see [`peripheral_id`].
    pub fn slot_id(&self) -> Option<String> {
        peripheral_id(self.peripheral)
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        unsafe {
            let payload = nsdata(data);
            // 0 is CBCharacteristicWriteWithResponse: the keyboard is being
            // reconfigured, so an unacknowledged write is not good enough.
            let f: extern "C" fn(Id, Sel, Id, Id, isize) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            f(
                self.peripheral,
                sel("writeValue:forCharacteristic:type:"),
                payload,
                self.characteristic,
                0,
            );
            pump(0.02);
            Ok(())
        }
    }

    /// The next notification, or empty if none arrives in time.
    pub fn read(&mut self, timeout: Duration) -> Vec<u8> {
        if let Some(p) = with(|s| s.queue.pop_front()) {
            return p;
        }
        wait_until(timeout, || with(|s| !s.queue.is_empty()));
        with(|s| s.queue.pop_front()).unwrap_or_default()
    }
}

/// Whether a keyboard is reachable over Bluetooth, cheaply enough for a bar tick.
///
/// This only asks CoreBluetooth what is already connected; it neither connects
/// nor discovers, so it costs a run-loop turn rather than a session.
/// The identifier of the connected keyboard, if there is one.
///
/// Cheaper than opening a session: it asks the shared manager which peripherals
/// carrying the configuration service are already connected, and names the
/// first. That name is the slot.
pub fn connected_peripheral_id() -> Option<String> {
    unsafe {
        let central = central_manager(Duration::from_secs(2))?;
        let found = msg!(
            central,
            sel("retrieveConnectedPeripheralsWithServices:"),
            nsarray_one(cbuuid(SERVICE_UUID))
        );
        if array_count(found) == 0 {
            return None;
        }
        peripheral_id(array_at(found, 0))
    }
}

pub fn present() -> bool {
    unsafe {
        // A live session is proof enough, and asking again would spend two
        // seconds waiting for a radio that is already answering.
        if with(|s| s.connected) {
            return true;
        }
        let central = match central_manager(Duration::from_secs(2)) {
            Some(c) => c,
            None => return false,
        };
        let found = msg!(
            central,
            sel("retrieveConnectedPeripheralsWithServices:"),
            nsarray_one(cbuuid(SERVICE_UUID))
        );
        array_count(found) > 0
    }
}

/// The one `CBCentralManager` this process owns.
///
/// Built once and kept. Each manager comes with a delegate that appends every
/// notification to the single shared queue, so a second one does not mean a
/// second radio — it means one queue with two writers, and replies interleaved
/// inside it. That is a corruption that surfaces as a base64 error and reads
/// like an encoding fault.
unsafe fn central_manager(wait: Duration) -> Option<Id> {
    static mut CENTRAL: Id = NIL;
    let slot = &raw mut CENTRAL;
    if (*slot).is_null() {
        let delegate: Id = {
            let cls = delegate_class() as Id;
            let obj = msg!(cls, sel("alloc"));
            msg!(obj, sel("init"))
        };
        let cls = class("CBCentralManager") as Id;
        let obj = msg!(cls, sel("alloc"));
        *slot = msg!(obj, sel("initWithDelegate:queue:"), delegate, NIL);
        DELEGATE = delegate;
    }
    if !wait_until(wait, || with(|s| s.powered)) {
        return None;
    }
    Some(*slot)
}

/// The delegate belonging to that manager, so a peripheral can be pointed at it.
static mut DELEGATE: Id = NIL;
