//! What each function key sends, and how to change it.
//!
//! The keyboard stores this in `ProfileSettings.keyboard` — an ordinary field
//! of the same message that carries the backlight, written by the same
//! `SET_SETTINGS`. Nothing about remapping needs a different transport, a
//! different permission, or a different risk profile from changing a colour.
//!
//! ## The shape
//!
//! ```text
//! ProfileSettings.keyboard (5)
//!   └ fKeys (1)
//!       ├ F1 (13) … F12 (24)
//!       │   └ slots (5)
//!       │       └ slot (1..4) { page = 1, usage = 2 }
//! ```
//!
//! Four slots per key, because a binding can be a chord: the factory F12 is
//! Left Shift + Left GUI + `4`, the macOS screenshot shortcut. Empty slots are
//! `{page: 0, usage: 0}` and are written out in full — the array is fixed
//! width, exactly like the colour markers.
//!
//! `page` selects the HID usage table: **0 is the Keyboard page (0x07)** and
//! **1 is the Consumer page (0x0C)**. So the values are standard HID usages
//! rather than a private table, which is what makes this decodable at all.
//!
//! ## Provenance
//!
//! Read off a CLVX S and decoded against the published HID usage tables — the
//! factory function row came back as brightness, transport controls, mute and
//! the screenshot chord, which is what confirms the reading. See
//! `docs/PROTOCOL.md`. No vendor code or table was copied.

use crate::json::Json;
use crate::proto;

/// `ProfileSettings.keyboard`.
pub const PROFILE_KEYBOARD: u32 = 5;
/// The submessage holding the function row.
const FKEYS: u32 = 1;
/// The slot array inside one key's entry.
const SLOTS: u32 = 5;
const SLOT_PAGE: u32 = 1;
const SLOT_USAGE: u32 = 2;

/// How many slots a binding holds. Fixed width, like the colour markers.
const SLOT_COUNT: usize = 4;

/// HID usage pages, as this message numbers them.
pub const PAGE_KEYBOARD: u32 = 0;
pub const PAGE_CONSUMER: u32 = 1;

/// F1 lives at field 13, so F*n* is at 12 + *n*.
pub fn field_for(fkey: u32) -> Option<u32> {
    if (1..=12).contains(&fkey) { Some(12 + fkey) } else { None }
}

#[allow(dead_code)] // asserted by the tests, which `cargo check` cannot see
pub fn fkey_for(field: u32) -> Option<u32> {
    if (13..=24).contains(&field) { Some(field - 12) } else { None }
}

/// One usage in a binding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot {
    pub page: u32,
    pub usage: u32,
}

impl Slot {
    pub fn is_empty(self) -> bool {
        self.usage == 0
    }
}

/// What one key sends: up to four usages at once.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Binding {
    pub slots: Vec<Slot>,
}

impl Binding {
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    /// The name this project gives the binding, or the raw usages when it is
    /// something we have no name for. An unknown binding is shown, not hidden:
    /// a remapper that silently renders "—" for anything it did not expect is
    /// how a person loses a mapping they set in the vendor's app.
    pub fn label(&self) -> String {
        if self.is_empty() {
            return "nothing".into();
        }
        if let Some(a) = ACTIONS.iter().find(|a| a.slots() == self.slots) {
            return a.name.to_string();
        }
        self.slots
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| usage_name(s.page, s.usage))
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

/// A binding a person can pick by name.
pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    /// `(page, usage)` in order; the rest of the four slots are padding.
    pub usages: &'static [(u32, u32)],
}

impl Action {
    fn slots(&self) -> Vec<Slot> {
        let mut v: Vec<Slot> = self
            .usages
            .iter()
            .map(|&(page, usage)| Slot { page, usage })
            .collect();
        while v.len() < SLOT_COUNT {
            v.push(Slot { page: 0, usage: 0 });
        }
        v
    }

    pub fn binding(&self) -> Binding {
        Binding { slots: self.slots() }
    }
}

/// Everything a function key can be set to.
///
/// Deliberately the things a laptop function row does, plus the plain F-keys.
/// The device will take any HID usage; this is the list worth offering, and
/// `keys <n> raw <page> <usage>` exists for anything else.
pub static ACTIONS: &[Action] = &[
    Action { id: "nothing",         name: "Nothing",            usages: &[] },
    Action { id: "brightness-down", name: "Brightness down",    usages: &[(PAGE_CONSUMER, 0x70)] },
    Action { id: "brightness-up",   name: "Brightness up",      usages: &[(PAGE_CONSUMER, 0x6F)] },
    Action { id: "previous",        name: "Previous track",     usages: &[(PAGE_CONSUMER, 0xB6)] },
    Action { id: "play-pause",      name: "Play / pause",       usages: &[(PAGE_CONSUMER, 0xCD)] },
    Action { id: "next",            name: "Next track",         usages: &[(PAGE_CONSUMER, 0xB5)] },
    Action { id: "mute",            name: "Mute",               usages: &[(PAGE_CONSUMER, 0xE2)] },
    Action { id: "volume-down",     name: "Volume down",        usages: &[(PAGE_CONSUMER, 0xEA)] },
    Action { id: "volume-up",       name: "Volume up",          usages: &[(PAGE_CONSUMER, 0xE9)] },
    Action { id: "screenshot",      name: "Screenshot area",
             usages: &[(PAGE_KEYBOARD, 0xE1), (PAGE_KEYBOARD, 0xE3), (PAGE_KEYBOARD, 0x21)] },
    Action { id: "screenshot-full", name: "Screenshot screen",
             usages: &[(PAGE_KEYBOARD, 0xE1), (PAGE_KEYBOARD, 0xE3), (PAGE_KEYBOARD, 0x20)] },
    Action { id: "mission-control", name: "Mission Control",    usages: &[(PAGE_CONSUMER, 0x29F)] },
    Action { id: "spotlight",       name: "Spotlight",
             usages: &[(PAGE_KEYBOARD, 0xE3), (PAGE_KEYBOARD, 0x2C)] },
    Action { id: "lock",            name: "Lock screen",
             usages: &[(PAGE_KEYBOARD, 0xE0), (PAGE_KEYBOARD, 0xE3), (PAGE_KEYBOARD, 0x16)] },
    Action { id: "f1",  name: "F1",  usages: &[(PAGE_KEYBOARD, 0x3A)] },
    Action { id: "f2",  name: "F2",  usages: &[(PAGE_KEYBOARD, 0x3B)] },
    Action { id: "f3",  name: "F3",  usages: &[(PAGE_KEYBOARD, 0x3C)] },
    Action { id: "f4",  name: "F4",  usages: &[(PAGE_KEYBOARD, 0x3D)] },
    Action { id: "f5",  name: "F5",  usages: &[(PAGE_KEYBOARD, 0x3E)] },
    Action { id: "f6",  name: "F6",  usages: &[(PAGE_KEYBOARD, 0x3F)] },
    Action { id: "f7",  name: "F7",  usages: &[(PAGE_KEYBOARD, 0x40)] },
    Action { id: "f8",  name: "F8",  usages: &[(PAGE_KEYBOARD, 0x41)] },
    Action { id: "f9",  name: "F9",  usages: &[(PAGE_KEYBOARD, 0x42)] },
    Action { id: "f10", name: "F10", usages: &[(PAGE_KEYBOARD, 0x43)] },
    Action { id: "f11", name: "F11", usages: &[(PAGE_KEYBOARD, 0x44)] },
    Action { id: "f12", name: "F12", usages: &[(PAGE_KEYBOARD, 0x45)] },
];

pub fn action(id: &str) -> Option<&'static Action> {
    let want = id.trim().to_ascii_lowercase().replace([' ', '_'], "-");
    ACTIONS.iter().find(|a| a.id == want)
}

/// A readable name for a usage we have no action for.
fn usage_name(page: u32, usage: u32) -> String {
    if page == PAGE_KEYBOARD {
        let named = match usage {
            0xE0 => "Ctrl", 0xE1 => "Shift", 0xE2 => "Opt", 0xE3 => "Cmd",
            0xE4 => "Right Ctrl", 0xE5 => "Right Shift", 0xE6 => "Right Opt",
            0xE7 => "Right Cmd", 0x2C => "Space", 0x28 => "Return", 0x29 => "Esc",
            _ => "",
        };
        if !named.is_empty() {
            return named.into();
        }
        if (0x04..=0x1D).contains(&usage) {
            return ((b'A' + (usage - 0x04) as u8) as char).to_string();
        }
        if (0x1E..=0x26).contains(&usage) {
            return ((b'1' + (usage - 0x1E) as u8) as char).to_string();
        }
        if usage == 0x27 {
            return "0".into();
        }
        if (0x3A..=0x45).contains(&usage) {
            return format!("F{}", usage - 0x39);
        }
    }
    format!("{}/0x{:X}", if page == PAGE_CONSUMER { "consumer" } else { "key" }, usage)
}

// ─────────────────────────────── reading ─────────────────────────────────

/// Every function key's binding, read out of an `AppSettings` blob.
///
/// A keyboard that carries no keyboard section returns an empty map rather
/// than an error: not every model in the family has a remappable row.
pub fn read(settings: &[u8]) -> Vec<(u32, Binding)> {
    let mut out = Vec::new();
    let Some(app) = proto::parse(settings) else { return out };
    let Some(profile) = proto::first_bytes(&app, crate::backlight::APPSETTINGS_PROFILE) else {
        return out;
    };
    let Some(profile) = proto::parse(profile) else { return out };
    let Some(kb) = proto::first_bytes(&profile, PROFILE_KEYBOARD) else { return out };
    let Some(kb) = proto::parse(kb) else { return out };
    let Some(fkeys) = proto::first_bytes(&kb, FKEYS) else { return out };
    let Some(fkeys) = proto::parse(&fkeys.clone()) else { return out };

    for n in 1..=12u32 {
        let Some(field) = field_for(n) else { continue };
        let Some(entry) = proto::first_bytes(&fkeys, field) else { continue };
        let Some(entry) = proto::parse(entry) else { continue };
        let Some(slots) = proto::first_bytes(&entry, SLOTS) else { continue };
        let Some(slots) = proto::parse(slots) else { continue };

        let mut b = Binding::default();
        for i in 1..=SLOT_COUNT as u32 {
            let Some(raw) = proto::first_bytes(&slots, i) else { continue };
            let Some(one) = proto::parse(raw) else { continue };
            b.slots.push(Slot {
                page: proto::first_varint(&one, SLOT_PAGE).unwrap_or(0) as u32,
                usage: proto::first_varint(&one, SLOT_USAGE).unwrap_or(0) as u32,
            });
        }
        out.push((n, b));
    }
    out
}

// ─────────────────────────────── writing ─────────────────────────────────

fn encode_slots(b: &Binding) -> Vec<u8> {
    let mut slots = Vec::new();
    for i in 0..SLOT_COUNT {
        let s = b.slots.get(i).copied().unwrap_or(Slot { page: 0, usage: 0 });
        let mut one = Vec::new();
        proto::field_varint(SLOT_PAGE, s.page as u64, &mut one);
        proto::field_varint(SLOT_USAGE, s.usage as u64, &mut one);
        proto::field_bytes(i as u32 + 1, &one, &mut slots);
    }
    slots
}

/// Set one function key, carrying everything else through untouched.
///
/// Every level is parsed and re-serialised rather than spliced, which is what
/// keeps a field this code has never heard of — on a firmware this code has
/// never seen — exactly where it was.
pub fn write(settings: &[u8], fkey: u32, binding: &Binding) -> Result<Vec<u8>, String> {
    let field = field_for(fkey).ok_or_else(|| format!("there is no F{}", fkey))?;

    let mut app = proto::parse(settings).ok_or("settings could not be parsed")?;
    let profile_raw = proto::first_bytes(&app, crate::backlight::APPSETTINGS_PROFILE)
        .cloned()
        .ok_or("these settings carry no profile")?;
    let mut profile = proto::parse(&profile_raw).ok_or("the profile could not be parsed")?;
    let kb_raw = proto::first_bytes(&profile, PROFILE_KEYBOARD)
        .cloned()
        .ok_or("this keyboard does not carry a remappable function row")?;
    let mut kb = proto::parse(&kb_raw).ok_or("the keyboard section could not be parsed")?;
    let fkeys_raw = proto::first_bytes(&kb, FKEYS).cloned().unwrap_or_default();
    let mut fkeys = proto::parse(&fkeys_raw).ok_or("the function row could not be parsed")?;

    // The entry keeps whatever else it holds; only its slot array is replaced.
    let entry_raw = proto::first_bytes(&fkeys, field).cloned().unwrap_or_default();
    let mut entry = proto::parse(&entry_raw).unwrap_or_default();
    entry.insert(SLOTS, vec![proto::Value::Bytes(encode_slots(binding))]);

    fkeys.insert(field, vec![proto::Value::Bytes(proto::serialize(&entry))]);
    kb.insert(FKEYS, vec![proto::Value::Bytes(proto::serialize(&fkeys))]);
    profile.insert(PROFILE_KEYBOARD, vec![proto::Value::Bytes(proto::serialize(&kb))]);
    app.insert(
        crate::backlight::APPSETTINGS_PROFILE,
        vec![proto::Value::Bytes(proto::serialize(&profile))],
    );
    Ok(proto::serialize(&app))
}

/// The function row as JSON, for a window to lay out.
pub fn model(settings: &[u8]) -> Json {
    let rows: Vec<Json> = read(settings)
        .into_iter()
        .map(|(n, b)| {
            let matched = ACTIONS.iter().find(|a| a.slots() == b.slots);
            Json::obj(vec![
                ("key", Json::Str(format!("f{}", n))),
                ("name", Json::Str(format!("F{}", n))),
                ("label", Json::Str(b.label())),
                ("action", matched.map(|a| Json::Str(a.id.into())).unwrap_or(Json::Null)),
            ])
        })
        .collect();
    let offered: Vec<Json> = ACTIONS
        .iter()
        .map(|a| {
            Json::obj(vec![
                ("id", Json::Str(a.id.into())),
                ("name", Json::Str(a.name.into())),
            ])
        })
        .collect();
    Json::obj(vec![("keys", Json::Arr(rows)), ("actions", Json::Arr(offered))])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ProfileSettings.keyboard` exactly as a CLVX S returned it, wrapped in
    /// the two levels above it. A fixture from the device rather than from this
    /// encoder: a test that builds its own input with the code under test
    /// cannot fail.
    fn live_settings() -> Vec<u8> {
        let kb = hex(concat!(
            "0ad10192011a2a180a04080110701204080010001a04080010002204080010009a011a2a180a0408",
            "01106f1204080010001a0408001000220408001000a2011b2a190a05080110b6011204080010001a",
            "0408001000220408001000aa011b2a190a05080110cd011204080010001a0408001000220408001000",
            "b2011b2a190a05080110b5011204080010001a0408001000220408001000ba011b2a190a05080110",
            "e2011204080010001a0408001000220408001000c2011c2a1a0a05080010e1011205080010e3011a",
            "0408001021220408001000"
        ));
        let mut profile = Vec::new();
        proto::field_bytes(PROFILE_KEYBOARD, &kb, &mut profile);
        let mut app = Vec::new();
        proto::field_bytes(crate::backlight::APPSETTINGS_PROFILE, &profile, &mut app);
        app
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn the_factory_function_row_decodes_to_what_is_printed_on_it() {
        let map = read(&live_settings());
        let by = |n: u32| map.iter().find(|(k, _)| *k == n).map(|(_, b)| b.label());
        // These are the glyphs on the physical keys, which is what makes this
        // a check rather than a restatement of the encoder.
        assert_eq!(by(6).as_deref(), Some("Brightness down"));
        assert_eq!(by(7).as_deref(), Some("Brightness up"));
        assert_eq!(by(8).as_deref(), Some("Previous track"));
        assert_eq!(by(9).as_deref(), Some("Play / pause"));
        assert_eq!(by(10).as_deref(), Some("Next track"));
        assert_eq!(by(11).as_deref(), Some("Mute"));
        assert_eq!(by(12).as_deref(), Some("Screenshot area"));
    }

    #[test]
    fn setting_a_key_changes_that_key_and_nothing_else() {
        let before = live_settings();
        let after = write(&before, 9, &action("mute").unwrap().binding()).unwrap();

        let a = read(&after);
        let b = read(&before);
        for (n, binding) in &b {
            let now = a.iter().find(|(k, _)| k == n).map(|(_, x)| x).unwrap();
            if *n == 9 {
                assert_eq!(now.label(), "Mute", "F9 did not change");
            } else {
                assert_eq!(now, binding, "F{} changed and should not have", n);
            }
        }
    }

    #[test]
    fn a_chord_survives_the_round_trip() {
        // Three usages at once, which is the shape the factory F12 uses and the
        // one a naive single-usage encoder would quietly flatten.
        let shot = action("screenshot").unwrap().binding();
        let out = write(&live_settings(), 5, &shot).unwrap();
        let got = read(&out).into_iter().find(|(n, _)| *n == 5).unwrap().1;
        assert_eq!(got.slots.iter().filter(|s| !s.is_empty()).count(), 3);
        assert_eq!(got.label(), "Screenshot area");
    }

    #[test]
    fn the_slot_array_is_always_four_wide() {
        // Same rule as the colour markers: a short array is a refusal, so the
        // padding is written out rather than omitted.
        let out = write(&live_settings(), 3, &action("mute").unwrap().binding()).unwrap();
        let got = read(&out).into_iter().find(|(n, _)| *n == 3).unwrap().1;
        assert_eq!(got.slots.len(), 4);
    }

    #[test]
    fn an_unknown_binding_is_described_rather_than_hidden() {
        let odd = Binding { slots: vec![Slot { page: PAGE_KEYBOARD, usage: 0x04 }] };
        assert_eq!(odd.label(), "A");
        let weird = Binding { slots: vec![Slot { page: PAGE_CONSUMER, usage: 0x999 }] };
        assert!(weird.label().contains("0x999"), "{}", weird.label());
    }

    #[test]
    fn every_action_round_trips_through_the_encoder() {
        for a in ACTIONS {
            let out = write(&live_settings(), 4, &a.binding()).unwrap();
            let got = read(&out).into_iter().find(|(n, _)| *n == 4).unwrap().1;
            assert_eq!(got.slots, a.slots(), "{} did not survive", a.id);
        }
    }

    #[test]
    fn f_numbers_map_to_the_fields_the_device_uses() {
        assert_eq!(field_for(1), Some(13));
        assert_eq!(field_for(12), Some(24));
        assert_eq!(field_for(13), None);
        assert_eq!(fkey_for(18), Some(6));
    }
}
