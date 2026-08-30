//! A look, as a person describes it — and the arithmetic that turns it into the
//! device's numbers.
//!
//! The keyboard does not store "speed 7500, 80% bright". It stores a *period*
//! in milliseconds, a *transparency* percentage, and a direction measured from
//! a different zero than any gradient control uses. Every surface that offers a
//! slider therefore needs the same three conversions, and the moment two of
//! them disagree the tool writes schemes the firmware refuses. So the
//! conversions live here once, with the ranges beside them, and the builder,
//! the randomiser, the preset themes and the wallpaper matcher all encode
//! through this module.
//!
//! ## Provenance
//!
//! The field numbers and the wire shapes come from `crate::backlight`, which
//! was recovered from the wire. The *units* below — that speed and period are
//! the same scale counted from opposite ends, that opacity is the complement of
//! transparency, that a gradient angle and the stored direction are *mirrors*
//! of one another, and the closed ranges each control accepts — are facts about the
//! device, established by observing the vendor application's traffic and its
//! published source maps, and re-checked against a real CLVX S. They are
//! interoperability facts, not anyone's expression: two correct
//! implementations have no choice but to agree on them. No third-party code,
//! constant table, palette or layout was copied; see `NOTICE`.

use crate::json::Json;

// ─────────────────────────── the ranges the device accepts ───────────────────

/// Animation rate. Higher is faster, which is the opposite of the stored
/// period — see [`period_from_speed`].
pub const SPEED_MIN: u32 = 500;
pub const SPEED_MAX: u32 = 10_000;
pub const SPEED_DEFAULT: u32 = 7_500;

/// How far a wave's gradient is stretched along the zone.
pub const LENGTH_MIN: u32 = 100;
pub const LENGTH_MAX: u32 = 1_000;
pub const LENGTH_DEFAULT: u32 = 500;

/// The zone's own dial. 100 is fully lit; the device stores its complement.
pub const OPACITY_MAX: u8 = 100;

/// How long a key stays lit after it is pressed: 1 low, 2 medium, 3 high.
pub const DURATION_MIN: u32 = 1;
pub const DURATION_MAX: u32 = 3;
pub const DURATION_DEFAULT: u32 = 2;

/// How far a gesture's trail follows the finger: 1 short … 5 long.
pub const TRACE_MIN: u32 = 1;
pub const TRACE_MAX: u32 = 5;
pub const TRACE_DEFAULT: u32 = 3;

/// The marker array is a fixed-width slot, not a list.
///
/// Four markers are refused outright — and the refusal names a timing fault, so
/// it does not read as a schema error. Shorter palettes are padded; see
/// [`Look::markers`].
pub const MARKERS: usize = 5;

/// How finely a preview resolves a gradient.
///
/// Enough that a five-stop palette reads as a gradient rather than as bands,
/// and few enough that the whole model stays a small piece of JSON.
pub const PREVIEW_SAMPLES: usize = 48;

/// The two constants that turn a speed into a period and back.
///
/// `period = SPEED_PIVOT - speed`, so the fastest speed is the shortest period.
const SPEED_PIVOT: u32 = 10_500;

/// A gradient angle and the stored direction turn **opposite ways**.
///
/// Not an offset — a mirror. Measured on a CLVX S on 2026-08-30 by setting each
/// of the four cardinals in turn and watching which way the light actually ran:
///
/// | the control says | the keyboard does |
/// |---|---|
/// | 0° right | down |
/// | 90° up | left |
/// | 180° left | up |
/// | 270° down | right |
///
/// Right↔down and up↔left is a reflection, and **no offset can produce a
/// reflection**. Two earlier attempts moved a constant — 90, then 270 — and
/// each rotated the mirror into a different wrong place, which is why the fault
/// survived both. A single spot check cannot tell a half turn from a mirror;
/// four cardinals can, and the table above is what settled it.
///
/// `direction = (180 − angle) mod 360`, which is its own inverse.
const ANGLE_MIRROR: u32 = 180;

// ──────────────────────────────── conversions ────────────────────────────────

/// Speed (higher is faster) → the stored period in milliseconds.
pub fn period_from_speed(speed: u32) -> u32 {
    SPEED_PIVOT - speed.clamp(SPEED_MIN, SPEED_MAX)
}

/// The stored period → speed. The relation is its own inverse.
pub fn speed_from_period(period: u32) -> u32 {
    SPEED_PIVOT - period.clamp(SPEED_MIN, SPEED_MAX)
}

/// Opacity as a person sets it → transparency as the device stores it.
pub fn transparency_from_opacity(opacity: u8) -> u32 {
    (OPACITY_MAX - opacity.min(OPACITY_MAX)) as u32
}

pub fn opacity_from_transparency(transparency: u32) -> u8 {
    OPACITY_MAX.saturating_sub(transparency.min(OPACITY_MAX as u32) as u8)
}

/// A gradient angle (0° = the direction a colour picker calls "to the right")
/// → the stored direction.
pub fn direction_from_angle(angle: u32) -> u32 {
    (ANGLE_MIRROR + 360 - angle % 360) % 360
}

pub fn angle_from_direction(direction: u32) -> u32 {
    // The same mapping: a mirror undoes itself.
    (ANGLE_MIRROR + 360 - direction % 360) % 360
}

/// The direction to store for a zone, which is not the same question on a strip.
///
/// A slider's two directions are **tokens**, not geometry: §7 records that it
/// only ever holds 90 or 270, and those mean "along it, one way" and "along it,
/// the other". They do not obey the areas' mirror — running the reflection over
/// them would write 180 and 0, which a strip does not take.
pub fn direction_for_zone(zone: &str, angle: u32) -> u32 {
    if is_slider(zone) {
        if angle_for_zone(zone, angle) == 0 { 270 } else { 90 }
    } else {
        direction_from_angle(angle)
    }
}

/// And back, by the same pairing.
pub fn angle_for_zone_from_direction(zone: &str, direction: u32) -> u32 {
    if is_slider(zone) {
        if direction % 360 == 270 { 0 } else { 180 }
    } else {
        angle_from_direction(direction)
    }
}

/// Scale a colour towards black.
///
/// The device has one dial, and it is opacity. Brightness is ours: it changes
/// the colours themselves before they are sent, which is the only honest way to
/// offer the control without inventing a device parameter that does not exist.
pub fn dim(color: [u8; 3], brightness: u8) -> [u8; 3] {
    let b = brightness.min(100) as u32;
    color.map(|c| ((c as u32 * b + 50) / 100).min(255) as u8)
}

// ────────────────────────────────── effects ──────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    Solid,
    Breathing,
    ColorCycle,
    ColorWave,
    Aurora,
}

impl Effect {
    /// The name this effect carries on the wire.
    pub fn key(self) -> &'static str {
        match self {
            Effect::Solid => "solidColor",
            Effect::Breathing => "breathing",
            Effect::ColorCycle => "colorCycle",
            Effect::ColorWave => "colorWave",
            Effect::Aurora => "aurora",
        }
    }

    pub fn label(self) -> &'static str {
        crate::ui::effect_label(self.key())
    }

    pub fn from_key(key: &str) -> Option<Effect> {
        Some(match key {
            "solidColor" => Effect::Solid,
            "breathing" => Effect::Breathing,
            "colorCycle" => Effect::ColorCycle,
            "colorWave" => Effect::ColorWave,
            "aurora" => Effect::Aurora,
            _ => return None,
        })
    }

    /// Accept the spellings a person types.
    pub fn parse(s: &str) -> Option<Effect> {
        match s.trim().to_ascii_lowercase().replace([' ', '_'], "-").as_str() {
            "solid" | "solid-colour" | "solid-color" | "solidcolor" => Some(Effect::Solid),
            "breathing" | "breathe" | "breath" => Some(Effect::Breathing),
            "cycle" | "colour-cycle" | "color-cycle" | "colorcycle" => Some(Effect::ColorCycle),
            "wave" | "colour-wave" | "color-wave" | "colorwave" => Some(Effect::ColorWave),
            "aurora" => Some(Effect::Aurora),
            _ => None,
        }
    }

    /// Whether the effect animates, which is what decides if speed means
    /// anything for it.
    pub fn animated(self) -> bool {
        matches!(self, Effect::Breathing | Effect::ColorCycle | Effect::ColorWave | Effect::Aurora)
    }

    /// Whether it reads a palette rather than a single colour.
    pub fn gradient(self) -> bool {
        matches!(self, Effect::ColorCycle | Effect::ColorWave)
    }

    /// Whether the effect is given any colour at all.
    ///
    /// The aurora is the firmware's own animation and carries no colour field —
    /// so offering a colour well for it is a control that does nothing, which
    /// is the same defect as a menu row that always fails.
    pub fn takes_colour(self) -> bool {
        self != Effect::Aurora
    }

    /// Whether it stores a period, and therefore has a speed to set.
    pub fn takes_speed(self) -> bool {
        matches!(self, Effect::Breathing | Effect::ColorCycle | Effect::ColorWave)
    }

    /// Only a wave is stretched along the zone, and only a wave has a
    /// direction to spread it in.
    pub fn takes_length(self) -> bool {
        self == Effect::ColorWave
    }

    /// The effects a given zone will accept.
    ///
    /// The sliders are a strip of a few LEDs; the aurora has nowhere to happen
    /// on them and the device does not offer it there.
    pub fn for_zone(zone: &str) -> &'static [Effect] {
        const ALL: [Effect; 5] = [
            Effect::Solid,
            Effect::Breathing,
            Effect::ColorCycle,
            Effect::ColorWave,
            Effect::Aurora,
        ];
        const STRIP: [Effect; 4] = [
            Effect::Solid,
            Effect::Breathing,
            Effect::ColorCycle,
            Effect::ColorWave,
        ];
        if is_slider(zone) {
            &STRIP
        } else {
            &ALL
        }
    }
}

/// Two colours, `t` of the way between them.
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// The colour a sorted stop list shows at `at` (0–100), flat outside the ends.
fn sample_at(stops: &[Stop], at: f32) -> [u8; 3] {
    let first = stops[0];
    let last = stops[stops.len() - 1];
    if at <= first.position as f32 {
        return first.color;
    }
    if at >= last.position as f32 {
        return last.color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if at >= a.position as f32 && at <= b.position as f32 {
            let span = (b.position as f32 - a.position as f32).max(0.001);
            return mix(a.color, b.color, (at - a.position as f32) / span);
        }
    }
    last.color
}

/// The touch sliders, which take a narrower set of settings than the two areas.
pub fn is_slider(zone: &str) -> bool {
    zone == "leftSlider" || zone == "rightSlider"
}

/// A wave along a strip can only run one way or the other.
///
/// The areas take any angle; a slider is one-dimensional, so the device offers
/// only the two angles that mean "along it".
///
/// **Along it is 0° and 180°, not 90° and 270°.** The sliders are strips lying
/// along the function row — see `assets/clvx-s-layout.json` — so a wave runs
/// left or right, which at `0° = to the right` is 0 and 180. Those are also the
/// two angles that encode to the only directions §7 of `docs/PROTOCOL.md`
/// records a slider ever holding: `direction = (angle + 90) mod 360` gives 90
/// and 270. Snapping to 90/270 instead confused an angle with a direction and
/// wrote a wave running *across* a strip one LED thick, which is a strip that
/// does not appear to animate at all.
pub fn angle_for_zone(zone: &str, angle: u32) -> u32 {
    if !is_slider(zone) {
        return angle % 360;
    }
    let a = angle % 360;
    if a > 90 && a <= 270 {
        180
    } else {
        0
    }
}

// ─────────────────────────────────── a look ──────────────────────────────────

/// One colour stop of a palette.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stop {
    pub color: [u8; 3],
    /// 0–100, per stop, independent of the zone's own dial.
    pub opacity: u8,
    /// 0–100 along the gradient.
    pub position: u8,
}

impl Stop {
    pub fn new(color: [u8; 3], position: u8) -> Stop {
        Stop { color, opacity: 100, position: position.min(100) }
    }
}

/// What a zone should look like, in the words the controls use.
#[derive(Clone, PartialEq, Debug)]
pub struct Look {
    pub effect: Effect,
    /// The palette. A solid colour or a breath uses the first stop only.
    pub stops: Vec<Stop>,
    /// The zone's dial, 0–100.
    pub opacity: u8,
    /// Ours, not the device's: scales the colours before they are sent.
    pub brightness: u8,
    pub speed: u32,
    pub length: u32,
    /// Gradient angle in degrees, 0 = along the zone to the right.
    pub angle: u32,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            effect: Effect::Solid,
            stops: vec![Stop::new([0, 200, 255], 0)],
            opacity: 100,
            brightness: 100,
            speed: SPEED_DEFAULT,
            length: LENGTH_DEFAULT,
            angle: 0,
        }
    }
}

impl Look {
    pub fn solid(color: [u8; 3]) -> Look {
        Look { effect: Effect::Solid, stops: vec![Stop::new(color, 0)], ..Look::default() }
    }

    pub fn breathing(color: [u8; 3], speed: u32) -> Look {
        Look {
            effect: Effect::Breathing,
            stops: vec![Stop::new(color, 0)],
            speed,
            ..Look::default()
        }
    }

    /// A palette spread evenly across the zone.
    pub fn spread(effect: Effect, colors: &[[u8; 3]]) -> Look {
        let n = colors.len().max(1);
        let stops = colors
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let pos = if n == 1 { 0 } else { (i * 100 / (n - 1)) as u8 };
                Stop::new(*c, pos)
            })
            .collect();
        Look { effect, stops, ..Look::default() }
    }

    // Only the tests build a wave this way — the themes name the effect
    // explicitly. `cargo check` cannot see a `#[cfg(test)]` caller, so it
    // reports this as dead on any target whose tests it is not compiling.
    #[allow(dead_code)]
    pub fn wave(colors: &[[u8; 3]]) -> Look {
        Look::spread(Effect::ColorWave, colors)
    }

    pub fn aurora() -> Look {
        Look { effect: Effect::Aurora, ..Look::default() }
    }

    /// Every colour this look shows, brightness applied, for a swatch strip.
    pub fn palette(&self) -> Vec<[u8; 3]> {
        if self.effect == Effect::Aurora {
            // The aurora's colours are the firmware's, not ours. This is what
            // it looks like, so a picker can draw something rather than
            // nothing — it is a label, and it is never written to the device.
            return vec![[0, 200, 255], [80, 90, 255], [200, 60, 255], [40, 220, 180]];
        }
        let mut out: Vec<[u8; 3]> = Vec::new();
        for s in self.effect_stops() {
            let c = dim(s.color, self.brightness);
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }

    /// The look resolved into `n` colours across the zone, at full strength.
    ///
    /// A preview drawn from stops and positions would have to interpolate, and
    /// interpolating is where a second answer to "what colour is this" creeps
    /// in. So the gradient is resolved here, once, and a window only has to
    /// paint the list it is handed.
    ///
    /// Brightness is deliberately **not** applied. It and opacity are the two
    /// controls a preview can honour by itself — one scales a colour, the other
    /// is an alpha — and a window that has to ask for a new list every time a
    /// slider moves is a window with a process launch between the thumb and the
    /// pixels. What reaches the *device* still has brightness applied here, in
    /// `to_zone_json`; this is the display copy.
    pub fn samples(&self, n: usize) -> Vec<[u8; 3]> {
        let n = n.max(1);
        if self.effect == Effect::Aurora {
            // The firmware draws this one; these are the colours it moves
            // through, so the preview is honest about being an impression.
            let base = self.palette();
            return (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32 * base.len() as f32;
                    let a = base[t as usize % base.len()];
                    let b = base[(t as usize + 1) % base.len()];
                    mix(a, b, t.fract())
                })
                .collect();
        }
        if !self.effect.gradient() {
            return vec![self.stops.first().map(|s| s.color).unwrap_or([0, 0, 0]); n];
        }

        let mut stops: Vec<Stop> = self.stops.clone();
        stops.sort_by_key(|s| s.position);
        if stops.is_empty() {
            return vec![[0, 0, 0]; n];
        }
        (0..n)
            .map(|i| {
                let at = if n == 1 { 0.0 } else { i as f32 * 100.0 / (n - 1) as f32 };
                sample_at(&stops, at)
            })
            .collect()
    }

    /// The stops this effect actually uses: one for the solid and breathing
    /// effects, the whole palette for the two gradients.
    fn effect_stops(&self) -> &[Stop] {
        if self.effect.gradient() {
            &self.stops
        } else {
            &self.stops[..self.stops.len().min(1)]
        }
    }

    /// The marker array, padded to the width the device demands.
    ///
    /// `markersNumber` stays at the real count while the array is filled out to
    /// [`MARKERS`] with copies of the first stop. Both halves matter: a short
    /// array is refused by the firmware, and a `markersNumber` that counts the
    /// padding makes the vendor application read three duplicate stops back.
    fn markers(&self) -> (usize, Vec<Stop>) {
        let mut used: Vec<Stop> = self.stops.iter().take(MARKERS).copied().collect();
        if used.is_empty() {
            used.push(Stop::new([0, 0, 0], 0));
        }
        let count = used.len();
        while used.len() < MARKERS {
            used.push(used[0]);
        }
        (count, used)
    }

    /// This look as the zone would actually hold it.
    ///
    /// A zone that cannot do an effect gets the nearest thing it can, and that
    /// correction has to happen *before* anything is shown as well as before
    /// anything is written — otherwise a builder reports a touch slider running
    /// an aurora, draws the aurora's colours, and the device quietly gets a
    /// solid instead. One rule, applied once, used by both.
    pub fn for_zone(&self, zone: &str) -> Look {
        let mut out = self.clone();
        if !Effect::for_zone(zone).contains(&out.effect) {
            out.effect = Effect::Solid;
        }
        out.angle = angle_for_zone(zone, out.angle);
        out
    }

    /// Encode for one zone, correcting anything that zone will not take.
    pub fn to_zone_json(&self, zone: &str) -> Json {
        let color_json = |c: [u8; 3]| {
            let c = dim(c, self.brightness);
            Json::obj(vec![
                ("red", Json::Num(c[0] as f64)),
                ("green", Json::Num(c[1] as f64)),
                ("blue", Json::Num(c[2] as f64)),
            ])
        };
        let picker = || {
            let (count, stops) = self.markers();
            let markers: Vec<Json> = stops
                .iter()
                .map(|s| {
                    Json::obj(vec![
                        ("color", color_json(s.color)),
                        ("transparency", Json::Num(transparency_from_opacity(s.opacity) as f64)),
                        ("position", Json::Num(s.position.min(100) as f64)),
                    ])
                })
                .collect();
            Json::obj(vec![
                ("markersNumber", Json::Num(count as f64)),
                ("markersArray", Json::Arr(markers)),
            ])
        };
        let first = self.stops.first().map(|s| s.color).unwrap_or([0, 0, 0]);
        let period = period_from_speed(self.speed) as f64;

        // An effect the zone will not take falls back to a solid colour rather
        // than to nothing: a zone that goes dark reads as a failure.
        let effect = if Effect::for_zone(zone).contains(&self.effect) {
            self.effect
        } else {
            Effect::Solid
        };

        let body = match effect {
            Effect::Solid => Json::obj(vec![("color", color_json(first))]),
            Effect::Breathing => Json::obj(vec![
                ("color", color_json(first)),
                ("period", Json::Num(period)),
            ]),
            Effect::ColorCycle => Json::obj(vec![
                ("colorLinePicker", picker()),
                ("period", Json::Num(period)),
            ]),
            Effect::ColorWave => Json::obj(vec![
                ("colorLinePicker", picker()),
                ("period", Json::Num(period)),
                (
                    "direction",
                    Json::Num(direction_for_zone(zone, self.angle) as f64),
                ),
                ("length", Json::Num(self.length.clamp(LENGTH_MIN, LENGTH_MAX) as f64)),
            ]),
            // The aurora carries no settings of its own; the zone's dial is
            // the only thing that shapes it.
            Effect::Aurora => Json::obj(vec![]),
        };

        Json::obj(vec![
            (effect.key(), body),
            ("transparency", Json::Num(transparency_from_opacity(self.opacity) as f64)),
        ])
    }

    /// Read a look back out of a zone, so the builder opens on what the
    /// keyboard is actually doing.
    ///
    /// Takes the zone's *name* as well as its JSON because a stored direction
    /// does not mean the same thing on a strip as on an area — see
    /// [`angle_for_zone_from_direction`].
    pub fn from_zone_json(name: &str, zone: &Json) -> Look {
        let rgb = |v: &Json| -> [u8; 3] {
            let ch = |k: &str| v.get(k).and_then(|x| x.as_u32()).unwrap_or(0).min(255) as u8;
            [ch("red"), ch("green"), ch("blue")]
        };
        let opacity = opacity_from_transparency(
            zone.get("transparency").and_then(|v| v.as_u32()).unwrap_or(0),
        );
        let mut look = Look { opacity, ..Look::default() };

        for effect in [
            Effect::Aurora,
            Effect::Breathing,
            Effect::ColorCycle,
            Effect::ColorWave,
            Effect::Solid,
        ] {
            let body = match zone.get(effect.key()) {
                Some(b) => b,
                None => continue,
            };
            look.effect = effect;
            if let Some(p) = body.get("period").and_then(|v| v.as_u32()) {
                look.speed = speed_from_period(p);
            }
            if let Some(l) = body.get("length").and_then(|v| v.as_u32()) {
                look.length = l.clamp(LENGTH_MIN, LENGTH_MAX);
            }
            if let Some(d) = body.get("direction").and_then(|v| v.as_u32()) {
                look.angle = angle_for_zone_from_direction(name, d);
            }
            if let Some(c) = body.get("color") {
                look.stops = vec![Stop::new(rgb(c), 0)];
            }
            if let Some(picker) = body.get("colorLinePicker") {
                let all = picker
                    .get("markersArray")
                    .and_then(|a| a.as_array())
                    .cloned()
                    .unwrap_or_default();
                // Read only as many markers as the device says are real: the
                // rest are padding, and showing them would put phantom stops
                // in the builder.
                let count = picker
                    .get("markersNumber")
                    .and_then(|v| v.as_u32())
                    .map(|n| (n as usize).min(all.len()))
                    .unwrap_or(all.len());
                let stops: Vec<Stop> = all
                    .iter()
                    .take(count.max(1))
                    .map(|m| Stop {
                        color: m.get("color").map(rgb).unwrap_or([0, 0, 0]),
                        opacity: opacity_from_transparency(
                            m.get("transparency").and_then(|v| v.as_u32()).unwrap_or(0),
                        ),
                        position: m
                            .get("position")
                            .and_then(|v| v.as_u32())
                            .unwrap_or(0)
                            .min(100) as u8,
                    })
                    .collect();
                if !stops.is_empty() {
                    look.stops = stops;
                }
            }
            break;
        }
        look
    }
}

// ─────────────────────────── the model a builder edits ───────────────────────

/// `#RRGGBB`, which is what a colour well speaks.
pub fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

pub fn from_hex(s: &str) -> Option<[u8; 3]> {
    let t = s.trim().trim_start_matches('#');
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

impl Look {
    /// The look as a builder's controls see it: names, hexes and slider values.
    ///
    /// A visual builder must not have to know that speed is a period counted
    /// backwards or that a gradient angle is ninety degrees off. It edits this,
    /// hands it back, and every conversion still happens in exactly one place.
    pub fn to_model(&self, zone: &str) -> Json {
        let corrected = self.for_zone(zone);
        if corrected != *self {
            return corrected.to_model(zone);
        }
        let stops: Vec<Json> = self
            .stops
            .iter()
            .map(|s| {
                Json::obj(vec![
                    ("color", Json::Str(to_hex(s.color))),
                    ("opacity", Json::Num(s.opacity as f64)),
                    ("position", Json::Num(s.position as f64)),
                ])
            })
            .collect();
        let offered: Vec<Json> = Effect::for_zone(zone)
            .iter()
            .map(|e| {
                Json::obj(vec![
                    ("key", Json::Str(e.key().into())),
                    ("label", Json::Str(e.label().into())),
                    ("gradient", Json::Bool(e.gradient())),
                    ("animated", Json::Bool(e.animated())),
                    // Which controls this effect actually reads. A window that
                    // worked these out for itself would be a second answer to
                    // a question `Effect` already answers — and the first
                    // version of the builder guessed `length` from the effect
                    // name in Swift, which is exactly that.
                    ("colours", Json::Bool(e.takes_colour())),
                    ("speed", Json::Bool(e.takes_speed())),
                    ("length", Json::Bool(e.takes_length())),
                ])
            })
            .collect();
        Json::obj(vec![
            ("effect", Json::Str(self.effect.key().into())),
            ("stops", Json::Arr(stops)),
            ("opacity", Json::Num(self.opacity as f64)),
            ("brightness", Json::Num(self.brightness as f64)),
            ("speed", Json::Num(self.speed as f64)),
            // The same speed as the device keeps it: one cycle of the animation
            // takes this many milliseconds. A preview that wants to run at the
            // rate the keyboard runs at needs the period, and working it back
            // out of `speed` in the window would be this module's arithmetic
            // living somewhere else — which is how a preview drifts from the
            // hardware it claims to be showing.
            ("periodMs", Json::Num(period_from_speed(self.speed) as f64)),
            ("length", Json::Num(self.length as f64)),
            ("angle", Json::Num(self.angle as f64)),
            ("swatch", Json::Arr(self.palette().iter().map(|c| Json::Str(to_hex(*c))).collect())),
            // Resolved for a preview to paint straight onto the keys, so no
            // window ever interpolates a gradient for itself. At full strength:
            // brightness and opacity are the window's to apply, so those two
            // sliders move without a round trip. See `Look::samples`.
            ("preview", Json::Arr(self.samples(PREVIEW_SAMPLES).iter().map(|c| Json::Str(to_hex(*c))).collect())),
            ("offers", Json::Arr(offered)),
            ("anglesFree", Json::Bool(!is_slider(zone))),
        ])
    }

    /// Read that model back, clamping anything a control let through.
    pub fn from_model(v: &Json) -> Look {
        let num = |k: &str, or: u32| v.get(k).and_then(|x| x.as_u32()).unwrap_or(or);
        let stops: Vec<Stop> = v
            .get("stops")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        let color = match s.get("color") {
                            Some(Json::Str(h)) => from_hex(h)?,
                            _ => return None,
                        };
                        Some(Stop {
                            color,
                            opacity: s.get("opacity").and_then(|x| x.as_u32()).unwrap_or(100).min(100) as u8,
                            position: s.get("position").and_then(|x| x.as_u32()).unwrap_or(0).min(100) as u8,
                        })
                    })
                    .take(MARKERS)
                    .collect()
            })
            .unwrap_or_default();
        let effect = match v.get("effect") {
            Some(Json::Str(k)) => Effect::from_key(k).or_else(|| Effect::parse(k)).unwrap_or(Effect::Solid),
            _ => Effect::Solid,
        };
        Look {
            effect,
            stops: if stops.is_empty() { Look::default().stops } else { stops },
            opacity: num("opacity", 100).min(100) as u8,
            brightness: num("brightness", 100).min(100) as u8,
            speed: num("speed", SPEED_DEFAULT).clamp(SPEED_MIN, SPEED_MAX),
            length: num("length", LENGTH_DEFAULT).clamp(LENGTH_MIN, LENGTH_MAX),
            angle: num("angle", 0) % 360,
        }
    }
}

impl Scheme {
    /// Every zone's controls, plus the two reactive layers and the ranges the
    /// sliders must respect — so a builder can lay itself out from this alone.
    pub fn to_model(&self) -> Json {
        let zones: Vec<(&str, Json)> = crate::backlight::ZONES
            .iter()
            .map(|(name, _)| (*name, self.look(name).to_model(name)))
            .collect();
        let reactive = |r: Option<Reactive>, zone: &str| -> Json {
            let (lo, hi, label) = if zone == "touchpad" {
                (TRACE_MIN, TRACE_MAX, "Trace")
            } else {
                (DURATION_MIN, DURATION_MAX, "Duration")
            };
            let r = r.unwrap_or(Reactive {
                enabled: false,
                color: [255, 255, 255],
                amount: if zone == "touchpad" { TRACE_DEFAULT } else { DURATION_DEFAULT },
            });
            Json::obj(vec![
                ("enabled", Json::Bool(r.enabled)),
                ("color", Json::Str(to_hex(r.color))),
                ("amount", Json::Num(r.amount.clamp(lo, hi) as f64)),
                ("min", Json::Num(lo as f64)),
                ("max", Json::Num(hi as f64)),
                ("label", Json::Str(label.into())),
            ])
        };
        let range = |lo: u32, hi: u32| Json::Arr(vec![Json::Num(lo as f64), Json::Num(hi as f64)]);
        Json::obj(vec![
            ("zones", Json::obj(zones)),
            ("typing", reactive(self.typing, "keyboard")),
            ("gesture", reactive(self.gesture, "touchpad")),
            ("ranges", Json::obj(vec![
                ("speed", range(SPEED_MIN, SPEED_MAX)),
            // `period = pivot - speed`, in milliseconds.
            //
            // The window needs the relation, not just one answer from it: a
            // speed slider that only ever saw the `periodMs` of the look it was
            // handed animated at the speed the *previous* value implied, and
            // dragging it changed the number and nothing else. Handing over the
            // pivot keeps the arithmetic this module's while still letting a
            // preview follow a drag.
            ("speedPivot", Json::Num(SPEED_PIVOT as f64)),
                ("length", range(LENGTH_MIN, LENGTH_MAX)),
                ("opacity", range(0, OPACITY_MAX as u32)),
                ("brightness", range(0, 100)),
                ("angle", range(0, 359)),
                ("markers", Json::Num(MARKERS as f64)),
            ])),
        ])
    }

    pub fn from_model(v: &Json) -> Scheme {
        let zones = v.get("zones").cloned().unwrap_or(Json::Null);
        let look = |name: &str| zones.get(name).map(Look::from_model).unwrap_or_default();
        let reactive = |key: &str, zone: &str| -> Option<Reactive> {
            let r = v.get(key)?;
            let (lo, hi) = if zone == "touchpad" {
                (TRACE_MIN, TRACE_MAX)
            } else {
                (DURATION_MIN, DURATION_MAX)
            };
            Some(Reactive {
                enabled: r.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false),
                color: match r.get("color") {
                    Some(Json::Str(h)) => from_hex(h).unwrap_or([255, 255, 255]),
                    _ => [255, 255, 255],
                },
                amount: r.get("amount").and_then(|x| x.as_u32()).unwrap_or(lo).clamp(lo, hi),
            })
        };
        Scheme {
            keyboard: look("keyboard"),
            touchpad: look("touchpad"),
            left_slider: look("leftSlider"),
            right_slider: look("rightSlider"),
            typing: reactive("typing", "keyboard"),
            gesture: reactive("gesture", "touchpad"),
        }
    }
}

// ───────────────────────────── the reactive layers ───────────────────────────

/// The light that follows a keypress, and the one that follows a gesture.
///
/// One shape covers both because the device stores them the same way; only the
/// zone they hang off and the range of the third value differ.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Reactive {
    pub enabled: bool,
    pub color: [u8; 3],
    /// Duration on the keyboard (1–3), trace on the touchpad (1–5).
    pub amount: u32,
}

impl Reactive {
    pub fn typing(color: [u8; 3]) -> Reactive {
        Reactive { enabled: true, color, amount: DURATION_DEFAULT }
    }

    pub fn gesture(color: [u8; 3]) -> Reactive {
        Reactive { enabled: true, color, amount: TRACE_DEFAULT }
    }

    fn to_json(self, zone: &str) -> Json {
        let (lo, hi) = if zone == "touchpad" {
            (TRACE_MIN, TRACE_MAX)
        } else {
            (DURATION_MIN, DURATION_MAX)
        };
        let key = if zone == "touchpad" { "trace" } else { "duration" };
        Json::obj(vec![
            ("color", Json::obj(vec![
                ("red", Json::Num(self.color[0] as f64)),
                ("green", Json::Num(self.color[1] as f64)),
                ("blue", Json::Num(self.color[2] as f64)),
            ])),
            ("enable", Json::Bool(self.enabled)),
            (key, Json::Num(self.amount.clamp(lo, hi) as f64)),
        ])
    }
}

/// Whether a zone carries a reactive layer at all.
///
/// The sliders do not: the device leaves the field out for them, and writing
/// one there is a setting with nothing to act on.
pub fn zone_has_reactive(zone: &str) -> bool {
    zone == "keyboard" || zone == "touchpad"
}

// ────────────────────────────────── a scheme ─────────────────────────────────

/// A whole keyboard: a look per zone, plus the two reactive layers.
#[derive(Clone, PartialEq, Debug)]
pub struct Scheme {
    pub keyboard: Look,
    pub touchpad: Look,
    pub left_slider: Look,
    pub right_slider: Look,
    /// Lights the key that was just pressed. Lives on the keyboard zone.
    pub typing: Option<Reactive>,
    /// Follows a finger across the touchpad. Lives on the touchpad zone.
    pub gesture: Option<Reactive>,
}

impl Scheme {
    /// The same look everywhere, which is what most themes want.
    pub fn uniform(look: Look) -> Scheme {
        Scheme {
            keyboard: look.clone(),
            touchpad: look.clone(),
            left_slider: look.clone(),
            right_slider: look,
            typing: None,
            gesture: None,
        }
    }

    pub fn look(&self, zone: &str) -> &Look {
        match zone {
            "touchpad" => &self.touchpad,
            "leftSlider" => &self.left_slider,
            "rightSlider" => &self.right_slider,
            _ => &self.keyboard,
        }
    }

    #[allow(dead_code)] // used by the tests, which `cargo check` cannot see
    pub fn look_mut(&mut self, zone: &str) -> &mut Look {
        match zone {
            "touchpad" => &mut self.touchpad,
            "leftSlider" => &mut self.left_slider,
            "rightSlider" => &mut self.right_slider,
            _ => &mut self.keyboard,
        }
    }

    /// Turn the reactive layers on, in a colour drawn from the scheme itself.
    pub fn with_reactive(mut self) -> Scheme {
        let pick = |look: &Look| look.palette().into_iter().next().unwrap_or([255, 255, 255]);
        self.typing = Some(Reactive::typing(pick(&self.keyboard)));
        self.gesture = Some(Reactive::gesture(pick(&self.touchpad)));
        self
    }

    /// The scheme document this tool reads and writes.
    pub fn to_doc(&self) -> Json {
        self.to_doc_for(&crate::backlight::ZONES.map(|(n, _)| n.to_string()))
    }

    /// The same, restricted to the zones the caller asked for.
    pub fn to_doc_for(&self, zones: &[String]) -> Json {
        let mut out: Vec<(&str, Json)> = Vec::new();
        for (name, _) in crate::backlight::ZONES {
            if !zones.iter().any(|z| z == name) {
                continue;
            }
            let mut zone = self.look(name).to_zone_json(name);
            let reactive = match name {
                "keyboard" => self.typing,
                "touchpad" => self.gesture,
                _ => None,
            };
            if let (Some(r), true) = (reactive, zone_has_reactive(name)) {
                if let Json::Obj(fields) = &mut zone {
                    fields.insert("interactiveAnimation".to_string(), r.to_json(name));
                }
            }
            out.push((name, zone));
        }
        Json::obj(vec![
            (crate::backlight::SCHEMA_KEY, Json::Num(crate::backlight::SCHEMA_VERSION as f64)),
            ("backlight", Json::obj(out)),
        ])
    }

    /// Read a whole scheme back, so a builder can open on the live keyboard.
    pub fn from_doc(doc: &Json) -> Scheme {
        let backlight = doc.get("backlight").unwrap_or(doc);
        let look = |name: &str| {
            backlight
                .get(name)
                .map(|z| Look::from_zone_json(name, z))
                .unwrap_or_default()
        };
        let reactive = |name: &str| -> Option<Reactive> {
            let ia = backlight.get(name)?.get("interactiveAnimation")?;
            let ch = |k: &str| {
                ia.get("color")
                    .and_then(|c| c.get(k))
                    .and_then(|v| v.as_u32())
                    .unwrap_or(0)
                    .min(255) as u8
            };
            let amount = ["duration", "trace", "extra"]
                .iter()
                .find_map(|k| ia.get(k).and_then(|v| v.as_u32()))
                .unwrap_or(if name == "touchpad" { TRACE_DEFAULT } else { DURATION_DEFAULT });
            Some(Reactive {
                enabled: ia.get("enable").and_then(|v| v.as_bool()).unwrap_or(false),
                color: [ch("red"), ch("green"), ch("blue")],
                amount,
            })
        };
        Scheme {
            keyboard: look("keyboard"),
            touchpad: look("touchpad"),
            left_slider: look("leftSlider"),
            right_slider: look("rightSlider"),
            typing: reactive("keyboard"),
            gesture: reactive("touchpad"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    #[test]
    fn speed_and_period_are_the_same_scale_from_opposite_ends() {
        // The fastest setting is the shortest period, not the longest.
        assert!(period_from_speed(SPEED_MAX) < period_from_speed(SPEED_MIN));
        for speed in [SPEED_MIN, 1_000, SPEED_DEFAULT, 9_500, SPEED_MAX] {
            assert_eq!(speed_from_period(period_from_speed(speed)), speed);
        }
        // Out of range clamps rather than wrapping — a period of zero would be
        // a division by nothing in the firmware.
        assert_eq!(period_from_speed(0), period_from_speed(SPEED_MIN));
        assert_eq!(period_from_speed(99_999), period_from_speed(SPEED_MAX));
    }

    #[test]
    fn opacity_is_the_complement_of_transparency() {
        assert_eq!(transparency_from_opacity(100), 0, "fully lit stores zero");
        assert_eq!(transparency_from_opacity(0), 100);
        for o in [0u8, 15, 30, 50, 70, 85, 100] {
            assert_eq!(opacity_from_transparency(transparency_from_opacity(o)), o);
        }
    }

    #[test]
    fn a_gradient_angle_and_the_stored_direction_are_mirrors() {
        for angle in [0u32, 45, 90, 180, 270, 359] {
            assert_eq!(angle_from_direction(direction_from_angle(angle)), angle);
        }
        // The four cardinals, measured on hardware — see `ANGLE_MIRROR`. Two
        // earlier readings of this were *offsets*, 90 and then 270, and an
        // offset cannot mirror: it moved the fault rather than removing it.
        assert_eq!(direction_from_angle(0), 180);
        assert_eq!(direction_from_angle(90), 90);
        assert_eq!(direction_from_angle(180), 0);
        assert_eq!(direction_from_angle(270), 270);
        // A mirror is its own inverse, so one function serves both ways.
        for angle in 0..360 {
            assert_eq!(direction_from_angle(angle), angle_from_direction(angle));
        }
    }

    #[test]
    fn a_slider_only_takes_the_two_angles_that_run_along_it() {
        for zone in ["leftSlider", "rightSlider"] {
            for angle in 0..360 {
                let a = angle_for_zone(zone, angle);
                assert!(a == 0 || a == 180, "{} took {} for {}", zone, a, angle);
                // A strip's directions are tokens, not geometry: §7 records it
                // holding only 90 or 270, and the areas' mirror would write
                // 180 and 0 — which a strip does not take. So the pairing is
                // explicit, and it round-trips.
                let d = direction_for_zone(zone, angle);
                assert!(d == 90 || d == 270, "{} wrote direction {}", zone, d);
                assert_eq!(angle_for_zone_from_direction(zone, d), a,
                           "{} did not read back the way it was written", zone);
            }
        }
        assert_eq!(angle_for_zone("keyboard", 137), 137, "an area takes any angle");
    }

    #[test]
    fn the_aurora_is_not_offered_on_a_slider() {
        assert!(Effect::for_zone("keyboard").contains(&Effect::Aurora));
        assert!(!Effect::for_zone("leftSlider").contains(&Effect::Aurora));
        // Asking for it anyway must not leave the zone dark.
        let z = Look::aurora().to_zone_json("rightSlider");
        assert!(z.get("solidColor").is_some(), "fell back to a colour, not to nothing");
        assert!(z.get("aurora").is_none());
    }

    #[test]
    fn every_gradient_carries_exactly_five_markers_but_counts_only_the_real_ones() {
        let look = Look::wave(&[[255, 0, 0], [0, 0, 255]]);
        let z = look.to_zone_json("keyboard");
        let picker = z.get("colorWave").unwrap().get("colorLinePicker").unwrap();
        let arr = picker.get("markersArray").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), MARKERS, "a short array is refused by the firmware");
        assert_eq!(picker.get("markersNumber").unwrap().as_u32(), Some(2));
        // The padding repeats the first stop, so nothing new appears.
        assert_eq!(crate::ui::hex(arr[2].get("color").unwrap()), "#FF0000");
    }

    #[test]
    fn brightness_scales_the_colours_because_the_device_has_no_such_dial() {
        assert_eq!(dim([200, 100, 50], 100), [200, 100, 50]);
        assert_eq!(dim([200, 100, 50], 50), [100, 50, 25]);
        assert_eq!(dim([200, 100, 50], 0), [0, 0, 0]);
        let look = Look { brightness: 50, ..Look::solid([200, 100, 50]) };
        let z = look.to_zone_json("keyboard");
        assert_eq!(crate::ui::hex(z.get("solidColor").unwrap().get("color").unwrap()), "#643219");
        // …and it does not touch the device's own dial.
        assert_eq!(z.get("transparency").unwrap().as_u32(), Some(0));
    }

    #[test]
    fn a_look_survives_a_round_trip_through_the_wire_shape() {
        let look = Look {
            effect: Effect::ColorWave,
            stops: vec![
                Stop::new([255, 83, 83], 0),
                Stop { color: [0, 200, 255], opacity: 50, position: 60 },
            ],
            opacity: 70,
            brightness: 100,
            speed: 3_200,
            length: 640,
            angle: 45,
        };
        let back = Look::from_zone_json("keyboard", &look.to_zone_json("keyboard"));
        assert_eq!(back, look);
    }

    #[test]
    fn a_scheme_round_trips_including_the_reactive_layers() {
        let scheme = Scheme::uniform(Look::wave(&[[255, 0, 0], [0, 255, 0], [0, 0, 255]]))
            .with_reactive();
        let doc = scheme.to_doc();
        // Everything the encoder produced must satisfy the wire validator.
        crate::backlight::from_json(&doc).expect("the device would accept this");
        let back = Scheme::from_doc(&doc);
        assert_eq!(back.keyboard, scheme.keyboard);
        assert_eq!(back.typing, scheme.typing);
        assert_eq!(back.gesture.map(|g| g.amount), scheme.gesture.map(|g| g.amount));
    }

    #[test]
    fn sliders_never_carry_a_reactive_layer() {
        let doc = Scheme::uniform(Look::solid([1, 2, 3])).with_reactive().to_doc();
        let b = doc.get("backlight").unwrap();
        assert!(b.get("keyboard").unwrap().get("interactiveAnimation").is_some());
        assert!(b.get("touchpad").unwrap().get("interactiveAnimation").is_some());
        for slider in ["leftSlider", "rightSlider"] {
            assert!(
                b.get(slider).unwrap().get("interactiveAnimation").is_none(),
                "{} has nothing for a reactive layer to act on",
                slider
            );
        }
    }

    #[test]
    fn the_touchpad_stores_a_trace_and_the_keyboard_a_duration() {
        let doc = Scheme::uniform(Look::solid([1, 2, 3])).with_reactive().to_doc();
        let b = doc.get("backlight").unwrap();
        let kb = b.get("keyboard").unwrap().get("interactiveAnimation").unwrap();
        let tp = b.get("touchpad").unwrap().get("interactiveAnimation").unwrap();
        assert!(kb.get("duration").is_some() && kb.get("trace").is_none());
        assert!(tp.get("trace").is_some() && tp.get("duration").is_none());
    }

    #[test]
    fn a_scheme_can_be_built_for_a_subset_of_zones() {
        let doc = Scheme::uniform(Look::solid([9, 9, 9])).to_doc_for(&["touchpad".to_string()]);
        assert_eq!(crate::ui::zones_in(&doc), vec!["touchpad"]);
        crate::backlight::from_json(&doc).expect("a single zone is a valid scheme");
    }

    #[test]
    fn reading_a_gradient_ignores_the_padding_markers() {
        let doc = json::parse(
            r#"{"backlight":{"keyboard":{"colorWave":{"colorLinePicker":{
                 "markersNumber":2,"markersArray":[
                   {"color":{"red":255},"position":0},
                   {"color":{"blue":255},"position":100},
                   {"color":{"red":255},"position":0},
                   {"color":{"red":255},"position":0},
                   {"color":{"red":255},"position":0}]},
                 "period":1000,"direction":90,"length":300},"transparency":0}}}"#,
        )
        .unwrap();
        let look = Look::from_zone_json("keyboard", doc.get("backlight").unwrap().get("keyboard").unwrap());
        assert_eq!(look.stops.len(), 2, "three of the five are padding");
        // Direction 90 mirrors to angle 90 — see `ANGLE_MIRROR`.
        assert_eq!(look.angle, 90);
        assert_eq!(look.speed, speed_from_period(1000));
    }

    #[test]
    fn the_builder_model_round_trips_every_control() {
        // Angle 180 on purpose: it is one a touch slider can hold, so the whole
        // scheme survives unchanged. An angle a strip cannot point in is
        // snapped, which is asserted in
        // `a_zone_is_shown_holding_only_what_it_can_actually_hold`.
        let scheme = Scheme::uniform(Look {
            effect: Effect::ColorWave,
            stops: vec![Stop::new([255, 0, 0], 0), Stop::new([0, 0, 255], 100)],
            opacity: 80,
            brightness: 60,
            speed: 4_321,
            length: 777,
            angle: 180,
        })
        .with_reactive();
        let back = Scheme::from_model(&scheme.to_model());
        assert_eq!(back, scheme);
        // …and what comes back still encodes to something the device takes.
        crate::backlight::from_json(&back.to_doc()).expect("still writable");
    }

    #[test]
    fn a_zone_is_shown_holding_only_what_it_can_actually_hold() {
        // The defect this guards: a rolled aurora left the touch sliders
        // reporting `aurora` in the model. The effect menu fell back to the
        // first entry, so the window said "solid colour" while painting the
        // aurora's own colours — and the device got a solid. Three answers.
        let aurora = Look::aurora();
        let m = aurora.to_model("leftSlider");
        assert_eq!(m.get("effect"), Some(&Json::Str("solidColor".into())));
        let shown: Vec<String> = m.get("preview").unwrap().as_array().unwrap().iter()
            .filter_map(|c| match c { Json::Str(s) => Some(s.clone()), _ => None })
            .collect();
        assert_eq!(shown.iter().collect::<std::collections::BTreeSet<_>>().len(), 1,
                   "a solid colour previews as one colour, not as an aurora");
        // …and it is still an aurora where an aurora is possible.
        assert_eq!(aurora.to_model("keyboard").get("effect"), Some(&Json::Str("aurora".into())));

        // The same for an angle a strip cannot point in.
        let diagonal = Look { angle: 45, ..Look::wave(&[[255, 0, 0], [0, 0, 255]]) };
        let a = diagonal.to_model("rightSlider").get("angle").and_then(|v| v.as_u32());
        assert!(a == Some(0) || a == Some(180), "a strip runs one way or the other, got {:?}", a);
    }

    #[test]
    fn the_model_tells_a_builder_what_each_zone_will_take() {
        let m = Scheme::uniform(Look::default()).to_model();
        let kb = m.get("zones").unwrap().get("keyboard").unwrap();
        let slider = m.get("zones").unwrap().get("leftSlider").unwrap();
        let offers = |z: &Json| -> Vec<String> {
            z.get("offers")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|o| match o.get("key") {
                    Some(Json::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        };
        assert!(offers(kb).contains(&"aurora".to_string()));
        assert!(!offers(slider).contains(&"aurora".to_string()));
        // Every effect must say which controls it reads, so no window has to
        // guess — the aurora reads none of them.
        let aurora = kb.get("offers").unwrap().as_array().unwrap().iter()
            .find(|o| matches!(o.get("key"), Some(Json::Str(s)) if s == "aurora")).unwrap();
        assert_eq!(aurora.get("colours").unwrap().as_bool(), Some(false));
        assert_eq!(aurora.get("speed").unwrap().as_bool(), Some(false));
        assert_eq!(aurora.get("length").unwrap().as_bool(), Some(false));
        let wave = kb.get("offers").unwrap().as_array().unwrap().iter()
            .find(|o| matches!(o.get("key"), Some(Json::Str(s)) if s == "colorWave")).unwrap();
        for k in ["colours", "speed", "length", "gradient"] {
            assert_eq!(wave.get(k).unwrap().as_bool(), Some(true), "wave.{}", k);
        }
        assert_eq!(kb.get("anglesFree").unwrap().as_bool(), Some(true));
        assert_eq!(slider.get("anglesFree").unwrap().as_bool(), Some(false));
        // A builder lays its sliders out from these, so they must be present.
        let r = m.get("ranges").unwrap();
        for k in ["speed", "length", "opacity", "brightness", "angle"] {
            assert_eq!(r.get(k).unwrap().as_array().unwrap().len(), 2, "range {}", k);
        }
        assert_eq!(m.get("typing").unwrap().get("label"), Some(&Json::Str("Duration".into())));
        assert_eq!(m.get("gesture").unwrap().get("label"), Some(&Json::Str("Trace".into())));
    }

    #[test]
    fn a_model_with_impossible_numbers_is_clamped_not_refused() {
        // A slider that reports 9999 is a bug in a control, not a reason to
        // leave the keyboard as it was.
        // Doubled hashes: a `"#` inside a plain `r#"…"#` would end the string.
        let bad = json::parse(
            r##"{"zones":{"keyboard":{"effect":"colorWave","speed":99999,"length":0,
                 "opacity":300,"brightness":255,"angle":720,
                 "stops":[{"color":"#FF0000","position":900,"opacity":900},
                          {"color":"nonsense","position":10},
                          {"color":"#00FF00","position":50},
                          {"color":"#0000FF","position":60},
                          {"color":"#FFFFFF","position":70},
                          {"color":"#000000","position":80},
                          {"color":"#123456","position":90}]}}}"##,
        )
        .unwrap();
        let look = Look::from_model(bad.get("zones").unwrap().get("keyboard").unwrap());
        assert_eq!(look.speed, SPEED_MAX);
        assert_eq!(look.length, LENGTH_MIN);
        assert_eq!(look.opacity, 100);
        assert_eq!(look.angle, 0);
        assert_eq!(look.stops.len(), MARKERS, "an over-long palette is trimmed, not rejected");
        assert_eq!(look.stops[0].position, 100);
        crate::backlight::from_json(&Scheme::uniform(look).to_doc()).expect("still writable");
    }

    #[test]
    fn a_preview_resolves_the_gradient_so_no_window_has_to() {
        let look = Look {
            stops: vec![Stop::new([255, 0, 0], 0), Stop::new([0, 0, 255], 100)],
            ..Look::wave(&[])
        };
        let s = look.samples(5);
        assert_eq!(s.len(), 5);
        assert_eq!(s[0], [255, 0, 0], "the first stop holds the start");
        assert_eq!(s[4], [0, 0, 255], "the last stop holds the end");
        assert_eq!(s[2], [128, 0, 128], "and the middle is halfway between");

        // Outside the stops the gradient is flat, not extrapolated into a
        // colour the palette does not contain.
        let inset = Look {
            stops: vec![Stop::new([10, 20, 30], 40), Stop::new([40, 50, 60], 60)],
            ..Look::wave(&[])
        };
        let s = inset.samples(11);
        assert_eq!(s[0], [10, 20, 30]);
        assert_eq!(s[10], [40, 50, 60]);

        // Brightness is the preview's to apply, so the samples stay at full
        // strength — a slider must be able to move without asking for a new
        // list. What reaches the device is dimmed, and that is asserted in
        // `brightness_scales_the_colours_because_the_device_has_no_such_dial`.
        let solid = Look { brightness: 50, ..Look::solid([200, 100, 0]) };
        assert_eq!(solid.samples(3), vec![[200, 100, 0]; 3]);
        assert_eq!(
            crate::ui::hex(solid.to_zone_json("keyboard").get("solidColor").unwrap().get("color").unwrap()),
            "#643200",
            "the device still gets the dimmed colour"
        );

        // Stops given out of order still resolve left to right.
        let jumbled = Look {
            stops: vec![Stop::new([0, 0, 255], 100), Stop::new([255, 0, 0], 0)],
            ..Look::wave(&[])
        };
        assert_eq!(jumbled.samples(3)[0], [255, 0, 0]);

        // And the model carries the same list the preview will paint.
        let m = Look::wave(&[[255, 0, 0], [0, 0, 255]]).to_model("keyboard");
        assert_eq!(m.get("preview").unwrap().as_array().unwrap().len(), PREVIEW_SAMPLES);
    }

    #[test]
    fn hex_is_read_and_written_the_way_a_colour_well_speaks_it() {
        assert_eq!(to_hex([0, 200, 255]), "#00C8FF");
        assert_eq!(from_hex("#00c8ff"), Some([0, 200, 255]));
        assert_eq!(from_hex("00C8FF"), Some([0, 200, 255]));
        assert_eq!(from_hex("#00C8F"), None);
        assert_eq!(from_hex("#00C8FG"), None);
    }

    #[test]
    fn effect_names_are_accepted_as_people_type_them() {
        assert_eq!(Effect::parse("Wave"), Some(Effect::ColorWave));
        assert_eq!(Effect::parse("colour cycle"), Some(Effect::ColorCycle));
        assert_eq!(Effect::parse("breathe"), Some(Effect::Breathing));
        assert_eq!(Effect::parse("nope"), None);
        assert_eq!(Effect::from_key("aurora"), Some(Effect::Aurora));
    }
}
