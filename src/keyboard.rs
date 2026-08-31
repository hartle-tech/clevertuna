//! The physical keyboard, as data.
//!
//! The Clevetura CLVX S is a compact US ANSI board: a half-height function row,
//! five letter rows, a six-key column down the right edge and an inverted-T
//! arrow cluster — 16.29 key units wide by 5.89 tall.
//!
//! The part worth stating plainly, because every drawing of it gets this wrong:
//! **there is no separate touchpad.** The touch surface is a region of the key
//! field itself, and the two sliders are strips running along the F2–F6 and
//! F7–F11 keycaps. Drawing the pad as its own rectangle beside the keys draws a
//! keyboard that does not exist.
//!
//! The table lives in `assets/clvx-s-layout.json` so the macOS builder and the
//! design can read the same one.

/// The layout table, compiled in so the command works with no files to find.
pub const LAYOUT_JSON: &str = include_str!("../assets/clvx-s-layout.json");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn layout() -> json::Json {
        json::parse(LAYOUT_JSON).expect("the layout table is valid JSON")
    }

    #[test]
    fn every_row_is_inside_the_deck() {
        let l = layout();
        let width = l.get("unit").and_then(|u| u.get("width")).and_then(|v| v.as_f64()).unwrap();
        let height = l.get("unit").and_then(|u| u.get("height")).and_then(|v| v.as_f64()).unwrap();
        for row in l.get("rows").and_then(|r| r.as_array()).unwrap() {
            let y = row.get("y").and_then(|v| v.as_f64()).unwrap();
            let h = row.get("h").and_then(|v| v.as_f64()).unwrap();
            assert!(y + h <= height + 0.01, "a row runs past the bottom of the deck");
            for key in row.get("keys").and_then(|k| k.as_array()).unwrap() {
                let x = key.get("x").and_then(|v| v.as_f64()).unwrap();
                let w = key.get("w").and_then(|v| v.as_f64()).unwrap();
                assert!(x + w <= width + 0.01, "a key runs past the right edge of the deck");
            }
        }
    }

    #[test]
    fn keys_in_a_row_do_not_overlap() {
        let l = layout();
        for row in l.get("rows").and_then(|r| r.as_array()).unwrap() {
            let ry = row.get("y").and_then(|v| v.as_f64()).unwrap();
            let rh = row.get("h").and_then(|v| v.as_f64()).unwrap();
            // Rectangles, not spans: the arrow cluster stacks two half-height
            // keys in one column, so sharing an `x` range is legitimate as long
            // as the two do not also share a `y` range.
            let boxes: Vec<(f64, f64, f64, f64)> = row
                .get("keys")
                .and_then(|k| k.as_array())
                .unwrap()
                .iter()
                .map(|k| {
                    let f = |key: &str, or: f64| k.get(key).and_then(|v| v.as_f64()).unwrap_or(or);
                    let x = k.get("x").and_then(|v| v.as_f64()).unwrap();
                    let w = k.get("w").and_then(|v| v.as_f64()).unwrap();
                    let y = f("y", ry);
                    let h = f("h", rh);
                    (x, x + w, y, y + h)
                })
                .collect();
            for (i, a) in boxes.iter().enumerate() {
                for b in boxes.iter().skip(i + 1) {
                    let apart = a.1 <= b.0 + 0.001
                        || b.1 <= a.0 + 0.001
                        || a.3 <= b.2 + 0.001
                        || b.3 <= a.2 + 0.001;
                    assert!(apart, "two keys in one row sit on top of each other");
                }
            }
            // And a key that overrides its band still has to stay inside it.
            for b in &boxes {
                assert!(
                    b.2 >= ry - 0.001 && b.3 <= ry + rh + 0.001,
                    "a key hangs outside its own row"
                );
            }
        }
    }

    /// The whole point: the touch surface covers keys, and the sliders lie on
    /// the function row. If this ever passes with a pad outside the key field,
    /// the drawing has gone back to being a picture of a different keyboard.
    #[test]
    fn touch_zones_lie_over_the_keys() {
        let l = layout();
        let height = l.get("unit").and_then(|u| u.get("height")).and_then(|v| v.as_f64()).unwrap();
        let zones = l.get("zones").and_then(|z| z.as_array()).unwrap();

        let pad = zones
            .iter()
            .find(|z| z.get("id").and_then(|v| v.as_str()) == Some("touchpad"))
            .expect("the keyboard has a touch surface");
        let y = pad.get("y").and_then(|v| v.as_f64()).unwrap();
        let h = pad.get("h").and_then(|v| v.as_f64()).unwrap();
        assert!(y > 0.0 && y + h <= height, "the touch surface is not on the deck");

        let fn_row_bottom = l.get("rows").and_then(|r| r.as_array()).unwrap()[0]
            .get("h")
            .and_then(|v| v.as_f64())
            .unwrap()
            + l.get("rows").and_then(|r| r.as_array()).unwrap()[0]
                .get("y")
                .and_then(|v| v.as_f64())
                .unwrap();
        assert!(y >= fn_row_bottom - 0.06, "the touch surface starts above the letter rows");

        for id in ["leftSlider", "rightSlider"] {
            let s = zones
                .iter()
                .find(|z| z.get("id").and_then(|v| v.as_str()) == Some(id))
                .expect("both sliders are described");
            let sy = s.get("y").and_then(|v| v.as_f64()).unwrap();
            let sh = s.get("h").and_then(|v| v.as_f64()).unwrap();
            assert!(sy + sh <= fn_row_bottom + 0.01, "a slider hangs below the function row");
        }
    }
}
