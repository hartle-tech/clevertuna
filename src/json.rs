//! Just enough JSON to read and write a colour scheme.
//!
//! Owning this keeps the binary dependency-free, and a scheme file is a small,
//! closed shape: objects, arrays, numbers, strings, booleans. Parsing is
//! bounded — depth and input size are capped — because these files are meant to
//! be passed between people, and a shared file is untrusted input.

use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const MAX_DEPTH: usize = 32;
pub const MAX_INPUT: usize = 1 << 20; // 1 MiB; a scheme is a few KB

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(key),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= u32::MAX as f64 => {
                Some(*n as u32)
            }
            _ => None,
        }
    }

    // Read by the layout table's tests. Rust's dead-code pass cannot see
    // callers behind #[cfg(test)], so it reports both of these as unused.
    #[allow(dead_code)]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn obj(pairs: Vec<(&str, Json)>) -> Json {
        Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

pub fn parse(input: &str) -> Result<Json, String> {
    if input.len() > MAX_INPUT {
        return Err(format!(
            "input is {} bytes; refusing anything over {}",
            input.len(),
            MAX_INPUT
        ));
    }
    let b = input.as_bytes();
    let mut i = 0usize;
    let v = parse_value(b, &mut i, 0)?;
    skip_ws(b, &mut i);
    if i != b.len() {
        return Err(format!("trailing input at byte {}", i));
    }
    Ok(v)
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn parse_value(b: &[u8], i: &mut usize, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("nesting too deep".into());
    }
    skip_ws(b, i);
    match b.get(*i) {
        None => Err("unexpected end of input".into()),
        Some(b'{') => parse_obj(b, i, depth),
        Some(b'[') => parse_arr(b, i, depth),
        Some(b'"') => Ok(Json::Str(parse_str(b, i)?)),
        Some(b't') => lit(b, i, "true", Json::Bool(true)),
        Some(b'f') => lit(b, i, "false", Json::Bool(false)),
        Some(b'n') => lit(b, i, "null", Json::Null),
        Some(_) => parse_num(b, i),
    }
}

fn lit(b: &[u8], i: &mut usize, word: &str, v: Json) -> Result<Json, String> {
    if b[*i..].starts_with(word.as_bytes()) {
        *i += word.len();
        Ok(v)
    } else {
        Err(format!("invalid literal at byte {}", i))
    }
}

fn parse_obj(b: &[u8], i: &mut usize, depth: usize) -> Result<Json, String> {
    *i += 1; // {
    let mut m = BTreeMap::new();
    skip_ws(b, i);
    if b.get(*i) == Some(&b'}') {
        *i += 1;
        return Ok(Json::Obj(m));
    }
    loop {
        skip_ws(b, i);
        let k = parse_str(b, i)?;
        skip_ws(b, i);
        if b.get(*i) != Some(&b':') {
            return Err(format!("expected ':' at byte {}", i));
        }
        *i += 1;
        let v = parse_value(b, i, depth + 1)?;
        m.insert(k, v);
        skip_ws(b, i);
        match b.get(*i) {
            Some(b',') => *i += 1,
            Some(b'}') => {
                *i += 1;
                return Ok(Json::Obj(m));
            }
            _ => return Err(format!("expected ',' or '}}' at byte {}", i)),
        }
    }
}

fn parse_arr(b: &[u8], i: &mut usize, depth: usize) -> Result<Json, String> {
    *i += 1; // [
    let mut a = Vec::new();
    skip_ws(b, i);
    if b.get(*i) == Some(&b']') {
        *i += 1;
        return Ok(Json::Arr(a));
    }
    loop {
        let v = parse_value(b, i, depth + 1)?;
        a.push(v);
        skip_ws(b, i);
        match b.get(*i) {
            Some(b',') => *i += 1,
            Some(b']') => {
                *i += 1;
                return Ok(Json::Arr(a));
            }
            _ => return Err(format!("expected ',' or ']' at byte {}", i)),
        }
    }
}

fn parse_str(b: &[u8], i: &mut usize) -> Result<String, String> {
    if b.get(*i) != Some(&b'"') {
        return Err(format!("expected string at byte {}", i));
    }
    *i += 1;
    let mut s = String::new();
    loop {
        let c = *b.get(*i).ok_or("unterminated string")?;
        *i += 1;
        match c {
            b'"' => return Ok(s),
            b'\\' => {
                let e = *b.get(*i).ok_or("unterminated escape")?;
                *i += 1;
                match e {
                    b'"' => s.push('"'),
                    b'\\' => s.push('\\'),
                    b'/' => s.push('/'),
                    b'n' => s.push('\n'),
                    b't' => s.push('\t'),
                    b'r' => s.push('\r'),
                    b'b' => s.push('\u{8}'),
                    b'f' => s.push('\u{c}'),
                    b'u' => {
                        let hex = b
                            .get(*i..*i + 4)
                            .ok_or("truncated \\u escape")?;
                        let code = u32::from_str_radix(
                            std::str::from_utf8(hex).map_err(|_| "bad \\u escape")?,
                            16,
                        )
                        .map_err(|_| "bad \\u escape")?;
                        *i += 4;
                        s.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => return Err("unknown escape".into()),
                }
            }
            _ => {
                // pass UTF-8 through untouched
                let start = *i - 1;
                let len = utf8_len(c);
                let end = start + len;
                let chunk = b.get(start..end).ok_or("truncated UTF-8")?;
                s.push_str(std::str::from_utf8(chunk).map_err(|_| "invalid UTF-8")?);
                *i = end;
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

fn parse_num(b: &[u8], i: &mut usize) -> Result<Json, String> {
    let start = *i;
    if b.get(*i) == Some(&b'-') {
        *i += 1;
    }
    while matches!(b.get(*i), Some(c) if c.is_ascii_digit() || *c == b'.' || *c == b'e' || *c == b'E' || *c == b'+' || *c == b'-')
    {
        *i += 1;
    }
    let s = std::str::from_utf8(&b[start..*i]).map_err(|_| "bad number")?;
    s.parse::<f64>()
        .map(Json::Num)
        .map_err(|_| format!("bad number at byte {}", start))
}

pub fn to_string_pretty(v: &Json) -> String {
    let mut s = String::new();
    write_val(v, 0, &mut s);
    s
}

fn write_val(v: &Json, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let pad2 = "  ".repeat(indent + 1);
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => {
            let _ = write!(out, "{}", b);
        }
        Json::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                let _ = write!(out, "{}", *n as i64);
            } else {
                let _ = write!(out, "{}", n);
            }
        }
        Json::Str(s) => write_str(s, out),
        Json::Arr(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (n, item) in a.iter().enumerate() {
                out.push_str(&pad2);
                write_val(item, indent + 1, out);
                if n + 1 < a.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push(']');
        }
        Json::Obj(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (n, (k, val)) in m.iter().enumerate() {
                out.push_str(&pad2);
                write_str(k, out);
                out.push_str(": ");
                write_val(val, indent + 1, out);
                if n + 1 < m.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push('}');
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_scheme_shaped_document() {
        let src = r#"{"clvx_backlight":1,"backlight":{"keyboard":{"colorWave":{"period":3000,
                     "colorLinePicker":{"markersNumber":5,"markersArray":[
                       {"color":{"red":255,"green":83,"blue":83},"position":5}]}},
                     "interactiveAnimation":{"enable":true},"transparency":0}}}"#;
        let v = parse(src).expect("parses");
        let printed = to_string_pretty(&v);
        let again = parse(&printed).expect("reparses");
        assert_eq!(v, again);
    }

    #[test]
    fn rejects_oversized_and_deep_input() {
        let big = "0".repeat(MAX_INPUT + 1);
        assert!(parse(&big).is_err());
        let deep = format!("{}1{}", "[".repeat(MAX_DEPTH + 5), "]".repeat(MAX_DEPTH + 5));
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn rejects_trailing_garbage() {
        assert!(parse("{} nope").is_err());
        assert!(parse("{\"a\":1,}").is_err());
    }

    #[test]
    fn numbers_and_escapes() {
        assert_eq!(parse("255").unwrap().as_u32(), Some(255));
        assert_eq!(parse("-1").unwrap().as_u32(), None);
        assert_eq!(parse("1.5").unwrap().as_u32(), None);
        match parse(r#""a\nbA""#).unwrap() {
            Json::Str(s) => assert_eq!(s, "a\nbA"),
            _ => panic!("expected string"),
        }
    }
}
