//! The status-bar menu, modelled once.
//!
//! Every desktop wants a different file format for "put this in the bar", but
//! none of them should get its own idea of what the actions are, what they are
//! called, or which of them writes to the keyboard. So the menu is described
//! here and rendered per platform: the macOS menu-bar app, waybar, a plain
//! picker list for fuzzel/wofi/rofi, the Windows tray, or JSON for anything
//! else.
//!
//! Two rules keep it honest:
//!
//! - **Actions are addressed by a stable id.** A bar entry is
//!   `clevertuna do <id>` and the bar never has to know the CLI's flags —
//!   which matters because a bar cannot pass any.
//! - **Nothing appears that cannot be done.** An entry that always fails is
//!   worse than a missing feature: it teaches that the menu lies. Copying a
//!   scheme between transports needs two connections, and the keyboard accepts
//!   one, so it is a terminal command and not a menu row.

use crate::favourites;
use crate::gallery;
use crate::json::{self, Json};
use crate::themes;

/// The picture beside a row.
///
/// One list, two alphabets: macOS draws SF Symbols, and everything else gets a
/// character that survives a plain-text menu. Keeping them together is what
/// stops the Linux picker from quietly losing the icons the Mac has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Bookmark,
    Dice,
    Photo,
    Sliders,
    Save,
    Export,
    App,
    Power,
    Gear,
    Solid,
    Breathing,
    Wave,
    Cycle,
}

impl Icon {
    /// The SF Symbol name macOS draws.
    pub fn symbol(self) -> &'static str {
        match self {
        
            Icon::Bookmark => "bookmark",
        
            Icon::Dice => "die.face.5",
            Icon::Photo => "photo",
            Icon::Sliders => "slider.horizontal.3",
            Icon::Save => "square.and.arrow.down",
            Icon::Export => "square.and.arrow.up",
            Icon::App => "arrow.up.forward.app",
            Icon::Power => "moon.zzz",
            Icon::Gear => "gearshape",
            Icon::Solid => "circle.fill",
            Icon::Breathing => "waveform",
            Icon::Wave => "wave.3.right",
            Icon::Cycle => "arrow.triangle.2.circlepath",
        
        
        
        
        }
    }

    /// The character a text menu shows.
    pub fn glyph(self) -> &'static str {
        match self {
        
            Icon::Bookmark => "🔖",
        
            Icon::Dice => "🎲",
            Icon::Photo => "🖼",
            Icon::Sliders => "🎛",
            Icon::Save => "💾",
            Icon::Export => "📤",
            Icon::App => "↗",
            Icon::Power => "☾",
            Icon::Gear => "⚙",
            Icon::Solid => "●",
            Icon::Breathing => "∿",
            Icon::Cycle => "↻",
            Icon::Wave => "≈",
        
        
        
        
        }
    }

}

pub struct Item {
    /// What `clevertuna do <id>` takes. Empty for a submenu.
    pub id: String,
    pub label: String,
    pub detail: String,
    pub icon: Icon,
    /// Rewriting the keyboard, as opposed to reading it.
    pub writes: bool,
    /// The colours this entry stands for, so a bar can draw a look instead of
    /// spelling one. Empty for actions that are not a scheme.
    pub colors: Vec<String>,
    /// A heading above this row, inside its menu. Empty for no heading.
    pub heading: String,
    /// The key that puts this on, if its owner gave it one. Empty otherwise —
    /// nothing is bound until somebody chooses it.
    pub shortcut: String,
    /// The rows underneath this one. A non-empty list makes it a submenu, and
    /// a submenu is never itself an action.
    pub children: Vec<Item>,
}

impl Item {
    fn new(id: &str, label: &str, detail: &str, icon: Icon, writes: bool) -> Item {
        Item {
            id: id.into(),
            label: label.into(),
            detail: detail.into(),
            icon,
            writes,
            colors: Vec::new(),
            heading: String::new(),
            shortcut: String::new(),
            children: Vec::new(),
        }
    }

    fn submenu(label: &str, detail: &str, icon: Icon, children: Vec<Item>) -> Item {
        Item { children, ..Item::new("", label, detail, icon, false) }
    }

    fn under(mut self, heading: &str) -> Item {
        self.heading = heading.into();
        self
    }

    fn showing(mut self, colors: Vec<String>) -> Item {
        self.colors = colors;
        self
    }

    fn keyed(mut self, keys: &favourites::Favourites) -> Item {
        self.shortcut = favourites::shortcut_for(keys, &self.id).unwrap_or_default();
        self
    }

    pub fn is_submenu(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Everything under Themes: the ones that ship, the ones worked out on the
/// spot, and the ones you saved.
///
/// One menu with headings rather than a tree of submenus. They are categories,
/// and a category that costs a second hover before you can pick anything in it
/// is a filing cabinet, not a picker.
fn theme_items() -> Vec<Item> {
    let mut out = Vec::new();
    let keys = favourites::load();

    // First, because it is the thing you came here to do when none of the rest
    // is quite right.
    if cfg!(target_os = "macos") {
        out.push(Item::new(
            "builder",
            "Theme Builder…",
            "pick colours and movement per zone, then apply",
            Icon::Sliders,
            false,
        ));
    }

    for group in themes::Group::all() {
        let mut first = true;
        for p in themes::in_group(group) {
            let mut item = Item::new(
                &format!("theme:{}", p.id),
                p.name,
                p.blurb,
                match group {
                    themes::Group::Steady => Icon::Solid,
                    themes::Group::Breathing => Icon::Breathing,
                    themes::Group::Moving => Icon::Wave,
                },
                true,
            )
            .showing(p.swatch())
            .keyed(&keys);
            if first {
                item = item.under(group.label());
                first = false;
            }
            out.push(item);
        }
    }

    // Smart: a look worked out from something, rather than one written down.
    // Both carry colours like every other theme — one showing that it could be
    // anything, the other showing what the desktop picture actually is.
    out.push(
        Item::new("random", "Random", "roll a new theme and put it on", Icon::Dice, true)
            .under("Smart")
            .showing(spectrum_swatch())
            .keyed(&keys),
    );
    out.push(
        Item::new(
            "match-wallpaper",
            "Wallpaper",
            "take the desktop picture's colours, and follow it when it changes",
            Icon::Photo,
            true,
        )
        .showing(wallpaper_swatch())
        .keyed(&keys),
    );

    // Yours. Absent rather than empty: a heading over nothing is a dead end.
    // A saved file whose name is one of the app's own is not offered here: it
    // would sit beside the row it shadows looking like a duplicate. It is still
    // in the theme manager, to be renamed or removed.
    let saved: Vec<gallery::Entry> = gallery::list().into_iter().filter(|e| !e.shadowed).collect();
    for (i, e) in saved.iter().enumerate() {
        let zones: Vec<&str> = e.zones.iter().map(|z| crate::ui::zone_label(z)).collect();
        let mut item = Item::new(
            &format!("profile:{}", e.name),
            &e.name,
            &format!("put this back on ({})", zones.join(", ")),
            Icon::Bookmark,
            true,
        )
        .showing(e.colors.clone())
        .keyed(&keys);
        if i == 0 {
            item = item.under("My Themes");
        }
        out.push(item);
    }
    // Making a theme belongs with the themes. Renaming and deleting them does
    // not: that is the theme manager's job, and the builder opens it.
    out.push(
        Item::new(
            "save",
            "Save This Look…",
            "keep the lighting as a theme you can pick again",
            Icon::Save,
            false,
        )
        .under("Keep"),
    );
    out
}

/// The colours "it could be anything" looks like.
fn spectrum_swatch() -> Vec<String> {
    ["#FF0000", "#FFC800", "#00FF50", "#00A0FF", "#B400FF"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// What the desktop picture would put on the keys.
///
/// Cached against the picture's path and modification time, because this is
/// asked every time the menu is drawn — several times a minute — and decoding
/// a wallpaper to answer it each time would be a picture of a keyboard tool
/// eating a core. A failed read is cached too, or an unreadable picture would
/// be retried for ever at the same cost.
fn wallpaper_swatch() -> Vec<String> {
    let path = match crate::wallpaper::current_wallpaper() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let fingerprint = crate::rotate::wallpaper_fingerprint(&path);
    let cache = crate::gallery::config_dir().join("wallpaper-swatch.json");

    if let Some(v) = std::fs::read_to_string(&cache).ok().and_then(|t| json::parse(&t).ok()) {
        if matches!(v.get("for"), Some(Json::Str(f)) if *f == fingerprint) {
            return v
                .get("colors")
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| match x {
                            Json::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
    }

    let colors: Vec<String> = crate::wallpaper::load_image(&path)
        .ok()
        .map(|img| {
            crate::wallpaper::dominant_colours(&img, 5)
                .into_iter()
                .map(crate::wallpaper::vivid)
                .map(|c| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
                .collect()
        })
        .unwrap_or_default();

    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &cache,
        json::to_string_pretty(&Json::obj(vec![
            ("for", Json::Str(fingerprint)),
            ("colors", Json::Arr(colors.iter().map(|c| Json::Str(c.clone())).collect())),
        ])),
    );
    colors
}

/// Everything the bar can offer, given what is on this machine.
///
/// The connection state is *not* in this list. It belongs on the icon and in
/// its tooltip, where it is present without spending a row; a menu whose first
/// line is a status report has already wasted the reader's first glance.
pub fn items(connected: Option<&str>) -> Vec<Item> {
    // Themes are the menu, not a drawer inside it.
    //
    // Putting them behind a submenu cost a hover before anything could be
    // picked, which is the whole job. Everything that is not a theme moved the
    // other way, into Settings, so the top of the menu is only ever things you
    // can put on the keyboard.
    let mut v = theme_items();

    v.push(
        Item::submenu(
            "Settings",
            "the keyboard itself — power, touch, and what the keys do",
            Icon::Gear,
            settings_items(),
        )
        .under("Keyboard"),
    );
    let _ = connected;
    v
}

/// Everything that is not a theme.
fn settings_items() -> Vec<Item> {
    vec![
        Item::new(
            "settings",
            "Touch & Keyboard…",
            "the touchpad, the sliders, the multi-touch actions and the Fn row",
            Icon::Sliders,
            false,
        ),
        Item::new(
            "timeout",
            "Backlight & Power…",
            "when the light dims, when it goes out, and battery saving",
            Icon::Power,
            false,
        ),
        Item::new(
            "export",
            "Export a Backup…",
            "every setting to a file — gestures and key maps too, not just light",
            Icon::Export,
            false,
        )
        .under("Keep"),
        Item::new(
            "open-app",
            "Open TouchOnKeys",
            "the vendor app, for what this tool does not do",
            Icon::App,
            false,
        )
        .under("Elsewhere"),
    ]
}

/// What the icon's tooltip says.
pub fn status_line(connected: Option<&str>) -> String {
    match connected {
        Some(t) => format!("Clevertuna — connected over {}", t.to_uppercase()),
        None => "Clevertuna — no keyboard".to_string(),
    }
}

/// Which transport is live, if any — cheap enough to run on a bar tick.
pub fn detect_transport() -> Option<String> {
    let usb = crate::transport::find_usb();
    if !usb.is_empty() {
        return Some("usb".into());
    }
    // Not gated to Linux any more: macOS reaches the same keyboard through
    // CoreBluetooth, and a bar that says "no keyboard" while one is connected is
    // worse than a slow bar.
    if crate::transport::find_ble().is_some() {
        return Some("bluetooth".into());
    }
    None
}

/// Walk every action in the tree, submenus included.
///
/// The renderers each recurse in their own shape, so only the tests reach for
/// this — and `cargo check` cannot see a `#[cfg(test)]` caller, so it reports
/// it as dead on a target whose tests it is not compiling.
#[allow(dead_code)]
pub fn walk(items: &[Item], f: &mut impl FnMut(&Item)) {
    for i in items {
        f(i);
        walk(&i.children, f);
    }
}

/// waybar `custom/` module: one JSON object per tick.
pub fn render_waybar(connected: Option<&str>, profiles: usize) -> String {
    let class = match connected {
        Some(t) if t == "usb" => "connected-usb",
        Some(_) => "connected-ble",
        None => "disconnected",
    };
    let tooltip = match connected {
        Some(t) => format!(
            "Clevertuna — connected over {}\\n{} theme(s) to pick from\\nclick to choose a look",
            t.to_uppercase(),
            themes::all().len() + profiles
        ),
        None => "Clevertuna — no keyboard\\nplug in over USB, or connect a Bluetooth channel".into(),
    };
    json::to_string_pretty(&Json::obj(vec![
        ("text", Json::Str("CLVX".into())),
        ("tooltip", Json::Str(tooltip)),
        ("class", Json::Str(class.into())),
    ]))
    .replace('\n', "")
    .replace("  ", "")
}

/// SwiftBar / xbar plugin output: title, separator, then menu lines.
///
/// Kept for the people who already run one; the macOS interface Clevertuna
/// ships is its own menu-bar app, which needs no host.
pub fn render_swiftbar(connected: Option<&str>, exe: &str) -> String {
    fn lines(items: &[Item], exe: &str, depth: usize, out: &mut Vec<String>) {
        let prefix = "--".repeat(depth);
        let mut last_heading = String::new();
        for i in items {
            if !i.heading.is_empty() && i.heading != last_heading {
                if !out.is_empty() {
                    out.push(format!("{}---", prefix));
                }
                last_heading = i.heading.clone();
            }
            if i.is_submenu() {
                out.push(format!("{}{} {}", prefix, i.icon.glyph(), i.label));
                lines(&i.children, exe, depth + 1, out);
                continue;
            }
            out.push(format!(
                "{}{} {} | bash=\"{}\" param1=do param2=\"{}\" terminal=false refresh=true",
                prefix, i.icon.glyph(), i.label, exe, i.id
            ));
        }
    }
    let mut out = Vec::new();
    out.push(match connected {
        Some(t) => format!("CLVX | sfimage=keyboard.fill tooltip=\"connected over {}\"", t.to_uppercase()),
        None => "CLVX | sfimage=keyboard tooltip=\"no keyboard\"".to_string(),
    });
    out.push("---".into());
    let mut body = Vec::new();
    lines(&items(connected), exe, 0, &mut body);
    out.extend(body);
    out.push("---".into());
    out.push(format!(
        "{} Refresh | bash=\"{}\" param1=menu param2=--format param3=swiftbar terminal=false refresh=true",
        Icon::Cycle.glyph(),
        exe
    ));
    out.join("\n")
}

/// A flat list for fuzzel / wofi / rofi: "id\tglyph label — detail".
///
/// A picker has no submenus, so the tree is flattened and a parent's label is
/// carried into its children — "Themes › Reef" is still one line to type.
pub fn render_picker(connected: Option<&str>) -> String {
    fn flatten(items: &[Item], path: &str, out: &mut Vec<String>) {
        for i in items {
            let label = if path.is_empty() {
                i.label.clone()
            } else {
                format!("{} › {}", path, i.label)
            };
            if i.is_submenu() {
                flatten(&i.children, &label, out);
                continue;
            }
            out.push(format!("{}\t{} {} — {}", i.id, i.icon.glyph(), label, i.detail));
        }
    }
    let mut out = Vec::new();
    flatten(&items(connected), "", &mut out);
    out.join("\n")
}

fn item_json(i: &Item) -> Json {
    Json::obj(vec![
        ("id", Json::Str(i.id.clone())),
        ("label", Json::Str(i.label.clone())),
        ("detail", Json::Str(i.detail.clone())),
        ("icon", Json::Str(i.icon.symbol().to_string())),
        ("glyph", Json::Str(i.icon.glyph().to_string())),
        ("heading", Json::Str(i.heading.clone())),
        ("shortcut", Json::Str(i.shortcut.clone())),
        ("writes", Json::Bool(i.writes)),
        ("colors", Json::Arr(i.colors.iter().map(|c| Json::Str(c.clone())).collect())),
        ("items", Json::Arr(i.children.iter().map(item_json).collect())),
    ])
}

pub fn render_json(connected: Option<&str>) -> String {
    json::to_string_pretty(&Json::obj(vec![
        ("connected", match connected {
            Some(t) => Json::Str(t.to_string()),
            None => Json::Null,
        }),
        ("status", Json::Str(status_line(connected))),
        ("items", Json::Arr(items(connected).iter().map(item_json).collect())),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hold the same lock the gallery tests use: they share one process
    /// environment, and whoever sets CLEVERTUNA_HOME last would otherwise win.
    fn isolate() -> std::sync::MutexGuard<'static, ()> {
        let g = crate::gallery::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-menu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("CLEVERTUNA_HOME", &tmp);
        g
    }

    fn all_ids(connected: Option<&str>) -> Vec<String> {
        let mut ids = Vec::new();
        walk(&items(connected), &mut |i| {
            if !i.id.is_empty() {
                ids.push(i.id.clone())
            }
        });
        ids
    }

    #[test]
    fn the_menu_offers_the_core_actions() {
        let _g = isolate();
        let ids = all_ids(Some("usb"));
        for want in ["random", "match-wallpaper", "save", "export", "open-app", "settings", "timeout"] {
            assert!(ids.iter().any(|i| i == want), "missing {} in {:?}", want, ids);
        }
        assert!(ids.iter().any(|i| i.starts_with("theme:")), "no built-in themes");
        // The builder is a window, so it is offered only where one exists —
        // and it is the first thing inside Themes, because it is what you came
        // for when none of the rest is quite right.
        assert_eq!(
            ids.iter().any(|i| i == "builder"),
            cfg!(target_os = "macos"),
            "a row must never point at a window this platform has not got"
        );
        if cfg!(target_os = "macos") {
            assert_eq!(
                items(Some("usb")).first().map(|i| i.id.clone()),
                Some("builder".to_string()),
                "the builder opens the menu"
            );
        }
    }

    #[test]
    fn no_row_promises_something_the_bar_cannot_do() {
        let _g = isolate();
        // Copying a scheme between transports needs two connections and the
        // keyboard grants one, so it was a row that could only ever fail.
        assert!(!all_ids(Some("usb")).iter().any(|i| i == "copy"));
    }

    #[test]
    fn the_connection_state_is_not_a_menu_row() {
        let _g = isolate();
        let ids = all_ids(None);
        assert!(!ids.iter().any(|i| i == "status"));
        // It is still stated in words, on the icon.
        assert!(status_line(None).to_lowercase().contains("no keyboard"));
        assert!(status_line(Some("usb")).contains("USB"));
    }

    #[test]
    fn the_top_level_is_themes_and_one_way_into_everything_else() {
        let _g = isolate();
        let top = items(Some("usb"));
        // Deliberately long now: themes ARE the menu. What is not a theme is
        // behind exactly one door.
        let submenus: Vec<&str> = top.iter().filter(|i| i.is_submenu()).map(|i| i.label.as_str()).collect();
        assert_eq!(submenus, vec!["Settings"], "only one drawer, and it holds no themes");
        assert!(top.iter().any(|i| i.id.starts_with("theme:")), "themes are at the top level");
        assert!(top.last().map(|i| i.is_submenu()).unwrap_or(false), "and it comes last");
    }

    #[test]
    fn every_row_carries_an_icon() {
        let _g = isolate();
        let top = items(Some("usb"));
        walk(&top, &mut |i| {
            assert!(!i.icon.symbol().is_empty(), "{} has no symbol", i.label);
            assert!(!i.icon.glyph().is_empty(), "{} has no glyph", i.label);
            assert!(!i.label.is_empty());
        });
    }

    #[test]
    fn a_submenu_is_never_also_an_action() {
        let _g = isolate();
        walk(&items(Some("usb")), &mut |i| {
            assert!(
                !(i.is_submenu() && !i.id.is_empty()),
                "{} is both a submenu and an action",
                i.label
            );
        });
    }

    #[test]
    fn built_in_themes_and_saved_profiles_cannot_collide() {
        let _g = isolate();
        // A saved profile named after a built-in must still be reachable, which
        // is why the two live in different id namespaces.
        let doc = crate::json::parse(
            r#"{"backlight":{"keyboard":{"solidColor":{"color":{"red":1,"green":2,"blue":3}}}}}"#,
        )
        .unwrap();
        gallery::save("Tide", &doc).unwrap();
        let ids = all_ids(Some("usb"));
        assert!(ids.iter().any(|i| i == "theme:tide"));
        assert!(ids.iter().any(|i| i == "profile:Tide"));
        gallery::delete("Tide").unwrap();
    }

    #[test]
    fn with_nothing_saved_there_is_no_my_themes_heading() {
        let _g = isolate();
        let mut headings: Vec<String> = Vec::new();
        walk(&items(Some("usb")), &mut |i| {
            if !i.heading.is_empty() {
                headings.push(i.heading.clone())
            }
        });
        assert!(!headings.iter().any(|h| h == "My Themes"), "a heading over nothing is a dead end");
        assert!(headings.iter().any(|h| h == "Smart"), "the worked-out looks need their own heading");
    }

    #[test]
    fn no_theme_holds_a_key_until_somebody_gives_it_one() {
        let _g = isolate();
        // The complaint this answers: the keys were wired to whichever five
        // themes happened to be listed first, which nobody picked.
        let mut keyed = 0;
        walk(&items(Some("usb")), &mut |i| {
            if !i.shortcut.is_empty() {
                keyed += 1
            }
        });
        assert_eq!(keyed, 0, "{} rows claim a key nobody assigned", keyed);
    }

    #[test]
    fn every_theme_row_shows_the_colours_it_stands_for() {
        let _g = isolate();
        // A menu of swatches with blanks in it reads as broken rows.
        for row in items(Some("usb")) {
            if !(row.id.starts_with("theme:") || row.id == "random") {
                continue;
            }
            assert!(!row.colors.is_empty(), "{} has no colours", row.label);
        }
    }

    #[test]
    fn nothing_that_is_not_a_theme_sits_among_the_themes() {
        let _g = isolate();
        // Renaming and deleting is the theme manager's job — the builder opens
        // it — so it is not a row here; and the device's own settings are
        // behind Settings rather than mixed in with the looks.
        let ids = all_ids(Some("usb"));
        assert!(!ids.iter().any(|i| i == "manage"), "the manager is a window, not a row");
        let top: Vec<String> = items(Some("usb")).iter().map(|i| i.id.clone()).collect();
        for behind_the_door in ["settings", "timeout", "export", "open-app"] {
            assert!(!top.iter().any(|i| i == behind_the_door), "{} belongs in Settings", behind_the_door);
            assert!(ids.iter().any(|i| i == behind_the_door), "{} must still be reachable", behind_the_door);
        }
    }

    #[test]
    fn nothing_animates_from_the_bar_any_more() {
        let _g = isolate();
        // The builder owns changing how a look moves; two places to do it is
        // one place too many.
        let ids = all_ids(Some("usb"));
        for gone in ["effect:colorWave", "speed:faster", "bright:up"] {
            assert!(!ids.iter().any(|i| i == gone), "{} should be gone", gone);
        }
    }

    #[test]
    fn theme_rows_carry_the_colours_they_stand_for() {
        let _g = isolate();
        let mut seen = 0;
        walk(&items(Some("usb")), &mut |i| {
            if i.id.starts_with("theme:") {
                assert!(!i.colors.is_empty(), "{} has no swatch", i.label);
                assert!(i.colors[0].starts_with('#'), "{:?} is not a colour", i.colors[0]);
                seen += 1;
            }
        });
        assert_eq!(
            seen,
            themes::Group::all().len() * themes::PER_GROUP,
            "the Themes menu should hold every shipped preset and nothing else"
        );
    }

    #[test]
    fn waybar_output_is_one_line_of_json() {
        let _g = isolate();
        let s = render_waybar(Some("usb"), 3);
        assert!(!s.contains('\n'), "waybar needs one line per tick");
        assert!(s.contains("\"text\""));
        assert!(s.contains("connected-usb"));
    }

    #[test]
    fn swiftbar_output_nests_submenus_with_dashes() {
        let _g = isolate();
        let s = render_swiftbar(Some("bluetooth"), "/usr/local/bin/clevertuna");
        let mut lines = s.lines();
        assert!(lines.next().unwrap().starts_with("CLVX"));
        assert!(s.contains("param1=do"));
        assert!(s.lines().any(|l| l.starts_with("--")), "no nested rows");
        assert!(s.contains("/usr/local/bin/clevertuna"));
    }

    #[test]
    fn picker_lines_start_with_the_action_id_and_name_their_menu() {
        let _g = isolate();
        let s = render_picker(Some("usb"));
        for line in s.lines() {
            let mut parts = line.splitn(2, '\t');
            let id = parts.next().unwrap();
            assert!(!id.is_empty());
            assert!(!id.contains(' '), "id {:?} must be a single token", id);
            assert!(parts.next().is_some(), "every line needs a label");
        }
        // Themes are top level now, so only what came out of Settings needs
        // to say where it came from.
        assert!(s.contains("Settings › "), "a flattened row must say where it came from");
        assert!(s.contains("theme:deep-current\t"), "a theme is a row in its own right");
    }

    #[test]
    fn json_render_marks_which_actions_write() {
        let _g = isolate();
        let v = json::parse(&render_json(Some("usb"))).unwrap();
        let top = v.get("items").unwrap().as_array().unwrap();
        let find = |id: &str| -> Json {
            fn dig(items: &[Json], id: &str) -> Option<Json> {
                for i in items {
                    if matches!(i.get("id"), Some(json::Json::Str(s)) if s == id) {
                        return Some(i.clone());
                    }
                    if let Some(kids) = i.get("items").and_then(|k| k.as_array()) {
                        if let Some(found) = dig(kids, id) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            dig(top, id).unwrap_or_else(|| panic!("no {} in the model", id))
        };
        assert_eq!(find("match-wallpaper").get("writes").unwrap().as_bool(), Some(true));
        assert_eq!(find("random").get("writes").unwrap().as_bool(), Some(true));
        // The builder opens a window; it writes when you tell it to, not when
        // you open it.
        if cfg!(target_os = "macos") {
            assert_eq!(find("builder").get("writes").unwrap().as_bool(), Some(false));
        }
        assert_eq!(find("export").get("writes").unwrap().as_bool(), Some(false));
        assert!(
            matches!(find("theme:hartle").get("icon"), Some(json::Json::Str(s)) if !s.is_empty()),
            "a theme row must name the symbol its menu should draw"
        );
    }
}
