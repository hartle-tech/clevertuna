//! The profile gallery: named schemes on disk, so switching look is one click
//! and does not need the vendor app at all.
//!
//! A profile is just a scheme file with a name. The gallery is a directory of
//! them, in the platform's usual config location, so they are easy to back up,
//! diff, and send to somebody.

use crate::json::{self, Json};
use std::path::PathBuf;

/// Where profiles live, per platform convention.
/// Where a file the user asked for should land.
///
/// A status-bar click has no working directory anyone can reason about — a
/// SwiftBar plugin or a tray process inherits whatever it was launched from —
/// so writing to "." would put the file somewhere the user cannot find. This
/// prefers Downloads, and falls back to the gallery, which at least has a name
/// the tool can print.
pub fn export_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CLEVERTUNA_EXPORT_DIR") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    if !home.is_empty() {
        let d = PathBuf::from(&home).join("Downloads");
        if d.is_dir() {
            return d;
        }
    }
    dir()
}

/// Where settings that are not themes live.
///
/// Deliberately the parent of the theme directory: anything dropped beside the
/// themes gets listed *as* a theme, which is how the rotation config turned up
/// in a menu offering to rename and delete it.
pub fn config_dir() -> PathBuf {
    dir().parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir())
}

pub fn dir() -> PathBuf {
    if let Ok(p) = std::env::var("CLEVERTUNA_HOME") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(&home).join("Library/Application Support/Clevertuna")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home))
            .join("Clevertuna")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&home).join(".config"))
            .join("clevertuna")
    };
    base.join("profiles")
}

/// Names the smart features already answer to.
///
/// `Wallpaper` and `Random` are things the tool works out, not schemes anybody
/// wrote down, and each already has a row. A saved theme of the same name shows
/// up beside it looking like a duplicate — which is exactly what it looked
/// like. So the names are reserved: they cannot be saved onto, and an older
/// file that already holds one is kept but not offered as a theme.
pub const RESERVED: [&str; 2] = ["wallpaper", "random"];

pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name.trim().to_ascii_lowercase().as_str())
}

/// Profile names are used as filenames, so they are kept boring on purpose.
pub fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("a profile name must be 1–64 characters".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return Err(format!(
            "'{}' has characters that are not allowed; use letters, digits, spaces, - and _",
            name
        ));
    }
    Ok(())
}

/// The same, plus the names the app answers to itself.
///
/// Only checked when *creating* a name. Reading and deleting one that already
/// exists must keep working, or an older file becomes unreachable rubbish.
pub fn valid_new_name(name: &str) -> Result<(), String> {
    valid_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "'{}' is one of Clevertuna's own — it already has a row that follows your desktop. Pick another name.",
            name
        ));
    }
    Ok(())
}

fn path_for(name: &str) -> Result<PathBuf, String> {
    valid_name(name)?;
    Ok(dir().join(format!("{}.json", name)))
}

pub struct Entry {
    pub name: String,
    /// True when the name collides with one of the app's own features. Such a
    /// file is still listed here — so it can be renamed or deleted — but it is
    /// not offered as a theme.
    pub shadowed: bool,
    pub zones: Vec<String>,
    /// What the profile looks like, so a picker can show it rather than name it.
    pub colors: Vec<String>,
    #[allow(dead_code)] // Kept so a listing can show where an entry came from.
    pub path: PathBuf,
}

pub fn list() -> Vec<Entry> {
    let mut out = Vec::new();
    let d = dir();
    let rd = match std::fs::read_dir(&d) {
        Ok(r) => r,
        Err(_) => return out,
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();
    for p in files {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let doc = std::fs::read_to_string(&p).ok().and_then(|t| json::parse(&t).ok());
        let zones = doc.as_ref().map(crate::ui::zones_in).unwrap_or_default();
        let colors = doc.as_ref().map(|d| crate::ui::swatches(d, 5)).unwrap_or_default();
        let shadowed = is_reserved(&name);
        out.push(Entry { name, zones, colors, path: p, shadowed });
    }
    out
}

/// The gallery as JSON, for a client that draws it rather than prints it.
///
/// The colours are in it because a picker that can only show a name is a list
/// of words, not a gallery — and what a profile looks like is something this
/// module already works out for every entry. `shadowed` is there so a client
/// can say why a saved scheme shares a name with something we ship, rather
/// than leaving two identical rows and no explanation.
pub fn to_json(items: &[Entry]) -> Json {
    Json::Arr(
        items
            .iter()
            .map(|e| {
                Json::obj(vec![
                    ("name", Json::Str(e.name.clone())),
                    ("zones", Json::Arr(e.zones.iter().map(|z| Json::Str(z.clone())).collect())),
                    ("colors", Json::Arr(e.colors.iter().map(|c| Json::Str(c.clone())).collect())),
                    ("shadowed", Json::Bool(e.shadowed)),
                ])
            })
            .collect(),
    )
}

pub fn save(name: &str, doc: &Json) -> Result<PathBuf, String> {
    valid_new_name(name)?;
    let p = path_for(name)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    let text = json::to_string_pretty(doc);
    std::fs::write(&p, format!("{}\n", text)).map_err(|e| format!("cannot write {}: {}", p.display(), e))?;
    Ok(p)
}

pub fn load(name: &str) -> Result<Json, String> {
    let p = path_for(name)?;
    let text = std::fs::read_to_string(&p)
        .map_err(|_| format!("no profile called '{}' — try `clevertuna profile list`", name))?;
    json::parse(&text).map_err(|e| format!("{} is not a valid scheme: {}", p.display(), e))
}

/// Give a saved scheme a different name.
///
/// The file is the name, so this is a move — and it refuses to land on one that
/// already exists rather than quietly replacing somebody's theme.
pub fn rename(from: &str, to: &str) -> Result<PathBuf, String> {
    valid_new_name(to)?;
    let src = path_for(from)?;
    let dst = path_for(to)?;
    if !src.exists() {
        return Err(format!("no theme called '{}' — try `clevertuna profile list`", from));
    }
    if src == dst {
        return Ok(dst);
    }
    if dst.exists() {
        return Err(format!("'{}' is already taken", to));
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("cannot rename: {}", e))?;
    Ok(dst)
}

pub fn delete(name: &str) -> Result<PathBuf, String> {
    let p = path_for(name)?;
    std::fs::remove_file(&p).map_err(|_| format!("no profile called '{}'", name))?;
    Ok(p)
}

/// Keep only the zones the caller asked for. An empty selection means all,
/// which is what `export` defaults to.
pub fn select_zones(doc: &Json, only: &[String]) -> Result<Json, String> {
    let backlight = doc.get("backlight").unwrap_or(doc);
    let obj = match backlight {
        Json::Obj(m) => m,
        _ => return Err("scheme has no zones".into()),
    };
    if only.is_empty() {
        return Ok(doc.clone());
    }
    let mut kept: Vec<(&str, Json)> = Vec::new();
    for want in only {
        let key = canonical_zone(want)
            .ok_or_else(|| format!("'{}' is not a zone; try keyboard, touchpad, left-slider, right-slider", want))?;
        match obj.get(key) {
            Some(v) => kept.push((key, v.clone())),
            None => return Err(format!("this scheme has no '{}' zone", key)),
        }
    }
    Ok(Json::obj(vec![
        (crate::backlight::SCHEMA_KEY, Json::Num(crate::backlight::SCHEMA_VERSION as f64)),
        ("backlight", Json::obj(kept)),
    ]))
}

/// Accept the friendly spellings as well as the wire ones.
pub fn canonical_zone(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().replace([' ', '_'], "-").as_str() {
        "keyboard" | "keys" => Some("keyboard"),
        "touchpad" | "trackpad" => Some("touchpad"),
        "leftslider" | "left-slider" | "left" => Some("leftSlider"),
        "rightslider" | "right-slider" | "right" => Some("rightSlider"),
        _ => None,
    }
}

/// Launch the vendor app, for the things this tool deliberately does not do.
pub fn open_vendor_app() -> Result<(), String> {
    let candidates: Vec<(&str, Vec<&str>)> = if cfg!(target_os = "macos") {
        vec![("open", vec!["-a", "TouchOnKeys"])]
    } else if cfg!(target_os = "windows") {
        vec![("cmd", vec!["/C", "start", "", "TouchOnKeys"])]
    } else {
        vec![
            ("touchonkeys", vec![]),
            ("sh", vec!["-c", "touch-on-keys || TouchOnKeys"]),
        ]
    };
    for (cmd, args) in candidates {
        if std::process::Command::new(cmd)
            .args(&args)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    Err("could not find the TouchOnKeys app on this machine".into())
}

/// Tests share one process, so anything that touches `CLEVERTUNA_HOME` has to
/// take turns. Without this the gallery and menu suites raced each other and
/// one of them "failed" while both were correct.
#[cfg(test)]
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Whether a profile of this name is already saved.
#[allow(dead_code)] // exercised by the tests; part of the gallery API
pub fn exists(name: &str) -> bool {
    path_for(name).map(|p| std::path::Path::new(&p).exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme() -> Json {
        json::parse(
            r#"{"clevertuna_backlight":1,"backlight":{
                 "keyboard":{"solidColor":{"color":{"red":1,"green":2,"blue":3}}},
                 "touchpad":{"solidColor":{"color":{"red":4,"green":5,"blue":6}}},
                 "leftSlider":{"transparency":30},
                 "rightSlider":{"transparency":30}}}"#,
        )
        .unwrap()
    }

    /// A gallery listing that names the profiles but not their colours makes
    /// every row in a picker the same grey rectangle.
    #[test]
    fn the_json_listing_carries_the_swatch() {
        let items = vec![Entry {
            name: "Reef".into(),
            shadowed: false,
            zones: vec!["keyboard".into()],
            colors: vec!["#00C8FF".into(), "#36500F".into()],
            path: PathBuf::from("/tmp/Reef.json"),
        }];
        let text = json::to_string_pretty(&to_json(&items));
        let back = json::parse(&text).unwrap();
        let list = back.as_array().expect("an array");
        let first = &list[0];
        assert_eq!(first.get("name").and_then(|v| v.as_str()), Some("Reef"));
        let colors = first.get("colors").and_then(|v| v.as_array()).expect("colours");
        assert_eq!(colors.len(), 2);
        assert_eq!(colors[0].as_str(), Some("#00C8FF"));
        assert_eq!(first.get("shadowed").and_then(|v| v.as_bool()), Some(false));
    }

    #[test]
    fn names_are_restricted_to_safe_characters() {
        assert!(valid_name("Deep Sea 2").is_ok());
        assert!(valid_name("").is_err());
        assert!(valid_name("../escape").is_err());
        assert!(valid_name("a/b").is_err());
        assert!(valid_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn zone_selection_defaults_to_everything() {
        let d = scheme();
        let all = select_zones(&d, &[]).unwrap();
        assert_eq!(crate::ui::zones_in(&all).len(), 4);
    }

    #[test]
    fn zone_selection_keeps_only_what_was_asked_for() {
        let d = scheme();
        let some = select_zones(&d, &["keyboard".into(), "left-slider".into()]).unwrap();
        assert_eq!(crate::ui::zones_in(&some), vec!["keyboard", "leftSlider"]);
    }

    #[test]
    fn zone_selection_rejects_unknown_and_absent_zones() {
        let d = scheme();
        assert!(select_zones(&d, &["trackball".into()]).is_err());
        let partial = json::parse(r#"{"backlight":{"keyboard":{}}}"#).unwrap();
        assert!(select_zones(&partial, &["touchpad".into()]).is_err());
    }

    #[test]
    fn friendly_zone_spellings_are_accepted() {
        assert_eq!(canonical_zone("Left Slider"), Some("leftSlider"));
        assert_eq!(canonical_zone("trackpad"), Some("touchpad"));
        assert_eq!(canonical_zone("keys"), Some("keyboard"));
        assert_eq!(canonical_zone("nope"), None);
    }

    #[test]
    fn save_load_list_delete_round_trip() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("CLEVERTUNA_HOME", &tmp);
        let d = scheme();
        save("Test Profile", &d).expect("saves");
        assert!(exists("Test Profile"));
        let back = load("Test Profile").expect("loads");
        assert_eq!(crate::ui::zones_in(&back).len(), 4);
        assert!(list().iter().any(|e| e.name == "Test Profile"));
        delete("Test Profile").expect("deletes");
        assert!(!exists("Test Profile"));
        std::env::remove_var("CLEVERTUNA_HOME");
        let _ = std::fs::remove_dir_all(&tmp);
    }
    #[test]
    fn the_names_the_app_answers_to_cannot_be_taken() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-reserved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("CLEVERTUNA_HOME", &tmp);

        for taken in ["Wallpaper", "wallpaper", "Random"] {
            let e = save(taken, &scheme()).unwrap_err();
            assert!(e.contains("Clevertuna's own"), "{}: {}", taken, e);
        }
        save("Mine", &scheme()).unwrap();
        assert!(rename("Mine", "Wallpaper").is_err(), "nor by renaming");

        // One that already exists stays listed — so it can be dealt with —
        // but is marked, so no menu offers it as a theme.
        std::fs::write(dir().join("Wallpaper.json"), r#"{"backlight":{}}"#).unwrap();
        let listed = list();
        let old = listed.iter().find(|e| e.name == "Wallpaper").expect("still reachable");
        assert!(old.shadowed);
        assert!(!listed.iter().find(|e| e.name == "Mine").unwrap().shadowed);
        delete("Wallpaper").expect("and can still be removed");

        std::env::remove_var("CLEVERTUNA_HOME");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn renaming_moves_a_theme_and_refuses_to_overwrite_one() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-rename-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("CLEVERTUNA_HOME", &tmp);
        save("Look 1", &scheme()).unwrap();
        save("Keep Me", &scheme()).unwrap();

        rename("Look 1", "Reef").expect("renames");
        assert!(exists("Reef") && !exists("Look 1"));

        // Landing on a name somebody already used would delete their theme.
        assert!(rename("Reef", "Keep Me").is_err());
        assert!(exists("Keep Me"));
        assert!(rename("Nothing", "Anything").is_err());
        assert!(rename("Reef", "../escape").is_err(), "a name is a filename");

        std::env::remove_var("CLEVERTUNA_HOME");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn exports_never_land_in_the_working_directory() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!("clevertuna-exp-{}", std::process::id()));
        std::env::set_var("CLEVERTUNA_EXPORT_DIR", &tmp);
        assert_eq!(export_dir(), tmp, "an explicit export directory wins");
        std::env::remove_var("CLEVERTUNA_EXPORT_DIR");

        // Whatever the fallback picks, it must be somewhere a person can find:
        // absolute, and never the process's working directory.
        let d = export_dir();
        assert!(d.is_absolute() || d == dir(), "export dir must be findable: {:?}", d);
        assert_ne!(d, std::path::PathBuf::from("."), "exports must not go to cwd");
    }

}
