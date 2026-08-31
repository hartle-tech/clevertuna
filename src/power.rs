//! When the light goes out: the two timeouts, and nothing else.
//!
//! The backlight is the largest thing this keyboard spends power on, so the
//! firmware turns it off twice: once when nobody has touched it for a while
//! (*idle*), and once outright after longer (*backlight*). They are the vendor
//! application's "Idle timeout" and "Backlight timeout", and they live in
//! `GlobalSettings` rather than in a profile — they are how the keyboard
//! behaves, not how it looks, and they survive a change of theme.
//!
//! ## Provenance
//!
//! The two field numbers were read off a real CLVX S: `GlobalSettings` field
//! **20** held 180 and field **21** held 300, which are exactly two of the
//! values the vendor's own dropdowns offer, and they satisfy the rule that
//! application enforces — idle never longer than backlight. The option lists
//! below are the closed sets the device is offered, which is interoperability
//! fact, not anyone's expression. `docs/PROTOCOL.md` §8 records it. No
//! third-party code was copied; see `NOTICE`.

use crate::proto;

/// `GlobalSettings.idleBacklightTime`, seconds. 0 means "never dim".
pub const IDLE_TIMEOUT: u32 = 20;
/// `GlobalSettings.backlightTime`, seconds. 0 means "always on".
pub const BACKLIGHT_TIMEOUT: u32 = 21;

/// What the backlight timeout accepts, in seconds. 0 is always on.
pub const BACKLIGHT_CHOICES: [u32; 5] = [0, 300, 600, 1_800, 3_600];
/// What the idle timeout accepts, in seconds. 0 is off.
pub const IDLE_CHOICES: [u32; 7] = [0, 30, 60, 180, 300, 600, 1_800];

/// The two timeouts, as the keyboard holds them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timeouts {
    /// Dim after this long without input. 0 = never.
    pub idle: u32,
    /// Off after this long. 0 = always on.
    pub backlight: u32,
}

impl Default for Timeouts {
    fn default() -> Timeouts {
        Timeouts { idle: 180, backlight: 300 }
    }
}

/// Seconds as a person says them.
pub fn describe(seconds: u32, zero: &str) -> String {
    match seconds {
        0 => zero.to_string(),
        s if s < 60 => format!("{} seconds", s),
        s if s == 60 => "1 minute".into(),
        s if s % 3_600 == 0 => {
            let h = s / 3_600;
            if h == 1 { "1 hour".into() } else { format!("{} hours", h) }
        }
        s => format!("{} minutes", s / 60),
    }
}

/// Accept seconds, or the words the choices are usually written as.
pub fn parse_seconds(s: &str) -> Option<u32> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "off" | "never" | "always" | "always-on" | "always on" | "0" => return Some(0),
        _ => {}
    }
    if let Some(n) = t.strip_suffix('h').and_then(|n| n.trim().parse::<u32>().ok()) {
        return Some(n * 3_600);
    }
    if let Some(n) = t.strip_suffix('m').and_then(|n| n.trim().parse::<u32>().ok()) {
        return Some(n * 60);
    }
    if let Some(n) = t.strip_suffix('s').and_then(|n| n.trim().parse::<u32>().ok()) {
        return Some(n);
    }
    t.parse::<u32>().ok()
}

/// Snap to the nearest value the device is offered.
///
/// The firmware may well take any number, but nobody has checked and a value
/// the vendor application cannot display is a setting its owner can no longer
/// see. So an unusual request lands on the nearest offered one rather than
/// being refused or written blind.
pub fn nearest(choices: &[u32], want: u32) -> u32 {
    // 0 is a mode ("always on"), not a short duration, so it is only ever
    // chosen when asked for exactly.
    if want == 0 {
        return 0;
    }
    choices
        .iter()
        .copied()
        .filter(|c| *c != 0)
        .min_by_key(|c| c.abs_diff(want))
        .unwrap_or(want)
}

impl Timeouts {
    /// Apply the rule the vendor application enforces: the light cannot dim
    /// after it has already gone out.
    pub fn coherent(self) -> Timeouts {
        let backlight = nearest(&BACKLIGHT_CHOICES, self.backlight);
        let mut idle = nearest(&IDLE_CHOICES, self.idle);
        if backlight != 0 && idle > backlight {
            idle = backlight;
        }
        Timeouts { idle, backlight }
    }

    pub fn describe(self) -> String {
        format!(
            "dims after {}, off after {}",
            describe(self.idle, "never"),
            describe(self.backlight, "never")
        )
    }
}

/// Read the two timeouts out of a whole `AppSettings` message.
pub fn read(settings: &[u8]) -> Option<Timeouts> {
    let top = proto::parse(settings)?;
    let global = proto::parse(proto::first_bytes(&top, crate::backlight::APPSETTINGS_GLOBAL)?)?;
    Some(Timeouts {
        idle: proto::first_varint(&global, IDLE_TIMEOUT).unwrap_or(0) as u32,
        backlight: proto::first_varint(&global, BACKLIGHT_TIMEOUT).unwrap_or(0) as u32,
    })
}

/// Put them back, leaving every other byte of the settings alone.
///
/// The same discipline as the backlight splice: everything this tool does not
/// model — dominant hand, gestures, key maps, whatever a firmware adds next —
/// is carried through untouched, so changing when the light goes out cannot
/// quietly change anything else.
pub fn write(settings: &[u8], want: Timeouts) -> Result<Vec<u8>, String> {
    let want = want.coherent();
    let top = proto::parse(settings).ok_or("device settings could not be parsed")?;
    let raw = proto::first_bytes(&top, crate::backlight::APPSETTINGS_GLOBAL)
        .ok_or("device settings carry no global section")?;
    let mut global = proto::parse(raw).ok_or("the global section could not be parsed")?;
    global.insert(IDLE_TIMEOUT, vec![proto::Value::Varint(want.idle as u64)]);
    global.insert(BACKLIGHT_TIMEOUT, vec![proto::Value::Varint(want.backlight as u64)]);
    Ok(proto::replace_field(&top, crate::backlight::APPSETTINGS_GLOBAL, proto::serialize(&global)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings message shaped like the device's: a global section, a
    /// profile, and a counter.
    fn settings(idle: u64, backlight: u64) -> Vec<u8> {
        let mut global = Vec::new();
        proto::field_varint(2, 1, &mut global);
        proto::field_varint(IDLE_TIMEOUT, idle, &mut global);
        proto::field_varint(BACKLIGHT_TIMEOUT, backlight, &mut global);
        proto::field_varint(22, 1, &mut global);
        let mut out = Vec::new();
        proto::field_bytes(crate::backlight::APPSETTINGS_GLOBAL, &global, &mut out);
        proto::field_bytes(crate::backlight::APPSETTINGS_PROFILE, b"profile", &mut out);
        proto::field_varint(3, 283, &mut out);
        out
    }

    #[test]
    fn the_timeouts_read_back_as_written() {
        let s = settings(180, 300);
        assert_eq!(read(&s), Some(Timeouts { idle: 180, backlight: 300 }));
        let changed = write(&s, Timeouts { idle: 60, backlight: 1_800 }).unwrap();
        assert_eq!(read(&changed), Some(Timeouts { idle: 60, backlight: 1_800 }));
    }

    #[test]
    fn writing_a_timeout_touches_nothing_else() {
        let s = settings(180, 300);
        let changed = write(&s, Timeouts { idle: 30, backlight: 600 }).unwrap();
        let before = proto::parse(&s).unwrap();
        let after = proto::parse(&changed).unwrap();
        // The profile and the counter are carried through byte-for-byte.
        assert_eq!(
            proto::first_bytes(&after, crate::backlight::APPSETTINGS_PROFILE),
            proto::first_bytes(&before, crate::backlight::APPSETTINGS_PROFILE)
        );
        assert_eq!(proto::first_varint(&after, 3), Some(283));
        // …and so is every global field this tool does not model.
        let g_before = proto::parse(proto::first_bytes(&before, 1).unwrap()).unwrap();
        let g_after = proto::parse(proto::first_bytes(&after, 1).unwrap()).unwrap();
        assert_eq!(proto::first_varint(&g_after, 2), proto::first_varint(&g_before, 2));
        assert_eq!(proto::first_varint(&g_after, 22), proto::first_varint(&g_before, 22));
    }

    #[test]
    fn the_light_cannot_dim_after_it_has_gone_out() {
        // The rule the vendor application enforces, and the reason it does:
        // an idle timeout longer than the backlight one can never fire.
        let t = Timeouts { idle: 1_800, backlight: 300 }.coherent();
        assert_eq!(t.idle, 300);
        // "Always on" is not a short backlight, so it does not clamp the idle.
        let t = Timeouts { idle: 1_800, backlight: 0 }.coherent();
        assert_eq!(t, Timeouts { idle: 1_800, backlight: 0 });
    }

    #[test]
    fn an_unusual_value_lands_on_the_nearest_one_the_device_is_offered() {
        // A value the vendor app cannot display is a setting its owner can no
        // longer see, so this snaps rather than writing something exotic.
        assert_eq!(nearest(&BACKLIGHT_CHOICES, 400), 300);
        assert_eq!(nearest(&BACKLIGHT_CHOICES, 700), 600);
        assert_eq!(nearest(&IDLE_CHOICES, 45), 30);
        assert_eq!(nearest(&IDLE_CHOICES, 200), 180);
        // Zero is a mode, not a duration: only an exact ask gets it.
        assert_eq!(nearest(&BACKLIGHT_CHOICES, 0), 0);
        assert_eq!(nearest(&BACKLIGHT_CHOICES, 10), 300);
    }

    #[test]
    fn durations_are_read_and_written_the_way_people_say_them() {
        assert_eq!(describe(0, "never"), "never");
        assert_eq!(describe(30, "off"), "30 seconds");
        assert_eq!(describe(60, "off"), "1 minute");
        assert_eq!(describe(300, "off"), "5 minutes");
        assert_eq!(describe(3_600, "off"), "1 hour");
        assert_eq!(parse_seconds("5m"), Some(300));
        assert_eq!(parse_seconds("1h"), Some(3_600));
        assert_eq!(parse_seconds("30s"), Some(30));
        assert_eq!(parse_seconds("off"), Some(0));
        assert_eq!(parse_seconds("1800"), Some(1_800));
        assert_eq!(parse_seconds("soon"), None);
    }

    #[test]
    fn a_settings_message_without_a_global_section_is_refused_not_invented() {
        let mut out = Vec::new();
        proto::field_bytes(crate::backlight::APPSETTINGS_PROFILE, b"profile", &mut out);
        assert!(read(&out).is_none());
        assert!(write(&out, Timeouts::default()).is_err());
    }
}
