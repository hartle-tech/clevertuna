//! Device operations, and the honesty that has to come with them.
//!
//! A write that returned zero is not a write that worked. Every write here goes
//! through the same ladder, and each rung is a distinct, reportable state:
//!
//!     validated -> sent -> acknowledged -> read back -> compared -> verified
//!
//! "Acknowledged" only means the device liked the request. "Verified" means we
//! asked it again afterwards and the answer matched. A mismatch is a result,
//! not an error to swallow.

use crate::backlight;
use crate::json::Json;
use crate::proto;
use crate::transport::{Device, Error, Result};

#[allow(dead_code)] // Protocol opcode, cited by docs/PROTOCOL.md.
pub const GET_SETTINGS: u64 = 0;
pub const SET_SETTINGS: u64 = 1;
pub const GET_DEVICE_INFO: u64 = 2;

// Request fields
const REQ_TYPE: u32 = 1;
const REQ_GET_SETTINGS: u32 = 2;
const REQ_SET_SETTINGS: u32 = 3;
const REQ_GET_DEVICE_INFO: u32 = 5;
// Response fields
const RSP_TYPE: u32 = 1;
const RSP_GET_SETTINGS: u32 = 2;
const RSP_SET_SETTINGS: u32 = 3;
const RSP_GET_DEVICE_INFO: u32 = 5;
const RSP_BAD_REQUEST: u32 = 7;
// nested
const SETTINGS_IN_RESPONSE: u32 = 2;
const SETTINGS_IN_REQUEST: u32 = 1;
const STATUS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Validated,
    Sent,
    /// Never a final outcome: a write the device accepts always continues to
    /// the read-back, so this stage exists to name the step, not to report it.
    #[allow(dead_code)]
    Acknowledged,
    ReadBack,
    Verified,
    Mismatch,
    Failed,
}

impl Stage {
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Validated => "validated",
            Stage::Sent => "sent",
            Stage::Acknowledged => "acknowledged",
            Stage::ReadBack => "read_back",
            Stage::Verified => "verified",
            Stage::Mismatch => "mismatch",
            Stage::Failed => "failed",
        }
    }
}

pub struct WriteOutcome {
    pub stage: Stage,
    pub protocol_status: Option<u64>,
    pub expected: Option<Json>,
    pub actual: Option<Json>,
    pub message: String,
}

fn bad_request_reason(code: u64) -> &'static str {
    match code {
        0 => "the device could not parse the request",
        1 => "the device refused the request as unsupported \
              (usually two requests sent too close together)",
        _ => "the device rejected the request",
    }
}

fn set_status_reason(code: u64) -> &'static str {
    match code {
        0 => "accepted",
        1 => "the device reported an unknown error",
        2 => "the device rejected the settings as invalid",
        _ => "the device returned an unrecognised status",
    }
}

fn check_bad_request(resp: &proto::Message) -> Result<()> {
    if let Some(b) = proto::first_bytes(resp, RSP_BAD_REQUEST) {
        let code = proto::parse(b).and_then(|m| proto::first_varint(&m, STATUS));
        return Err(Error::Protocol(match code {
            Some(c) => format!("{} (BAD_REQUEST {})", bad_request_reason(c), c),
            // A refusal with no status inside it is not a verdict from the
            // device — it is a reply that did not arrive whole. This used to
            // default the missing code to u64::MAX and print
            // "BAD_REQUEST 18446744073709551615", which reads as a firmware
            // rejection of the scheme and sent two sessions looking for a
            // schema fault that was never there.
            None => "the reply came back without a status in it".into(),
        }));
    }
    if proto::first_varint(resp, RSP_TYPE) == Some(4) {
        return Err(Error::Protocol("the device returned BAD_REQUEST".into()));
    }
    Ok(())
}

/// The whole `AppSettings` message, exactly as the device holds it.
pub fn get_settings(dev: &mut Device) -> Result<Vec<u8>> {
    let req = proto::bytes_field(REQ_GET_SETTINGS, &[]);
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    let inner = proto::first_bytes(&resp, RSP_GET_SETTINGS)
        .ok_or_else(|| Error::Protocol("response carried no settings".into()))?;
    let inner = proto::parse(inner).ok_or_else(|| Error::Protocol("unparsable settings".into()))?;
    proto::first_bytes(&inner, SETTINGS_IN_RESPONSE)
        .cloned()
        .ok_or_else(|| Error::Protocol("settings field was absent".into()))
}

/// Send a full `AppSettings`. Returns the protocol status (0 = accepted).
pub fn set_settings(dev: &mut Device, settings: &[u8]) -> Result<u64> {
    let inner = proto::bytes_field(SETTINGS_IN_REQUEST, settings);
    let mut req = proto::varint_field(REQ_TYPE, SET_SETTINGS);
    req.extend(proto::bytes_field(REQ_SET_SETTINGS, &inner));
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    let inner = proto::first_bytes(&resp, RSP_SET_SETTINGS)
        .ok_or_else(|| Error::Protocol("response carried no status".into()))?;
    let inner = proto::parse(inner).ok_or_else(|| Error::Protocol("unparsable status".into()))?;
    Ok(proto::first_varint(&inner, STATUS).unwrap_or(0))
}

// ── the rest of the request set ──────────────────────────────────────────────
//
// Recovered from the vendor's published protocol definitions, which name every
// request type and tag. Only the ones that are useful and answerable are
// offered; the AI and diagnostics requests exist and are listed in the handoff
// rather than half-wired here.

/// Which operating system the keyboard should behave as if attached to.
pub const OS_WINDOWS: u64 = 0;
pub const OS_MAC: u64 = 1;
pub const OS_LINUX: u64 = 2;

const SET_OS_MODE: u64 = 8;
const REQ_SET_OS_MODE: u32 = 10;
const OS_MODE_FIELD: u32 = 1;
const GET_DEFAULT_SETTINGS: u64 = 9;
const REQ_GET_DEFAULT_SETTINGS: u32 = 11;
/// There is no `getDefaultSettings` in the response at all — the protocol
/// defines the request and answers it in the ordinary settings shape. Reading
/// field 11 got "the device returned no default settings", which sounds like a
/// keyboard that has none rather than a client looking in the wrong place.
const RSP_GET_DEFAULT_SETTINGS: u32 = RSP_GET_SETTINGS;
const PERFORM_FULL_RESET: u64 = 10;
const REQ_PERFORM_FULL_RESET: u32 = 12;
const PERFORM_RESTART: u64 = 12;
const REQ_PERFORM_RESTART: u32 = 14;

/// Tell the keyboard which OS it is plugged into.
///
/// It changes what the modifier keys and the media row do, which is why the
/// vendor application sets it on connect and why moving a keyboard between a
/// Mac and a PC otherwise feels broken.
pub fn set_os_mode(dev: &mut Device, mode: u64) -> Result<()> {
    let inner = proto::varint_field(OS_MODE_FIELD, mode);
    let mut req = proto::varint_field(REQ_TYPE, SET_OS_MODE);
    req.extend(proto::bytes_field(REQ_SET_OS_MODE, &inner));
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    Ok(())
}

/// The settings the keyboard would have if it were new.
///
/// This is what makes "put it back how it was" safe: the defaults can be
/// fetched, looked at, and written through the ordinary verified path, instead
/// of firing a full reset and hoping.
pub fn get_default_settings(dev: &mut Device) -> Result<Vec<u8>> {
    let mut req = proto::varint_field(REQ_TYPE, GET_DEFAULT_SETTINGS);
    req.extend(proto::bytes_field(REQ_GET_DEFAULT_SETTINGS, &[]));
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    let inner = proto::first_bytes(&resp, RSP_GET_DEFAULT_SETTINGS)
        .ok_or_else(|| Error::Protocol("the device returned no default settings".into()))?;
    let inner = proto::parse(inner).ok_or_else(|| Error::Protocol("unparsable defaults".into()))?;
    proto::first_bytes(&inner, SETTINGS_IN_RESPONSE)
        .cloned()
        .ok_or_else(|| Error::Protocol("the defaults carried no settings".into()))
}

/// Reboot the keyboard, keeping every setting.
///
/// The recovery move when it stops accepting writes while still answering
/// reads: the settings survive, the link is rebuilt, and nothing is lost. It is
/// the gentlest thing in this file that changes anything at all.
pub fn perform_restart(dev: &mut Device) -> Result<()> {
    let mut req = proto::varint_field(REQ_TYPE, PERFORM_RESTART);
    req.extend(proto::bytes_field(REQ_PERFORM_RESTART, &[]));
    // A keyboard that is rebooting cannot answer, so a lost reply here is the
    // expected case rather than a failure.
    match dev.request(&req) {
        Ok(raw) => {
            if let Some(resp) = proto::parse(&raw) {
                check_bad_request(&resp)?;
            }
            Ok(())
        }
        Err(Error::Protocol(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Put the keyboard back to how it left the factory. Everything, at once.
pub fn perform_full_reset(dev: &mut Device) -> Result<()> {
    let mut req = proto::varint_field(REQ_TYPE, PERFORM_FULL_RESET);
    req.extend(proto::bytes_field(REQ_PERFORM_FULL_RESET, &[]));
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    Ok(())
}

pub fn get_device_info(dev: &mut Device) -> Result<Vec<u8>> {
    let mut req = proto::varint_field(REQ_TYPE, GET_DEVICE_INFO);
    req.extend(proto::bytes_field(REQ_GET_DEVICE_INFO, &[]));
    let raw = dev.request(&req)?;
    let resp = proto::parse(&raw).ok_or_else(|| Error::Protocol("unparsable response".into()))?;
    check_bad_request(&resp)?;
    proto::first_bytes(&resp, RSP_GET_DEVICE_INFO)
        .cloned()
        .ok_or_else(|| Error::Protocol("response carried no device info".into()))
}

pub fn get_backlight_json(dev: &mut Device) -> Result<Json> {
    let settings = get_settings(dev)?;
    let bl = backlight::extract(&settings)
        .ok_or_else(|| Error::Protocol("settings contain no backlight".into()))?;
    backlight::to_json(&bl).ok_or_else(|| Error::Protocol("backlight could not be decoded".into()))
}

/// Apply a scheme and then prove it landed.
pub fn set_backlight_verified(dev: &mut Device, doc: &Json) -> Result<WriteOutcome> {
    // validated
    let encoded = backlight::from_json(doc).map_err(Error::Protocol)?;
    let intended = backlight::to_json(&encoded)
        .ok_or_else(|| Error::Protocol("could not re-read the scheme just built".into()))?;

    let settings = get_settings(dev)?;
    let spliced = backlight::splice(&settings, encoded.clone()).map_err(Error::Protocol)?;

    // sent -> acknowledged
    //
    // An acknowledgement that does not arrive is **not** evidence that the
    // write failed, and this is the second time that mistake has cost a
    // session. Over Bluetooth the reply is the fragile half of the exchange:
    // the device is documented to hand back the *previous* response, and
    // `Device::request` has already retried the whole exchange up to three
    // times before it gives up — so by the time an error surfaces here, the
    // settings have very likely been written and only the answer was lost.
    //
    // Declaring failure there is wrong twice over: it reports a write that
    // landed as a transport fault, and it invites the caller to write again.
    // So an unclear acknowledgement is carried forward as *unknown*, and the
    // question is settled the only way it can be — by reading the keyboard.
    // That is what "verified" has meant here from the start; this simply stops
    // treating the acknowledgement as if it outranked the read-back.
    let ack: std::result::Result<u64, String> = match set_settings(dev, &spliced) {
        Ok(status) => Ok(status),
        // A protocol-level failure leaves the link usable, so the read-back can
        // still answer. An I/O failure does not — there is nothing to ask.
        Err(Error::Protocol(why)) => Err(why),
        Err(e) => return Err(e),
    };

    // A clean, non-zero status *is* a verdict from the device: it parsed the
    // request and refused it. Nothing to reconcile.
    if let Ok(status) = ack {
        if status != 0 {
            return Ok(WriteOutcome {
                stage: Stage::Failed,
                protocol_status: Some(status),
                expected: Some(intended),
                actual: None,
                message: format!("the device did not accept the write: {}", set_status_reason(status)),
            });
        }
    }

    // read back -> compared
    let after = match get_backlight_json(dev) {
        Ok(a) => a,
        // If the acknowledgement was already lost, the read-back failing too
        // means the link is unhappy — report the first reason, which is the
        // one that describes what actually went wrong.
        Err(e) => match ack {
            Err(why) => return Err(Error::Protocol(why)),
            Ok(_) => return Err(e),
        },
    };
    let expected_bl = intended.get("backlight").cloned().unwrap_or(Json::Null);
    let actual_bl = after.get("backlight").cloned().unwrap_or(Json::Null);

    if compare_requested(&expected_bl, &actual_bl) {
        Ok(WriteOutcome {
            stage: Stage::Verified,
            protocol_status: ack.as_ref().ok().copied(),
            expected: Some(expected_bl),
            actual: Some(actual_bl),
            message: match &ack {
                Ok(_) => "written and read back; the device matches the scheme".into(),
                Err(why) => format!(
                    "the acknowledgement was lost ({}), but the keyboard reads back matching the scheme",
                    why
                ),
            },
        })
    } else {
        Ok(WriteOutcome {
            stage: match &ack {
                // Acknowledged, then different: the device changed something.
                Ok(_) => Stage::Mismatch,
                // Never acknowledged, and different: the write did not land,
                // and now the original reason can be reported as the cause
                // rather than as a guess.
                Err(_) => Stage::Failed,
            },
            protocol_status: ack.as_ref().ok().copied(),
            expected: Some(expected_bl),
            actual: Some(actual_bl),
            message: match &ack {
                Ok(_) => "the device accepted the write, but reading it back does not match".into(),
                Err(why) => format!("the write did not land: {}", why),
            },
        })
    }
}

/// Compare only what the scheme actually asked for.
///
/// The device fills in fields the caller left out, so a strict equality check
/// would report a mismatch for a scheme that applied perfectly. Every key the
/// caller named must match; keys they did not name are the device's business.
pub fn compare_requested(expected: &Json, actual: &Json) -> bool {
    match (expected, actual) {
        (Json::Obj(e), Json::Obj(a)) => e.iter().all(|(k, v)| match a.get(k) {
            Some(av) => compare_requested(v, av),
            None => false,
        }),
        (Json::Arr(e), Json::Arr(a)) => {
            e.len() == a.len() && e.iter().zip(a).all(|(x, y)| compare_requested(x, y))
        }
        (x, y) => x == y,
    }
}

/// Human-readable device summary. Identifiers stay out unless asked for.
/// The device, in words rather than field numbers.
///
/// `DeviceInfo`'s field names come from the vendor's published protocol
/// definitions, so this prints "firmware 6.3.1" where it used to print
/// "field 3 393985". The three version numbers are packed one byte per part.
///
/// Anything that identifies *this* keyboard rather than the model — its serial,
/// its barcode, its chip id — stays hidden unless asked for, so a pasted
/// terminal is safe by default.
pub fn describe(info: &[u8], show_identifiers: bool) -> Vec<(String, String)> {
    fn version(v: u64) -> String {
        format!("{}.{}.{}", (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF)
    }
    let mut rows: Vec<(String, String)> = Vec::new();
    let m = match proto::parse(info) {
        Some(m) => m,
        None => return rows,
    };
    // The info arrives wrapped; the descriptor is the first submessage that
    // parses into something with a name in it.
    let inner = m
        .values()
        .flatten()
        .filter_map(|v| match v {
            proto::Value::Bytes(b) => proto::parse(b),
            _ => None,
        })
        .find(|p| p.contains_key(&1))
        .unwrap_or(m);

    let text = |f: u32| -> Option<String> {
        proto::first_bytes(&inner, f).and_then(|b| std::str::from_utf8(b).ok().map(|s| s.trim().to_string()))
    };
    let num = |f: u32| proto::first_varint(&inner, f);

    if let Some(name) = text(1) {
        rows.push(("model".into(), name));
    }
    if let Some(v) = num(3) {
        rows.push(("firmware".into(), version(v)));
    }
    if let Some(v) = num(11) {
        rows.push(("protocol".into(), version(v)));
    }
    if let Some(v) = num(15) {
        rows.push(("board".into(), version(v)));
    }
    if let Some(v) = num(13) {
        rows.push(("device type".into(), v.to_string()));
    }
    if let Some(v) = num(17) {
        rows.push(("colour".into(), match v {
            0 => "black".into(),
            1 => "white".into(),
            other => other.to_string(),
        }));
    }
    if num(12) == Some(1) {
        rows.push(("build".into(), "insider".into()));
    }
    if show_identifiers {
        if let Some(v) = num(10) {
            rows.push(("serial".into(), v.to_string()));
        }
        if let Some(b) = text(9) {
            rows.push(("barcode".into(), b));
        }
        if let Some(chip) = proto::first_bytes(&inner, 16) {
            rows.push((
                "chip id".into(),
                chip.iter().map(|b| format!("{:02X}", b)).collect::<String>(),
            ));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    #[test]
    fn requested_comparison_ignores_fields_the_caller_omitted() {
        let want = json::parse(r#"{"keyboard":{"transparency":0}}"#).unwrap();
        let got = json::parse(
            r#"{"keyboard":{"transparency":0,"interactiveAnimation":{"enable":true}},
                "touchpad":{"transparency":5}}"#,
        )
        .unwrap();
        assert!(compare_requested(&want, &got));
    }

    #[test]
    fn requested_comparison_catches_a_changed_value() {
        let want = json::parse(r#"{"keyboard":{"transparency":0}}"#).unwrap();
        let got = json::parse(r#"{"keyboard":{"transparency":30}}"#).unwrap();
        assert!(!compare_requested(&want, &got));
    }

    #[test]
    fn requested_comparison_catches_a_missing_key() {
        let want = json::parse(r#"{"keyboard":{"transparency":0}}"#).unwrap();
        let got = json::parse(r#"{"touchpad":{"transparency":0}}"#).unwrap();
        assert!(!compare_requested(&want, &got));
    }

    #[test]
    fn arrays_must_match_in_length_and_order() {
        let a = json::parse(r#"{"m":[1,2,3]}"#).unwrap();
        let b = json::parse(r#"{"m":[1,2]}"#).unwrap();
        let c = json::parse(r#"{"m":[1,3,2]}"#).unwrap();
        assert!(!compare_requested(&a, &b));
        assert!(!compare_requested(&a, &c));
        assert!(compare_requested(&a, &a));
    }

    /// A device-info message shaped like the real one.
    fn device_info() -> Vec<u8> {
        let mut inner = Vec::new();
        proto::field_bytes(1, b"CLVX S", &mut inner);          // name
        proto::field_varint(3, 393_985, &mut inner);            // fwVersion 6.3.1
        proto::field_bytes(9, b"CA25D403025P", &mut inner);     // barCode
        proto::field_varint(10, 3_025, &mut inner);             // serialNumber
        proto::field_varint(11, 2_304, &mut inner);             // protocolVersion
        proto::field_bytes(16, &[0xDE, 0xAD, 0xBE, 0xEF], &mut inner); // chipUID
        let mut outer = Vec::new();
        proto::field_bytes(5, &inner, &mut outer);
        outer
    }

    #[test]
    fn what_identifies_this_particular_keyboard_is_hidden_by_default() {
        // Redaction is by field now, not by whether a string happens to look
        // like a serial. The schema says which fields identify the unit, so a
        // serial that does not look like one is still hidden.
        let hidden = describe(&device_info(), false);
        let flat: String = hidden.iter().map(|(k, v)| format!("{}={} ", k, v)).collect();
        assert!(!flat.contains("CA25D403025P"), "barcode leaked: {}", flat);
        assert!(!flat.contains("3025"), "serial leaked: {}", flat);
        assert!(!flat.to_uppercase().contains("DEADBEEF"), "chip id leaked: {}", flat);
        // …while what describes the model is always useful and never private.
        assert!(hidden.iter().any(|(k, v)| k == "model" && v == "CLVX S"), "{:?}", hidden);

        let shown = describe(&device_info(), true);
        let flat: String = shown.iter().map(|(k, v)| format!("{}={} ", k, v)).collect();
        assert!(flat.contains("CA25D403025P") && flat.contains("3025") && flat.to_uppercase().contains("DEADBEEF"),
                "asking for them must produce them: {}", flat);
    }

    #[test]
    fn versions_are_read_as_versions_rather_than_as_large_numbers() {
        let rows = describe(&device_info(), false);
        let get = |k: &str| rows.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert_eq!(get("firmware").as_deref(), Some("6.3.1"), "{:?}", rows);
        assert_eq!(get("protocol").as_deref(), Some("0.9.0"));
    }

    #[test]
    fn stages_have_stable_labels() {
        assert_eq!(Stage::Verified.label(), "verified");
        assert_eq!(Stage::Mismatch.label(), "mismatch");
    }
}
