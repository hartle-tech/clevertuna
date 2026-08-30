//! The backlight subtree: protobuf <-> JSON, with validation.
//!
//! Only the backlight is modelled. Everything else in the device's settings —
//! gestures, touch zones, key mappings, and anything a future firmware adds —
//! is carried through as opaque bytes, so changing a colour cannot quietly
//! rewrite something else.

use crate::json::Json;
use crate::proto::{self, Message, Value};

// The whole settings schema is recorded here, including the fields this tool
// does not write, because docs/PROTOCOL.md cites these numbers.
#[allow(dead_code)]
pub const APPSETTINGS_GLOBAL: u32 = 1;
pub const APPSETTINGS_PROFILE: u32 = 2;
#[allow(dead_code)]
pub const APPSETTINGS_COUNTER: u32 = 3;
pub const PROFILE_BACKLIGHT: u32 = 4;

pub const SCHEMA_KEY: &str = "clevertuna_backlight";
pub const SCHEMA_VERSION: u32 = 1;

pub const ZONES: [(&str, u32); 4] = [
    ("keyboard", 1),
    ("touchpad", 2),
    ("leftSlider", 3),
    ("rightSlider", 4),
];

/// Effect fields inside a zone.
const EFFECTS: [(&str, u32); 5] = [
    ("solidColor", 1),
    ("breathing", 4),
    ("colorCycle", 5),
    ("colorWave", 6),
    ("aurora", 8),
];
const INTERACTIVE: u32 = 2;
const TRANSPARENCY: u32 = 7;

// colour
const COLOR_R: u32 = 1;
const COLOR_G: u32 = 2;
const COLOR_B: u32 = 3;
// marker
const MARK_COLOR: u32 = 1;
const MARK_TRANSPARENCY: u32 = 2;
const MARK_POSITION: u32 = 3;
// picker
const PICK_COUNT: u32 = 1;
const PICK_ARRAY: u32 = 2;
// wave / cycle / breathing / solid
const WAVE_PICKER: u32 = 1;
const WAVE_PERIOD: u32 = 2;
const WAVE_DIRECTION: u32 = 3;
const WAVE_LENGTH: u32 = 4;
const SOLID_COLOR: u32 = 1;
const BREATH_COLOR: u32 = 1;
const BREATH_PERIOD: u32 = 2;
// interactive animation
const IA_COLOR: u32 = 2;
const IA_ENABLE: u32 = 3;
const IA_EXTRA: u32 = 4; // duration (keyboard) / trace (touchpad)

pub const MAX_MARKERS: usize = 5;

pub fn validation_error(msg: impl Into<String>) -> String {
    msg.into()
}

// ─────────────────────────────── decode ──────────────────────────────────

fn color_to_json(m: &Message) -> Json {
    let mut pairs = Vec::new();
    for (name, field) in [("red", COLOR_R), ("green", COLOR_G), ("blue", COLOR_B)] {
        if let Some(v) = proto::first_varint(m, field) {
            pairs.push((name, Json::Num(v as f64)));
        }
    }
    Json::obj(pairs)
}

fn picker_to_json(m: &Message) -> Json {
    let mut pairs = Vec::new();
    if let Some(n) = proto::first_varint(m, PICK_COUNT) {
        pairs.push(("markersNumber", Json::Num(n as f64)));
    }
    let mut markers = Vec::new();
    if let Some(list) = m.get(&PICK_ARRAY) {
        for v in list {
            if let Value::Bytes(b) = v {
                if let Some(mm) = proto::parse(b) {
                    let mut mp = Vec::new();
                    if let Some(c) = proto::first_bytes(&mm, MARK_COLOR) {
                        if let Some(cm) = proto::parse(c) {
                            mp.push(("color", color_to_json(&cm)));
                        }
                    }
                    if let Some(t) = proto::first_varint(&mm, MARK_TRANSPARENCY) {
                        mp.push(("transparency", Json::Num(t as f64)));
                    }
                    if let Some(p) = proto::first_varint(&mm, MARK_POSITION) {
                        mp.push(("position", Json::Num(p as f64)));
                    }
                    markers.push(Json::obj(mp));
                }
            }
        }
    }
    pairs.push(("markersArray", Json::Arr(markers)));
    Json::obj(pairs)
}

fn effect_to_json(name: &str, m: &Message) -> Json {
    let mut pairs = Vec::new();
    match name {
        "solidColor" => {
            if let Some(c) = proto::first_bytes(m, SOLID_COLOR) {
                if let Some(cm) = proto::parse(c) {
                    pairs.push(("color", color_to_json(&cm)));
                }
            }
        }
        "breathing" => {
            if let Some(c) = proto::first_bytes(m, BREATH_COLOR) {
                if let Some(cm) = proto::parse(c) {
                    pairs.push(("color", color_to_json(&cm)));
                }
            }
            if let Some(p) = proto::first_varint(m, BREATH_PERIOD) {
                pairs.push(("period", Json::Num(p as f64)));
            }
        }
        "colorCycle" | "colorWave" => {
            if let Some(p) = proto::first_bytes(m, WAVE_PICKER) {
                if let Some(pm) = proto::parse(p) {
                    pairs.push(("colorLinePicker", picker_to_json(&pm)));
                }
            }
            if let Some(v) = proto::first_varint(m, WAVE_PERIOD) {
                pairs.push(("period", Json::Num(v as f64)));
            }
            if name == "colorWave" {
                if let Some(v) = proto::first_varint(m, WAVE_DIRECTION) {
                    pairs.push(("direction", Json::Num(v as f64)));
                }
                if let Some(v) = proto::first_varint(m, WAVE_LENGTH) {
                    pairs.push(("length", Json::Num(v as f64)));
                }
            }
        }
        _ => {}
    }
    Json::obj(pairs)
}

fn zone_to_json(m: &Message) -> Json {
    let mut pairs = Vec::new();
    for (name, field) in EFFECTS {
        if let Some(b) = proto::first_bytes(m, field) {
            if let Some(em) = proto::parse(b) {
                pairs.push((name, effect_to_json(name, &em)));
            }
        }
    }
    if let Some(b) = proto::first_bytes(m, INTERACTIVE) {
        if let Some(im) = proto::parse(b) {
            let mut ip = Vec::new();
            if let Some(c) = proto::first_bytes(&im, IA_COLOR) {
                if let Some(cm) = proto::parse(c) {
                    ip.push(("color", color_to_json(&cm)));
                }
            }
            if let Some(e) = proto::first_varint(&im, IA_ENABLE) {
                ip.push(("enable", Json::Bool(e != 0)));
            }
            if let Some(x) = proto::first_varint(&im, IA_EXTRA) {
                ip.push(("extra", Json::Num(x as f64)));
            }
            pairs.push(("interactiveAnimation", Json::obj(ip)));
        }
    }
    if let Some(t) = proto::first_varint(m, TRANSPARENCY) {
        pairs.push(("transparency", Json::Num(t as f64)));
    }
    Json::obj(pairs)
}

/// Decode a `BacklightSettings` message into a scheme document.
pub fn to_json(backlight: &[u8]) -> Option<Json> {
    let m = proto::parse(backlight)?;
    let mut zones = Vec::new();
    for (name, field) in ZONES {
        if let Some(b) = proto::first_bytes(&m, field) {
            if let Some(zm) = proto::parse(b) {
                zones.push((name, zone_to_json(&zm)));
            }
        }
    }
    Some(Json::obj(vec![
        (SCHEMA_KEY, Json::Num(SCHEMA_VERSION as f64)),
        ("backlight", Json::obj(zones)),
    ]))
}

// ─────────────────────────────── encode ──────────────────────────────────

fn want_u32(v: &Json, what: &str, max: u32) -> Result<u32, String> {
    let n = v
        .as_u32()
        .ok_or_else(|| validation_error(format!("{} must be a whole number 0..={}", what, max)))?;
    if n > max {
        return Err(validation_error(format!("{} is {}, over the {} maximum", what, n, max)));
    }
    Ok(n)
}

fn color_from_json(v: &Json, where_: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (name, field) in [("red", COLOR_R), ("green", COLOR_G), ("blue", COLOR_B)] {
        if let Some(c) = v.get(name) {
            let n = want_u32(c, &format!("{}.{}", where_, name), 255)?;
            proto::field_varint(field, n as u64, &mut out);
        }
    }
    Ok(out)
}

fn picker_from_json(v: &Json, where_: &str) -> Result<Vec<u8>, String> {
    let markers = v
        .get("markersArray")
        .and_then(|m| m.as_array())
        .ok_or_else(|| validation_error(format!("{}.markersArray is required", where_)))?;
    if markers.len() > MAX_MARKERS {
        return Err(validation_error(format!(
            "{}.markersArray has {} markers; the device accepts at most {}",
            where_,
            markers.len(),
            MAX_MARKERS
        )));
    }
    let mut out = Vec::new();
    let count = v
        .get("markersNumber")
        .and_then(|n| n.as_u32())
        .unwrap_or(markers.len() as u32);
    proto::field_varint(PICK_COUNT, count as u64, &mut out);
    for (i, m) in markers.iter().enumerate() {
        let mut mo = Vec::new();
        if let Some(c) = m.get("color") {
            let cb = color_from_json(c, &format!("{}.markersArray[{}].color", where_, i))?;
            proto::field_bytes(MARK_COLOR, &cb, &mut mo);
        }
        if let Some(t) = m.get("transparency") {
            let n = want_u32(t, &format!("{}.markersArray[{}].transparency", where_, i), 100)?;
            proto::field_varint(MARK_TRANSPARENCY, n as u64, &mut mo);
        }
        if let Some(p) = m.get("position") {
            let n = want_u32(p, &format!("{}.markersArray[{}].position", where_, i), 100)?;
            proto::field_varint(MARK_POSITION, n as u64, &mut mo);
        }
        proto::field_bytes(PICK_ARRAY, &mo, &mut out);
    }
    Ok(out)
}

fn effect_from_json(name: &str, v: &Json, where_: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match name {
        "solidColor" => {
            if let Some(c) = v.get("color") {
                let cb = color_from_json(c, &format!("{}.color", where_))?;
                proto::field_bytes(SOLID_COLOR, &cb, &mut out);
            }
        }
        "breathing" => {
            if let Some(c) = v.get("color") {
                let cb = color_from_json(c, &format!("{}.color", where_))?;
                proto::field_bytes(BREATH_COLOR, &cb, &mut out);
            }
            if let Some(p) = v.get("period") {
                proto::field_varint(
                    BREATH_PERIOD,
                    want_u32(p, &format!("{}.period", where_), 600_000)? as u64,
                    &mut out,
                );
            }
        }
        "colorCycle" | "colorWave" => {
            if let Some(p) = v.get("colorLinePicker") {
                let pb = picker_from_json(p, &format!("{}.colorLinePicker", where_))?;
                proto::field_bytes(WAVE_PICKER, &pb, &mut out);
            }
            if let Some(p) = v.get("period") {
                proto::field_varint(
                    WAVE_PERIOD,
                    want_u32(p, &format!("{}.period", where_), 600_000)? as u64,
                    &mut out,
                );
            }
            if name == "colorWave" {
                if let Some(d) = v.get("direction") {
                    proto::field_varint(
                        WAVE_DIRECTION,
                        want_u32(d, &format!("{}.direction", where_), 360)? as u64,
                        &mut out,
                    );
                }
                if let Some(l) = v.get("length") {
                    proto::field_varint(
                        WAVE_LENGTH,
                        want_u32(l, &format!("{}.length", where_), 42_949_672)? as u64,
                        &mut out,
                    );
                }
            }
        }
        "aurora" => {}
        _ => return Err(validation_error(format!("{}: unknown effect '{}'", where_, name))),
    }
    Ok(out)
}

fn zone_from_json(v: &Json, zone: &str) -> Result<Vec<u8>, String> {
    let obj = match v {
        Json::Obj(m) => m,
        _ => return Err(validation_error(format!("{} must be an object", zone))),
    };
    let chosen: Vec<&String> = obj
        .keys()
        .filter(|k| EFFECTS.iter().any(|(n, _)| *n == k.as_str()))
        .collect();
    if chosen.len() > 1 {
        return Err(validation_error(format!(
            "{} names {} effects ({}); exactly one is allowed",
            zone,
            chosen.len(),
            chosen
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let mut fields: Vec<(u32, Vec<u8>)> = Vec::new();
    for (name, field) in EFFECTS {
        if let Some(e) = obj.get(name) {
            fields.push((field, effect_from_json(name, e, &format!("{}.{}", zone, name))?));
        }
    }
    if let Some(ia) = obj.get("interactiveAnimation") {
        let mut io = Vec::new();
        if let Some(c) = ia.get("color") {
            let cb = color_from_json(c, &format!("{}.interactiveAnimation.color", zone))?;
            proto::field_bytes(IA_COLOR, &cb, &mut io);
        }
        if let Some(e) = ia.get("enable") {
            let b = e.as_bool().ok_or_else(|| {
                validation_error(format!("{}.interactiveAnimation.enable must be true or false", zone))
            })?;
            proto::field_varint(IA_ENABLE, b as u64, &mut io);
        }
        for key in ["extra", "duration", "trace"] {
            if let Some(x) = ia.get(key) {
                proto::field_varint(
                    IA_EXTRA,
                    want_u32(x, &format!("{}.interactiveAnimation.{}", zone, key), 65_535)? as u64,
                    &mut io,
                );
                break;
            }
        }
        fields.push((INTERACTIVE, io));
    }
    // Every field above is a submessage; transparency is the one varint.
    let mut out = Vec::new();
    for (field, body) in &fields {
        proto::field_bytes(*field, body, &mut out);
    }
    if let Some(t) = obj.get("transparency") {
        let n = want_u32(t, &format!("{}.transparency", zone), 100)?;
        proto::field_varint(TRANSPARENCY, n as u64, &mut out);
    }
    Ok(reorder(out))
}

/// The firmware rejects settings whose fields are not ascending, so every
/// message this module builds is normalised through a parse/serialize pass.
fn reorder(bytes: Vec<u8>) -> Vec<u8> {
    match proto::parse(&bytes) {
        Some(m) => proto::serialize(&m),
        None => bytes,
    }
}

/// Encode a scheme document into a `BacklightSettings` message.
pub fn from_json(doc: &Json) -> Result<Vec<u8>, String> {
    let backlight = doc.get("backlight").unwrap_or(doc);
    if let Some(v) = doc.get(SCHEMA_KEY).and_then(|v| v.as_u32()) {
        if v != SCHEMA_VERSION {
            return Err(validation_error(format!(
                "scheme is version {}; this build understands version {}",
                v, SCHEMA_VERSION
            )));
        }
    }
    let obj = match backlight {
        Json::Obj(m) => m,
        _ => return Err(validation_error("backlight must be an object")),
    };
    for key in obj.keys() {
        if !ZONES.iter().any(|(n, _)| n == key) {
            return Err(validation_error(format!(
                "unknown zone '{}'; expected one of keyboard, touchpad, leftSlider, rightSlider",
                key
            )));
        }
    }
    let mut out = Vec::new();
    for (name, field) in ZONES {
        if let Some(z) = obj.get(name) {
            let body = zone_from_json(z, name)?;
            proto::field_bytes(field, &body, &mut out);
        }
    }
    if out.is_empty() {
        return Err(validation_error("scheme names no zones"));
    }
    Ok(reorder(out))
}

/// Splice a new backlight into a full settings message, preserving everything
/// else byte-for-byte.
pub fn splice(settings: &[u8], backlight: Vec<u8>) -> Result<Vec<u8>, String> {
    let top = proto::parse(settings).ok_or("device settings could not be parsed")?;
    let profile_raw = proto::first_bytes(&top, APPSETTINGS_PROFILE)
        .ok_or("device settings contain no profile")?;
    let profile = proto::parse(profile_raw).ok_or("device profile could not be parsed")?;
    let new_profile = proto::replace_field(&profile, PROFILE_BACKLIGHT, backlight);
    Ok(proto::replace_field(&top, APPSETTINGS_PROFILE, new_profile))
}

/// Pull the backlight subtree out of a full settings message.
pub fn extract(settings: &[u8]) -> Option<Vec<u8>> {
    let top = proto::parse(settings)?;
    let profile = proto::parse(proto::first_bytes(&top, APPSETTINGS_PROFILE)?)?;
    proto::first_bytes(&profile, PROFILE_BACKLIGHT).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn sample() -> Json {
        json::parse(
            r#"{"clevertuna_backlight":1,"backlight":{"keyboard":{
                 "colorWave":{"colorLinePicker":{"markersNumber":2,"markersArray":[
                   {"color":{"red":255,"green":83,"blue":83},"position":5},
                   {"color":{"red":0,"green":200,"blue":255},"position":29}]},
                 "period":3000,"direction":270,"length":1000},
                 "interactiveAnimation":{"color":{"red":0,"green":57,"blue":255},
                                          "enable":true,"duration":3},
                 "transparency":0}}}"#,
        )
        .unwrap()
    }

    #[test]
    fn json_to_proto_to_json_round_trips() {
        let encoded = from_json(&sample()).expect("encodes");
        let decoded = to_json(&encoded).expect("decodes");
        let reencoded = from_json(&decoded).expect("re-encodes");
        assert_eq!(encoded, reencoded, "encoding must be stable");
    }

    #[test]
    fn splice_touches_only_the_backlight() {
        // settings: global(1), profile(2){gestures(2), touch(3), backlight(4), keys(5)}, counter(3)
        let mut profile = Vec::new();
        proto::field_bytes(2, b"gestures", &mut profile);
        proto::field_bytes(3, b"touchzone", &mut profile);
        proto::field_bytes(4, b"old", &mut profile);
        proto::field_bytes(5, b"keymap", &mut profile);
        let mut settings = Vec::new();
        proto::field_bytes(1, b"global", &mut settings);
        proto::field_bytes(2, &profile, &mut settings);
        proto::field_varint(3, 168, &mut settings);

        let out = splice(&settings, b"NEW".to_vec()).expect("splices");
        let top = proto::parse(&out).unwrap();
        assert_eq!(proto::first_bytes(&top, 1).unwrap(), b"global");
        assert_eq!(proto::first_varint(&top, 3).unwrap(), 168);
        let prof = proto::parse(proto::first_bytes(&top, 2).unwrap()).unwrap();
        assert_eq!(proto::first_bytes(&prof, 2).unwrap(), b"gestures");
        assert_eq!(proto::first_bytes(&prof, 3).unwrap(), b"touchzone");
        assert_eq!(proto::first_bytes(&prof, 4).unwrap(), b"NEW");
        assert_eq!(proto::first_bytes(&prof, 5).unwrap(), b"keymap");
    }

    #[test]
    fn splice_output_is_field_ascending() {
        let mut profile = Vec::new();
        proto::field_bytes(5, b"keymap", &mut profile);
        proto::field_bytes(2, b"gestures", &mut profile);
        let mut settings = Vec::new();
        proto::field_bytes(2, &profile, &mut settings);
        proto::field_bytes(1, b"global", &mut settings);
        let out = splice(&settings, b"N".to_vec()).unwrap();
        assert_eq!(out[0] >> 3, 1, "first field must be the lowest number");
    }

    #[test]
    fn rejects_out_of_range_values() {
        let bad = json::parse(
            r#"{"backlight":{"keyboard":{"solidColor":{"color":{"red":999}}}}}"#,
        )
        .unwrap();
        let err = from_json(&bad).unwrap_err();
        assert!(err.contains("999"), "error should name the value: {}", err);
    }

    #[test]
    fn rejects_too_many_markers() {
        let mut markers = String::new();
        for i in 0..6 {
            if i > 0 {
                markers.push(',');
            }
            markers.push_str(r#"{"color":{"red":1},"position":1}"#);
        }
        let doc = json::parse(&format!(
            r#"{{"backlight":{{"keyboard":{{"colorWave":{{"colorLinePicker":{{"markersArray":[{}]}}}}}}}}}}"#,
            markers
        ))
        .unwrap();
        let err = from_json(&doc).unwrap_err();
        assert!(err.contains("at most"), "got: {}", err);
    }

    #[test]
    fn rejects_two_effects_in_one_zone() {
        let doc = json::parse(
            r#"{"backlight":{"keyboard":{"solidColor":{"color":{"red":1}},
                 "colorWave":{"colorLinePicker":{"markersArray":[]}}}}}"#,
        )
        .unwrap();
        assert!(from_json(&doc).unwrap_err().contains("exactly one"));
    }

    #[test]
    fn rejects_unknown_zone_and_wrong_version() {
        let doc = json::parse(r#"{"backlight":{"trackball":{}}}"#).unwrap();
        assert!(from_json(&doc).unwrap_err().contains("unknown zone"));
        let doc = json::parse(r#"{"clevertuna_backlight":99,"backlight":{}}"#).unwrap();
        assert!(from_json(&doc).unwrap_err().contains("version"));
    }
}
