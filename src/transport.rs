//! Getting bytes to the keyboard.
//!
//! Two ways in, and which one you use decides which slot you are configuring —
//! the keyboard holds a single live connection, so plugging the cable in drops
//! the Bluetooth link.
//!
//! USB: raw HID reports on the vendor interface.
//! BLE: a GATT characteristic, driven through `busctl` so there is no D-Bus
//!      binding to install.

#[cfg(target_os = "linux")]
use std::fs::{File, OpenOptions};
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::{Duration, Instant};

pub const VENDOR_ID: u16 = 0x36F7;
pub const REPORT_OUT: u8 = 0x23;
pub const REPORT_IN: u8 = 0x24;
pub const END_OF_PACKET: u8 = 0x0A;
pub const PACKET_SIZE: usize = 64;
pub const BLE_CHUNK: usize = 56;
#[allow(dead_code)] // the GATT service that owns CHAR_UUID; kept for documentation
pub const SERVICE_UUID: &str = "d0bf1500-c402-424a-80b0-bc7aeced077e";
pub const CHAR_UUID: &str = "d0bf0001-c402-424a-80b0-bc7aeced077e";

#[cfg(target_os = "linux")]
const O_NONBLOCK: i32 = 0o4000;
const READ_SLICE: Duration = Duration::from_millis(50);
const READ_MAX_PACKETS: u32 = 400;

/// How long to wait for a reply to begin.
///
/// Counting empty reads instead of watching the clock is what made a write over
/// Bluetooth report failure while it had in fact landed: four 50 ms reads gave
/// the keyboard 200 ms to answer, and writing settings is a flash operation
/// that takes longer than that. The tool then retried, so a "failed" write was
/// really several successful ones.
const FIRST_REPLY_WAIT_BLE: Duration = Duration::from_secs(4);
const FIRST_REPLY_WAIT_USB: Duration = Duration::from_millis(750);

/// How long to wait for the *rest* of a reply once some of it has arrived.
/// A gap inside a message means the message is over or lost, not that the
/// device is still thinking.
const REPLY_GAP_WAIT: Duration = Duration::from_millis(600);

/// A pause before every exchange.
///
/// Not superstition: this firmware answers a request issued immediately after
/// the previous response with BAD_REQUEST / UNSUPPORTED_REQUEST, and accepts
/// the identical bytes after a short wait. A one-byte colour edit reproduces it
/// as reliably as a three-hundred-byte one, so it is timing, not size.
pub const SETTLE: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum Error {
    #[allow(dead_code)] // Reported through the exit code instead, but kept in the error model.
    NoDevice(String),
    Io(String),
    Protocol(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoDevice(s) => write!(f, "{}", s),
            Error::Io(s) => write!(f, "{}", s),
            Error::Protocol(s) => write!(f, "{}", s),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Usb,
    Ble,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Usb => "usb",
            Kind::Ble => "bluetooth",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Found {
    pub path: String,
    pub hid_name: String,
    pub kind: Kind,
}

/// Locate the configuration interface on any supported platform.
///
/// It is the HID interface whose usage page is 0xFF00. Typing, pointer and
/// touchpad live on the other interfaces, which is exactly why a keyboard can
/// work perfectly while a configuration tool sees nothing at all.
#[cfg(target_os = "macos")]
pub fn find_usb() -> Vec<Found> {
    crate::hid_macos::find(VENDOR_ID)
        .into_iter()
        .map(|f| Found { path: f.path, hid_name: f.description, kind: Kind::Usb })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn find_usb() -> Vec<Found> {
    crate::hid_windows::find(VENDOR_ID)
        .into_iter()
        .map(|f| Found { path: f.path, hid_name: f.description, kind: Kind::Usb })
        .collect()
}

#[cfg(target_os = "linux")]
pub fn find_usb() -> Vec<Found> {
    let mut out = Vec::new();
    let dir = match std::fs::read_dir("/sys/class/hidraw") {
        Ok(d) => d,
        Err(_) => return out,
    };
    let mut names: Vec<String> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        let base = PathBuf::from(format!("/sys/class/hidraw/{}/device", name));
        let real = match std::fs::canonicalize(&base) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let hid_name = real
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let parts: Vec<&str> = hid_name.split(':').collect();
        if parts.len() < 3 {
            continue;
        }
        let vid = u16::from_str_radix(parts[1], 16).unwrap_or(0);
        if vid != VENDOR_ID {
            continue;
        }
        let desc = match std::fs::read(base.join("report_descriptor")) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let (page, usage) = first_collection(&desc);
        if page == 0xFF00 && usage == 1 {
            let kind = if parts[0] == "0005" { Kind::Ble } else { Kind::Usb };
            out.push(Found {
                path: format!("/dev/{}", name),
                hid_name,
                kind,
            });
        }
    }
    out
}

/// Usage page and usage of the first top-level collection in a HID report
/// descriptor. Item prefix: bits 0-1 size, 2-3 type, 4-7 tag.
#[allow(dead_code)] // HID descriptor helper, kept with the parser it belongs to.
fn first_collection(desc: &[u8]) -> (u32, u32) {
    let (mut i, mut page, mut usage, mut depth) = (0usize, 0u32, 0u32, 0i32);
    let mut first: Option<(u32, u32)> = None;
    while i < desc.len() {
        let b = desc[i];
        let size = match b & 3 {
            3 => 4usize,
            n => n as usize,
        };
        let tag = b & 0xFC;
        let mut val = 0u32;
        for k in 0..size {
            if let Some(v) = desc.get(i + 1 + k) {
                val |= (*v as u32) << (8 * k);
            }
        }
        match tag {
            0x04 => page = val,               // Usage Page (global)
            0x08 => {
                if usage == 0 {
                    usage = val;              // Usage (local)
                }
            }
            0xA0 => {                          // Collection
                if depth == 0 && first.is_none() {
                    first = Some((page, usage));
                }
                depth += 1;
                usage = 0;
            }
            0xC0 => {                          // End Collection
                depth -= 1;
                if depth == 0 {
                    usage = 0;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }
    first.unwrap_or((0, 0))
}

/// Find a connected Clevetura GATT characteristic through BlueZ.
#[cfg(target_os = "linux")]
pub fn find_ble() -> Option<String> {
    let tree = Command::new("busctl")
        .args(["tree", "org.bluez", "--list", "--no-pager"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&tree.stdout);
    for path in text.split_whitespace() {
        if !path.contains("/char") {
            continue;
        }
        let got = Command::new("busctl")
            .args([
                "get-property",
                "org.bluez",
                path,
                "org.bluez.GattCharacteristic1",
                "UUID",
            ])
            .output()
            .ok()?;
        if String::from_utf8_lossy(&got.stdout).contains(CHAR_UUID) {
            return Some(path.to_string());
        }
    }
    None
}

/// macOS reaches the same GATT service through CoreBluetooth.
///
/// There is no path to name, so this reports a marker: the keyboard macOS has
/// already connected is the only candidate there can be.
#[cfg(target_os = "macos")]
pub fn find_ble() -> Option<String> {
    if crate::ble_macos::present() {
        Some("corebluetooth".to_string())
    } else {
        None
    }
}

/// Which slot is live right now, without opening a full session.
///
/// Used by the rotation tick, which asks often and must stay cheap.
pub fn live_slot_id() -> String {
    if let Some(f) = find_usb().first() {
        return format!("usb:{}", f.path);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(id) = crate::ble_macos::connected_peripheral_id() {
            return format!("ble:{}", id);
        }
    }
    match find_ble() {
        Some(p) => format!("ble:{}", p),
        None => String::new(),
    }
}

/// Windows has no implementation of this transport, so it answers "no link"
/// rather than making every caller carry a `cfg`. It still finds the keyboard
/// over USB.
#[cfg(target_os = "windows")]
pub fn find_ble() -> Option<String> {
    None
}

enum Io {
    #[cfg(target_os = "linux")]
    Hid(File),
    #[cfg(target_os = "macos")]
    Mac(crate::hid_macos::MacHid),
    #[cfg(target_os = "windows")]
    Win(crate::hid_windows::WinHid),
    #[cfg(target_os = "linux")]
    Ble(BleLink),
    #[cfg(target_os = "macos")]
    MacBle(crate::ble_macos::MacBle),
}

/// A GATT link held open for the whole exchange.
///
/// The connection must outlive the individual calls: BlueZ turns notifications
/// off when the client that enabled them goes away.
#[cfg(target_os = "linux")]
pub struct BleLink {
    conn: crate::dbus::Conn,
    path: String,
    queue: std::collections::VecDeque<Vec<u8>>,
}

pub struct Device {
    io: Io,
    pub kind: Kind,
    #[allow(dead_code)] // Kept so diagnostics can name the device that answered.
    pub path: String,
}

impl Device {
    /// Which slot this connection is: the USB cable, or one of the keyboard's
    /// Bluetooth channels.
    ///
    /// The protocol has no slot field — the slot is simply the connection you
    /// arrived on. Each Bluetooth channel is a separate pairing, so on macOS
    /// each is a different peripheral with its own identifier, which is enough
    /// to answer "is this the same slot as last time?".
    pub fn slot_id(&self) -> String {
        match &self.io {
            #[cfg(target_os = "macos")]
            Io::MacBle(b) => b.slot_id().map(|id| format!("ble:{}", id)).unwrap_or_else(|| "ble".into()),
            _ => match self.kind {
                Kind::Usb => format!("usb:{}", self.path),
                Kind::Ble => format!("ble:{}", self.path),
            },
        }
    }

    #[cfg(target_os = "linux")]
    pub fn open_usb(path: &str) -> Result<Device> {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(path)
            .map_err(|e| Error::Io(format!("cannot open {}: {}", path, e)))?;
        Ok(Device {
            io: Io::Hid(f),
            kind: Kind::Usb,
            path: path.to_string(),
        })
    }

    #[cfg(target_os = "macos")]
    pub fn open_usb(path: &str) -> Result<Device> {
        let d = crate::hid_macos::MacHid::open(VENDOR_ID, path).map_err(Error::Io)?;
        Ok(Device { io: Io::Mac(d), kind: Kind::Usb, path: path.to_string() })
    }

    #[cfg(target_os = "windows")]
    pub fn open_usb(path: &str) -> Result<Device> {
        let d = crate::hid_windows::WinHid::open(VENDOR_ID, path).map_err(Error::Io)?;
        Ok(Device { io: Io::Win(d), kind: Kind::Usb, path: path.to_string() })
    }

    #[cfg(target_os = "macos")]
    pub fn open_ble(_marker: &str) -> Result<Device> {
        let link = crate::ble_macos::MacBle::open().map_err(Error::Io)?;
        Ok(Device {
            io: Io::MacBle(link),
            kind: Kind::Ble,
            path: "corebluetooth".to_string(),
        })
    }

    #[cfg(target_os = "windows")]
    pub fn open_ble(_marker: &str) -> Result<Device> {
        Err(Error::Io(
            "Bluetooth control is not implemented on Windows yet. \
             Connect the keyboard over USB."
                .to_string(),
        ))
    }

    #[cfg(target_os = "linux")]
    pub fn open_ble(char_path: &str) -> Result<Device> {
        let mut conn = crate::dbus::Conn::system().map_err(Error::Io)?;
        conn.add_match(&format!(
            "type='signal',interface='org.freedesktop.DBus.Properties',\
             member='PropertiesChanged',path='{}'",
            char_path
        ))
        .map_err(Error::Io)?;
        // Enable notifications on this connection, and keep it open.
        conn.call("org.bluez", char_path, "org.bluez.GattCharacteristic1",
                  "StartNotify", "", &[])
            .map_err(|e| Error::Io(format!("StartNotify failed: {}", e)))?;
        Ok(Device {
            io: Io::Ble(BleLink { conn, path: char_path.to_string(),
                                  queue: std::collections::VecDeque::new() }),
            kind: Kind::Ble,
            path: char_path.to_string(),
        })
    }

    fn write_packet(&mut self, data: &[u8]) -> Result<()> {
        match &mut self.io {
            #[cfg(target_os = "linux")]
            Io::Hid(f) => f
                .write_all(data)
                .map_err(|e| Error::Io(format!("HID write failed: {}", e))),
            #[cfg(target_os = "macos")]
            Io::Mac(d) => d.write_report(data).map_err(Error::Io),
            #[cfg(target_os = "windows")]
            Io::Win(d) => d.write_report(data).map_err(Error::Io),
            #[cfg(target_os = "macos")]
            Io::MacBle(link) => link.write(data).map_err(Error::Io),
            #[cfg(target_os = "linux")]
            Io::Ble(link) => {
                let body = crate::dbus::marshal_write_value(data);
                let path = link.path.clone();
                link.conn
                    .call("org.bluez", &path, "org.bluez.GattCharacteristic1",
                          "WriteValue", "aya{sv}", &body)
                    .map(|_| ())
                    .map_err(|e| Error::Io(format!("GATT write failed: {}", e)))
            }
        }
    }

    fn read_packet(&mut self) -> Result<Vec<u8>> {
        match &mut self.io {
            #[cfg(target_os = "macos")]
            Io::Mac(d) => Ok(d.read_report(READ_SLICE)),
            #[cfg(target_os = "windows")]
            Io::Win(d) => Ok(d.read_report(READ_SLICE)),
            #[cfg(target_os = "linux")]
            Io::Hid(f) => {
                let deadline = Instant::now() + READ_SLICE;
                let mut buf = vec![0u8; PACKET_SIZE];
                loop {
                    match f.read(&mut buf) {
                        Ok(0) => {}
                        Ok(n) => {
                            buf.truncate(n);
                            return Ok(buf);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => return Err(Error::Io(format!("HID read failed: {}", e))),
                    }
                    if Instant::now() >= deadline {
                        return Ok(Vec::new());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            #[cfg(target_os = "macos")]
            Io::MacBle(link) => Ok(link.read(READ_SLICE)),
            #[cfg(target_os = "linux")]
            Io::Ble(link) => {
                if let Some(p) = link.queue.pop_front() {
                    return Ok(p);
                }
                let deadline = Instant::now() + READ_SLICE;
                loop {
                    match link.conn.recv(deadline) {
                        Ok(m) => {
                            if m.member == "PropertiesChanged" {
                                if let Some(v) = crate::dbus::value_from_properties_changed(&m.body) {
                                    if !v.is_empty() {
                                        return Ok(v);
                                    }
                                }
                            }
                        }
                        Err(_) => return Ok(Vec::new()),
                    }
                    if Instant::now() >= deadline {
                        return Ok(Vec::new());
                    }
                }
            }
        }
    }

    /// ⚠️ DRAIN BEFORE WRITING, ON BLE.
    ///
    /// The characteristic keeps whatever the previous exchange left behind, so
    /// a fresh request starts reading mid-stream and the accumulated base64 is
    /// garbage — measured: a read issued before any write returned 50 stale
    /// bytes. Reading until it comes back empty puts the stream back at a
    /// message boundary.
    fn drain(&mut self) {
        if self.kind != Kind::Ble {
            return;
        }
        for _ in 0..64 {
            match self.read_packet() {
                Ok(p) if !p.is_empty() => continue,
                _ => return,
            }
        }
    }

    fn write_framed(&mut self, payload: &[u8]) -> Result<()> {
        let mut data = payload.to_vec();
        data.push(END_OF_PACKET);
        match self.kind {
            Kind::Ble => {
                for chunk in data.chunks(BLE_CHUNK) {
                    self.write_packet(chunk)?;
                }
            }
            Kind::Usb => {
                for chunk in data.chunks(PACKET_SIZE - 1) {
                    let mut pkt = Vec::with_capacity(PACKET_SIZE);
                    pkt.push(REPORT_OUT);
                    pkt.extend_from_slice(chunk);
                    self.write_packet(&pkt)?;
                }
            }
        }
        Ok(())
    }

    fn read_framed(&mut self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let first_wait = match self.kind {
            Kind::Ble => FIRST_REPLY_WAIT_BLE,
            Kind::Usb => FIRST_REPLY_WAIT_USB,
        };
        let mut deadline = Instant::now() + first_wait;
        for _ in 0..READ_MAX_PACKETS {
            let pkt = self.read_packet()?;
            if pkt.is_empty() {
                if Instant::now() >= deadline {
                    return Err(Error::Protocol(if out.is_empty() {
                        format!("the keyboard did not answer within {:?}", first_wait)
                    } else {
                        format!("the reply stopped after {} bytes", out.len())
                    }));
                }
                continue;
            }
            // Something is coming, so stop waiting on the long clock.
            deadline = Instant::now() + REPLY_GAP_WAIT;
            let body: &[u8] = match self.kind {
                Kind::Ble => &pkt,
                Kind::Usb => {
                    if pkt[0] != REPORT_IN {
                        continue;
                    }
                    &pkt[1..]
                }
            };
            if let Some(end) = body.iter().position(|b| *b == END_OF_PACKET) {
                out.extend_from_slice(&body[..end]);
                return Ok(out);
            }
            out.extend_from_slice(body);
        }
        Err(Error::Protocol("no end-of-packet marker seen".into()))
    }

    /// One protobuf request in, one protobuf response out.
    pub fn request(&mut self, proto: &[u8]) -> Result<Vec<u8>> {
        let mut last = Error::Protocol("no attempt made".into());
        for attempt in 0..3u32 {
            std::thread::sleep(if attempt == 0 { SETTLE } else { SETTLE * 4 });
            self.drain();
            let framed = frame(self.kind, proto);
            if let Err(e) = self.write_framed(&framed) {
                last = e;
                continue;
            }
            match self.read_framed() {
                Ok(raw) => match unframe(self.kind, &raw) {
                    Ok(v) => return Ok(v),
                    Err(e) => last = e,
                },
                Err(e) => last = e,
            }
        }
        Err(last)
    }

}

/// Wrap a request for the transport that will carry it.
pub fn frame(kind: Kind, proto: &[u8]) -> Vec<u8> {
        match kind {
            Kind::Usb => base64_encode(proto).into_bytes(),
            Kind::Ble => {
                let mut with_crc = proto.to_vec();
                with_crc.extend_from_slice(&crc32(proto).to_le_bytes());
                let mut out = vec![b'#'];
                out.extend_from_slice(base64_encode(&with_crc).as_bytes());
                out
            }
        }
    }

/// Unwrap a reply. A pure function of the transport kind, so the framing can
/// be tested without a keyboard on the bench — which is the only way the
/// resynchronisation below could be covered at all.
pub fn unframe(kind: Kind, payload: &[u8]) -> Result<Vec<u8>> {
        match kind {
            Kind::Usb => base64_decode(payload)
                .ok_or_else(|| Error::Protocol("response was not valid base64".into())),
            Kind::Ble => {
                // Resynchronise on the last frame marker.
                //
                // '#' opens a BLE frame and can never occur inside one: it is
                // not in the base64 alphabet. So when the buffer holds the tail
                // of an earlier reply — which happens when an exchange is
                // abandoned and its answer arrives late, without the terminator
                // that would have ended it — the frame actually being answered
                // begins at the *last* '#'.
                //
                // Reading from the first one instead concatenates two replies,
                // and the '#' now sitting in the middle is exactly the byte
                // base64 cannot accept. That surfaced as
                // "response was not valid base64", which names the symptom and
                // hides the cause: nothing was wrong with the encoding.
                // Try every marker, newest first, and take the frame whose CRC
                // checks. The last '#' is *usually* the frame being answered,
                // but not always: when a late reply lands mid-read the buffer
                // can end with a truncated frame, and reading only from the
                // last marker then fails its CRC on every retry — the tail
                // never clears, so three attempts fail identically and a
                // perfectly good reply sitting earlier in the buffer is never
                // looked at. Checking the CRC is what tells the frames apart,
                // so let it do that job.
                let markers: Vec<usize> = payload
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| **b == b'#')
                    .map(|(i, _)| i)
                    .collect();
                if markers.is_empty() {
                    return Err(Error::Protocol("BLE response carried no frame marker".into()));
                }

                let mut best: Option<Vec<u8>> = None;
                let mut why = Error::Protocol("BLE response CRC mismatch".into());
                for start in markers.into_iter().rev() {
                    let Some(body) = base64_decode(&payload[start + 1..]) else {
                        why = Error::Protocol("response was not valid base64".into());
                        continue;
                    };
                    if body.len() < 5 {
                        why = Error::Protocol("BLE response too short".into());
                        continue;
                    }
                    let (proto, crc) = body.split_at(body.len() - 4);
                    let want = u32::from_le_bytes([crc[0], crc[1], crc[2], crc[3]]);
                    if crc32(proto) == want {
                        best = Some(proto.to_vec());
                        break;
                    }
                    why = Error::Protocol("BLE response CRC mismatch".into());
                }

                let Some(proto) = best else {
                    return Err(why);
                };
                Ok(proto)
            }
        }
}

// ── base64 and CRC-32, owned so the binary stays dependency-free ───────────

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn base64_decode(data: &[u8]) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    // ⚠️ NUL padding is normal here, mid-message.
    //
    // A response shorter than the report size still arrives as a full 64-byte
    // HID report, so its tail is zero padding — and because a reply spans
    // several reports, those NULs land *inside* the accumulated base64, not
    // only at the end. A strict decoder rejects the whole reply; a decoder that
    // ignores every unknown byte silently accepts corruption. So drop exactly
    // what the transport is known to insert — NUL and whitespace — and still
    // reject anything else.
    let clean: Vec<u8> = data
        .iter()
        .copied()
        .filter(|c| *c != 0 && !c.is_ascii_whitespace())
        .collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &c in clean.iter() {
        if c == b'=' {
            break;
        }
        let v = val(c)?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// CRC-32 (IEEE), the same polynomial zlib uses.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    /// A late reply can leave a truncated frame at the end of the buffer. The
    /// frame being answered is then *not* the one after the last marker, and
    /// reading only from there fails its CRC on every retry — which is exactly
    /// how a write came back "BLE response CRC mismatch" three times running
    /// while a good reply sat earlier in the same buffer.
    #[test]
    fn a_good_frame_is_found_behind_a_truncated_one() {
        let wanted = b"the reply that matters".to_vec();
        let good = frame(Kind::Ble, &wanted);

        // A whole frame, then the head of another that never finished.
        let mut buffer = good.clone();
        let orphan = frame(Kind::Ble, b"a later answer, cut short");
        buffer.extend_from_slice(&orphan[..orphan.len() / 2]);

        assert_eq!(unframe(Kind::Ble, &buffer).unwrap(), wanted);
    }

    /// And when nothing in the buffer checks out, it still says so rather than
    /// handing back one of the broken frames.
    #[test]
    fn a_buffer_of_only_bad_frames_is_still_an_error() {
        let mut buffer = frame(Kind::Ble, b"hello");
        let n = buffer.len();
        buffer[n - 2] ^= 0x01; // corrupt the CRC
        let mut two = buffer.clone();
        two.extend_from_slice(&buffer);
        assert!(unframe(Kind::Ble, &two).is_err());
    }

    use super::*;

    /// Build the bytes a BLE reply arrives as: `#` + base64(proto + crc).
    fn ble_frame(proto: &[u8]) -> Vec<u8> {
        let mut body = proto.to_vec();
        body.extend_from_slice(&crc32(proto).to_le_bytes());
        let mut out = b"#".to_vec();
        out.extend_from_slice(base64_encode(&body).as_bytes());
        out
    }

    #[test]
    fn a_ble_reply_decodes() {
        assert_eq!(unframe(Kind::Ble, &ble_frame(b"hello")).unwrap(), b"hello");
    }

    #[test]
    fn a_late_reply_left_in_the_buffer_does_not_poison_the_next_one() {
        // The defect this guards: an abandoned exchange's answer arrives with
        // no terminator, the next read appends the real reply to it, and the
        // '#' now sitting in the middle is rejected as invalid base64 — which
        // names the encoding and hides the cause.
        let mut buf = b"#c3RhbGU".to_vec();      // a truncated earlier reply
        buf.extend_from_slice(&ble_frame(b"fresh"));
        assert_eq!(unframe(Kind::Ble, &buf).unwrap(), b"fresh");
    }

    #[test]
    fn a_reply_with_no_frame_marker_is_named_as_such() {
        let err = unframe(Kind::Ble, b"no marker here").unwrap_err().to_string();
        assert!(err.contains("frame marker"), "got: {}", err);
    }

    #[test]
    fn a_corrupt_body_is_still_rejected() {
        // Resynchronising must not become "ignore anything unexpected": a
        // decoder that accepts everything accepts corruption.
        let mut buf = ble_frame(b"hello");
        buf.insert(4, b'!');
        assert!(unframe(Kind::Ble, &buf).is_err());
    }

    #[test]
    fn a_crc_that_does_not_match_is_refused() {
        let mut proto = b"hello".to_vec();
        proto.extend_from_slice(&0u32.to_le_bytes());
        let mut buf = b"#".to_vec();
        buf.extend_from_slice(base64_encode(&proto).as_bytes());
        assert!(unframe(Kind::Ble, &buf).unwrap_err().to_string().contains("CRC"));
    }


    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        for s in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            let enc = base64_encode(s);
            assert_eq!(base64_decode(enc.as_bytes()).unwrap(), s.to_vec());
        }
    }

    #[test]
    fn base64_tolerates_nul_padding_but_not_garbage() {
        // the device pads short reports with NULs, mid-message
        let mut padded = b"Zm9v".to_vec();
        padded.extend_from_slice(&[0, 0, 0]);
        padded.extend_from_slice(b"YmFy");
        assert_eq!(base64_decode(&padded).unwrap(), b"foobar".to_vec());
        // but a genuinely corrupt byte must still be refused
        assert!(base64_decode(b"Zm9v!!YmFy").is_none());
    }

    #[test]
    fn crc32_matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn finds_the_vendor_collection_in_a_descriptor() {
        // Usage Page (Vendor 0xFF00), Usage (1), Collection(Application), End
        let desc = [0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0xC0];
        assert_eq!(first_collection(&desc), (0xFF00, 1));
        // Generic Desktop / Keyboard must NOT look like the config interface
        let kb = [0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0xC0];
        assert_eq!(first_collection(&kb), (0x01, 6));
    }
}
