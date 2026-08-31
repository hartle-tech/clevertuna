//! Match the keyboard to the desktop.
//!
//! Finds the current wallpaper, pulls the colours that actually carry the
//! image, and turns them into a scheme. Everything here is owned — a DEFLATE
//! decoder, a PNG reader and a quantiser — because the alternative is three
//! image crates for one feature.
//!
//! Only PNG is decoded directly. Other formats are converted to PNG first
//! using whatever the platform already has (`sips` on macOS, ImageMagick
//! elsewhere), so a JPEG wallpaper still works without a JPEG decoder here.

use crate::json::Json;
use std::path::PathBuf;
use std::process::Command;

// ───────────────────────────── DEFLATE ────────────────────────────────────

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    bit: u32,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Bits<'a> {
        Bits { d, pos: 0, bit: 0 }
    }
    fn bit(&mut self) -> Option<u32> {
        let byte = *self.d.get(self.pos)?;
        let b = (byte >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        Some(b as u32)
    }
    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut out = 0u32;
        for i in 0..n {
            out |= self.bit()? << i;
        }
        Some(out)
    }
    fn align(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
    }
}

/// Canonical Huffman decoding from code lengths.
struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { counts, symbols }
    }

    fn decode(&self, b: &mut Bits) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.bit()? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LEN_BASE: [u16; 29] = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
const LEN_EXTRA: [u8; 29] = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
const DIST_BASE: [u16; 30] = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
const DIST_EXTRA: [u8; 30] = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];

/// Raw DEFLATE (RFC 1951).
pub fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut b = Bits::new(data);
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.bit()?;
        let kind = b.bits(2)?;
        match kind {
            0 => {
                b.align();
                let len = u16::from_le_bytes([*b.d.get(b.pos)?, *b.d.get(b.pos + 1)?]) as usize;
                b.pos += 4;
                let end = b.pos.checked_add(len)?;
                out.extend_from_slice(b.d.get(b.pos..end)?);
                b.pos = end;
            }
            1 | 2 => {
                let (lit, dist) = if kind == 1 {
                    let mut l = [0u8; 288];
                    for (i, v) in l.iter_mut().enumerate() {
                        *v = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                    }
                    (Huff::new(&l), Huff::new(&[5u8; 30]))
                } else {
                    let hlit = b.bits(5)? as usize + 257;
                    let hdist = b.bits(5)? as usize + 1;
                    let hclen = b.bits(4)? as usize + 4;
                    const ORDER: [usize; 19] = [16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
                    let mut cl = [0u8; 19];
                    for i in 0..hclen {
                        cl[ORDER[i]] = b.bits(3)? as u8;
                    }
                    let clh = Huff::new(&cl);
                    let mut lens = vec![0u8; hlit + hdist];
                    let mut i = 0;
                    while i < lens.len() {
                        let sym = clh.decode(&mut b)?;
                        match sym {
                            0..=15 => {
                                lens[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                let prev = if i > 0 { lens[i - 1] } else { return None };
                                let n = 3 + b.bits(2)? as usize;
                                for _ in 0..n {
                                    if i >= lens.len() { break }
                                    lens[i] = prev;
                                    i += 1;
                                }
                            }
                            17 => {
                                let n = 3 + b.bits(3)? as usize;
                                i = (i + n).min(lens.len());
                            }
                            18 => {
                                let n = 11 + b.bits(7)? as usize;
                                i = (i + n).min(lens.len());
                            }
                            _ => return None,
                        }
                    }
                    (Huff::new(&lens[..hlit]), Huff::new(&lens[hlit..]))
                };
                loop {
                    let sym = lit.decode(&mut b)?;
                    if sym == 256 {
                        break;
                    }
                    if sym < 256 {
                        out.push(sym as u8);
                    } else {
                        let idx = sym as usize - 257;
                        if idx >= 29 { return None }
                        let len = LEN_BASE[idx] as usize + b.bits(LEN_EXTRA[idx] as u32)? as usize;
                        let dsym = dist.decode(&mut b)? as usize;
                        if dsym >= 30 { return None }
                        let d = DIST_BASE[dsym] as usize + b.bits(DIST_EXTRA[dsym] as u32)? as usize;
                        if d > out.len() { return None }
                        let start = out.len() - d;
                        for k in 0..len {
                            let byte = out[start + k];
                            out.push(byte);
                        }
                    }
                }
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}

// ────────────────────────────── PNG ───────────────────────────────────────

pub struct Image {
    #[allow(dead_code)] // The decoder reports the full image; the quantiser only needs the pixels.
    pub width: usize,
    #[allow(dead_code)]
    pub height: usize,
    pub rgb: Vec<[u8; 3]>,
}

fn paeth(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
}

/// Decode a non-interlaced 8-bit RGB/RGBA/grey PNG.
pub fn decode_png(data: &[u8]) -> Option<Image> {
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let (mut i, mut idat) = (8usize, Vec::new());
    let (mut w, mut h, mut depth, mut color, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    while i + 8 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i+1], data[i+2], data[i+3]]) as usize;
        let typ = &data[i + 4..i + 8];
        let body = data.get(i + 8..i + 8 + len)?;
        match typ {
            b"IHDR" => {
                w = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                h = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
                depth = body[8];
                color = body[9];
                interlace = body[12];
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        i += 12 + len;
    }
    if w == 0 || h == 0 || depth != 8 || interlace != 0 {
        return None;
    }
    let channels = match color {
        0 => 1usize, // grey
        2 => 3,      // rgb
        4 => 2,      // grey+alpha
        6 => 4,      // rgba
        _ => return None, // palette not handled; converters avoid it
    };
    // zlib wrapper: 2-byte header, then DEFLATE
    let raw = inflate(idat.get(2..)?)?;
    let stride = w * channels;
    let mut rgb = Vec::with_capacity(w * h);
    let mut prev = vec![0u8; stride];
    let mut line = vec![0u8; stride];
    let mut p = 0usize;
    for _ in 0..h {
        let filter = *raw.get(p)?;
        p += 1;
        line.copy_from_slice(raw.get(p..p + stride)?);
        p += stride;
        for x in 0..stride {
            let a = if x >= channels { line[x - channels] as i32 } else { 0 };
            let b = prev[x] as i32;
            let c = if x >= channels { prev[x - channels] as i32 } else { 0 };
            let v = line[x] as i32;
            line[x] = match filter {
                0 => v,
                1 => v + a,
                2 => v + b,
                3 => v + (a + b) / 2,
                4 => v + paeth(a, b, c),
                _ => v,
            } as u8;
        }
        for x in 0..w {
            let o = x * channels;
            let px = match channels {
                1 | 2 => [line[o], line[o], line[o]],
                _ => [line[o], line[o + 1], line[o + 2]],
            };
            rgb.push(px);
        }
        prev.copy_from_slice(&line);
    }
    Some(Image { width: w, height: h, rgb })
}

// ─────────────────────────── wallpaper lookup ─────────────────────────────

/// The picture macOS records as the one on the desktop.
///
/// The store is a binary plist, and `plutil` — which ships with the system, as
/// `sips` does — turns it into something the JSON reader this crate already
/// owns can read. A second file format to own for one lookup is not worth it.
///
/// The schema is Apple's and moves between releases, so this does not walk a
/// fixed path through it: it takes the first picture named anywhere inside,
/// which survives the keys being renamed or nested one level deeper.
#[cfg(target_os = "macos")]
fn from_wallpaper_store() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let index = PathBuf::from(&home)
        .join("Library/Application Support/com.apple.wallpaper/Store/Index.plist");
    if !index.exists() {
        return None;
    }
    let out = Command::new("plutil")
        .args(["-convert", "json", "-o", "-", index.to_str()?])
        .output()
        .ok()?;
    let doc = crate::json::parse(&String::from_utf8_lossy(&out.stdout)).ok()?;
    let mut found = None;
    first_picture(&doc, &mut found);
    found
}

/// The first value in a document that names an image file that exists.
#[cfg(target_os = "macos")]
fn first_picture(v: &Json, out: &mut Option<PathBuf>) {
    if out.is_some() {
        return;
    }
    match v {
        Json::Str(s) => {
            let raw = s.strip_prefix("file://").unwrap_or(s);
            // Percent-encoding is what a URL brings and a path does not want.
            let path = PathBuf::from(percent_decode(raw));
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            let known = matches!(ext.as_deref(),
                Some("heic" | "heif" | "jpg" | "jpeg" | "png" | "tif" | "tiff"));
            if known && path.is_absolute() && path.exists() {
                *out = Some(path);
            }
        }
        Json::Arr(items) => {
            for item in items {
                first_picture(item, out);
            }
        }
        Json::Obj(map) => {
            for value in map.values() {
                first_picture(value, out);
            }
        }
        _ => {}
    }
}

/// `%20` and friends, as a URL taken out of a plist carries them.
#[cfg(target_os = "macos")]
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where the desktop keeps its current wallpaper.
pub fn current_wallpaper() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // The store first, because AppleScript is not a way to ask this.
        //
        // Measured on a Mac plainly showing a picture: `System Events` answers
        // `missing value` and the Finder fallback then fails with -1700. Both
        // scripts only ever answer for a wallpaper that is a file the person
        // chose, and neither answers at all without an Automation grant — so
        // the second grab of a session reported "could not find the current
        // wallpaper", which the helper menu showed as nothing happening.
        if let Some(p) = from_wallpaper_store() {
            return Some(p);
        }
        // `picture of current desktop` is already a POSIX path, so asking for
        // "POSIX path of" it fails with -1728. Finder is the fallback because
        // System Events needs an Automation grant that a fresh install lacks.
        for script in [
            "tell application \"System Events\" to get picture of current desktop",
            "tell application \"Finder\" to get POSIX path of (get desktop picture as alias)",
        ] {
            let out = match Command::new("osascript").args(["-e", script]).output() {
                Ok(o) => o,
                Err(_) => continue,
            };
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() && std::path::Path::new(&p).exists() {
                return Some(PathBuf::from(p));
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Ask the wallpaper daemon what it is actually showing.
        //
        // Config files lie: they may be absent, stale, or managed declaratively
        // somewhere else entirely. The process that painted the desktop has the
        // real answer in its own argv — measured on this machine, swaybg was
        // launched with an image path no config file mentioned.
        if let Some(p) = from_running_daemon() {
            return Some(p);
        }
        let home = std::env::var("HOME").ok()?;
        for rel in [
            ".config/hypr/hyprpaper.conf",
            ".config/hypr/hyprpaper.conf.d/wallpaper.conf",
        ] {
            if let Ok(text) = std::fs::read_to_string(PathBuf::from(&home).join(rel)) {
                for line in text.lines() {
                    let l = line.trim();
                    if let Some(v) = l.strip_prefix("wallpaper") {
                        if let Some(p) = v.split(',').nth(1) {
                            return Some(PathBuf::from(expand(p.trim(), &home)));
                        }
                    }
                    if let Some(v) = l.strip_prefix("preload") {
                        return Some(PathBuf::from(expand(v.trim_start_matches('=').trim(), &home)));
                    }
                }
            }
        }
        if let Ok(out) = Command::new("swww").arg("query").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(p) = s.split("image: ").nth(1) {
                return Some(PathBuf::from(p.trim().to_string()));
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("Microsoft/Windows/Themes/TranscodedWallpaper");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// Read the image path out of a running wallpaper daemon's command line.
#[cfg(target_os = "linux")]
fn from_running_daemon() -> Option<PathBuf> {
    let rd = std::fs::read_dir("/proc").ok()?;
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cmd = match std::fs::read(e.path().join("cmdline")) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let args: Vec<String> = cmd
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        let exe = match args.first() {
            Some(a) => a.rsplit('/').next().unwrap_or(a).to_string(),
            None => continue,
        };
        if !matches!(exe.as_str(), "swaybg" | "swww" | "swww-daemon" | "hyprpaper" | "mpvpaper" | "feh") {
            continue;
        }
        // the image is either after -i/--image, or simply the last path-like argument
        for (i, a) in args.iter().enumerate() {
            if (a == "-i" || a == "--image") && i + 1 < args.len() {
                return Some(PathBuf::from(&args[i + 1]));
            }
        }
        if let Some(last) = args.iter().rev().find(|a| {
            let l = a.to_ascii_lowercase();
            l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg")
        }) {
            return Some(PathBuf::from(last));
        }
    }
    None
}

#[allow(dead_code)]
fn expand(p: &str, home: &str) -> String {
    let p = p.trim_matches('"');
    if let Some(rest) = p.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else {
        p.to_string()
    }
}

/// Load any image by converting to PNG first when it is not already one.
pub fn load_image(path: &PathBuf) -> Result<Image, String> {
    let data = std::fs::read(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    if let Some(img) = decode_png(&data) {
        return Ok(img);
    }
    // Not a PNG we can read — ask the platform to convert a small copy.
    let tmp = std::env::temp_dir().join("clevertuna-wallpaper.png");
    let converted = if cfg!(target_os = "macos") {
        Command::new("sips")
            .args(["-s", "format", "png", "-Z", "400"])
            .arg(path)
            .arg("--out")
            .arg(&tmp)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        ["magick", "convert"].iter().any(|bin| {
            Command::new(bin)
                .arg(path)
                .args(["-resize", "400x400", "-strip", "-define", "png:color-type=2"])
                .arg(&tmp)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    };
    if !converted {
        return Err(format!(
            "{} is not a PNG this build can read, and no converter was available \
             (install ImageMagick, or point --wallpaper at a PNG)",
            path.display()
        ));
    }
    let data = std::fs::read(&tmp).map_err(|e| format!("cannot read the converted image: {}", e))?;
    let _ = std::fs::remove_file(&tmp);
    decode_png(&data).ok_or_else(|| "the converted image could not be decoded".to_string())
}

// ──────────────────────────── quantising ──────────────────────────────────

/// Pick the colours that actually carry the image.
///
/// Near-black and near-white are skipped: a wallpaper is usually mostly sky or
/// mostly shadow, and a keyboard lit in those is not "matching the wallpaper",
/// it is off. Saturation-weighted counting is what makes the result look like
/// the picture.
/// The most colourful colours in an image, one per hue family.
///
/// Ranking by how much of the image a colour covers is the obvious approach and
/// it is wrong for this job. A painting is mostly midtones — bark, earth, shadow
/// — so frequency picks five browns out of a picture whose subject is a red
/// maple against an orange sunset. Weighting frequency by saturation does not
/// save it either: a drab colour over half the canvas still outscores a vivid
/// one over two percent of it.
///
/// So saturation leads. Pixels are sorted into hue families, each family is
/// represented by its most saturated members rather than by its average — an
/// average is precisely what turns a red family into brown — and families are
/// ranked by how vivid they are, with only a slight preference for the ones
/// that cover more ground.
pub fn dominant_colours(img: &Image, want: usize) -> Vec<[u8; 3]> {
    const BINS: usize = 24; // 15° of hue each
    const KEEP: usize = 64; // most saturated members remembered per family

    struct Family {
        count: u64,
        best: Vec<(u32, [u8; 3])>, // (score, colour), worst-first
    }
    let mut families: Vec<Family> = (0..BINS)
        .map(|_| Family { count: 0, best: Vec::new() })
        .collect();

    let step = ((img.rgb.len() / 120_000).max(1)) as usize;
    let mut sampled = 0u64;
    for px in img.rgb.iter().step_by(step) {
        let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let v = max / 255.0;
        if v < 0.18 {
            continue; // near-black carries no hue worth using
        }
        let sat = if max <= 0.0 { 0.0 } else { (max - min) / max };
        // White is not "bright", it is *unsaturated* — a pure red has a maxed
        // channel and is the most colourful thing in the picture. Rejecting on
        // brightness threw away exactly the colours this is looking for.
        if sat < 0.22 {
            continue; // grey, and greys are what made the old picks look muddy
        }
        sampled += 1;

        let d = max - min;
        let mut hue = if max == r {
            60.0 * (((g - b) / d) % 6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        if hue < 0.0 {
            hue += 360.0;
        }
        let bin = ((hue / 360.0 * BINS as f32) as usize).min(BINS - 1);

        // Bright and saturated is what reads on an LED. Dimness is penalised;
        // brightness above about 70% is not, because there is nothing wrong
        // with a colour being fully lit.
        let score = (sat * (v / 0.70).min(1.0) * 10_000.0) as u32;
        let f = &mut families[bin];
        f.count += 1;
        if f.best.len() < KEEP {
            f.best.push((score, [px[0], px[1], px[2]]));
            f.best.sort_unstable_by_key(|e| e.0);
        } else if score > f.best[0].0 {
            f.best[0] = (score, [px[0], px[1], px[2]]);
            f.best.sort_unstable_by_key(|e| e.0);
        }
    }

    if sampled == 0 {
        return Vec::new();
    }
    // A family needs to be more than a handful of stray pixels to count.
    let floor = (sampled / 400).max(6);

    let mut ranked: Vec<(f32, [u8; 3], f32)> = families
        .into_iter()
        .enumerate()
        .filter(|(_, f)| f.count >= floor && !f.best.is_empty())
        .map(|(bin, f)| {
            // Average only the most saturated quarter: enough members to be
            // stable, few enough that the family keeps its character.
            let take = (f.best.len() / 4).max(1);
            let top = &f.best[f.best.len() - take..];
            let mut sum = [0u64; 3];
            for (_, c) in top {
                for i in 0..3 {
                    sum[i] += c[i] as u64;
                }
            }
            let colour = [
                (sum[0] / take as u64) as u8,
                (sum[1] / take as u64) as u8,
                (sum[2] / take as u64) as u8,
            ];
            let vividness = top.iter().map(|(s, _)| *s as f32).sum::<f32>() / take as f32;
            // Coverage counts, but only as a tie-breaker between vivid families.
            let hue = (bin as f32 + 0.5) * (360.0 / BINS as f32);
            (vividness * (f.count as f32).powf(0.18), colour, hue)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Separation is enforced on hue, not on RGB distance. Two neighbouring
    // families can sit close together in RGB and still be the same colour to a
    // viewer — and the vividness lift applied afterwards pushes them closer
    // still, so an RGB test done here would pass a pair that ends up identical.
    const MIN_HUE_GAP: f32 = 30.0;
    let mut out: Vec<[u8; 3]> = Vec::new();
    let mut taken: Vec<f32> = Vec::new();
    for (_, c, hue) in &ranked {
        let clear = taken.iter().all(|t: &f32| {
            let d = (t - hue).abs();
            d.min(360.0 - d) >= MIN_HUE_GAP
        });
        if clear {
            out.push(*c);
            taken.push(*hue);
            if out.len() == want {
                return out;
            }
        }
    }
    // A picture built from one or two hues cannot fill the quota that way, so
    // rather than return two colours, let the rest in by vividness order.
    for (_, c, _) in &ranked {
        if out.len() == want {
            break;
        }
        if !out.contains(c) {
            out.push(*c);
        }
    }
    out
}

/// Make a colour worth putting on an LED.
///
/// A backlight is emitting light, not reflecting it, so a colour sampled from a
/// picture reads dimmer and greyer on the keys than it does on screen. This
/// lifts saturation and brightness while leaving the hue alone, which is what
/// carries the resemblance.
pub fn vivid(c: [u8; 3]) -> [u8; 3] {
    let (r, g, b) = (c[0] as f32, c[1] as f32, c[2] as f32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max <= 0.0 {
        return c;
    }
    let sat = (max - min) / max;
    let target_sat = (sat * 1.35).clamp(0.65, 1.0);
    let target_val = (max / 255.0 * 1.25).clamp(0.72, 1.0);

    // Rebuild the colour at the same hue with the stronger saturation and value.
    let scale = if sat <= 0.0 { 0.0 } else { target_sat / sat };
    let out = [r, g, b].map(|ch| {
        let lifted = max - (max - ch) * scale; // pull the low channels down
        (lifted / max * target_val * 255.0).clamp(0.0, 255.0) as u8
    });
    out
}

/// Build a colour-wave scheme from an image.
/// The number of colour stops a zone must carry.
///
/// Not a preference: the device rejects anything else.
const MARKERS_PER_ZONE: usize = 5;

pub fn scheme_from_image(img: &Image, zones: &[String]) -> Result<Json, String> {
    // Four families: the picture's palette, not a gradient of one hue.
    let palette: Vec<[u8; 3]> = dominant_colours(img, 4).into_iter().map(vivid).collect();
    if palette.len() < 2 {
        return Err("that wallpaper has too little colour to build a scheme from".into());
    }

    // The firmware wants exactly five stops — measured: every zone of a stock
    // device carries five, and a four-stop write is refused outright with
    // BAD_REQUEST. Repeating the first colour at the end fills the quota and
    // closes the loop, so a wave does not jump when it comes back round.
    let mut colours = palette.clone();
    while colours.len() < MARKERS_PER_ZONE {
        colours.push(palette[colours.len() % palette.len()]);
    }
    colours.truncate(MARKERS_PER_ZONE);
    if palette.len() < MARKERS_PER_ZONE {
        let first = palette[0];
        let last = colours.len() - 1;
        colours[last] = first;
    }

    let markers: Vec<Json> = colours
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let pos = if colours.len() == 1 { 50 } else { i * 100 / (colours.len() - 1) };
            Json::obj(vec![
                ("color", Json::obj(vec![
                    ("red", Json::Num(c[0] as f64)),
                    ("green", Json::Num(c[1] as f64)),
                    ("blue", Json::Num(c[2] as f64)),
                ])),
                ("position", Json::Num(pos.min(100) as f64)),
                ("transparency", Json::Num(0.0)),
            ])
        })
        .collect();
    let picker = Json::obj(vec![
        ("markersNumber", Json::Num(markers.len() as f64)),
        ("markersArray", Json::Arr(markers)),
    ]);
    let brightest = colours
        .iter()
        .max_by_key(|c| c[0] as u32 + c[1] as u32 + c[2] as u32)
        .copied()
        .unwrap_or([0, 200, 255]);

    let zone = |dir: u32, period: u32, length: u32, slider: bool| {
        let mut pairs = vec![
            ("colorWave", Json::obj(vec![
                ("colorLinePicker", picker.clone()),
                ("period", Json::Num(period as f64)),
                ("direction", Json::Num(dir as f64)),
                ("length", Json::Num(length as f64)),
            ])),
            ("transparency", Json::Num(if slider { 30.0 } else { 0.0 })),
        ];
        pairs.push((
            "interactiveAnimation",
            if slider {
                Json::obj(vec![("enable", Json::Bool(true))])
            } else {
                Json::obj(vec![
                    ("enable", Json::Bool(true)),
                    ("color", Json::obj(vec![
                        ("red", Json::Num(brightest[0] as f64)),
                        ("green", Json::Num(brightest[1] as f64)),
                        ("blue", Json::Num(brightest[2] as f64)),
                    ])),
                ])
            },
        ));
        Json::obj(pairs)
    };

    let mut out: Vec<(&str, Json)> = Vec::new();
    for z in zones {
        match z.as_str() {
            "keyboard" => out.push(("keyboard", zone(270, 3000, 1000, false))),
            "touchpad" => out.push(("touchpad", zone(90, 8000, 100, false))),
            "leftSlider" => out.push(("leftSlider", zone(180, 4000, 100, true))),
            "rightSlider" => out.push(("rightSlider", zone(0, 7500, 160, true))),
            _ => {}
        }
    }
    if out.is_empty() {
        return Err("no zones selected".into());
    }
    Ok(Json::obj(vec![
        (crate::backlight::SCHEMA_KEY, Json::Num(crate::backlight::SCHEMA_VERSION as f64)),
        ("backlight", Json::obj(out)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 PNG, written by hand so the test needs no fixtures.
    fn tiny_png() -> Vec<u8> {
        // built with a stored (uncompressed) DEFLATE block
        fn crc32(d: &[u8]) -> u32 { crate::transport::crc32(d) }
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let chunk = |typ: &[u8], body: &[u8], out: &mut Vec<u8>| {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(typ);
            out.extend_from_slice(body);
            let mut c = typ.to_vec();
            c.extend_from_slice(body);
            out.extend_from_slice(&crc32(&c).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&2u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(b"IHDR", &ihdr, &mut png);
        // rows: filter byte + 2 px RGB
        let raw: Vec<u8> = vec![0, 255, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255];
        let mut z = vec![0x78, 0x01];
        z.push(1); // final, stored
        z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        z.extend_from_slice(&raw);
        chunk(b"IDAT", &z, &mut png);
        chunk(b"IEND", &[], &mut png);
        png
    }

    #[test]
    fn decodes_a_hand_built_png() {
        let img = decode_png(&tiny_png()).expect("decodes");
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(img.rgb.len(), 4);
        assert_eq!(img.rgb[0], [255, 0, 0]);
        assert_eq!(img.rgb[1], [0, 255, 0]);
    }

    #[test]
    fn inflate_handles_a_stored_block() {
        let payload = b"clevertuna";
        let mut z = vec![1u8];
        z.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(payload.len() as u16)).to_le_bytes());
        z.extend_from_slice(payload);
        assert_eq!(inflate(&z).unwrap(), payload.to_vec());
    }

    #[test]
    fn rejects_things_that_are_not_png() {
        assert!(decode_png(b"not a png at all").is_none());
        assert!(decode_png(&[]).is_none());
    }

    #[test]
    fn dominant_colours_skip_black_and_white() {
        let mut rgb = vec![[0u8, 0, 0]; 500];
        rgb.extend(vec![[255u8, 255, 255]; 500]);
        rgb.extend(vec![[255u8, 83, 83]; 300]);
        rgb.extend(vec![[0u8, 200, 255]; 300]);
        let img = Image { width: 40, height: 40, rgb };
        let c = dominant_colours(&img, 5);
        assert!(!c.is_empty());
        for x in &c {
            let max = x[0].max(x[1]).max(x[2]);
            let min = x[0].min(x[1]).min(x[2]);
            assert!(max >= 28 && min <= 232, "kept a black/white colour: {:?}", x);
        }
    }

    #[test]
    fn builds_a_valid_scheme_from_an_image() {
        let mut rgb = vec![[255u8, 83, 83]; 400];
        rgb.extend(vec![[0u8, 200, 255]; 400]);
        rgb.extend(vec![[54u8, 240, 177]; 400]);
        let img = Image { width: 60, height: 20, rgb };
        let zones: Vec<String> = ["keyboard", "touchpad", "leftSlider", "rightSlider"]
            .iter().map(|s| s.to_string()).collect();
        let doc = scheme_from_image(&img, &zones).expect("builds");
        // it must be something the encoder accepts
        crate::backlight::from_json(&doc).expect("encodes for the device");
        assert_eq!(crate::ui::zones_in(&doc).len(), 4);
    }

    #[test]
    fn refuses_an_image_with_no_usable_colour() {
        let img = Image { width: 4, height: 4, rgb: vec![[0, 0, 0]; 16] };
        assert!(scheme_from_image(&img, &["keyboard".to_string()]).is_err());
    }
    #[test]
    fn a_fully_lit_colour_is_not_mistaken_for_white() {
        // A pure red maxes a channel, so a "too bright" test throws away the
        // most colourful thing in the picture. White is unsaturated, not bright.
        let mut rgb = vec![[255u8, 0, 0]; 300];
        rgb.extend(vec![[255u8, 255, 255]; 900]); // genuine white, must go
        rgb.extend(vec![[0u8, 255, 0]; 300]);
        let img = Image { width: 50, height: 30, rgb };
        let c = dominant_colours(&img, 4);
        assert!(c.iter().any(|x| x[0] > 180 && x[1] < 90 && x[2] < 90), "lost the red: {:?}", c);
        assert!(c.iter().any(|x| x[1] > 180 && x[0] < 90 && x[2] < 90), "lost the green: {:?}", c);
        for x in &c {
            let min = x[0].min(x[1]).min(x[2]);
            assert!(min < 200, "kept white: {:?}", x);
        }
    }

    #[test]
    fn the_most_colourful_wins_over_the_most_common() {
        // The failure this replaced: a drab colour covering most of a picture
        // outranked a vivid one, so a painting produced five browns.
        let mut rgb = vec![[105u8, 92, 70]; 4000]; // muddy, everywhere
        rgb.extend(vec![[220u8, 20, 30]; 200]); // vivid, rare
        let img = Image { width: 100, height: 42, rgb };
        let c = dominant_colours(&img, 2);
        assert!(!c.is_empty());
        let first = c[0];
        assert!(
            first[0] > 150 && first[1] < 110,
            "the vivid colour must come first, got {:?}", c
        );
    }

    #[test]
    fn vivid_lifts_without_moving_the_hue() {
        // Muted olive: it should get brighter and stronger, not become another
        // colour, because hue is what carries the resemblance to the picture.
        let before = [90u8, 100, 40];
        let after = vivid(before);
        assert!(after.iter().max() > before.iter().max(), "no lift: {:?}", after);
        // green stays the dominant channel
        assert!(after[1] >= after[0] && after[1] > after[2], "hue moved: {:?}", after);
    }

    #[test]
    fn a_scheme_always_carries_five_stops() {
        // The device refuses anything else with BAD_REQUEST, so a palette of
        // four has to be padded — and the padding closes the loop.
        let mut rgb = vec![[220u8, 30, 20]; 400];
        rgb.extend(vec![[30u8, 200, 60]; 400]);
        rgb.extend(vec![[240u8, 190, 0]; 400]);
        let img = Image { width: 60, height: 20, rgb };
        let doc = scheme_from_image(&img, &["keyboard".to_string()]).expect("builds");
        let markers = doc
            .get("backlight").and_then(|b| b.get("keyboard"))
            .and_then(|k| k.get("colorWave"))
            .and_then(|w| w.get("colorLinePicker"))
            .and_then(|p| p.get("markersArray"))
            .and_then(|a| a.as_array())
            .expect("has markers");
        assert_eq!(markers.len(), 5, "the device rejects any other count");
        let hex = |m: &Json| crate::ui::hex(m.get("color").unwrap());
        assert_eq!(hex(&markers[0]), hex(&markers[4]), "the wave must close the loop");
        let last_pos = markers[4].get("position").and_then(|p| p.as_u32());
        assert_eq!(last_pos, Some(100));
    }

}
