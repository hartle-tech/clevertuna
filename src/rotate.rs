//! Letting the keyboard change its own mind, on a clock.
//!
//! Two things people ask of a lighting tool once it has themes: *give me a
//! different one every so often*, and *be warm in the evening*. They are the
//! same feature — a rule that turns the time into the name of a theme — so
//! there is one engine and one config file rather than two.
//!
//! ## Why this is all pure functions
//!
//! Nothing here talks to a keyboard, reads a clock, or spawns anything. The
//! whole engine is `plan + a timestamp -> which theme, or nothing`, which means
//! a year of behaviour can be checked in a test suite in milliseconds. The
//! caller supplies the time and performs the write.
//!
//! ## Why nothing here is a daemon
//!
//! A background process that owns the keyboard would take the one connection
//! the hardware grants, and then no other surface could write at all. So this
//! is a *tick*: something already resident asks "has the slot changed?", and
//! almost always the answer is no and nothing is opened. On macOS the menu-bar
//! app is that something; elsewhere it is a cron line.

use crate::json::{self, Json};

/// How often the theme is allowed to change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Every {
    /// Every N minutes. Never fewer than [`MIN_MINUTES`].
    Minutes(u32),
    Hour,
    Day,
    Week,
    Month,
}

/// The shortest gap on offer.
///
/// Every change is a flash write, and flash has a finite erase budget. Once a
/// minute is 1440 writes a day, which is a decision nobody should make by
/// accident; five is 288, and the longer offers below cost less again. So the
/// short end of the range is a floor rather than a warning.
pub const MIN_MINUTES: u32 = 5;

/// The minute cadences offered, shortest first.
pub const MINUTE_CHOICES: [u32; 3] = [5, 15, 30];

impl Every {
    pub fn key(self) -> String {
        match self {
            Every::Minutes(n) => format!("{}m", n),
            Every::Hour => "hour".into(),
            Every::Day => "day".into(),
            Every::Week => "week".into(),
            Every::Month => "month".into(),
        }
    }

    /// How a menu should name it.
    pub fn label(self) -> String {
        match self {
            Every::Minutes(n) => format!("Every {} minutes", n),
            Every::Hour => "Every hour".into(),
            Every::Day => "Every day".into(),
            Every::Week => "Every week".into(),
            Every::Month => "Every month".into(),
        }
    }

    pub fn parse(s: &str) -> Option<Every> {
        let t = s.trim().to_ascii_lowercase();
        let t = t.strip_prefix("every").unwrap_or(&t).trim();
        match t {
            "hour" | "hourly" | "1h" => return Some(Every::Hour),
            "day" | "daily" | "1d" => return Some(Every::Day),
            "week" | "weekly" | "1w" => return Some(Every::Week),
            "month" | "monthly" | "1mo" => return Some(Every::Month),
            _ => {}
        }
        // "5m", "15 minutes", "30"
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let rest = t[digits.len()..].trim();
        if !(rest.is_empty() || rest.starts_with('m')) {
            return None;
        }
        let n: u32 = digits.parse().ok()?;
        Some(Every::Minutes(n.max(MIN_MINUTES)))
    }

    pub fn all() -> Vec<Every> {
        let mut v: Vec<Every> = MINUTE_CHOICES.iter().map(|n| Every::Minutes(*n)).collect();
        v.extend([Every::Hour, Every::Day, Every::Week, Every::Month]);
        v
    }

    /// Roughly how many writes a day this cadence costs.
    ///
    /// Applying a theme is a flash write. Flash has a finite erase budget, and
    /// a minute cadence spends about fourteen hundred of them a day — which is
    /// a decision somebody should make on purpose rather than discover.
    pub fn writes_per_day(self) -> u32 {
        match self {
            Every::Minutes(n) => 1_440 / n.max(1),
            Every::Hour => 24,
            Every::Day => 1,
            Every::Week => 1,
            Every::Month => 1,
        }
    }

    /// The number of the slot this instant falls in.
    ///
    /// Slots are absolute, not relative to when rotation was switched on, so
    /// "every hour" changes on the hour rather than seventeen minutes past it,
    /// and two machines told the same plan agree without talking.
    pub fn slot(self, unix_secs: u64) -> u64 {
        match self {
            Every::Minutes(n) => unix_secs / (60 * n.max(MIN_MINUTES) as u64),
            Every::Hour => unix_secs / 3_600,
            Every::Day => unix_secs / 86_400,
            // Anchored to the epoch, which was a Thursday; the week it starts
            // on does not matter, only that it is the same every time.
            Every::Week => unix_secs / 604_800,
            Every::Month => {
                let (y, m, _) = civil_from_days((unix_secs / 86_400) as i64);
                (y as u64) * 12 + m as u64
            }
        }
    }
}

/// What the clock is used to decide.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Nothing happens.
    Off,
    /// Step through the chosen themes in order.
    Cycle,
    /// A different theme each slot, rolled from the slot number — so the same
    /// hour of the same day always produces the same theme, and "what was that
    /// one?" has an answer.
    Random,
    /// One theme by day, another by night.
    DayNight,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Cycle => "cycle",
            Mode::Random => "random",
            Mode::DayNight => "day-night",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" => Some(Mode::Off),
            "cycle" | "themes" | "list" => Some(Mode::Cycle),
            "random" | "surprise" => Some(Mode::Random),
            "day-night" | "daynight" | "day/night" => Some(Mode::DayNight),
            _ => None,
        }
    }
}

/// The default hours. Sunrise-ish and sunset-ish, on average, somewhere.
pub const DAY_FROM: u32 = 7;
pub const NIGHT_FROM: u32 = 19;

/// What to rotate through, and how often.
#[derive(Clone, PartialEq, Debug)]
pub struct Plan {
    pub mode: Mode,
    pub every: Every,
    /// Theme references: a preset id, a saved name, `random`, or `wallpaper`.
    pub picks: Vec<String>,
    pub day: String,
    pub night: String,
    /// Local hour the day theme starts, and the hour the night theme starts.
    pub day_from: u32,
    pub night_from: u32,
    /// The slot last acted on, so a tick that changes nothing does nothing.
    pub last_slot: Option<u64>,
    pub last_theme: Option<String>,
    /// Minutes east of UTC. The clock this reads is UTC; day and night are a
    /// local idea, so the offset has to be carried rather than assumed.
    pub utc_offset_minutes: i32,
    /// Put the current theme back on when the keyboard is reached over a
    /// different slot than it was last written on.
    pub follow_slots: bool,
}

impl Default for Plan {
    fn default() -> Plan {
        Plan {
            mode: Mode::Off,
            every: Every::Hour,
            picks: Vec::new(),
            day: "deep-current".into(),
            night: "nightshift".into(),
            day_from: DAY_FROM,
            night_from: NIGHT_FROM,
            last_slot: None,
            last_theme: None,
            utc_offset_minutes: 0,
            follow_slots: false,
        }
    }
}

impl Plan {
    /// The theme this instant calls for, whatever happened before.
    pub fn theme_at(&self, unix_secs: u64) -> Option<String> {
        match self.mode {
            Mode::Off => None,
            Mode::Cycle => {
                if self.picks.is_empty() {
                    return None;
                }
                let slot = self.every.slot(unix_secs);
                Some(self.picks[(slot % self.picks.len() as u64) as usize].clone())
            }
            // The slot number is the seed, so the roll is reproducible: the
            // theme is a fact about the hour, not about when you asked.
            Mode::Random => Some(format!("random:{}", self.every.slot(unix_secs))),
            Mode::DayNight => Some(if self.is_daytime(unix_secs) {
                self.day.clone()
            } else {
                self.night.clone()
            }),
        }
    }

    /// Local hour of day, 0–23.
    pub fn local_hour(&self, unix_secs: u64) -> u32 {
        let shifted = unix_secs as i64 + self.utc_offset_minutes as i64 * 60;
        let shifted = shifted.rem_euclid(86_400);
        (shifted / 3_600) as u32
    }

    pub fn is_daytime(&self, unix_secs: u64) -> bool {
        let h = self.local_hour(unix_secs);
        if self.day_from == self.night_from {
            return true;
        }
        if self.day_from < self.night_from {
            h >= self.day_from && h < self.night_from
        } else {
            // Day wraps past midnight, which is unusual but not nonsense.
            h >= self.day_from || h < self.night_from
        }
    }

    /// What a tick should do now: the theme to apply, or nothing.
    ///
    /// Day and night are checked by *which* theme they call for rather than by
    /// the slot, because a daily cadence has one slot and two answers — without
    /// this, an evening switch would wait until tomorrow.
    pub fn due(&self, unix_secs: u64) -> Option<String> {
        let want = self.theme_at(unix_secs)?;
        let slot = self.every.slot(unix_secs);
        let same_slot = self.last_slot == Some(slot);
        let same_theme = self.last_theme.as_deref() == Some(want.as_str());
        if self.mode == Mode::DayNight {
            if same_theme {
                return None;
            }
        } else if same_slot {
            return None;
        }
        Some(want)
    }

    /// Record what was applied, so the next tick knows.
    pub fn mark(&mut self, unix_secs: u64, theme: &str) {
        self.last_slot = Some(self.every.slot(unix_secs));
        self.last_theme = Some(theme.to_string());
    }

    /// One line a person can read.
    pub fn describe(&self) -> String {
        match self.mode {
            Mode::Off => "not rotating".into(),
            Mode::Cycle => format!(
                "{} through {}",
                self.every.label().to_lowercase(),
                if self.picks.is_empty() { "nothing yet".into() } else { self.picks.join(", ") }
            ),
            Mode::Random => format!("a new theme {}", self.every.label().to_lowercase()),
            Mode::DayNight => format!(
                "{} from {:02}:00, {} from {:02}:00",
                self.day, self.day_from, self.night, self.night_from
            ),
        }
    }

    // ── on disk ─────────────────────────────────────────────────────────────

    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("mode", Json::Str(self.mode.key().into())),
            ("every", Json::Str(self.every.key())),
            ("picks", Json::Arr(self.picks.iter().map(|p| Json::Str(p.clone())).collect())),
            ("day", Json::Str(self.day.clone())),
            ("night", Json::Str(self.night.clone())),
            ("dayFrom", Json::Num(self.day_from as f64)),
            ("nightFrom", Json::Num(self.night_from as f64)),
            ("utcOffsetMinutes", Json::Num(self.utc_offset_minutes as f64)),
            ("followSlots", Json::Bool(self.follow_slots)),
            ("lastSlot", match self.last_slot {
                Some(s) => Json::Num(s as f64),
                None => Json::Null,
            }),
            ("lastTheme", match &self.last_theme {
                Some(t) => Json::Str(t.clone()),
                None => Json::Null,
            }),
        ])
    }

    pub fn from_json(v: &Json) -> Plan {
        let d = Plan::default();
        let text = |k: &str, or: &str| match v.get(k) {
            Some(Json::Str(s)) => s.clone(),
            _ => or.to_string(),
        };
        Plan {
            mode: match v.get("mode") {
                Some(Json::Str(s)) => Mode::parse(s).unwrap_or(Mode::Off),
                _ => Mode::Off,
            },
            every: match v.get("every") {
                Some(Json::Str(s)) => Every::parse(s).unwrap_or(Every::Hour),
                _ => Every::Hour,
            },
            picks: v
                .get("picks")
                .and_then(|p| p.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| match x {
                            Json::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            day: text("day", &d.day),
            night: text("night", &d.night),
            day_from: v.get("dayFrom").and_then(|x| x.as_u32()).unwrap_or(DAY_FROM).min(23),
            night_from: v.get("nightFrom").and_then(|x| x.as_u32()).unwrap_or(NIGHT_FROM).min(23),
            utc_offset_minutes: v
                .get("utcOffsetMinutes")
                .and_then(|x| x.as_u32())
                .map(|n| n as i32)
                .or_else(|| match v.get("utcOffsetMinutes") {
                    Some(Json::Num(n)) => Some(*n as i32),
                    _ => None,
                })
                .unwrap_or(0),
            follow_slots: v.get("followSlots").and_then(|x| x.as_bool()).unwrap_or(false),
            last_slot: v.get("lastSlot").and_then(|x| x.as_u32()).map(|n| n as u64),
            last_theme: match v.get("lastTheme") {
                Some(Json::Str(s)) => Some(s.clone()),
                _ => None,
            },
        }
    }
}

pub fn path() -> std::path::PathBuf {
    // Beside the themes, not among them: a file in the theme directory is a
    // theme, and this one turned up in the menu that renames and deletes them.
    crate::gallery::config_dir().join("rotate.json")
}

/// Move a plan written by an older build out of the theme directory.
///
/// It used to live beside the themes, which made it *look* like one: it turned
/// up in the menu offering to rename and delete it. Anyone upgrading still has
/// the old file, so it is moved once rather than left to confuse them.
pub fn migrate() {
    let old = crate::gallery::dir().join("rotate.json");
    let new = path();
    if old.exists() && !new.exists() {
        if let Some(parent) = new.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::rename(&old, &new);
    } else if old.exists() {
        // Both exist: the new one is authoritative, and the old one is only
        // still there to be listed as a theme.
        let _ = std::fs::remove_file(&old);
    }
}

pub fn load() -> Plan {
    migrate();
    std::fs::read_to_string(path())
        .ok()
        .and_then(|t| json::parse(&t).ok())
        .map(|v| Plan::from_json(&v))
        .unwrap_or_default()
}

pub fn save(plan: &Plan) -> Result<(), String> {
    let p = path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    std::fs::write(&p, format!("{}\n", json::to_string_pretty(&plan.to_json())))
        .map_err(|e| format!("cannot write {}: {}", p.display(), e))
}

// ───────────────────────── what is on the keys right now ────────────────────

/// The last thing applied, and the picture it came from.
///
/// Kept so two questions have answers a tick can act on: *is the wallpaper
/// theme still the one in use?* and *has that wallpaper changed?* Without the
/// first, following the desktop picture would fight whatever you chose after
/// it; without the second it would rebuild the same theme for ever.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Current {
    /// A theme reference — `theme:reef`, `profile:Mine`, `wallpaper`, `random`,
    /// `builder` — or empty if nothing has been applied through this tool.
    pub source: String,
    /// Path and modification time of the picture the wallpaper theme was built
    /// from. Cheap to compare, and it changes when the picture does.
    pub wallpaper: String,
    /// Which connection it was written over — the cable, or one of the
    /// keyboard's Bluetooth channels. See `Device::slot_id`.
    pub slot: String,
}

impl Current {
    pub fn to_json(&self) -> Json {
        Json::obj(vec![
            ("source", Json::Str(self.source.clone())),
            ("wallpaper", Json::Str(self.wallpaper.clone())),
            ("slot", Json::Str(self.slot.clone())),
        ])
    }

    pub fn from_json(v: &Json) -> Current {
        let text = |k: &str| match v.get(k) {
            Some(Json::Str(s)) => s.clone(),
            _ => String::new(),
        };
        Current { source: text("source"), wallpaper: text("wallpaper"), slot: text("slot") }
    }
}

pub fn current_path() -> std::path::PathBuf {
    crate::gallery::config_dir().join("current.json")
}

pub fn load_current() -> Current {
    std::fs::read_to_string(current_path())
        .ok()
        .and_then(|t| json::parse(&t).ok())
        .map(|v| Current::from_json(&v))
        .unwrap_or_default()
}

pub fn save_current(c: &Current) {
    let p = current_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&p, format!("{}\n", json::to_string_pretty(&c.to_json())));
}

/// A fingerprint of a picture, for "has this changed?".
///
/// Path plus modification time. Not a hash: this is asked on a timer, and
/// reading a four-megabyte picture every thirty seconds to answer "no" is not
/// a reasonable way to answer "no".
pub fn wallpaper_fingerprint(path: &std::path::Path) -> String {
    let stamp = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}|{}", path.display(), stamp)
}

/// Howard Hinnant's civil-from-days: the standard way to get a calendar date
/// out of a day count without a calendar library.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;

    #[test]
    fn a_plan_written_by_an_older_build_stops_being_listed_as_a_theme() {
        let _guard = crate::gallery::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("CLEVERTUNA_HOME", tmp.join("profiles"));
        std::fs::create_dir_all(tmp.join("profiles")).unwrap();

        // An old build's file, sitting among the themes.
        std::fs::write(tmp.join("profiles/rotate.json"), r#"{"mode":"cycle","every":"hour"}"#).unwrap();
        assert!(crate::gallery::list().iter().any(|e| e.name == "rotate"), "setup");

        let plan = load();
        assert_eq!(plan.mode, Mode::Cycle, "the settings came with it");
        assert!(!crate::gallery::list().iter().any(|e| e.name == "rotate"),
                "it must not still be offered as a theme to rename or delete");

        std::env::remove_var("CLEVERTUNA_HOME");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn the_current_source_survives_a_round_trip_and_defaults_to_nothing() {
        let c = Current { source: "wallpaper".into(), wallpaper: "/a/b.jpg|123".into(), slot: "ble:ABC".into() };
        assert_eq!(Current::from_json(&c.to_json()), c);
        assert_eq!(Current::from_json(&Json::Null), Current::default());
        assert!(Current::default().source.is_empty());
    }

    #[test]
    fn a_wallpaper_fingerprint_changes_with_the_file_and_not_otherwise() {
        let dir = std::env::temp_dir().join(format!("clevertuna-fp-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let a = dir.join("one.png");
        std::fs::write(&a, b"x").unwrap();
        let first = wallpaper_fingerprint(&a);
        assert_eq!(first, wallpaper_fingerprint(&a), "asking twice must not change the answer");
        assert_ne!(first, wallpaper_fingerprint(&dir.join("two.png")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slots_are_anchored_to_the_clock_not_to_when_you_started() {
        // Two machines told the same plan must agree without talking, so a slot
        // is a property of the instant, not of when rotation was switched on.
        assert_eq!(Every::Minutes(5).slot(0), Every::Minutes(5).slot(299));
        assert_ne!(Every::Minutes(5).slot(299), Every::Minutes(5).slot(300));
        assert_eq!(Every::Hour.slot(0), Every::Hour.slot(HOUR - 1));
        assert_ne!(Every::Hour.slot(HOUR - 1), Every::Hour.slot(HOUR));
        assert_eq!(Every::Day.slot(DAY * 5 + 1), Every::Day.slot(DAY * 5 + DAY - 1));
        assert_ne!(Every::Week.slot(0), Every::Week.slot(604_800));
    }

    #[test]
    fn months_are_real_months_not_thirty_day_blocks() {
        // 2026-01-15 and 2026-01-31 are one month; 2026-02-01 is the next.
        let d = |y: i64, m: u32, day: u32| -> u64 {
            let mut n = 0i64;
            // walk from the epoch; slow but obviously correct in a test
            loop {
                if civil_from_days(n) == (y, m, day) {
                    return (n as u64) * DAY;
                }
                n += 1;
            }
        };
        let jan15 = d(2026, 1, 15);
        let jan31 = d(2026, 1, 31);
        let feb01 = d(2026, 2, 1);
        assert_eq!(Every::Month.slot(jan15), Every::Month.slot(jan31));
        assert_ne!(Every::Month.slot(jan31), Every::Month.slot(feb01));
    }

    #[test]
    fn a_cycle_steps_through_its_themes_in_order_and_wraps() {
        let p = Plan {
            mode: Mode::Cycle,
            every: Every::Hour,
            picks: vec!["a".into(), "b".into(), "c".into()],
            ..Plan::default()
        };
        let seen: Vec<String> = (0..5).map(|i| p.theme_at(i * HOUR).unwrap()).collect();
        assert_eq!(seen, ["a", "b", "c", "a", "b"]);
    }

    #[test]
    fn a_random_rotation_is_reproducible_for_a_given_slot() {
        // "What was that one?" needs an answer, so the slot number is the seed.
        let p = Plan { mode: Mode::Random, every: Every::Hour, ..Plan::default() };
        assert_eq!(p.theme_at(HOUR * 9 + 12), p.theme_at(HOUR * 9 + 3_599));
        assert_ne!(p.theme_at(HOUR * 9), p.theme_at(HOUR * 10));
        assert!(p.theme_at(0).unwrap().starts_with("random:"));
    }

    #[test]
    fn a_tick_inside_the_same_slot_does_nothing() {
        let mut p = Plan {
            mode: Mode::Cycle,
            every: Every::Hour,
            picks: vec!["a".into(), "b".into()],
            ..Plan::default()
        };
        let t = HOUR * 3;
        let due = p.due(t).expect("first tick of a slot is due");
        p.mark(t, &due);
        assert_eq!(p.due(t + 60), None, "still the same hour");
        assert_eq!(p.due(t + 1_800), None);
        assert!(p.due(t + HOUR).is_some(), "the next hour is due again");
    }

    #[test]
    fn day_and_night_switch_on_the_hour_not_on_the_slot() {
        // The bug this guards: with a daily cadence there is one slot a day, so
        // a slot-based check would make the evening theme wait until tomorrow.
        let mut p = Plan {
            mode: Mode::DayNight,
            every: Every::Day,
            day: "bright".into(),
            night: "dim".into(),
            ..Plan::default()
        };
        let morning = DAY * 100 + 9 * HOUR;
        let evening = DAY * 100 + 20 * HOUR;

        let d = p.due(morning).unwrap();
        assert_eq!(d, "bright");
        p.mark(morning, &d);
        assert_eq!(p.due(morning + HOUR), None, "still daytime, nothing to do");

        let n = p.due(evening).expect("the evening must not wait for tomorrow");
        assert_eq!(n, "dim");
        p.mark(evening, &n);
        assert_eq!(p.due(evening + HOUR), None);
    }

    #[test]
    fn daytime_is_local_and_can_wrap_past_midnight() {
        let p = Plan { day_from: 7, night_from: 19, ..Plan::default() };
        assert!(p.is_daytime(12 * HOUR));
        assert!(!p.is_daytime(23 * HOUR));
        assert!(!p.is_daytime(3 * HOUR));

        // Three hours east: 05:00 UTC is 08:00 locally, which is day.
        let east = Plan { utc_offset_minutes: 180, ..p.clone() };
        assert!(!p.is_daytime(5 * HOUR));
        assert!(east.is_daytime(5 * HOUR));

        // A night shift: day runs 20:00 to 06:00.
        let night_shift = Plan { day_from: 20, night_from: 6, ..Plan::default() };
        assert!(night_shift.is_daytime(22 * HOUR));
        assert!(night_shift.is_daytime(2 * HOUR));
        assert!(!night_shift.is_daytime(12 * HOUR));
    }

    #[test]
    fn switched_off_it_never_asks_for_anything() {
        let p = Plan { mode: Mode::Off, ..Plan::default() };
        for t in [0u64, HOUR, DAY, DAY * 400] {
            assert_eq!(p.due(t), None);
            assert_eq!(p.theme_at(t), None);
        }
    }

    #[test]
    fn a_cycle_with_nothing_in_it_is_not_an_error_it_is_nothing() {
        let p = Plan { mode: Mode::Cycle, picks: vec![], ..Plan::default() };
        assert_eq!(p.due(HOUR), None);
    }

    #[test]
    fn a_plan_survives_a_round_trip_through_its_file_shape() {
        let p = Plan {
            mode: Mode::DayNight,
            every: Every::Week,
            picks: vec!["reef".into(), "random".into()],
            day: "spectrum".into(),
            night: "sleep".into(),
            day_from: 6,
            night_from: 21,
            last_slot: Some(4_242),
            last_theme: Some("spectrum".into()),
            utc_offset_minutes: -300,
            follow_slots: true,
        };
        assert_eq!(Plan::from_json(&p.to_json()), p);
    }

    #[test]
    fn an_unreadable_or_absent_plan_means_off_rather_than_a_crash() {
        assert_eq!(Plan::from_json(&Json::Null).mode, Mode::Off);
        assert_eq!(Plan::from_json(&json::parse("{}").unwrap()).mode, Mode::Off);
        let junk = json::parse(r#"{"mode":"sideways","every":"fortnight"}"#).unwrap();
        let p = Plan::from_json(&junk);
        assert_eq!(p.mode, Mode::Off);
        assert_eq!(p.every, Every::Hour);
    }

    #[test]
    fn every_cadence_states_what_it_costs_in_writes() {
        // Applying a theme is a flash write, and flash wears out. The minute
        // cadence is the one that needs saying out loud.
        // Five minutes is the floor, and it costs a fifth of what a minute
        // would have: 288 writes a day rather than 1440.
        assert_eq!(Every::Minutes(5).writes_per_day(), 288);
        assert_eq!(Every::Minutes(30).writes_per_day(), 48);
        assert!(Every::Hour.writes_per_day() < 100);
        for e in Every::all() {
            assert!(e.writes_per_day() >= 1);
            assert_eq!(Every::parse(&e.key()), Some(e));
        }
        // Nothing shorter than the floor is reachable, however it is asked for.
        assert_eq!(Every::parse("1m"), Some(Every::Minutes(MIN_MINUTES)));
        assert_eq!(Every::parse("2"), Some(Every::Minutes(MIN_MINUTES)));
        assert!(Every::all().iter().all(|e| e.writes_per_day() <= 288));
    }

    #[test]
    fn cadences_and_modes_are_accepted_as_people_type_them() {
        assert_eq!(Every::parse("every hour"), Some(Every::Hour));
        assert_eq!(Every::parse("Daily"), Some(Every::Day));
        assert_eq!(Every::parse("1w"), Some(Every::Week));
        assert_eq!(Every::parse("15m"), Some(Every::Minutes(15)));
        assert_eq!(Every::parse("30 minutes"), Some(Every::Minutes(30)));
        assert_eq!(Every::parse("fortnight"), None);
        assert_eq!(Mode::parse("day/night"), Some(Mode::DayNight));
        assert_eq!(Mode::parse("surprise"), Some(Mode::Random));
    }
}
