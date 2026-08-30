//! A protobuf codec small enough to read in one sitting.
//!
//! Only what the keyboard protocol actually uses: varints, length-delimited
//! submessages, and enough of a generic parser to walk a message whose schema
//! we deliberately do not model.
//!
//! That last part matters. `AppSettings` carries gestures, touch zones and key
//! mappings that this tool has no business rewriting, so it parses them as
//! opaque bytes and hands them back untouched. A firmware revision can add
//! fields and nothing here notices.

use std::collections::BTreeMap;

/// One field's value, still in wire form.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Varint(u64),
    Bytes(Vec<u8>),
    Fixed32([u8; 4]),
    Fixed64([u8; 8]),
}

/// A parsed message: field number -> values, in ascending field order.
///
/// `BTreeMap` is not incidental. This firmware rejects a settings write whose
/// fields are not in ascending order, so the map that holds them sorts them.
pub type Message = BTreeMap<u32, Vec<Value>>;

pub fn encode_varint(mut n: u64, out: &mut Vec<u8>) {
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub fn field_varint(num: u32, value: u64, out: &mut Vec<u8>) {
    encode_varint(((num as u64) << 3) | 0, out);
    encode_varint(value, out);
}

pub fn field_bytes(num: u32, value: &[u8], out: &mut Vec<u8>) {
    encode_varint(((num as u64) << 3) | 2, out);
    encode_varint(value.len() as u64, out);
    out.extend_from_slice(value);
}

pub fn varint_field(num: u32, value: u64) -> Vec<u8> {
    let mut v = Vec::new();
    field_varint(num, value, &mut v);
    v
}

pub fn bytes_field(num: u32, value: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    field_bytes(num, value, &mut v);
    v
}

fn read_varint(data: &[u8], i: &mut usize) -> Option<u64> {
    let mut shift = 0u32;
    let mut out = 0u64;
    loop {
        let b = *data.get(*i)?;
        *i += 1;
        out |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(out);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// Parse without a schema. Unknown fields are preserved as-is, which is the
/// whole point.
pub fn parse(data: &[u8]) -> Option<Message> {
    let mut out: Message = BTreeMap::new();
    let mut i = 0usize;
    while i < data.len() {
        let key = read_varint(data, &mut i)?;
        let num = (key >> 3) as u32;
        let wire = (key & 7) as u8;
        let val = match wire {
            0 => Value::Varint(read_varint(data, &mut i)?),
            2 => {
                let len = read_varint(data, &mut i)? as usize;
                let end = i.checked_add(len)?;
                if end > data.len() {
                    return None;
                }
                let v = data[i..end].to_vec();
                i = end;
                Value::Bytes(v)
            }
            5 => {
                let end = i.checked_add(4)?;
                if end > data.len() {
                    return None;
                }
                let mut b = [0u8; 4];
                b.copy_from_slice(&data[i..end]);
                i = end;
                Value::Fixed32(b)
            }
            1 => {
                let end = i.checked_add(8)?;
                if end > data.len() {
                    return None;
                }
                let mut b = [0u8; 8];
                b.copy_from_slice(&data[i..end]);
                i = end;
                Value::Fixed64(b)
            }
            _ => return None,
        };
        out.entry(num).or_default().push(val);
    }
    Some(out)
}

/// Re-emit a parsed message, in ascending field order.
pub fn serialize(msg: &Message) -> Vec<u8> {
    let mut out = Vec::new();
    for (num, values) in msg {
        for v in values {
            match v {
                Value::Varint(n) => field_varint(*num, *n, &mut out),
                Value::Bytes(b) => field_bytes(*num, b, &mut out),
                Value::Fixed32(b) => {
                    encode_varint(((*num as u64) << 3) | 5, &mut out);
                    out.extend_from_slice(b);
                }
                Value::Fixed64(b) => {
                    encode_varint(((*num as u64) << 3) | 1, &mut out);
                    out.extend_from_slice(b);
                }
            }
        }
    }
    out
}

/// Replace one field, keep everything else byte-for-byte, stay in order.
pub fn replace_field(msg: &Message, field: u32, value: Vec<u8>) -> Vec<u8> {
    let mut copy = msg.clone();
    copy.insert(field, vec![Value::Bytes(value)]);
    serialize(&copy)
}

pub fn first_bytes(msg: &Message, field: u32) -> Option<&Vec<u8>> {
    match msg.get(&field)?.first()? {
        Value::Bytes(b) => Some(b),
        _ => None,
    }
}

pub fn first_varint(msg: &Message, field: u32) -> Option<u64> {
    match msg.get(&field)?.first()? {
        Value::Varint(n) => Some(*n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for n in [0u64, 1, 127, 128, 255, 300, 16384, u32::MAX as u64] {
            let mut buf = Vec::new();
            encode_varint(n, &mut buf);
            let mut i = 0;
            assert_eq!(read_varint(&buf, &mut i), Some(n), "value {}", n);
            assert_eq!(i, buf.len());
        }
    }

    #[test]
    fn parse_then_serialize_is_identity() {
        // field 1 varint, field 2 submessage, field 3 varint
        let mut src = Vec::new();
        field_varint(1, 42, &mut src);
        field_bytes(2, &[0x08, 0x01], &mut src);
        field_varint(3, 168, &mut src);
        let parsed = parse(&src).expect("parses");
        assert_eq!(serialize(&parsed), src);
    }

    #[test]
    fn serialize_sorts_fields_ascending() {
        // The firmware rejects out-of-order fields, so this is load-bearing.
        let mut msg: Message = BTreeMap::new();
        msg.insert(4, vec![Value::Varint(4)]);
        msg.insert(1, vec![Value::Varint(1)]);
        msg.insert(2, vec![Value::Varint(2)]);
        let out = serialize(&msg);
        let reparsed = parse(&out).unwrap();
        let order: Vec<u32> = reparsed.keys().copied().collect();
        assert_eq!(order, vec![1, 2, 4]);
        // and the bytes themselves must be in that order
        assert_eq!(out[0] >> 3, 1);
    }

    #[test]
    fn replace_keeps_unknown_fields() {
        let mut src = Vec::new();
        field_bytes(2, b"gestures", &mut src);
        field_bytes(3, b"touchzone", &mut src);
        field_bytes(4, b"old-backlight", &mut src);
        field_bytes(5, b"keyboard", &mut src);
        let parsed = parse(&src).unwrap();
        let out = replace_field(&parsed, 4, b"new".to_vec());
        let back = parse(&out).unwrap();
        assert_eq!(first_bytes(&back, 2).unwrap(), b"gestures");
        assert_eq!(first_bytes(&back, 3).unwrap(), b"touchzone");
        assert_eq!(first_bytes(&back, 4).unwrap(), b"new");
        assert_eq!(first_bytes(&back, 5).unwrap(), b"keyboard");
    }

    #[test]
    fn truncated_input_is_rejected_not_panicked() {
        let mut src = Vec::new();
        field_bytes(2, &[1, 2, 3, 4], &mut src);
        for cut in 1..src.len() {
            let _ = parse(&src[..cut]); // must not panic
        }
    }
}
