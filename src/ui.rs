//! Output grammar shared by every surface.
//!
//! `<STATE>  <plain-language detail>` — state uppercase and column-aligned, so
//! a terminal, a log and a screen reader all say the same thing. The state
//! words are the write ladder, which is the point: SENT is not VERIFIED.

use crate::json::{self, Json};

pub const STATE_WIDTH: usize = 9;

#[derive(Clone, Copy)]
pub struct Style {
    pub color: bool,
    pub ascii: bool,
}

impl Style {
    pub fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", code, s)
        } else {
            s.to_string()
        }
    }
    // Brand palette, mapped to 256-colour ANSI.
    pub fn current(&self, s: &str) -> String { self.paint("38;5;45", s) }   // #00C8FF
    pub fn mint(&self, s: &str) -> String { self.paint("38;5;43", s) }      // #36F0B1
    pub fn trench(&self, s: &str) -> String { self.paint("38;5;200", s) }   // #FF00E8
    #[allow(dead_code)] // Brand palette; kept complete so themes can use every colour.
    pub fn coral(&self, s: &str) -> String { self.paint("38;5;203", s) }    // #FF5353
    pub fn amber(&self, s: &str) -> String { self.paint("38;5;214", s) }    // #FFB100
    pub fn dim(&self, s: &str) -> String { self.paint("2", s) }
    pub fn bold(&self, s: &str) -> String { self.paint("1", s) }
}

/// One line of the grammar: a state word, then plain language.
pub fn line(st: &Style, state: &str, detail: &str) -> String {
    let painted = match state {
        "VERIFIED" | "READY" | "BACKED UP" => st.mint(state),
        "MISMATCH" | "ERROR" => st.trench(state),
        "SENT" | "READ BACK" | "ACKNOWLEDGED" => st.current(state),
        "NEXT" | "CODE" => st.dim(state),
        _ => state.to_string(),
    };
    // pad on the visible width, not the escaped length
    let pad = STATE_WIDTH.saturating_sub(state.chars().count());
    format!("{}{}  {}", painted, " ".repeat(pad), detail)
}

pub fn say(st: &Style, state: &str, detail: &str) {
    println!("{}", line(st, state, detail));
}

/// Errors follow the same shape, plus what to do and a stable code.
pub fn error(st: &Style, detail: &str, next: &str, code: &str, exit: i32) {
    eprintln!("{}", line(st, "ERROR", detail));
    if !next.is_empty() {
        eprintln!("{}", line(st, "NEXT", next));
    }
    eprintln!("{}", line(st, "CODE", &format!("{} (exit {})", code, exit)));
}

pub fn error_json(detail: &str, code: &str) {
    println!(
        "{}",
        json::to_string_pretty(&Json::obj(vec![
            ("stage", Json::Str("failed".into())),
            ("code", Json::Str(code.into())),
            ("message", Json::Str(detail.into())),
        ]))
    );
}

/// Zone names as people say them, not as the wire spells them.
pub fn zone_label(key: &str) -> &'static str {
    match key {
        "keyboard" => "keyboard",
        "touchpad" => "touchpad",
        "leftSlider" => "left slider",
        "rightSlider" => "right slider",
        _ => "zone",
    }
}

pub fn effect_label(key: &str) -> &'static str {
    match key {
        "solidColor" => "Solid colour",
        "breathing" => "Breathing",
        "colorCycle" => "Colour cycle",
        "colorWave" => "Colour wave",
        "aurora" => "Aurora",
        _ => "Unknown",
    }
}

/// The zones a scheme actually names, in display order.
pub fn zones_in(doc: &Json) -> Vec<String> {
    let backlight = doc.get("backlight").unwrap_or(doc);
    let mut out = Vec::new();
    for (key, _) in crate::backlight::ZONES {
        if backlight.get(key).is_some() {
            out.push(key.to_string());
        }
    }
    out
}

/// A colour, shown as a colour.
///
/// A terminal that can paint gets a true-colour block; one that cannot gets the
/// hex, because a row of identical grey squares is worse than a row of numbers.
pub fn block(st: &Style, c: [u8; 3]) -> String {
    if !st.color {
        return format!("#{:02X}{:02X}{:02X} ", c[0], c[1], c[2]);
    }
    let glyph = if st.ascii { "#" } else { "██" };
    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", c[0], c[1], c[2], glyph)
}

pub fn hex(color: &Json) -> String {
    let c = |k: &str| color.get(k).and_then(|v| v.as_u32()).unwrap_or(0);
    format!("#{:02X}{:02X}{:02X}", c("red"), c("green"), c("blue"))
}

/// Every colour a scheme names, in the order it names them.
///
/// The shapes differ per effect — a wave carries a marker array, a solid colour
/// carries one object — so rather than teach this every effect, it walks for
/// anything with red/green/blue. Duplicates are dropped: a swatch strip of five
/// identical squares tells the viewer nothing.
pub fn swatches(doc: &Json, max: usize) -> Vec<String> {
    fn walk(v: &Json, out: &mut Vec<String>, max: usize) {
        if out.len() >= max {
            return;
        }
        match v {
            Json::Obj(fields) => {
                let has = |k: &str| fields.iter().any(|(n, _)| n == k);
                if has("red") && has("green") && has("blue") {
                    let h = hex(v);
                    if !out.contains(&h) {
                        out.push(h);
                    }
                    return;
                }
                for (_, child) in fields {
                    walk(child, out, max);
                }
            }
            Json::Arr(items) => {
                for child in items {
                    walk(child, out, max);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(doc, &mut out, max);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Style {
        Style { color: false, ascii: true }
    }

    #[test]
    fn states_are_column_aligned() {
        let st = plain();
        let a = line(&st, "SENT", "backlight");
        let b = line(&st, "READ BACK", "zones");
        let c = line(&st, "VERIFIED", "match");
        // the detail must begin at the same column on every line; searching for
        // the first double space would land inside the padding, so measure the
        // prefix width directly
        for (l, state) in [(&a, "SENT"), (&b, "READ BACK"), (&c, "VERIFIED")] {
            let prefix: String = l.chars().take(STATE_WIDTH + 2).collect();
            assert!(prefix.starts_with(state), "{:?} should start with {}", prefix, state);
            assert!(prefix.ends_with("  "), "{:?} should pad to the detail column", prefix);
            let detail = &l[STATE_WIDTH + 2..];
            assert!(!detail.starts_with(' '), "detail should start at the column: {:?}", l);
        }
    }

    #[test]
    fn no_color_output_has_no_escapes() {
        let st = plain();
        let s = line(&st, "VERIFIED", "device matches");
        assert!(!s.contains('\x1b'), "unexpected escape in {:?}", s);
    }

    #[test]
    fn color_output_still_aligns_visible_text() {
        let st = Style { color: true, ascii: false };
        let s = line(&st, "SENT", "x");
        let visible: String = strip_ansi(&s);
        assert!(visible.starts_with("SENT     "), "{:?}", visible);
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for c2 in chars.by_ref() {
                    if c2 == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn zone_labels_are_human() {
        assert_eq!(zone_label("leftSlider"), "left slider");
        assert_eq!(effect_label("colorWave"), "Colour wave");
    }

    #[test]
    fn hex_formats_colours() {
        let c = json::parse(r#"{"red":255,"green":83,"blue":83}"#).unwrap();
        assert_eq!(hex(&c), "#FF5353");
    }
    #[test]
    fn swatches_find_colours_at_any_depth_and_keep_order() {
        let doc = json::parse(
            r#"{"backlight":{"keyboard":{"colorWave":{"colorLinePicker":{"markersArray":[
                 {"color":{"red":255,"green":0,"blue":0},"position":0},
                 {"color":{"red":0,"green":255,"blue":0},"position":50}]}}},
                 "touchpad":{"solidColor":{"color":{"red":0,"green":0,"blue":255}}}}}"#,
        )
        .unwrap();
        assert_eq!(swatches(&doc, 5), vec!["#FF0000", "#00FF00", "#0000FF"]);
    }

    #[test]
    fn swatches_drop_repeats_and_respect_the_limit() {
        let doc = json::parse(
            r#"{"a":{"red":1,"green":2,"blue":3},
                "b":{"red":1,"green":2,"blue":3},
                "c":{"red":9,"green":9,"blue":9},
                "d":{"red":8,"green":8,"blue":8}}"#,
        )
        .unwrap();
        // A strip of identical squares says nothing, so repeats collapse.
        assert_eq!(swatches(&doc, 5), vec!["#010203", "#090909", "#080808"]);
        assert_eq!(swatches(&doc, 2), vec!["#010203", "#090909"]);
    }

    #[test]
    fn a_scheme_with_no_colours_yields_no_swatches() {
        let doc = json::parse(r#"{"backlight":{"keyboard":{"off":{"enable":true}}}}"#).unwrap();
        assert!(swatches(&doc, 5).is_empty());
    }

}
