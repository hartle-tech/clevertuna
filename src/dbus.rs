//! A D-Bus client, just big enough to talk to BlueZ.
//!
//! Shelling out to `busctl` cannot work for GATT notifications, and the reason
//! is structural: **BlueZ stops notifying when the D-Bus client that called
//! `StartNotify` disconnects**, and every `busctl` invocation is its own
//! short-lived connection. Polling `ReadValue` instead is not a substitute —
//! measured on a real keyboard, it returns a cycling buffer that never empties
//! (30 KB drained across 600 reads) and duplicates the response, so the CRC
//! never checks out.
//!
//! So: one connection, held open, that calls `StartNotify` and then reads
//! `PropertiesChanged` signals. That is what this file is for. It implements
//! only the slice of the D-Bus wire protocol BlueZ needs — little-endian
//! marshalling, EXTERNAL auth, method calls, and signal reception.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const MSG_METHOD_CALL: u8 = 1;
const MSG_METHOD_RETURN: u8 = 2;
const MSG_ERROR: u8 = 3;
const MSG_SIGNAL: u8 = 4;

// header field codes
const F_PATH: u8 = 1;
const F_INTERFACE: u8 = 2;
const F_MEMBER: u8 = 3;
const F_ERROR_NAME: u8 = 4;
const F_REPLY_SERIAL: u8 = 5;
const F_DESTINATION: u8 = 6;
const F_SIGNATURE: u8 = 8;

pub struct Conn {
    sock: UnixStream,
    serial: u32,
}

/// A received message, decoded only as far as we need it.
pub struct Msg {
    pub kind: u8,
    pub member: String,
    pub path: String,
    pub error_name: String,
    pub reply_serial: u32,
    pub body: Vec<u8>,
    pub signature: String,
}

// ── marshalling ───────────────────────────────────────────────────────────

struct Buf {
    v: Vec<u8>,
}

impl Buf {
    fn new() -> Buf {
        Buf { v: Vec::new() }
    }
    fn align(&mut self, n: usize) {
        while self.v.len() % n != 0 {
            self.v.push(0);
        }
    }
    fn u8(&mut self, x: u8) {
        self.v.push(x);
    }
    fn u32(&mut self, x: u32) {
        self.align(4);
        self.v.extend_from_slice(&x.to_le_bytes());
    }
    fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.v.extend_from_slice(s.as_bytes());
        self.v.push(0);
    }
    fn signature(&mut self, s: &str) {
        self.v.push(s.len() as u8);
        self.v.extend_from_slice(s.as_bytes());
        self.v.push(0);
    }
}

fn read_u32(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}

fn align_up(n: usize, a: usize) -> usize {
    (n + a - 1) / a * a
}

/// Read a `s`/`o` at `off`, returning the value and the new offset.
fn read_string(d: &[u8], off: usize) -> Option<(String, usize)> {
    let off = align_up(off, 4);
    if off + 4 > d.len() {
        return None;
    }
    let n = read_u32(d, off) as usize;
    let start = off + 4;
    let end = start.checked_add(n)?;
    if end + 1 > d.len() {
        return None;
    }
    Some((String::from_utf8_lossy(&d[start..end]).to_string(), end + 1))
}

impl Conn {
    /// Connect to the system bus and authenticate.
    pub fn system() -> Result<Conn, String> {
        let addr = std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
            .ok()
            .and_then(|a| {
                a.split(',')
                    .find_map(|p| p.strip_prefix("unix:path=").map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "/run/dbus/system_bus_socket".to_string());
        let mut sock =
            UnixStream::connect(&addr).map_err(|e| format!("cannot reach the system bus: {}", e))?;
        sock.set_read_timeout(Some(Duration::from_millis(400))).ok();

        // EXTERNAL auth: a NUL byte, then our uid in hex
        let uid = unsafe { libc_getuid() };
        let hex: String = uid
            .to_string()
            .bytes()
            .map(|b| format!("{:02x}", b))
            .collect();
        sock.write_all(&[0u8]).map_err(|e| e.to_string())?;
        sock.write_all(format!("AUTH EXTERNAL {}\r\n", hex).as_bytes())
            .map_err(|e| e.to_string())?;
        let line = read_line(&mut sock)?;
        if !line.starts_with("OK") {
            return Err(format!("bus rejected authentication: {}", line.trim()));
        }
        sock.write_all(b"BEGIN\r\n").map_err(|e| e.to_string())?;

        let mut c = Conn { sock, serial: 0 };
        // Hello is mandatory before anything else
        c.call("org.freedesktop.DBus", "/org/freedesktop/DBus",
               "org.freedesktop.DBus", "Hello", "", &[])?;
        Ok(c)
    }

    fn next_serial(&mut self) -> u32 {
        self.serial += 1;
        self.serial
    }

    /// Send a method call and wait for its reply.
    pub fn call(
        &mut self,
        dest: &str,
        path: &str,
        iface: &str,
        member: &str,
        signature: &str,
        body: &[u8],
    ) -> Result<Msg, String> {
        let serial = self.next_serial();
        let mut h = Buf::new();
        h.u8(b'l');
        h.u8(MSG_METHOD_CALL);
        h.u8(0);
        h.u8(1);
        h.u32(body.len() as u32);
        h.u32(serial);

        // header fields: a(yv)
        let mut f = Buf::new();
        let mut field = |code: u8, sig: &str, write: &mut dyn FnMut(&mut Buf)| {
            f.align(8);
            f.u8(code);
            f.signature(sig);
            write(&mut f);
        };
        {
            let p = path.to_string();
            field(F_PATH, "o", &mut |b: &mut Buf| b.string(&p));
        }
        {
            let i = iface.to_string();
            field(F_INTERFACE, "s", &mut |b: &mut Buf| b.string(&i));
        }
        {
            let m = member.to_string();
            field(F_MEMBER, "s", &mut |b: &mut Buf| b.string(&m));
        }
        {
            let d = dest.to_string();
            field(F_DESTINATION, "s", &mut |b: &mut Buf| b.string(&d));
        }
        if !signature.is_empty() {
            let s = signature.to_string();
            field(F_SIGNATURE, "g", &mut |b: &mut Buf| b.signature(&s));
        }

        h.u32(f.v.len() as u32);
        h.v.extend_from_slice(&f.v);
        h.align(8);
        h.v.extend_from_slice(body);

        self.sock.write_all(&h.v).map_err(|e| e.to_string())?;
        self.sock.flush().ok();

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let m = self.recv(deadline)?;
            if m.kind == MSG_ERROR && m.reply_serial == serial {
                return Err(format!("{}", m.error_name));
            }
            if m.kind == MSG_METHOD_RETURN && m.reply_serial == serial {
                return Ok(m);
            }
            if Instant::now() > deadline {
                return Err("timed out waiting for a reply".into());
            }
        }
    }

    /// Receive the next message, whatever it is.
    pub fn recv(&mut self, deadline: Instant) -> Result<Msg, String> {
        let mut head = [0u8; 16];
        loop {
            match self.sock.read_exact(&mut head) {
                Ok(_) => break,
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if Instant::now() > deadline {
                        return Err("timed out".into());
                    }
                }
                Err(e) => return Err(format!("bus read failed: {}", e)),
            }
        }
        let kind = head[1];
        let body_len = read_u32(&head, 4) as usize;
        let serial_fields = read_u32(&head, 12) as usize;
        let mut fields = vec![0u8; serial_fields];
        read_exact_deadline(&mut self.sock, &mut fields, deadline)?;
        // header is padded to 8 bytes before the body
        let pad = align_up(16 + serial_fields, 8) - (16 + serial_fields);
        if pad > 0 {
            let mut skip = vec![0u8; pad];
            read_exact_deadline(&mut self.sock, &mut skip, deadline)?;
        }
        let mut body = vec![0u8; body_len];
        if body_len > 0 {
            read_exact_deadline(&mut self.sock, &mut body, deadline)?;
        }

        // walk the header fields
        let (mut member, mut path, mut error_name, mut signature) =
            (String::new(), String::new(), String::new(), String::new());
        let mut reply_serial = 0u32;
        let mut i = 0usize;
        while i < fields.len() {
            i = align_up(i, 8);
            if i >= fields.len() {
                break;
            }
            let code = fields[i];
            i += 1;
            // variant signature
            let sl = fields[i] as usize;
            i += 1;
            let sig = String::from_utf8_lossy(&fields[i..i + sl]).to_string();
            i += sl + 1;
            match sig.as_str() {
                "s" | "o" => {
                    let (v, ni) = read_string(&fields, i).ok_or("bad header string")?;
                    i = ni;
                    match code {
                        F_MEMBER => member = v,
                        F_PATH => path = v,
                        F_ERROR_NAME => error_name = v,
                        _ => {}
                    }
                }
                "g" => {
                    let l = fields[i] as usize;
                    let v = String::from_utf8_lossy(&fields[i + 1..i + 1 + l]).to_string();
                    i += l + 2;
                    if code == F_SIGNATURE {
                        signature = v;
                    }
                }
                "u" => {
                    i = align_up(i, 4);
                    let v = read_u32(&fields, i);
                    i += 4;
                    if code == F_REPLY_SERIAL {
                        reply_serial = v;
                    }
                }
                _ => break, // anything else: stop parsing headers, we have enough
            }
        }

        Ok(Msg {
            kind,
            member,
            path,
            error_name,
            reply_serial,
            body,
            signature,
        })
    }

    /// Ask the bus to deliver a class of signals to us.
    pub fn add_match(&mut self, rule: &str) -> Result<(), String> {
        let mut b = Buf::new();
        b.string(rule);
        self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            "s",
            &b.v,
        )?;
        Ok(())
    }
}

fn read_exact_deadline(sock: &mut UnixStream, buf: &mut [u8], deadline: Instant) -> Result<(), String> {
    let mut got = 0usize;
    while got < buf.len() {
        match sock.read(&mut buf[got..]) {
            Ok(0) => return Err("bus closed the connection".into()),
            Ok(n) => got += n,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if Instant::now() > deadline {
                    return Err("timed out mid-message".into());
                }
            }
            Err(e) => return Err(format!("bus read failed: {}", e)),
        }
    }
    Ok(())
}

fn read_line(sock: &mut UnixStream) -> Result<String, String> {
    let mut out = Vec::new();
    let mut b = [0u8; 1];
    for _ in 0..512 {
        match sock.read(&mut b) {
            Ok(1) => {
                out.push(b[0]);
                if out.ends_with(b"\r\n") {
                    return Ok(String::from_utf8_lossy(&out).to_string());
                }
            }
            Ok(_) => break,
            Err(e) => return Err(format!("auth read failed: {}", e)),
        }
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
}

/// Marshal `ay` + `a{sv}` — the argument shape of GattCharacteristic1.WriteValue.
pub fn marshal_write_value(data: &[u8]) -> Vec<u8> {
    let mut b = Buf::new();
    b.u32(data.len() as u32);
    b.v.extend_from_slice(data);
    // empty a{sv}: length 0, then alignment to 8 for the (absent) entries
    b.u32(0);
    b.align(8);
    b.v.truncate(b.v.len()); // keep as-is; the empty array needs no padding after
    b.v
}

/// Pull the `Value` byte array out of a PropertiesChanged body.
///
/// Body is `s a{sv} as`. Rather than fully decode a variant tree, find the
/// "Value" key and read the `ay` that follows it — the shape is fixed and
/// BlueZ only ever sends this one property for a characteristic update.
pub fn value_from_properties_changed(body: &[u8]) -> Option<Vec<u8>> {
    let needle = b"Value";
    let mut i = 0usize;
    while i + needle.len() <= body.len() {
        if &body[i..i + needle.len()] == needle {
            // "Value\0" then variant signature "ay" then the array
            let mut j = i + needle.len() + 1;
            // variant: signature length byte, signature, NUL
            if j >= body.len() {
                return None;
            }
            let sl = body[j] as usize;
            j += 1;
            let sig = &body[j..(j + sl).min(body.len())];
            j += sl + 1;
            if sig != b"ay" {
                i += 1;
                continue;
            }
            j = align_up(j, 4);
            if j + 4 > body.len() {
                return None;
            }
            let n = read_u32(body, j) as usize;
            j += 4;
            if j + n > body.len() {
                return None;
            }
            return Some(body[j..j + n].to_vec());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_helper_matches_dbus_rules() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 4), 12);
    }

    #[test]
    fn strings_marshal_with_length_and_nul() {
        let mut b = Buf::new();
        b.string("hi");
        assert_eq!(b.v, vec![2, 0, 0, 0, b'h', b'i', 0]);
    }

    #[test]
    fn write_value_body_starts_with_the_byte_array_length() {
        let body = marshal_write_value(&[1, 2, 3]);
        assert_eq!(read_u32(&body, 0), 3);
        assert_eq!(&body[4..7], &[1, 2, 3]);
    }

    #[test]
    fn finds_the_value_array_in_a_properties_changed_body() {
        // …"Value\0" + variant "ay" + padded length + bytes
        let mut b: Vec<u8> = Vec::new();
        b.extend_from_slice(b"Value\0");
        b.push(2);
        b.extend_from_slice(b"ay");
        b.push(0);
        while b.len() % 4 != 0 {
            b.push(0);
        }
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&[9, 8, 7, 6]);
        assert_eq!(value_from_properties_changed(&b), Some(vec![9, 8, 7, 6]));
    }

    #[test]
    fn ignores_a_value_key_with_the_wrong_variant_type() {
        let mut b: Vec<u8> = Vec::new();
        b.extend_from_slice(b"Value\0");
        b.push(1);
        b.extend_from_slice(b"s");
        b.push(0);
        assert_eq!(value_from_properties_changed(&b), None);
    }
}
