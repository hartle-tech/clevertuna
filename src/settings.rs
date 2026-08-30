//! Everything about the keyboard that is not a colour.
//!
//! Power saving, the touch controls, the multi-touch actions, and the handful
//! of keyboard behaviours the vendor application exposes — modelled as one
//! table so that every surface renders the same list without keeping its own
//! copy of it. A window asks for this table, draws a control per row from the
//! `kind` it is given, and hands values back by name.
//!
//! ## Provenance
//!
//! These field numbers, names, types and enum values are the protocol's own.
//! The vendor's Electron bundle publishes its generated protobuf definitions in
//! its source maps — `@clevetura/clv-firmware-clvx/app-settings.ts` — so the
//! shape below is read from the specification rather than guessed from a
//! hexdump, and every value here was then checked against a real CLVX S.
//! Field numbers and wire types are interoperability facts: two correct
//! implementations have no choice but to agree on them. Nothing expressive was
//! taken — the wording, grouping and defaults below are this project's own, and
//! the vendor's curated preset bundles were deliberately not copied. See
//! `NOTICE` and `docs/PROTOCOL.md` §9.

use crate::json::{self, Json};
use crate::power;
use crate::proto;

/// What kind of control a setting wants.
#[derive(Clone, PartialEq, Debug)]
pub enum Kind {
    /// On or off.
    Switch,
    /// One of a named set, stored as its number.
    Choice(&'static [(i64, &'static str)]),
    /// A duration in seconds, from a closed set. 0 has its own name.
    Seconds(&'static [u32], &'static str),
    /// A small integer scale, low to high.
    Level(u32, u32),
}

/// Where a setting lives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Where {
    /// `AppSettings.global` — one set for the whole keyboard.
    Global,
    /// `AppSettings.profile.touchZone.touchpad`.
    Touchpad,
    /// `…touchZone.slider.left`, and its twin on the right.
    SliderLeft,
    SliderRight,
}

pub struct Setting {
    /// What a command line and a window both call it.
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub help: &'static str,
    pub place: Where,
    pub field: u32,
    pub kind: Kind,
}

const HAND: [(i64, &str); 3] = [(0, "Not chosen"), (1, "Right"), (2, "Left")];

/// The highest slider sensitivity this firmware accepts.
///
/// **Four, not five.** The vendor's own control offers five labels — Low, Low
/// Medium, Medium, Medium High, High — but a CLVX S refuses a 5 outright with
/// `status 2`, "the settings are invalid", while 1 to 4 are all accepted and
/// read back. Measured, not assumed: every value was written to a real device
/// in turn. The published option list is not the same thing as the range the
/// firmware will take.
pub const SENSITIVITY_MAX: u32 = 4;

/// Every setting this tool can read and write.
///
/// Deliberately only the scalars. The gesture and key-mapping trees are richer
/// than a switch and are described in the handoff rather than half-offered
/// here — a control that writes a shortcut it cannot show you again would be
/// worse than no control.
pub fn all() -> Vec<Setting> {
    use Kind::*;
    use Where::*;
    vec![
        // ── Power ───────────────────────────────────────────────────────────
        Setting { key: "backlight-timeout", label: "Backlight off after", group: "Power",
            help: "the light goes out entirely after this long",
            place: Global, field: power::BACKLIGHT_TIMEOUT,
            kind: Seconds(&power::BACKLIGHT_CHOICES, "Never — always on") },
        Setting { key: "idle-timeout", label: "Dim after", group: "Power",
            help: "the light dims after this long without input; never later than the timeout above",
            place: Global, field: power::IDLE_TIMEOUT,
            kind: Seconds(&power::IDLE_CHOICES, "Never") },
        Setting { key: "battery-saving", label: "Battery saving", group: "Power",
            help: "the keyboard's own power thrift, on battery",
            place: Global, field: 14, kind: Switch },
        Setting { key: "auto-brightness", label: "Automatic brightness", group: "Power",
            help: "let the keyboard set its own brightness from the light around it",
            place: Global, field: 11, kind: Switch },

        // ── Touch ───────────────────────────────────────────────────────────
        Setting { key: "touchpad", label: "Touchpad", group: "Touch",
            help: "the touch area as a pointer at all",
            place: Touchpad, field: 1, kind: Switch },
        Setting { key: "tap", label: "Tap to click", group: "Touch",
            help: "one finger, tapped",
            place: Global, field: 2, kind: Switch },
        Setting { key: "two-finger-tap", label: "Two-finger tap", group: "Touch",
            help: "the second button, without a second button",
            place: Global, field: 3, kind: Switch },
        Setting { key: "tap-and-hold", label: "Tap and hold", group: "Touch",
            help: "hold after a tap to drag",
            place: Global, field: 4, kind: Switch },
        Setting { key: "swap-buttons", label: "Swap the click buttons", group: "Touch",
            help: "left becomes right",
            place: Global, field: 5, kind: Switch },
        Setting { key: "touch-after-lift", label: "Keep touch alive after lifting off", group: "Touch",
            help: "a finger lifted for an instant does not end the gesture",
            place: Global, field: 8, kind: Switch },
        Setting { key: "edge-hold-delay", label: "Pause at the edge", group: "Touch",
            help: "hold at the border of the pad rather than stopping dead",
            place: Global, field: 16, kind: Switch },
        Setting { key: "dominant-hand", label: "Dominant hand", group: "Touch",
            help: "which hand the keyboard should expect",
            place: Global, field: 12, kind: Choice(&HAND) },

        // ── Multi-touch ─────────────────────────────────────────────────────
        Setting { key: "one-finger-zone", label: "One-finger gestures", group: "Multi-touch",
            help: "the touch area answers a single finger",
            place: Touchpad, field: 2, kind: Switch },
        Setting { key: "two-finger-zone", label: "Two-finger gestures", group: "Multi-touch",
            help: "the touch area answers two",
            place: Touchpad, field: 3, kind: Switch },
        Setting { key: "left-slider-sensitivity", label: "Left slider sensitivity", group: "Multi-touch",
            help: "how far a finger has to travel on the left strip",
            place: SliderLeft, field: 2, kind: Level(1, SENSITIVITY_MAX) },
        Setting { key: "right-slider-sensitivity", label: "Right slider sensitivity", group: "Multi-touch",
            help: "and on the right",
            place: SliderRight, field: 2, kind: Level(1, SENSITIVITY_MAX) },

        // ── Keyboard ────────────────────────────────────────────────────────
        Setting { key: "fn-lock", label: "Fn lock", group: "Keyboard",
            help: "the function row without holding Fn",
            place: Global, field: 9, kind: Switch },
        Setting { key: "swap-fn-ctrl", label: "Swap Fn and Control", group: "Keyboard",
            help: "for people who came from another layout",
            place: Global, field: 17, kind: Switch },
        Setting { key: "key-suppressor", label: "Suppress accidental keys", group: "Keyboard",
            help: "ignore a key that was brushed rather than pressed",
            place: Global, field: 15, kind: Switch },
        Setting { key: "auto-usb-switch", label: "Switch to the cable automatically", group: "Keyboard",
            help: "plugging in moves the keyboard to USB without asking",
            place: Global, field: 22, kind: Switch },
        Setting { key: "beginner-mode", label: "Beginner mode", group: "Keyboard",
            help: "the vendor's gentler defaults for a new owner",
            place: Global, field: 7, kind: Switch },
    ]
}

pub fn find(key: &str) -> Option<Setting> {
    all().into_iter().find(|s| s.key == key)
}

pub fn groups() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for s in all() {
        if !out.contains(&s.group) {
            out.push(s.group);
        }
    }
    out
}

// ───────────────────────────── reading and writing ───────────────────────────

const PROFILE_TOUCHZONE: u32 = 3;
const TOUCHZONE_TOUCHPAD: u32 = 1;
const TOUCHZONE_SLIDER: u32 = 2;
const SLIDER_LEFT: u32 = 3;
const SLIDER_RIGHT: u32 = 4;

/// The path from `AppSettings` down to the message a setting lives in.
fn path(place: Where) -> Vec<u32> {
    match place {
        Where::Global => vec![crate::backlight::APPSETTINGS_GLOBAL],
        Where::Touchpad => vec![crate::backlight::APPSETTINGS_PROFILE, PROFILE_TOUCHZONE, TOUCHZONE_TOUCHPAD],
        Where::SliderLeft => vec![crate::backlight::APPSETTINGS_PROFILE, PROFILE_TOUCHZONE, TOUCHZONE_SLIDER, SLIDER_LEFT],
        Where::SliderRight => vec![crate::backlight::APPSETTINGS_PROFILE, PROFILE_TOUCHZONE, TOUCHZONE_SLIDER, SLIDER_RIGHT],
    }
}

fn descend<'a>(settings: &'a [u8], path: &[u32]) -> Option<proto::Message> {
    let mut current = proto::parse(settings)?;
    for step in path {
        let next = proto::first_bytes(&current, *step)?;
        current = proto::parse(next)?;
    }
    Some(current)
}

/// Read one setting, or `None` if the keyboard does not carry it.
pub fn read_one(settings: &[u8], s: &Setting) -> Option<i64> {
    let m = descend(settings, &path(s.place))?;
    proto::first_varint(&m, s.field).map(|v| v as i64)
}

/// Rebuild a settings message with one varint replaced, deep inside it.
///
/// Recursive because the touch settings are three levels down, and because the
/// alternative — a hand-written splice per depth — is where a tool starts
/// rewriting fields it never meant to touch. Every level is parsed and
/// re-serialised, so anything not named is carried through exactly.
fn set_deep(blob: &[u8], path: &[u32], field: u32, value: i64) -> Result<Vec<u8>, String> {
    let mut m = proto::parse(blob).ok_or("settings could not be parsed")?;
    match path.split_first() {
        None => {
            m.insert(field, vec![proto::Value::Varint(value.max(0) as u64)]);
            Ok(proto::serialize(&m))
        }
        Some((head, rest)) => {
            let inner = proto::first_bytes(&m, *head)
                .cloned()
                .ok_or_else(|| format!("this keyboard carries no section {}", head))?;
            let rebuilt = set_deep(&inner, rest, field, value)?;
            m.insert(*head, vec![proto::Value::Bytes(rebuilt)]);
            Ok(proto::serialize(&m))
        }
    }
}

/// Clamp to what the setting can actually hold.
pub fn coerce(s: &Setting, value: i64) -> i64 {
    match &s.kind {
        Kind::Switch => i64::from(value != 0),
        Kind::Choice(options) => {
            if options.iter().any(|(v, _)| *v == value) { value } else { options[0].0 }
        }
        Kind::Seconds(choices, _) => power::nearest(choices, value.max(0) as u32) as i64,
        Kind::Level(lo, hi) => value.clamp(*lo as i64, *hi as i64),
    }
}

pub fn write_one(settings: &[u8], s: &Setting, value: i64) -> Result<Vec<u8>, String> {
    set_deep(settings, &path(s.place), s.field, coerce(s, value))
}

/// How a value should be shown.
pub fn describe(s: &Setting, value: i64) -> String {
    match &s.kind {
        Kind::Switch => (if value != 0 { "on" } else { "off" }).to_string(),
        Kind::Choice(options) => options
            .iter()
            .find(|(v, _)| *v == value)
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| value.to_string()),
        Kind::Seconds(_, zero) => power::describe(value.max(0) as u32, zero),
        Kind::Level(lo, hi) => {
            let names = ["Low", "Low medium", "Medium", "Medium high", "High"];
            let span = (hi - lo).max(1) as i64;
            let idx = ((value - *lo as i64) * (names.len() as i64 - 1) / span).clamp(0, names.len() as i64 - 1);
            names[idx as usize].to_string()
        }
    }
}

/// Accept what a person or a window might send.
pub fn parse_value(s: &Setting, text: &str) -> Option<i64> {
    let t = text.trim().to_ascii_lowercase();
    match &s.kind {
        Kind::Switch => match t.as_str() {
            "on" | "yes" | "true" | "1" | "enable" | "enabled" => Some(1),
            "off" | "no" | "false" | "0" | "disable" | "disabled" => Some(0),
            _ => None,
        },
        Kind::Choice(options) => options
            .iter()
            .find(|(_, n)| n.to_ascii_lowercase() == t)
            .map(|(v, _)| *v)
            .or_else(|| t.parse().ok()),
        Kind::Seconds(_, _) => power::parse_seconds(&t).map(|v| v as i64),
        Kind::Level(_, _) => t.parse().ok(),
    }
}

/// The whole table with the keyboard's current values, for a window to draw.
pub fn model(settings: &[u8]) -> Json {
    let rows: Vec<Json> = all()
        .iter()
        .map(|s| {
            let value = read_one(settings, s);
            let kind = match &s.kind {
                Kind::Switch => Json::Str("switch".into()),
                Kind::Choice(_) => Json::Str("choice".into()),
                Kind::Seconds(_, _) => Json::Str("seconds".into()),
                Kind::Level(_, _) => Json::Str("level".into()),
            };
            let options = match &s.kind {
                Kind::Choice(opts) => Json::Arr(
                    opts.iter()
                        .map(|(v, n)| {
                            Json::obj(vec![("value", Json::Num(*v as f64)), ("label", Json::Str((*n).into()))])
                        })
                        .collect(),
                ),
                Kind::Seconds(choices, zero) => Json::Arr(
                    choices
                        .iter()
                        .map(|c| {
                            Json::obj(vec![
                                ("value", Json::Num(*c as f64)),
                                ("label", Json::Str(power::describe(*c, zero))),
                            ])
                        })
                        .collect(),
                ),
                Kind::Level(lo, hi) => Json::Arr(
                    (*lo..=*hi)
                        .map(|v| {
                            Json::obj(vec![
                                ("value", Json::Num(v as f64)),
                                ("label", Json::Str(describe(s, v as i64))),
                            ])
                        })
                        .collect(),
                ),
                Kind::Switch => Json::Arr(vec![]),
            };
            Json::obj(vec![
                ("key", Json::Str(s.key.into())),
                ("label", Json::Str(s.label.into())),
                ("group", Json::Str(s.group.into())),
                ("help", Json::Str(s.help.into())),
                ("kind", kind),
                ("options", options),
                // Absent means this keyboard does not carry the field, which is
                // not the same as "off" — a window must grey it, not lie.
                ("value", match value {
                    Some(v) => Json::Num(v as f64),
                    None => Json::Null,
                }),
                ("shown", match value {
                    Some(v) => Json::Str(describe(s, v)),
                    None => Json::Str("not on this keyboard".into()),
                }),
            ])
        })
        .collect();
    json::to_string_pretty(&Json::obj(vec![
        ("groups", Json::Arr(groups().iter().map(|g| Json::Str((*g).into())).collect())),
        ("settings", Json::Arr(rows)),
    ]))
    .parse_back()
}

/// Small helper so `model` can return a `Json` rather than a string.
trait ParseBack {
    fn parse_back(self) -> Json;
}
impl ParseBack for String {
    fn parse_back(self) -> Json {
        json::parse(&self).unwrap_or(Json::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings message shaped like the device's, three levels deep.
    fn settings() -> Vec<u8> {
        let mut global = Vec::new();
        proto::field_varint(2, 1, &mut global); // tap
        proto::field_varint(12, 1, &mut global); // dominant hand: right
        proto::field_varint(14, 1, &mut global); // battery saving
        proto::field_varint(power::IDLE_TIMEOUT as u32, 180, &mut global);
        proto::field_varint(power::BACKLIGHT_TIMEOUT as u32, 300, &mut global);

        let mut touchpad = Vec::new();
        proto::field_varint(1, 1, &mut touchpad);
        proto::field_varint(2, 1, &mut touchpad);
        proto::field_varint(3, 1, &mut touchpad);

        let mut left = Vec::new();
        proto::field_varint(2, 3, &mut left);
        let mut right = Vec::new();
        proto::field_varint(2, 4, &mut right);
        let mut slider = Vec::new();
        proto::field_bytes(SLIDER_LEFT, &left, &mut slider);
        proto::field_bytes(SLIDER_RIGHT, &right, &mut slider);

        let mut touchzone = Vec::new();
        proto::field_bytes(TOUCHZONE_TOUCHPAD, &touchpad, &mut touchzone);
        proto::field_bytes(TOUCHZONE_SLIDER, &slider, &mut touchzone);

        let mut profile = Vec::new();
        proto::field_varint(1, 7, &mut profile); // id
        proto::field_bytes(PROFILE_TOUCHZONE, &touchzone, &mut profile);
        proto::field_bytes(crate::backlight::PROFILE_BACKLIGHT, b"backlight", &mut profile);

        let mut out = Vec::new();
        proto::field_bytes(crate::backlight::APPSETTINGS_GLOBAL, &global, &mut out);
        proto::field_bytes(crate::backlight::APPSETTINGS_PROFILE, &profile, &mut out);
        proto::field_varint(3, 283, &mut out);
        out
    }

    fn get(s: &[u8], key: &str) -> Option<i64> {
        read_one(s, &find(key).unwrap())
    }

    #[test]
    fn settings_are_read_from_every_depth_they_live_at() {
        let s = settings();
        assert_eq!(get(&s, "tap"), Some(1), "global");
        assert_eq!(get(&s, "battery-saving"), Some(1), "global");
        assert_eq!(get(&s, "touchpad"), Some(1), "three levels down");
        assert_eq!(get(&s, "left-slider-sensitivity"), Some(3), "four levels down");
        assert_eq!(get(&s, "right-slider-sensitivity"), Some(4), "and its twin");
    }

    #[test]
    fn writing_one_setting_leaves_every_other_byte_alone() {
        let s = settings();
        let changed = write_one(&s, &find("left-slider-sensitivity").unwrap(), 4).unwrap();
        assert_eq!(get(&changed, "left-slider-sensitivity"), Some(4));
        // Its twin, its siblings and the whole rest of the tree survive.
        assert_eq!(get(&changed, "right-slider-sensitivity"), Some(4));
        assert_eq!(get(&changed, "touchpad"), Some(1));
        assert_eq!(get(&changed, "tap"), Some(1));
        let top = proto::parse(&changed).unwrap();
        assert_eq!(proto::first_varint(&top, 3), Some(283), "the write counter");
        let profile = proto::parse(proto::first_bytes(&top, 2).unwrap()).unwrap();
        assert_eq!(proto::first_varint(&profile, 1), Some(7), "the profile id");
        assert_eq!(
            proto::first_bytes(&profile, crate::backlight::PROFILE_BACKLIGHT).unwrap(),
            b"backlight",
            "the lighting is not this command's business"
        );
    }

    #[test]
    fn a_switch_takes_the_words_people_use_and_stores_one_or_zero() {
        let s = find("battery-saving").unwrap();
        for on in ["on", "yes", "true", "1", "Enabled"] {
            assert_eq!(parse_value(&s, on), Some(1), "{}", on);
        }
        for off in ["off", "no", "false", "0", "Disabled"] {
            assert_eq!(parse_value(&s, off), Some(0), "{}", off);
        }
        assert_eq!(parse_value(&s, "maybe"), None);
        assert_eq!(coerce(&s, 7), 1, "anything not zero is on");
        assert_eq!(describe(&s, 0), "off");
    }

    #[test]
    fn a_choice_only_ever_stores_one_of_its_options() {
        let s = find("dominant-hand").unwrap();
        assert_eq!(parse_value(&s, "Left"), Some(2));
        assert_eq!(parse_value(&s, "right"), Some(1));
        assert_eq!(describe(&s, 2), "Left");
        // A number nobody offers falls back rather than being written blind.
        assert_eq!(coerce(&s, 99), 0);
    }

    #[test]
    fn a_level_is_clamped_to_what_the_firmware_takes_not_to_what_the_ui_lists() {
        // The device refuses a 5 outright, so offering one would be a control
        // whose top setting always fails.
        let s = find("left-slider-sensitivity").unwrap();
        assert_eq!(coerce(&s, 0), 1);
        assert_eq!(coerce(&s, 9), SENSITIVITY_MAX as i64);
        assert_eq!(coerce(&s, 5), SENSITIVITY_MAX as i64);
        assert_eq!(describe(&s, 1), "Low");
        assert_eq!(describe(&s, SENSITIVITY_MAX as i64), "High");
    }

    #[test]
    fn a_field_this_keyboard_does_not_carry_reads_as_absent_not_as_off() {
        // The difference matters: a window must grey the row rather than draw
        // a switch that says "off" about something the firmware never had.
        let mut global = Vec::new();
        proto::field_varint(2, 1, &mut global);
        let mut out = Vec::new();
        proto::field_bytes(crate::backlight::APPSETTINGS_GLOBAL, &global, &mut out);
        assert_eq!(get(&out, "tap"), Some(1));
        assert_eq!(get(&out, "fn-lock"), None);
        assert_eq!(get(&out, "touchpad"), None, "no profile at all");
    }

    #[test]
    fn every_setting_is_uniquely_named_and_grouped() {
        let table = all();
        let mut keys: Vec<&str> = table.iter().map(|s| s.key).collect();
        keys.sort();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before, "two settings share a name");
        for s in &table {
            assert!(!s.label.is_empty() && !s.help.is_empty(), "{} is undocumented", s.key);
            assert!(groups().contains(&s.group));
            assert!(
                s.key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} must survive being one argument",
                s.key
            );
        }
        assert!(table.len() >= 20, "only {} settings modelled", table.len());
    }

    #[test]
    fn the_model_a_window_draws_from_names_every_row_and_its_options() {
        let m = model(&settings());
        let rows = m.get("settings").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), all().len());
        let hand = rows
            .iter()
            .find(|r| matches!(r.get("key"), Some(Json::Str(k)) if k == "dominant-hand"))
            .unwrap();
        assert_eq!(hand.get("kind"), Some(&Json::Str("choice".into())));
        assert_eq!(hand.get("options").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(hand.get("shown"), Some(&Json::Str("Right".into())));
        // A row the keyboard does not carry says so rather than reading "off".
        let missing = rows
            .iter()
            .find(|r| matches!(r.get("key"), Some(Json::Str(k)) if k == "fn-lock"))
            .unwrap();
        assert_eq!(missing.get("value"), Some(&Json::Null));
    }
}
