//! The terminal interface.
//!
//! Same domain layer as the CLI — this draws, it does not talk to hardware
//! itself. Two layouts: 120×32 keeps a device panel beside the editor, 80×24
//! collapses it into a top selector. Every action is on screen; nothing
//! destructive is one keystroke away.
//!
//! Raw mode is arranged with `stty`, so there is still no dependency.

use crate::json::Json;
use crate::service::{self, Stage};
use crate::transport::{Device, Kind};
use crate::ui::{effect_label, hex, zone_label, Style};
use std::io::{Read, Write};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const WIDE_COLS: usize = 100; // at or above this we show the side panel

struct Chrome {
    tl: &'static str, tr: &'static str, bl: &'static str, br: &'static str,
    h: &'static str, v: &'static str, ml: &'static str, mr: &'static str,
    dot: &'static str, arrow: &'static str, filled: &'static str, empty: &'static str,
}

fn chrome(ascii: bool) -> Chrome {
    if ascii {
        Chrome { tl: "+", tr: "+", bl: "+", br: "+", h: "-", v: "|",
                 ml: "+", mr: "+", dot: "*", arrow: ">", filled: "#", empty: "." }
    } else {
        Chrome { tl: "┌", tr: "┐", bl: "└", br: "┘", h: "─", v: "│",
                 ml: "├", mr: "┤", dot: "●", arrow: "›", filled: "■", empty: "□" }
    }
}

pub struct Tui {
    st: Style,
    cols: usize,
    #[allow(dead_code)] // Height is tracked with the width even though layout keys off columns.
    rows: usize,
    zones: Vec<String>,
    zone_idx: usize,
    scheme: Option<Json>,
    status: String,
    status_stage: Stage,
    pending: Option<&'static str>, // an action awaiting confirmation
    log: Vec<String>,
}

fn term_size() -> (usize, usize) {
    if let Ok(out) = Command::new("stty").arg("size").stdin(std::process::Stdio::inherit()).output() {
        let s = String::from_utf8_lossy(&out.stdout);
        let mut it = s.split_whitespace();
        if let (Some(r), Some(c)) = (it.next(), it.next()) {
            if let (Ok(r), Ok(c)) = (r.parse(), c.parse()) {
                return (c, r);
            }
        }
    }
    (80, 24)
}

fn raw_mode(on: bool) {
    let arg = if on { "raw" } else { "sane" };
    let _ = Command::new("stty")
        .arg(arg)
        .arg(if on { "-echo" } else { "echo" })
        .stdin(std::process::Stdio::inherit())
        .status();
}

fn clock() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = secs % 86400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

impl Tui {
    pub fn new(color: bool, ascii: bool) -> Tui {
        let (cols, rows) = term_size();
        Tui {
            st: Style { color, ascii },
            cols: cols.max(60),
            rows: rows.max(16),
            zones: Vec::new(),
            zone_idx: 0,
            scheme: None,
            status: "starting".into(),
            status_stage: Stage::Validated,
            pending: None,
            log: Vec::new(),
        }
    }

    fn note(&mut self, stage: Stage, msg: &str) {
        self.status_stage = stage;
        self.status = msg.to_string();
        self.log.push(format!("{}  {}", clock(), msg));
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }

    fn refresh(&mut self, dev: &mut Device) {
        self.note(Stage::Validated, "reading the keyboard…");
        self.draw(dev.kind, true);
        match service::get_backlight_json(dev) {
            Ok(doc) => {
                self.zones = crate::ui::zones_in(&doc);
                if self.zone_idx >= self.zones.len() {
                    self.zone_idx = 0;
                }
                self.scheme = Some(doc);
                self.note(Stage::ReadBack, "read back from device");
            }
            Err(e) => self.note(Stage::Failed, &format!("{}", e)),
        }
    }

    fn zone_view(&self) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        let doc = match &self.scheme {
            Some(d) => d,
            None => return rows,
        };
        let backlight = doc.get("backlight").unwrap_or(doc);
        let key = match self.zones.get(self.zone_idx) {
            Some(k) => k,
            None => return rows,
        };
        let zone = match backlight.get(key) {
            Some(z) => z,
            None => return rows,
        };
        let mut effect_name = "—";
        for (name, _) in [("solidColor", 0), ("breathing", 0), ("colorCycle", 0), ("colorWave", 0), ("aurora", 0)] {
            if zone.get(name).is_some() {
                effect_name = name;
            }
        }
        rows.push(("Effect".into(), effect_label(effect_name).to_string()));
        if let Some(eff) = zone.get(effect_name) {
            if let Some(picker) = eff.get("colorLinePicker") {
                if let Some(arr) = picker.get("markersArray").and_then(|m| m.as_array()) {
                    let ch = chrome(self.st.ascii);
                    let stops: Vec<String> = arr
                        .iter()
                        .map(|m| {
                            let c = m.get("color").cloned().unwrap_or(Json::Null);
                            let mark = if m.get("transparency").and_then(|t| t.as_u32()).unwrap_or(0) > 0 {
                                ch.empty
                            } else {
                                ch.filled
                            };
                            format!("{} {}", mark, hex(&c))
                        })
                        .collect();
                    rows.push(("Stops".into(), stops.join("  ")));
                }
            }
            if let Some(c) = eff.get("color") {
                rows.push(("Colour".into(), hex(c)));
            }
            let num = |k: &str| eff.get(k).and_then(|v| v.as_u32());
            if let (Some(d), Some(p)) = (num("direction"), num("period")) {
                rows.push(("Direction".into(), format!("{}°     Period  {} ms", d, p)));
            } else if let Some(p) = num("period") {
                rows.push(("Period".into(), format!("{} ms", p)));
            }
            if let Some(l) = num("length") {
                let ia = zone
                    .get("interactiveAnimation")
                    .and_then(|i| i.get("enable"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                rows.push((
                    "Length".into(),
                    format!("{}      Interactive  {}", l, if ia { "on" } else { "off" }),
                ));
            }
        }
        if let Some(t) = zone.get("transparency").and_then(|v| v.as_u32()) {
            rows.push(("Transparency".into(), format!("{}", t)));
        }
        rows
    }

}

/// Render a frame from a scheme file, with no device and no raw mode.
///
/// The frame is the interface, so being able to print one is what lets it be
/// reviewed, diffed and handed to a designer without the hardware present.
pub fn preview_frame(cols: usize, ascii: bool, scheme_file: Option<&str>) -> String {
    let mut t = Tui::new(false, ascii);
    t.cols = cols;
    // With no scheme to show there is no device either, so the frame must be
    // the empty state rather than a connected one with nothing in it.
    let mut kind = None;
    if let Some(f) = scheme_file {
        if let Ok(text) = std::fs::read_to_string(f) {
            if let Ok(doc) = crate::json::parse(&text) {
                t.zones = crate::ui::zones_in(&doc);
                t.scheme = Some(doc);
                t.note(Stage::Verified, "device matches the scheme");
                kind = Some(Kind::Usb);
            }
        }
    }
    t.render(kind, false)
}

impl Tui {
    /// Handle the keys that only move through what is already loaded.
    ///
    /// Split out from the loop because these need no device, which is what
    /// lets the tests press them for real instead of setting fields by hand.
    fn on_key(&mut self, key: char) {
        match key {
            'h' | 'k' | '[' => {
                if !self.zones.is_empty() {
                    self.zone_idx = (self.zone_idx + self.zones.len() - 1) % self.zones.len();
                }
            }
            'l' | 'j' | ']' => {
                if !self.zones.is_empty() {
                    self.zone_idx = (self.zone_idx + 1) % self.zones.len();
                }
            }
            's' => {
                if self.scheme.is_some() {
                    self.pending = Some("send this scheme to the keyboard");
                } else {
                    self.note(Stage::Failed, "no scheme loaded");
                }
            }
            'b' => self.pending = Some("back up every setting"),
            _ => {}
        }
    }

    /// Paint the frame.
    fn draw(&self, kind: Kind, busy: bool) {
        print!("\x1b[2J\x1b[H{}", self.render(Some(kind), busy));
        let _ = std::io::stdout().flush();
    }

    /// Build the frame as text.
    ///
    /// Layout is built into a string rather than written straight to stdout so
    /// the tests can assert on the frame the user actually sees. A helper that
    /// re-creates the layout for tests would only ever prove itself right.
    fn render(&self, kind: Option<Kind>, _busy: bool) -> String {
        let mut out = String::new();
        macro_rules! w {
            ($($a:tt)*) => {{ out.push_str(&format!($($a)*)); out.push('\n'); }};
        }
        let ch = chrome(self.st.ascii);
        let st = &self.st;
        let wide = self.cols >= WIDE_COLS;

        // ── header ───────────────────────────────────────────────────────
        let conn = match kind {
            Some(k) => format!("{}  {} connected", k.label().to_uppercase(), ch.dot),
            None => format!("NO KEYBOARD  {} not connected", ch.empty),
        };
        let title = format!(
            " {}  {}",
            st.bold(&st.current("CLEVERTUNA")),
            st.dim("Read the current.")
        );
        let visible = 1 + "CLEVERTUNA".len() + 2 + "Read the current.".len();
        let gap = self.cols.saturating_sub(visible + conn.chars().count());
        w!("{}{}{}", title, " ".repeat(gap), st.mint(&conn));

        // Every row is exactly `cols` wide. Wide: left border + left_w + border
        // + gap + border + panel_w + border. Narrow: border + panel_w + border.
        let left_w = 18usize;
        let panel_w = if wide {
            self.cols.saturating_sub(left_w + 5).max(20)
        } else {
            self.cols.saturating_sub(2).max(20)
        };

        // ── device selector ──────────────────────────────────────────────
        if !wide {
            let name = match kind { Some(_) => "CLVX S", None => "no keyboard" };
            w!(
                " {} {}   {}",
                st.dim("Device"),
                st.bold(name),
                st.dim("[r] refresh")
            );
        }

        // ── panels ───────────────────────────────────────────────────────
        let zone_title = self
            .zones
            .get(self.zone_idx)
            .map(|z| zone_label(z))
            .unwrap_or("—");
        let head = format!(" Lighting / {} ", zone_title);
        let right_top = format!(
            "{}{}{}{}",
            ch.tl,
            head,
            ch.h.repeat(panel_w.saturating_sub(head.chars().count())),
            ch.tr
        );

        let rows = self.zone_view();
        let mut body: Vec<String> = Vec::new();
        for (k, v) in &rows {
            body.push(format!(" {:<12} {}", k, v));
        }
        if body.is_empty() {
            body.push(" no lighting read yet — press [r]".into());
        }
        // tabs across zones
        let tabs: String = self
            .zones
            .iter()
            .enumerate()
            .map(|(i, z)| {
                let label = zone_label(z);
                if i == self.zone_idx {
                    format!("[{}]", st.current(label))
                } else {
                    format!(" {} ", st.dim(label))
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if wide {
            let lt = format!("{}{}{}{}", ch.tl, " Devices ", ch.h.repeat(left_w.saturating_sub(9)), ch.tr);
            w!("{} {}", lt, right_top);
            let left_rows = match kind {
                Some(k) => vec![
                    format!("{} CLVX S", ch.arrow),
                    format!("  on {}", k.label()),
                    String::new(),
                    "[r] refresh".to_string(),
                ],
                None => vec![
                    "  no keyboard".to_string(),
                    "  plug in, or pair".to_string(),
                    String::new(),
                    "[r] look again".to_string(),
                ],
            };
            let n = body.len().max(left_rows.len());
            for i in 0..n {
                let l = left_rows.get(i).cloned().unwrap_or_default();
                let r = body.get(i).cloned().unwrap_or_default();
                w!(
                    "{}{:<w$}{} {}{:<pw$}{}",
                    ch.v, l, ch.v, ch.v, r, ch.v,
                    w = left_w, pw = panel_w
                );
            }
            w!(
                "{}{}{} {}{}{}",
                ch.bl, ch.h.repeat(left_w), ch.br,
                ch.ml, ch.h.repeat(panel_w), ch.mr
            );
            w!("{:<w$} {}{:<pw$}{}", "", ch.v, format!(" {}", tabs), ch.v, w = left_w + 2, pw = panel_w);
            w!("{:<w$} {}{}{}", "", ch.bl, ch.h.repeat(panel_w), ch.br, w = left_w + 2);
        } else {
            w!("{}", right_top);
            for r in &body {
                w!("{}{:<pw$}{}", ch.v, r, ch.v, pw = panel_w);
            }
            w!("{}{}{}", ch.ml, ch.h.repeat(panel_w), ch.mr);
            w!("{}{:<pw$}{}", ch.v, format!(" {}", tabs), ch.v, pw = panel_w);
            w!("{}{}{}", ch.bl, ch.h.repeat(panel_w), ch.br);
        }

        // ── status + keys ────────────────────────────────────────────────
        let stage = self.status_stage;
        let word = stage.label().to_uppercase().replace('_', " ");
        let painted = match stage {
            Stage::Verified => st.mint(&word),
            Stage::Mismatch | Stage::Failed => st.trench(&word),
            _ => st.current(&word),
        };
        w!("");
        w!(" Status: {} · {} · {}", painted, self.status, st.dim(&clock()));
        for l in &self.log {
            w!("   {}", st.dim(l));
        }
        w!("");
        if let Some(action) = self.pending {
            w!(
                " {}  {} — press {} to confirm, any other key to cancel",
                st.amber("CONFIRM"),
                action,
                st.bold("y")
            );
        } else {
            w!(
                " {}",
                st.dim("[r] refresh  [p] preview  [s] send  [b] backup  [ ] zone  [?] help  [q] quit")
            );
        }
        out
    }

    fn help(&self) {
        let st = &self.st;
        print!("\x1b[2J\x1b[H");
        println!(" {}\n", st.bold(&st.current("CLEVERTUNA — keys")));
        for (k, d) in [
            ("↑ ↓ / k j", "move between zones"),
            ("← → / h l", "previous / next zone"),
            ("[  ]", "previous / next zone"),
            ("r", "re-read the keyboard"),
            ("p", "preview the loaded scheme without writing"),
            ("s", "send the loaded scheme, then verify it"),
            ("b", "back up every setting to a file"),
            ("?", "this help"),
            ("q", "quit"),
        ] {
            println!("   {:<12} {}", st.bold(k), d);
        }
        println!("\n {}", st.dim("Sending asks for confirmation. Restore is not bound to a key here —"));
        println!(" {}", st.dim("use `clevertuna import <file>` so it cannot happen by accident."));
        println!("\n {}", st.dim("press any key to go back"));
        let _ = std::io::stdout().flush();
    }

    pub fn run(&mut self, dev: &mut Device, scheme_file: Option<String>) -> i32 {
        if let Some(f) = &scheme_file {
            match std::fs::read_to_string(f).ok().and_then(|t| crate::json::parse(&t).ok()) {
                Some(doc) => {
                    self.scheme = Some(doc);
                    self.note(Stage::Validated, &format!("loaded {}", f));
                }
                None => self.note(Stage::Failed, &format!("could not read {}", f)),
            }
        }
        raw_mode(true);
        self.refresh(dev);
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 3];
        loop {
            self.draw(dev.kind, false);
            let n = match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let key = if n >= 3 && buf[0] == 0x1b && buf[1] == b'[' {
                match buf[2] {
                    b'A' => 'k',
                    b'B' => 'j',
                    b'C' => 'l',
                    b'D' => 'h',
                    _ => '\0',
                }
            } else {
                buf[0] as char
            };

            if let Some(action) = self.pending.take() {
                if key == 'y' || key == 'Y' {
                    match action {
                        "send this scheme to the keyboard" => self.do_send(dev),
                        "back up every setting" => self.do_backup(dev),
                        _ => {}
                    }
                } else {
                    self.note(Stage::Validated, "cancelled");
                }
                continue;
            }

            match key {
                'q' | '\x03' => break,
                'r' => self.refresh(dev),
                'p' => self.do_preview(),
                '?' => {
                    self.help();
                    let _ = stdin.read(&mut buf);
                }
                other => self.on_key(other),
            }
        }
        raw_mode(false);
        print!("\x1b[2J\x1b[H");
        let _ = std::io::stdout().flush();
        0
    }

    fn do_preview(&mut self) {
        match &self.scheme {
            Some(doc) => {
                let zones = crate::ui::zones_in(doc);
                let names: Vec<&str> = zones.iter().map(|z| zone_label(z)).collect();
                let msg = format!("would write {}", names.join(", "));
                self.note(Stage::Validated, &msg);
            }
            None => self.note(Stage::Failed, "no scheme loaded"),
        }
    }

    fn do_send(&mut self, dev: &mut Device) {
        let doc = match self.scheme.clone() {
            Some(d) => d,
            None => {
                self.note(Stage::Failed, "no scheme loaded");
                return;
            }
        };
        self.note(Stage::Sent, "sending…");
        self.draw(dev.kind, true);
        match service::set_backlight_verified(dev, &doc) {
            Ok(out) => match out.stage {
                Stage::Verified => self.note(Stage::Verified, "device matches the scheme"),
                Stage::Mismatch => self.note(Stage::Mismatch, &out.message),
                _ => self.note(out.stage, &out.message),
            },
            Err(e) => self.note(Stage::Failed, &format!("{}", e)),
        }
    }

    fn do_backup(&mut self, dev: &mut Device) {
        self.note(Stage::Sent, "reading every setting…");
        self.draw(dev.kind, true);
        match service::get_settings(dev) {
            Ok(blob) => {
                let name = format!("clevertuna-backup-{}.clvx", clock().replace(':', ""));
                match std::fs::write(&name, &blob) {
                    Ok(_) => self.note(
                        Stage::ReadBack,
                        &format!("backed up {} bytes to {}", blob.len(), name),
                    ),
                    Err(e) => self.note(Stage::Failed, &format!("cannot write {}: {}", name, e)),
                }
            }
            Err(e) => self.note(Stage::Failed, &format!("{}", e)),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn scheme() -> Json {
        json::parse(
            r#"{"backlight":{"keyboard":{"colorWave":{"colorLinePicker":{"markersNumber":2,
                 "markersArray":[{"color":{"red":255,"green":83,"blue":83},"position":5},
                                 {"color":{"red":0,"green":200,"blue":255},"position":29}]},
                 "period":3000,"direction":270,"length":1000},
                 "interactiveAnimation":{"enable":true},"transparency":0},
                 "touchpad":{"solidColor":{"color":{"red":1,"green":2,"blue":3}}}}}"#,
        )
        .unwrap()
    }

    /// Build a TUI in a known state and render it exactly as the terminal would.
    fn frame(cols: usize, ascii: bool, scheme: Option<Json>) -> String {
        let mut t = Tui::new(false, ascii);
        t.cols = cols;
        if let Some(doc) = scheme {
            t.zones = crate::ui::zones_in(&doc);
            t.scheme = Some(doc);
        }
        t.note(Stage::Verified, "device matches the scheme");
        t.render(Some(Kind::Usb), false)
    }

    #[test]
    fn frame_shows_effect_stops_and_keys() {
        let f = frame(120, true, Some(scheme()));
        assert!(f.contains("CLEVERTUNA"), "{}", f);
        assert!(f.contains("Colour wave"), "{}", f);
        assert!(f.contains("#FF5353"), "{}", f);
        assert!(f.contains("3000 ms"), "{}", f);
        for k in ["[r]", "[p]", "[s]", "[b]", "[?]", "[q]"] {
            assert!(f.contains(k), "missing {} in {}", k, f);
        }
    }

    #[test]
    fn ascii_fallback_has_no_box_drawing() {
        let f = frame(80, true, Some(scheme()));
        for bad in ['┌', '│', '└', '■', '●', '›'] {
            assert!(!f.contains(bad), "ascii frame contains {:?}", bad);
        }
    }

    #[test]
    fn zones_are_labelled_for_humans() {
        let mut t = Tui::new(false, true);
        let doc = scheme();
        t.zones = crate::ui::zones_in(&doc);
        t.scheme = Some(doc);
        assert_eq!(t.zones, vec!["keyboard", "touchpad"]);
        t.zone_idx = 1;
        let rows = t.zone_view();
        assert!(rows.iter().any(|(k, _)| k == "Effect"));
    }

    #[test]
    fn send_arms_a_confirmation_and_writes_nothing_by_itself() {
        let mut t = Tui::new(false, true);
        t.cols = 100;
        t.scheme = Some(scheme());
        t.on_key('s');
        assert!(t.pending.is_some(), "s must arm a confirmation");
        let f = t.render(Some(Kind::Usb), false);
        assert!(f.contains("CONFIRM"), "the armed state must be visible: {}", f);
        assert!(f.contains("press y to confirm"), "{}", f);
    }

    #[test]
    fn send_without_a_scheme_arms_nothing() {
        let mut t = Tui::new(false, true);
        t.on_key('s');
        assert!(t.pending.is_none(), "nothing to send, so nothing to confirm");
        assert_eq!(t.status_stage, Stage::Failed);
    }

    #[test]
    fn zone_keys_wrap_in_both_directions() {
        let mut t = Tui::new(false, true);
        let doc = scheme();
        t.zones = crate::ui::zones_in(&doc);
        assert_eq!(t.zone_idx, 0);
        t.on_key('[');
        assert_eq!(t.zone_idx, t.zones.len() - 1, "back from the first wraps to the last");
        t.on_key(']');
        assert_eq!(t.zone_idx, 0, "forward from the last wraps to the first");
    }

    #[test]
    fn narrow_and_wide_layouts_both_render() {
        for cols in [80usize, 120] {
            let f = frame(cols, true, Some(scheme()));
            assert!(f.contains("Lighting /"), "cols {}", cols);
        }
    }
    #[test]
    fn every_row_is_exactly_the_terminal_width() {
        // Counted in characters: a box-drawing glyph is three bytes and one
        // column, so measuring bytes would pass a frame that looks ragged.
        for cols in [80usize, 100, 120] {
            for ascii in [true, false] {
                for doc in [Some(scheme()), None] {
                    let mut t = Tui::new(false, ascii);
                    t.cols = cols;
                    let kind = doc.as_ref().map(|_| Kind::Usb);
                    if let Some(d) = doc {
                        t.zones = crate::ui::zones_in(&d);
                        t.scheme = Some(d);
                    }
                    let ch = chrome(ascii);
                    let frame = t.render(kind, false);
                    let mut checked = 0;
                    for row in frame.lines() {
                        // Only the framed rows must fill the width; the status
                        // and key lines are deliberately short.
                        if !row.contains(ch.v) && !row.contains(ch.tl) && !row.contains(ch.bl) {
                            continue;
                        }
                        checked += 1;
                        assert_eq!(
                            row.chars().count(), cols,
                            "framed row in a {}-column frame: {:?}", cols, row
                        );
                    }
                    assert!(checked >= 4, "expected framed rows to check, got {}", checked);
                }
            }
        }
    }

    #[test]
    fn the_empty_state_never_claims_a_connection() {
        let t = Tui::new(false, true);
        let f = t.render(None, false);
        assert!(f.contains("NO KEYBOARD"), "{}", f);
        assert!(f.contains("not connected"), "{}", f);
        assert!(!f.contains("USB  "), "no transport may be named: {}", f);
        assert!(!f.contains("BLUETOOTH"), "{}", f);
        // The device list must not invent a keyboard either.
        assert!(!f.contains("CLVX S"), "the device list must stay empty: {}", f);
    }
}
