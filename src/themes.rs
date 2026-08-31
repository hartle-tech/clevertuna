//! Themes that ship with the tool, and themes rolled on the spot.
//!
//! A picker with an empty gallery is a picker with nothing to pick, so the
//! keyboard has a look the first time it is opened and every time after. These
//! are built here rather than shipped as files: a scheme is only a few numbers
//! once [`crate::effects`] owns the arithmetic, and code cannot be half-copied
//! into a config directory the way a set of JSON files can.
//!
//! The palettes are this project's own. Nothing here is derived from any other
//! product's presets — only the parameter ranges are shared, and those are
//! facts about the hardware. See `NOTICE`.

use crate::effects::{Effect, Look, Reactive, Scheme, Stop, SPEED_MAX, SPEED_MIN};

/// A theme, and where it belongs in a menu.
pub struct Preset {
    /// Stable, lowercase, hyphenated — this is what a bar sends back.
    pub id: &'static str,
    pub name: &'static str,
    pub group: Group,
    /// One line, in the same voice as the rest of the tool.
    pub blurb: &'static str,
    build: fn() -> Scheme,
}

impl Preset {
    pub fn scheme(&self) -> Scheme {
        (self.build)()
    }

    /// The colours to draw beside the name.
    pub fn swatch(&self) -> Vec<String> {
        self.scheme()
            .keyboard
            .palette()
            .iter()
            .map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    /// One colour, standing still.
    Steady,
    /// One colour, moving.
    Breathing,
    /// A palette, moving across the keys.
    Moving,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Steady => "Steady",
            Group::Breathing => "Breathing",
            Group::Moving => "Moving",
        }
    }

    pub fn all() -> [Group; 3] {
        [Group::Steady, Group::Breathing, Group::Moving]
    }
}

// ────────────────────────────── the palettes ─────────────────────────────────

// A backlight emits rather than reflects, so these are chosen at full
// saturation: a colour that looks right on a screen reads grey on the keys.

const CURRENT: [u8; 3] = [0x00, 0xC8, 0xFF];
const MINT: [u8; 3] = [0x36, 0xF0, 0xB1];
const TRENCH: [u8; 3] = [0xFF, 0x00, 0xE8];
const CORAL: [u8; 3] = [0xFF, 0x53, 0x53];
const AMBER: [u8; 3] = [0xFF, 0xB1, 0x00];

const INK: [u8; 3] = [0x14, 0x1E, 0xFF];
const JADE: [u8; 3] = [0x00, 0xE0, 0x7A];
const GOLD: [u8; 3] = [0xFF, 0xE0, 0x3A];
const ICE: [u8; 3] = [0xCF, 0xF4, 0xFF];
const EMBER: [u8; 3] = [0xFF, 0x3C, 0x00];
const DEEP: [u8; 3] = [0x2B, 0x00, 0x8A];
const SEA: [u8; 3] = [0x00, 0x7A, 0xE0];

// ─────────────────────────────── the gallery ─────────────────────────────────

/// How many themes each group holds.
///
/// Five. A picker is for choosing, and a column of fourteen near-neighbours is
/// a list to read — the ones that survived are the ones that do something the
/// others do not, rather than the same idea in another colour.
#[allow(dead_code)] // asserted by the menu tests, which `cargo check` cannot see
pub const PER_GROUP: usize = 5;

/// Every theme that ships with the tool.
pub fn all() -> &'static [Preset] {
    &PRESETS
}

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The themes in one group, in the order they should be shown.
pub fn in_group(group: Group) -> Vec<&'static Preset> {
    PRESETS.iter().filter(|p| p.group == group).collect()
}

/// A steady colour everywhere, with the reactive layers picking it up.
fn steady(color: [u8; 3]) -> Scheme {
    Scheme::uniform(Look::solid(color)).with_reactive()
}

/// A breath everywhere. Slower than a wave wants to be: a breath that hurries
/// reads as a fault light.
fn breathe(color: [u8; 3], speed: u32) -> Scheme {
    Scheme::uniform(Look::breathing(color, speed)).with_reactive()
}

/// A palette running across the keys, with the sliders running along
/// themselves and the touchpad holding the palette still underneath.
fn moving(effect: Effect, colors: &[[u8; 3]], speed: u32, angle: u32) -> Scheme {
    let base = Look { speed, angle, ..Look::spread(effect, colors) };
    let mut scheme = Scheme::uniform(base.clone());
    // The touchpad is a small square: a long wave crossing it shows one colour
    // at a time, so it gets the palette compressed rather than the same stretch
    // as the whole deck.
    scheme.touchpad.length = 200;
    // Along the strip, one each way. 0 and 180 are the two angles a slider
    // holds; 90/270 were snapped to these anyway, which read as if they meant
    // something else.
    scheme.left_slider.angle = 0;
    scheme.right_slider.angle = 180;
    scheme.with_reactive()
}

static PRESETS: [Preset; 15] = [
    // ── Steady ──────────────────────────────────────────────────────────────
    Preset {
        id: "deep-current",
        name: "Deep Current",
        group: Group::Steady,
        blurb: "the house cyan, at full strength",
        build: || steady(CURRENT),
    },
    Preset {
        id: "amber-desk",
        name: "Amber Desk",
        group: Group::Steady,
        blurb: "lamplight, for working late",
        build: || steady(AMBER),
    },
    // The seat Typing Only gave up, and the one plain thing the set was
    // missing: a keyboard that is simply lit.
    Preset {
        id: "paper",
        name: "Paper",
        group: Group::Steady,
        blurb: "plain cool white, all the way up",
        build: || steady(ICE),
    },
    Preset {
        id: "nightshift",
        name: "Nightshift",
        group: Group::Steady,
        blurb: "deep blue at a third of the brightness",
        build: || {
            let mut s = Scheme::uniform(Look { brightness: 35, ..Look::solid(INK) });
            s.typing = Some(Reactive::typing(ICE));
            s.gesture = Some(Reactive::gesture(SEA));
            s
        },
    },
    // Blackout and "Typing Only" were the same theme.
    //
    // Both write every zone unlit, and neither touches `interactiveAnimation` —
    // so on the hardware both leave whatever the last theme set for typing in
    // place, and both light the keys you press. Two rows that do one thing is
    // one row too many; what made them look different was only that one of them
    // *also* rewrote the reactive colour, which the builder now does properly.
    Preset {
        id: "blackout",
        name: "Blackout",
        group: Group::Steady,
        blurb: "everything off; what you touch still lights, in the last colour set",
        build: || {
            Scheme::uniform(Look { opacity: 0, ..Look::solid([0, 0, 0]) })
        },
    },
    // ── Breathing ───────────────────────────────────────────────────────────
    Preset {
        id: "tide",
        name: "Tide",
        group: Group::Breathing,
        blurb: "cyan, in and out, slowly",
        build: || breathe(CURRENT, 2_000),
    },
    Preset {
        id: "pulse",
        name: "Pulse",
        group: Group::Breathing,
        blurb: "red, quick enough to notice",
        build: || breathe(CORAL, 8_500),
    },
    Preset {
        id: "lantern",
        name: "Lantern",
        group: Group::Breathing,
        blurb: "amber, at a walking pace",
        build: || breathe(AMBER, 5_000),
    },
    Preset {
        id: "sleep",
        name: "Sleep",
        group: Group::Breathing,
        blurb: "indigo, dim, about as slow as it goes",
        build: || {
            Scheme::uniform(Look { brightness: 45, ..Look::breathing(DEEP, 900) }).with_reactive()
        },
    },
    Preset {
        id: "moss",
        name: "Moss",
        group: Group::Breathing,
        blurb: "green, unhurried",
        build: || breathe(JADE, 3_000),
    },
    // ── Moving ──────────────────────────────────────────────────────────────
    Preset {
        id: "hartle",
        name: "HARTLE",
        group: Group::Moving,
        blurb: "the brand, left to right",
        build: || moving(Effect::ColorWave, &[CURRENT, MINT, TRENCH, CORAL, AMBER], 6_500, 0),
    },
    Preset {
        id: "spectrum",
        name: "Spectrum",
        group: Group::Moving,
        blurb: "the whole wheel, crossing the deck",
        build: || {
            moving(
                Effect::ColorWave,
                &[[255, 0, 0], [255, 200, 0], [0, 255, 80], [0, 160, 255], [180, 0, 255]],
                6_000,
                0,
            )
        },
    },
    Preset {
        id: "magma",
        name: "Magma",
        group: Group::Moving,
        blurb: "black-red to gold, welling upward",
        build: || moving(Effect::ColorWave, &[[90, 0, 0], EMBER, CORAL, AMBER, GOLD], 2_800, 90),
    },
    Preset {
        id: "neon-cycle",
        name: "Neon Cycle",
        group: Group::Moving,
        blurb: "magenta to cyan and back, quickly",
        build: || moving(Effect::ColorCycle, &[TRENCH, CURRENT], 9_000, 0),
    },
    Preset {
        id: "aurora",
        name: "Aurora",
        group: Group::Moving,
        blurb: "the firmware's own animation; no settings but brightness",
        build: || {
            let mut s = Scheme::uniform(Look::aurora());
            // The sliders cannot run it, so they take a colour it passes through
            // rather than falling back to whatever was there.
            s.left_slider = Look::solid(SEA);
            s.right_slider = Look::solid(SEA);
            s.with_reactive()
        },
    },
];

// ───────────────────────────────── the roll ──────────────────────────────────

/// A small, exact pseudo-random source.
///
/// Seeded and reproducible on purpose: a roll that cannot be repeated is a look
/// you lose the moment you try the next one. The seed is part of the theme's
/// name, so a good one can be asked for again by number.
pub struct Roll(u64);

impl Roll {
    pub fn new(seed: u64) -> Roll {
        // Zero is a fixed point for the mixer, so it would return the same
        // number forever.
        Roll(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// splitmix64 — a single multiply-xor round, which is enough for colours.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Inclusive on both ends.
    pub fn between(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % (hi - lo + 1) as u64) as u32
    }

    fn chance(&mut self, percent: u32) -> bool {
        self.between(1, 100) <= percent
    }
}

/// A seed drawn from the clock, for when nobody named one.
pub fn seed_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED)
}

/// Hue → RGB at full saturation and value, then scaled.
///
/// Written out rather than pulled from a crate because the whole tool is one
/// binary with nothing under it.
pub fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|ch| (((ch + m) * 255.0).round()).clamp(0.0, 255.0) as u8)
}

/// The name a hue answers to.
///
/// Thirteen families rather than the usual six: "blue" covering a third of the
/// wheel makes every second roll a Blue Wave.
fn hue_name(h: f32) -> &'static str {
    const NAMES: [(f32, &str); 13] = [
        (15.0, "Crimson"),
        (35.0, "Ember"),
        (50.0, "Amber"),
        (68.0, "Gold"),
        (95.0, "Lime"),
        (140.0, "Jade"),
        (170.0, "Sea"),
        (195.0, "Teal"),
        (215.0, "Cyan"),
        (250.0, "Azure"),
        (280.0, "Indigo"),
        (320.0, "Violet"),
        (345.0, "Rose"),
    ];
    let h = ((h % 360.0) + 360.0) % 360.0;
    for (edge, name) in NAMES {
        if h < edge {
            return name;
        }
    }
    "Crimson"
}

/// How a palette relates to its first colour.
///
/// Random hues on their own produce mud — three colours 20° apart look like one
/// colour that failed. Every scheme here keeps its members at least 30° apart,
/// which is the same rule the wallpaper matcher enforces and for the same
/// reason.
fn palette(roll: &mut Roll) -> (Vec<[u8; 3]>, f32) {
    let base = roll.between(0, 359) as f32;
    let sat = roll.between(78, 100) as f32 / 100.0;
    let val = roll.between(88, 100) as f32 / 100.0;

    // Every scheme keeps its members at least 45 degrees apart.
    //
    // The first version allowed 32, and a three-colour wave inside 64 degrees
    // reads as one colour that went wrong rather than as a palette — which is
    // exactly what "the randomiser prefers solid" turned out to mean. Nothing
    // here is narrower than a sixth of the wheel now.
    let offsets: Vec<f32> = match roll.between(0, 6) {
        // A family, but a wide one.
        0 => vec![0.0, 55.0, 110.0],
        1 => vec![0.0, 48.0, 96.0, 144.0],
        // Opposites.
        2 => vec![0.0, 180.0],
        // Split complement — an opposite that is not a straight line.
        3 => vec![0.0, 150.0, 210.0],
        // Thirds.
        4 => vec![0.0, 120.0, 240.0],
        // Quarters.
        5 => vec![0.0, 90.0, 180.0, 270.0],
        // The whole wheel.
        _ => vec![0.0, 72.0, 144.0, 216.0, 288.0],
    };

    let colors = offsets
        .iter()
        .map(|off| {
            // Vary each member a little so a five-colour roll is not a wheel
            // with the spokes filed down, but never far enough to close the
            // gap the offsets above opened.
            let jitter = roll.between(0, 16) as f32 - 8.0;
            let s = (sat - roll.between(0, 12) as f32 / 100.0).max(0.7);
            let v = (val - roll.between(0, 10) as f32 / 100.0).max(0.75);
            hsv(base + off + jitter, s, v)
        })
        .collect();
    (colors, base)
}

/// Roll a theme. Returns the name it earned and the scheme itself.
pub fn random_scheme(seed: u64) -> (String, Scheme) {
    let mut roll = Roll::new(seed);
    let (colors, base) = palette(&mut roll);

    // Weighted, and weighted towards movement.
    //
    // Nobody presses a dice button hoping for a steady colour, so a still one
    // is the rarest outcome rather than a seventh of them. The aurora is in the
    // hat too: it takes no settings, so every roll of it looks the same, but it
    // is a distinct look and leaving it out made the results feel narrower than
    // the hardware is.
    let effect = match roll.between(1, 100) {
        1..=40 => Effect::ColorWave,
        41..=62 => Effect::ColorCycle,
        63..=80 => Effect::Breathing,
        81..=93 => Effect::Aurora,
        _ => Effect::Solid,
    };

    let speed = roll.between(SPEED_MIN, SPEED_MAX);
    let angle = roll.between(0, 23) * 15;
    let length = roll.between(150, 900);

    let mut look = Look {
        effect,
        speed,
        angle,
        length,
        ..Look::spread(effect, &colors)
    };
    if !effect.gradient() {
        look.stops = vec![Stop::new(colors[0], 0)];
    }
    // Occasionally roll something restrained rather than another full-brightness
    // rainbow — an evening setting is a real thing to want.
    if roll.chance(20) {
        look.brightness = roll.between(35, 70) as u8;
    }

    let mut scheme = Scheme::uniform(look.clone());
    scheme.touchpad.length = length.min(300);
    // Along the strip, one each way. 0 and 180 are the two angles a slider
    // holds; 90/270 were snapped to these anyway, which read as if they meant
    // something else.
    scheme.left_slider.angle = 0;
    scheme.right_slider.angle = 180;

    // Sometimes the smaller zones get their own idea.
    //
    // Four zones showing one setting is a theme; four zones in conversation is
    // a keyboard. This stays a minority of rolls, because the other kind is
    // the one that looks deliberate.
    if roll.chance(35) {
        // The complement of the palette's own base, so the contrast is chosen
        // rather than accidental.
        let accent = hsv(base + 180.0, 0.95, 1.0);
        let quiet = Look { effect: Effect::Solid, ..Look::solid(accent) };
        match roll.between(0, 2) {
            0 => {
                scheme.touchpad = quiet;
            }
            1 => {
                scheme.left_slider = quiet.clone();
                scheme.right_slider = quiet;
            }
            _ => {
                // The sliders breathe against a moving deck.
                let pulse = Look { speed: roll.between(SPEED_MIN, 4_000), ..Look::breathing(accent, 2_000) };
                scheme.left_slider = pulse.clone();
                scheme.right_slider = pulse;
            }
        }
    }
    let scheme = scheme.with_reactive();

    let noun = match effect {
        Effect::ColorWave => "Wave",
        Effect::ColorCycle => "Cycle",
        Effect::Breathing => "Breath",
        Effect::Aurora => "Aurora",
        Effect::Solid => "Glow",
    };
    (format!("{} {} {:04x}", hue_name(base), noun, seed & 0xFFFF), scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_a_scheme_the_device_would_accept() {
        for p in all() {
            let doc = p.scheme().to_doc();
            crate::backlight::from_json(&doc)
                .unwrap_or_else(|e| panic!("preset '{}' would be refused: {}", p.id, e));
            assert_eq!(crate::ui::zones_in(&doc).len(), 4, "{} must set every zone", p.id);
        }
    }

    #[test]
    fn preset_ids_are_unique_and_bar_safe() {
        let mut seen: Vec<&str> = Vec::new();
        for p in all() {
            assert!(!seen.contains(&p.id), "duplicate id {}", p.id);
            seen.push(p.id);
            assert!(
                p.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id {:?} must survive being passed as one argument",
                p.id
            );
            assert!(!p.name.is_empty() && !p.blurb.is_empty());
        }
    }

    #[test]
    fn every_group_holds_exactly_five() {
        for g in Group::all() {
            assert_eq!(in_group(g).len(), PER_GROUP, "{:?} is the wrong size", g);
        }
        assert_eq!(
            Group::all().iter().map(|g| in_group(*g).len()).sum::<usize>(),
            all().len(),
            "every preset belongs to exactly one group"
        );
    }

    #[test]
    fn a_preset_shows_colours_even_when_it_is_off() {
        // Blackout writes nothing lit, but a menu row still needs a swatch, and
        // an aurora's colours come from the firmware rather than the scheme.
        assert!(!find("aurora").unwrap().swatch().is_empty());
        assert!(!find("blackout").unwrap().swatch().is_empty());
        assert_eq!(find("hartle").unwrap().swatch()[0], "#00C8FF");
    }

    #[test]
    fn the_same_seed_always_rolls_the_same_theme() {
        for seed in [0u64, 1, 42, 9_999, u64::MAX] {
            let (a_name, a) = random_scheme(seed);
            let (b_name, b) = random_scheme(seed);
            assert_eq!(a_name, b_name);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn different_seeds_roll_different_themes() {
        let names: Vec<String> = (0..40).map(|s| random_scheme(s).0).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert!(unique.len() > 30, "40 rolls produced only {} names", unique.len());
    }

    #[test]
    fn a_rolled_theme_is_always_writable() {
        for seed in 0..200u64 {
            let (name, scheme) = random_scheme(seed);
            let doc = scheme.to_doc();
            crate::backlight::from_json(&doc)
                .unwrap_or_else(|e| panic!("seed {} ('{}') would be refused: {}", seed, name, e));
        }
    }

    #[test]
    fn a_rolled_name_carries_its_seed() {
        let (name, _) = random_scheme(0xDEAD_BEEF);
        assert!(name.to_lowercase().ends_with("beef"), "{}", name);
    }

    #[test]
    fn rolled_palettes_are_separated_on_hue() {
        // The failure this guards against is three near-identical colours,
        // which read as one colour that went wrong.
        for seed in 0..100u64 {
            let (_, scheme) = random_scheme(seed);
            // The aurora's colours are the firmware's own, reported as a label
            // rather than rolled, so they are not this rule's business — and
            // measuring them here is what made this test fail once the roll
            // started producing auroras.
            if scheme.keyboard.effect == crate::effects::Effect::Aurora {
                continue;
            }
            let colours = scheme.keyboard.palette();
            if colours.len() < 2 {
                continue;
            }
            let hue = |c: [u8; 3]| -> f32 {
                let (r, g, b) = (c[0] as f32, c[1] as f32, c[2] as f32);
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                if max == min {
                    return 0.0;
                }
                let d = max - min;
                let h = if max == r {
                    60.0 * (((g - b) / d) % 6.0)
                } else if max == g {
                    60.0 * ((b - r) / d + 2.0)
                } else {
                    60.0 * ((r - g) / d + 4.0)
                };
                (h + 360.0) % 360.0
            };
            let hues: Vec<f32> = colours.iter().map(|c| hue(*c)).collect();
            for i in 0..hues.len() {
                for j in (i + 1)..hues.len() {
                    let d = (hues[i] - hues[j]).abs();
                    let sep = d.min(360.0 - d);
                    // Wide enough to read as two colours. The first version
                    // allowed 32° between neighbours and the result looked
                    // like one colour that had gone wrong.
                    assert!(sep >= 28.0, "seed {} put two hues {:.0}° apart", seed, sep);
                }
            }
        }
    }

    #[test]
    fn a_roll_is_far_more_likely_to_move_than_to_sit_still() {
        // The complaint this guards against is "the randomiser prefers a solid
        // colour with no animation". Nobody presses a dice button for a steady
        // colour, so it must be the rarest outcome, not a common one.
        let mut still = 0;
        let mut moving = 0;
        for seed in 0..400u64 {
            let (_, scheme) = random_scheme(seed);
            if scheme.keyboard.effect.animated() {
                moving += 1;
            } else {
                still += 1;
            }
        }
        assert!(moving > still * 6, "{} moving vs {} still is not enough movement", moving, still);
    }

    #[test]
    fn rolls_reach_every_effect_the_deck_can_show() {
        use std::collections::BTreeSet;
        let seen: BTreeSet<&str> = (0..200u64)
            .map(|s| random_scheme(s).1.keyboard.effect.key())
            .collect();
        for want in ["colorWave", "colorCycle", "breathing", "aurora", "solidColor"] {
            assert!(seen.contains(want), "200 rolls never produced {}; got {:?}", want, seen);
        }
    }

    #[test]
    fn some_rolls_give_the_smaller_zones_their_own_look() {
        // Four zones showing one setting is a theme; some of the time the roll
        // should produce a keyboard in conversation with itself instead.
        let mut varied = 0;
        for seed in 0..200u64 {
            let (_, s) = random_scheme(seed);
            if s.touchpad.effect != s.keyboard.effect
                || s.left_slider.effect != s.keyboard.effect
                || s.left_slider.stops.first().map(|x| x.color)
                    != s.keyboard.stops.first().map(|x| x.color)
            {
                varied += 1;
            }
        }
        assert!(varied > 30, "only {} of 200 rolls varied a zone", varied);
        assert!(varied < 160, "{} of 200 is too many; the uniform look is the common one", varied);
    }

    #[test]
    fn hues_are_named_across_the_whole_wheel() {
        let names: Vec<&str> = (0..24).map(|i| hue_name(i as f32 * 15.0)).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert!(unique.len() >= 10, "only {} names across the wheel", unique.len());
    }

    #[test]
    fn hsv_hits_the_corners() {
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv(240.0, 1.0, 1.0), [0, 0, 255]);
        assert_eq!(hsv(0.0, 0.0, 1.0), [255, 255, 255]);
        assert_eq!(hsv(999.0, 1.0, 0.0), [0, 0, 0]);
    }
}
