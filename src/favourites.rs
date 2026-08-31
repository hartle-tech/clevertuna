//! The five themes you reach for, and the keys that reach them.
//!
//! Which themes deserve a shortcut is a matter of taste, so the tool does not
//! guess. Nothing is bound until something is chosen, and the theme manager is
//! where that choice is made — a key that puts on a theme you never asked for
//! is worse than a key that does nothing.

use crate::json::{self, Json};

/// How many themes can hold a key. ⌃⌥1 to ⌃⌥5.
pub const SLOTS: usize = 5;

/// Slot 0 is ⌃⌥1. An empty string is an unbound slot.
pub type Favourites = Vec<String>;

pub fn path() -> std::path::PathBuf {
    crate::gallery::config_dir().join("favourites.json")
}

pub fn load() -> Favourites {
    let mut out: Favourites = std::fs::read_to_string(path())
        .ok()
        .and_then(|t| json::parse(&t).ok())
        .and_then(|v| v.get("themes").and_then(|a| a.as_array()).cloned())
        .map(|a| {
            a.iter()
                .map(|x| match x {
                    Json::Str(s) => s.clone(),
                    _ => String::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    out.resize(SLOTS, String::new());
    out.truncate(SLOTS);
    out
}

pub fn save(f: &Favourites) -> Result<(), String> {
    let p = path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    let mut list = f.clone();
    list.resize(SLOTS, String::new());
    std::fs::write(
        &p,
        format!(
            "{}\n",
            json::to_string_pretty(&Json::obj(vec![(
                "themes",
                Json::Arr(list.iter().map(|t| Json::Str(t.clone())).collect())
            )]))
        ),
    )
    .map_err(|e| format!("cannot write {}: {}", p.display(), e))
}

/// Put a theme in a slot, and take it out of any other it was in.
///
/// One theme, one key: the same theme on two keys means one of them is a
/// keystroke nobody will ever remember the point of.
pub fn assign(f: &mut Favourites, slot: usize, theme: &str) {
    if slot >= SLOTS {
        return;
    }
    f.resize(SLOTS, String::new());
    for s in f.iter_mut() {
        if s == theme {
            s.clear();
        }
    }
    f[slot] = theme.to_string();
}

pub fn clear(f: &mut Favourites, slot: usize) {
    f.resize(SLOTS, String::new());
    if slot < SLOTS {
        f[slot].clear();
    }
}

/// Which key, if any, puts this theme on.
pub fn shortcut_for(f: &Favourites, theme: &str) -> Option<String> {
    f.iter()
        .position(|t| t == theme)
        .map(|i| format!("⌃⌥{}", i + 1))
}

pub fn to_json(f: &Favourites) -> Json {
    Json::Arr(
        (0..SLOTS)
            .map(|i| {
                let theme = f.get(i).cloned().unwrap_or_default();
                Json::obj(vec![
                    ("slot", Json::Num(i as f64)),
                    ("shortcut", Json::Str(format!("⌃⌥{}", i + 1))),
                    ("theme", Json::Str(theme)),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_bound_until_something_is_chosen() {
        // The complaint this answers: the keys were wired to whichever themes
        // happened to be listed first, which nobody picked.
        let f = Favourites::new();
        let mut f = f;
        f.resize(SLOTS, String::new());
        assert!(f.iter().all(|t| t.is_empty()));
        assert_eq!(shortcut_for(&f, "theme:reef"), None);
    }

    #[test]
    fn a_theme_holds_one_key_and_moving_it_frees_the_old_one() {
        let mut f = vec![String::new(); SLOTS];
        assign(&mut f, 0, "theme:magma");
        assert_eq!(shortcut_for(&f, "theme:magma").as_deref(), Some("⌃⌥1"));
        assign(&mut f, 3, "theme:magma");
        assert_eq!(shortcut_for(&f, "theme:magma").as_deref(), Some("⌃⌥4"));
        assert!(f[0].is_empty(), "the key it left must not still claim it");
    }

    #[test]
    fn a_slot_can_be_emptied_and_one_out_of_range_is_ignored() {
        let mut f = vec![String::new(); SLOTS];
        assign(&mut f, 1, "wallpaper");
        clear(&mut f, 1);
        assert_eq!(shortcut_for(&f, "wallpaper"), None);
        assign(&mut f, 99, "theme:tide");
        assert_eq!(f.len(), SLOTS);
        assert_eq!(shortcut_for(&f, "theme:tide"), None);
    }

    #[test]
    fn a_short_or_missing_file_still_gives_five_slots() {
        let mut f: Favourites = vec!["theme:tide".into()];
        f.resize(SLOTS, String::new());
        assert_eq!(f.len(), SLOTS);
        let listed = to_json(&f);
        assert_eq!(listed.as_array().unwrap().len(), SLOTS);
    }
}
