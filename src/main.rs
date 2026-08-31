//! Clevertuna — read the current.
//!
//! A single self-contained binary for reading and writing Clevetura CLVX
//! keyboard settings, so a colour scheme is a file you can send someone.

mod backlight;
mod effects;
mod themes;
mod rotate;
mod power;
mod settings;
mod favourites;
#[cfg(target_os = "macos")]
mod ble_macos;
mod gallery;
mod menu;
mod wallpaper;
#[cfg(target_os = "linux")]
mod dbus;
#[cfg(target_os = "macos")]
mod hid_macos;
#[cfg(target_os = "windows")]
mod hid_windows;
mod json;
mod keyboard;
mod keymap;
mod proto;
mod service;
mod transport;
mod tui;
mod ui;
mod webui;

use json::Json;
use service::Stage;
use std::io::{IsTerminal, Write};
use transport::{Device, Kind};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Exit codes are part of the interface; scripts depend on them.
const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 2;
const EXIT_NO_DEVICE: i32 = 3;
const EXIT_TRANSPORT: i32 = 4;
const EXIT_MISMATCH: i32 = 5;
const EXIT_BACKUP: i32 = 6;

#[derive(Clone)]
struct Opts {
    json: bool,
    quiet: bool,
    color: bool,
    show_identifiers: bool,
    ble: bool,
    device: Option<String>,
    ascii: bool,
    port: u16,
    only: Vec<String>,
    wallpaper: Option<String>,
    from: Option<String>,
    print_frame: bool,
    cols: usize,
    to: Option<String>,
    dry_run: bool,
    format: String,
    /// A named seed for `random`, so a good roll can be asked for again.
    seed: Option<u64>,
}

/// The keyboard's own geometry — every key where it sits, what is printed on
/// it, and which of the four lighting zones covers it.
///
/// One table, shared: the Rust side reads it from here, and the macOS builder
/// asks for it over this command rather than keeping a second copy that can
/// drift out of step with the first.
fn cmd_layout() -> i32 {
    print!("{}", crate::keyboard::LAYOUT_JSON);
    println!();
    0
}

fn main() {
    let code = run();
    std::process::exit(code);
}

fn usage() -> String {
    format!(
        "clevertuna {v} — read the current

USAGE
  clevertuna [options] <command>

COMMANDS
  list                     show the keyboards this machine can configure
  info                     identify the connected keyboard
  get-backlight [file]     read the colour scheme (stdout if no file)
  set-backlight <file>     apply a colour scheme, then prove it landed
  export <file>            back up every setting, verbatim
  import <file>            restore such a backup
  tui [file]               keyboard-first terminal interface
  ui                       local interface on http://127.0.0.1:7331
  profile list             the schemes in your gallery
  profile save <name>      save what the keyboard has, under a name
  profile apply <name>     apply a saved scheme, then verify it
  profile rename <a> <b>   give a saved scheme a better name
  profile delete <name>    remove one
  theme [list|<id>]        the themes that ship with Clevertuna
  random [--seed <n>]      roll a theme and put it on
  builder                  open the visual theme builder (macOS app)
  rotate status|off        what the clock is doing to the lighting
  rotate every <c> <t…>    change theme every 5m|15m|30m|hour|day|week|month
  rotate day-night <d> <n> one theme by day, another by night
  rotate slots on|off      keep every slot (cable, 3 BLE channels) alike
  rotate tick              apply the rotation if it is due (cheap when not)
  timeout [<off> [<idle>]] when the backlight goes out (off|5m|10m|30m|1h)
  settings                 every device setting and its value
  settings <key> <value>   change one — power, touch, multi-touch, keyboard
  keys                     what each function key sends
  keys <fN> <action>       remap one — f5 mute, f4 nothing, f3 play-pause
  device os <mac|win|linux> tell the keyboard which OS it is plugged into
  device defaults [apply]  the factory settings — fetched, then written
  device restart           reboot the keyboard, keeping every setting
  device reset --yes       return the keyboard to factory settings
  favourites               the five themes that hold ⌃⌥1 … ⌃⌥5
  favourites set <n> --from <theme>   give one a key
  settings                 every device setting and its value
  settings <key> <value>   change one — power, touch, multi-touch, keyboard
  keys                     what each function key sends
  keys <fN> <action>       remap one — f5 mute, f4 nothing, f3 play-pause
  look [default|random]    the builder's controls, as JSON
  look of <theme>          a named theme's controls, without applying it
  layout                   the keyboard's real geometry, as JSON
  look apply <file>        write a model back to the keyboard, verified
  look preview <file>      the scheme a model would write, without writing
  match-wallpaper          build a scheme from the desktop wallpaper
  copy --from <t> --to <t> copy the lighting between slots (usb|ble)
  open-app                 launch TouchOnKeys for what this tool does not do
  menu [--format f]        status-bar menu: waybar|swiftbar|picker|json
  do <action-id>           run one menu action (used by the bar)
  version                  print the version

OPTIONS
  --ble                    talk over Bluetooth GATT instead of USB
  --device <path>          use a specific interface
  --json                   machine-readable output
  --quiet                  only the payload
  --no-color               never emit colour (NO_COLOR is honoured too)
  --show-identifiers       include serial numbers and similar in output
  --ascii                  no box drawing or colour blocks (TUI)
  --port <n>               port for `ui` (default 7331, loopback only)
  --only <zones>           limit to zones, comma separated (default: all)
  --wallpaper <file>       use this image instead of the desktop wallpaper
  --seed <n>               repeat a particular roll of `random`
  --from <t> / --to <t>    transports for `copy` (usb or ble)
  --dry-run                show what would be written, write nothing
  -h, --help               this text

EXIT CODES
  0 read completed, or write verified      4 transport or protocol failure
  2 usage or validation error              5 write accepted but readback differs
  3 no device found                        6 backup file rejected

The keyboard holds one connection at a time, so the transport you reach it on
is the slot you configure: plug in for USB, or select a Bluetooth channel and
pass --ble.",
        v = VERSION
    )
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = Opts {
        json: false,
        quiet: false,
        color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        show_identifiers: false,
        ble: false,
        device: None,
        ascii: std::env::var_os("TERM").map(|t| t == *"dumb").unwrap_or(false),
        port: 7331,
        only: Vec::new(),
        wallpaper: None,
        from: None,
        print_frame: false,
        cols: 100,
        to: None,
        dry_run: false,
        format: "picker".into(),
        seed: None,
    };
    let mut rest: Vec<String> = Vec::new();
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => opts.json = true,
            "--quiet" | "-q" => opts.quiet = true,
            "--no-color" => opts.color = false,
            "--show-identifiers" => opts.show_identifiers = true,
            "--ble" => opts.ble = true,
            "--ascii" => opts.ascii = true,
            "--dry-run" => opts.dry_run = true,
            "--print-frame" => opts.print_frame = true,
            "--cols" => match it.next() {
                Some(v) => match v.parse::<usize>() {
                    Ok(n) if (40..=400).contains(&n) => opts.cols = n,
                    _ => { eprintln!("--cols needs a width between 40 and 400"); return EXIT_USAGE; }
                },
                None => { eprintln!("--cols needs a width"); return EXIT_USAGE; }
            },
            "--only" => match it.next() {
                Some(v) => opts.only = v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                None => { eprintln!("--only needs a zone list"); return EXIT_USAGE; }
            },
            "--format" => match it.next() {
                Some(v) => opts.format = v,
                None => { eprintln!("--format needs a value"); return EXIT_USAGE; }
            },
            "--seed" => match it.next().and_then(|v| v.parse::<u64>().ok()) {
                Some(n) => opts.seed = Some(n),
                None => { eprintln!("--seed needs a whole number"); return EXIT_USAGE; }
            },
            "--wallpaper" => match it.next() {
                Some(v) => opts.wallpaper = Some(v),
                None => { eprintln!("--wallpaper needs a path"); return EXIT_USAGE; }
            },
            "--from" => match it.next() {
                Some(v) => opts.from = Some(v),
                None => { eprintln!("--from needs usb or ble"); return EXIT_USAGE; }
            },
            "--to" => match it.next() {
                Some(v) => opts.to = Some(v),
                None => { eprintln!("--to needs usb or ble"); return EXIT_USAGE; }
            },
            "--port" => match it.next().and_then(|v| v.parse::<u16>().ok()) {
                Some(p) => opts.port = p,
                None => {
                    eprintln!("--port needs a number");
                    return EXIT_USAGE;
                }
            },
            "--device" | "-d" => match it.next() {
                Some(v) => opts.device = Some(v),
                None => {
                    eprintln!("--device needs a path");
                    return EXIT_USAGE;
                }
            },
            "-h" | "--help" => {
                println!("{}", usage());
                return EXIT_OK;
            }
            other => rest.push(other.to_string()),
        }
    }
    if opts.json {
        opts.color = false;
    }
    let st = ui::Style { color: opts.color, ascii: opts.ascii };

    let cmd = match rest.first() {
        Some(c) => c.clone(),
        None => {
            println!("{}", usage());
            return EXIT_USAGE;
        }
    };
    let arg = rest.get(1).cloned();

    match cmd.as_str() {
        "version" => {
            println!("clevertuna {}", VERSION);
            EXIT_OK
        }
        "list" => cmd_list(&opts, &st),
        "profile" => cmd_profile(&opts, &st, arg.clone(), rest.get(2).cloned(), rest.get(3).cloned()),
        "open-app" => {
            let us = st;
            match gallery::open_vendor_app() {
                Ok(_) => { ui::say(&us, "OPENED", "TouchOnKeys"); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "Install the vendor app, or use clevertuna directly.", "app-not-found", EXIT_USAGE); EXIT_USAGE }
            }
        }
        "match-wallpaper" => cmd_match_wallpaper(&opts, &st),
        "theme" => cmd_theme(&opts, &st, arg.clone()),
        "rotate" => cmd_rotate(&opts, &st, arg.clone(), &rest[2.min(rest.len())..]),
        "timeout" | "power" => cmd_timeout(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "settings" | "set" => cmd_settings(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "device" => cmd_device(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "favourites" | "favorites" => cmd_favourites(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "look" => cmd_look(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "layout" => cmd_layout(),
        "ai" => with_device(&opts, &st, |dev| match service::get_ai_state(dev) {
            Ok(fields) if fields.is_empty() => {
                ui::say(&st, "AI", "the keyboard answered, with nothing in it");
                EXIT_OK
            }
            Ok(fields) => {
                for (f, what) in &fields {
                    ui::say(&st, "AI", &format!("field {:<3} {}", f, what));
                }
                EXIT_OK
            }
            Err(e) => fail(&e.to_string(), &opts, &st),
        }),
        "keys" => cmd_keys(&opts, &st, arg.clone(), rest.get(2).cloned()),
        "random" => cmd_random(&opts, &st, opts.seed),
        "builder" => cmd_builder(&opts, &st),
        "menu" => cmd_menu(&opts),
        "do" => match arg {
            Some(id) => cmd_do(&opts, &st, &id),
            None => { eprintln!("do needs an action id"); EXIT_USAGE }
        },
        "copy" => cmd_copy(&opts, &st),
        "tui" => with_device(&opts, &st, |dev| {
            tui::Tui::new(opts.color, opts.ascii).run(dev, arg.clone())
        }),
        "ui" => {
            // A frame on stdout, so the layout can be reviewed (and handed to a
            // designer) without a keyboard, a terminal size, or raw mode.
            if opts.print_frame {
                print!("{}", tui::preview_frame(opts.cols, opts.ascii, opts.from.as_deref()));
                return EXIT_OK;
            }
            with_device(&opts, &st, |dev| webui::serve(dev, opts.port, !opts.quiet))
        }
        "info" => with_device(&opts, &st, |dev| cmd_info(dev, &opts, &st)),
        "get-backlight" => with_device(&opts, &st, |dev| cmd_get_backlight(dev, arg.clone(), &opts, &st)),
        "set-backlight" => match arg {
            Some(f) => with_device(&opts, &st, |dev| cmd_set_backlight(dev, &f, &opts, &st)),
            None => {
                eprintln!("set-backlight needs a file");
                EXIT_USAGE
            }
        },
        "export" => match arg {
            Some(f) => with_device(&opts, &st, |dev| cmd_export(dev, &f, &opts, &st)),
            None => {
                eprintln!("export needs a file");
                EXIT_USAGE
            }
        },
        "import" => match arg {
            Some(f) => with_device(&opts, &st, |dev| cmd_import(dev, &f, &opts, &st)),
            None => {
                eprintln!("import needs a file");
                EXIT_USAGE
            }
        },
        other => {
            eprintln!("unknown command '{}'\n\n{}", other, usage());
            EXIT_USAGE
        }
    }
}

fn cmd_list(opts: &Opts, st: &ui::Style) -> i32 {
    let found = transport::find_usb();
    let ble = transport::find_ble();
    if opts.json {
        let mut arr: Vec<Json> = found
            .iter()
            .map(|f| {
                Json::obj(vec![
                    ("path", Json::Str(f.path.clone())),
                    ("interface", Json::Str(f.hid_name.clone())),
                    ("transport", Json::Str(f.kind.label().to_string())),
                ])
            })
            .collect();
        if let Some(p) = &ble {
            arr.push(Json::obj(vec![
                ("path", Json::Str(p.clone())),
                ("interface", Json::Str(transport::CHAR_UUID.to_string())),
                ("transport", Json::Str("bluetooth".into())),
            ]));
        }
        println!("{}", json::to_string_pretty(&Json::Arr(arr)));
        return EXIT_OK;
    }
    if found.is_empty() && ble.is_none() {
        let us = *st;
        ui::error(&us, "No configuration interface found.",
                  NO_DEVICE_HINT,
                  "device-not-found", EXIT_NO_DEVICE);
        return EXIT_NO_DEVICE;
    }
    let us = *st;
    for f in &found {
        ui::say(&us, "READY", &format!("CLVX  {}  {}", f.kind.label().to_uppercase(), f.path));
    }
    if let Some(p) = &ble {
        ui::say(&us, "READY", &format!("CLVX  BLE  {}", p));
    }
    EXIT_OK
}

fn with_device<F: FnMut(&mut Device) -> i32>(opts: &Opts, st: &ui::Style, mut f: F) -> i32 {
    let mut dev = match open_device(opts) {
        Ok(d) => d,
        Err(code) => {
            if code == EXIT_NO_DEVICE {
                let us = *st;
                if opts.json {
                    ui::error_json("No configuration interface found.", "device-not-found");
                } else {
                    ui::error(&us, "No configuration interface found.",
                              NO_DEVICE_HINT,
                              "device-not-found", EXIT_NO_DEVICE);
                }
            }
            return code;
        }
    };
    f(&mut dev)
}

/// Open whatever is actually there.
///
/// `--ble` forces Bluetooth and `--device` forces a path, but with neither the
/// right answer is simply "the transport the keyboard is on". The keyboard
/// holds one connection at a time, so falling back from USB to Bluetooth is
/// never ambiguous — and without this a status-bar click fails whenever the
/// cable is out, which is most of the time.
/// What to try when nothing was found.
///
/// Windows is named explicitly because it is the one platform where the answer
/// really is "use the cable".
#[cfg(target_os = "windows")]
const NO_DEVICE_HINT: &str = "Connect the keyboard over USB.";
#[cfg(not(target_os = "windows"))]
const NO_DEVICE_HINT: &str = "Connect it over USB, or pick its Bluetooth channel and pass --ble.";

fn open_device(opts: &Opts) -> std::result::Result<Device, i32> {
    if opts.ble {
        let path = match opts.device.clone().or_else(transport::find_ble) {
            Some(p) => p,
            None => return Err(EXIT_NO_DEVICE),
        };
        return Device::open_ble(&path).map_err(|e| { eprintln!("{}", e); EXIT_TRANSPORT });
    }
    if let Some(p) = opts.device.clone() {
        return Device::open_usb(&p).map_err(|e| {
            eprintln!("{}", e);
            EXIT_TRANSPORT
        });
    }
    if let Some(f) = transport::find_usb().first() {
        return Device::open_usb(&f.path).map_err(|e| {
            eprintln!("{}", e);
            EXIT_TRANSPORT
        });
    }
    // No cable: use the Bluetooth link if one is up. find_ble() answers None on
    // platforms with no Bluetooth backend, so this falls through to "no device".
    if let Some(path) = transport::find_ble() {
        return Device::open_ble(&path).map_err(|e| {
            eprintln!("{}", e);
            EXIT_TRANSPORT
        });
    }
    Err(EXIT_NO_DEVICE)
}

fn cmd_info(dev: &mut Device, opts: &Opts, st: &ui::Style) -> i32 {
    match service::get_device_info(dev) {
        Ok(info) => {
            let rows = service::describe(&info, opts.show_identifiers);
            if opts.json {
                let obj: Vec<(&str, Json)> = vec![
                    ("transport", Json::Str(dev.kind.label().into())),
                    (
                        "fields",
                        Json::Arr(
                            rows.iter()
                                .map(|(k, v)| {
                                    Json::obj(vec![
                                        ("name", Json::Str(k.clone())),
                                        ("value", Json::Str(v.clone())),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ];
                println!("{}", json::to_string_pretty(&Json::obj(obj)));
                return EXIT_OK;
            }
            if !opts.quiet {
                println!("{} {}", st.dim("transport"), st.current(dev.kind.label()));
            }
            for (k, v) in rows {
                println!("{} {}", st.dim(&k), v);
            }
            if !opts.show_identifiers && !opts.quiet {
                println!(
                    "{}",
                    st.dim("identifiers hidden; pass --show-identifiers to include them")
                );
            }
            EXIT_OK
        }
        Err(e) => fail(&e.to_string(), opts, st),
    }
}

fn cmd_get_backlight(dev: &mut Device, file: Option<String>, opts: &Opts, st: &ui::Style) -> i32 {
    match service::get_backlight_json(dev) {
        Ok(doc) => {
            let text = json::to_string_pretty(&doc);
            match file {
                Some(f) => {
                    if let Err(e) = std::fs::write(&f, format!("{}\n", text)) {
                        return fail(&format!("cannot write {}: {}", f, e), opts, st);
                    }
                    if !opts.quiet && !opts.json {
                        let us = *st;
                        ui::say(&us, "SAVED", &f);
                    }
                }
                None => println!("{}", text),
            }
            EXIT_OK
        }
        Err(e) => fail(&e.to_string(), opts, st),
    }
}

fn cmd_set_backlight(dev: &mut Device, file: &str, opts: &Opts, st: &ui::Style) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => return usage_fail(&format!("cannot read {}: {}", file, e), opts, st),
    };
    let doc = match json::parse(&text) {
        Ok(d) => d,
        Err(e) => return usage_fail(&format!("{} is not valid JSON: {}", file, e), opts, st),
    };
    // Validate before opening anything on the device.
    if let Err(e) = backlight::from_json(&doc) {
        return usage_fail(&e, opts, st);
    }
    match service::set_backlight_verified(dev, &doc) {
        Ok(out) => report_write(&out, dev.kind, opts, st),
        Err(e) => fail(&e.to_string(), opts, st),
    }
}

fn report_write(out: &service::WriteOutcome, kind: Kind, opts: &Opts, st: &ui::Style) -> i32 {
    if opts.json {
        let mut pairs: Vec<(&str, Json)> = vec![
            ("stage", Json::Str(out.stage.label().into())),
            ("transport", Json::Str(kind.label().into())),
            ("message", Json::Str(out.message.clone())),
        ];
        if let Some(s) = out.protocol_status {
            pairs.push(("protocol_status", Json::Num(s as f64)));
        }
        if out.stage == Stage::Mismatch {
            if let Some(e) = &out.expected {
                pairs.push(("expected", e.clone()));
            }
            if let Some(a) = &out.actual {
                pairs.push(("actual", a.clone()));
            }
        }
        println!("{}", json::to_string_pretty(&Json::obj(pairs)));
    } else if !opts.quiet {
        let us = *st;
        let zones: Vec<&str> = out
            .expected
            .as_ref()
            .map(|e| match e {
                Json::Obj(m) => m.keys().map(|k| ui::zone_label(k)).collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        ui::say(&us, "SENT", &format!("backlight → CLVX over {}", kind.label().to_uppercase()));
        match out.stage {
            Stage::Verified => {
                ui::say(&us, "READ BACK", &zones.join(", "));
                ui::say(&us, "VERIFIED", "device matches the scheme");
            }
            Stage::Mismatch => {
                ui::say(&us, "READ BACK", &zones.join(", "));
                ui::say(&us, "MISMATCH", &out.message);
            }
            _ => ui::error(&us, &out.message, if vendor_app_holding_it() {
                    "TouchOnKeys is open, and the keyboard takes one app at a time. Quit it, then retry."
                } else {
                    "Retry, or read the device with get-backlight."
                },
                           "write-failed", EXIT_TRANSPORT),
        }
    }
    match out.stage {
        Stage::Verified => EXIT_OK,
        Stage::Mismatch => EXIT_MISMATCH,
        _ => EXIT_TRANSPORT,
    }
}

fn cmd_export(dev: &mut Device, file: &str, opts: &Opts, st: &ui::Style) -> i32 {
    match service::get_settings(dev) {
        Ok(blob) => {
            if let Err(e) = std::fs::write(file, &blob) {
                return fail(&format!("cannot write {}: {}", file, e), opts, st);
            }
            if opts.json {
                println!(
                    "{}",
                    json::to_string_pretty(&Json::obj(vec![
                        ("stage", Json::Str("read_back".into())),
                        ("bytes", Json::Num(blob.len() as f64)),
                        ("file", Json::Str(file.into())),
                    ]))
                );
            } else if !opts.quiet {
                let us = *st;
                ui::say(&us, "BACKED UP", &format!("{} ({} bytes, every setting)", file, blob.len()));
            }
            EXIT_OK
        }
        Err(e) => fail(&e.to_string(), opts, st),
    }
}

fn cmd_import(dev: &mut Device, file: &str, opts: &Opts, st: &ui::Style) -> i32 {
    let blob = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => return usage_fail(&format!("cannot read {}: {}", file, e), opts, st),
    };
    // A restore rewrites more than lighting, so check it before touching the
    // device: it must parse, and it must look like AppSettings.
    if blob.is_empty() || blob.len() > 64 * 1024 {
        return backup_fail(
            &format!("{} is {} bytes; that is not a settings backup", file, blob.len()),
            opts,
            st,
        );
    }
    let parsed = match proto::parse(&blob) {
        Some(p) => p,
        None => return backup_fail(&format!("{} is not a readable backup", file), opts, st),
    };
    if proto::first_bytes(&parsed, backlight::APPSETTINGS_PROFILE).is_none() {
        return backup_fail(
            &format!("{} has no profile section; refusing to restore it", file),
            opts,
            st,
        );
    }
    if !opts.quiet && !opts.json && std::io::stdin().is_terminal() {
        print!(
            "{} restore every setting from {}? this is broader than a colour change [y/N] ",
            st.amber("!"),
            st.bold(file)
        );
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err()
            || !matches!(line.trim(), "y" | "Y" | "yes")
        {
            println!("{}", st.dim("cancelled"));
            return EXIT_OK;
        }
    }
    match service::set_settings(dev, &blob) {
        Ok(status) if status == 0 => {
            if opts.json {
                println!(
                    "{}",
                    json::to_string_pretty(&Json::obj(vec![
                        ("stage", Json::Str("acknowledged".into())),
                        ("protocol_status", Json::Num(0.0)),
                        ("bytes", Json::Num(blob.len() as f64)),
                    ]))
                );
            } else if !opts.quiet {
                let us = *st;
                ui::say(&us, "ACKNOWLEDGED", "the device accepted the backup");
            }
            EXIT_OK
        }
        Ok(status) => fail(&format!("the device rejected the backup (status {})", status), opts, st),
        Err(e) => fail(&e.to_string(), opts, st),
    }
}

/// Is the vendor application holding the keyboard?
///
/// This is worth asking, because the way it fails is so misleading. The
/// keyboard grants one configuration conversation at a time, and while
/// TouchOnKeys has it, **reads keep working and writes are refused** — with a
/// reply that carries no status, which reads like a protocol fault or a broken
/// scheme. An afternoon went into a hexdump before the answer turned out to be
/// another app being open.
///
/// Only asked when something has already gone wrong, so it costs nothing in the
/// normal case.
fn vendor_app_holding_it() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    std::process::Command::new("pgrep")
        .args(["-x", "TouchOnKeys"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

fn fail(msg: &str, opts: &Opts, st: &ui::Style) -> i32 {
    if opts.json {
        ui::error_json(msg, "transport-failure");
    } else {
        let us = *st;
        let next = if vendor_app_holding_it() {
            "TouchOnKeys is open, and the keyboard takes one app at a time. Quit it, then retry."
        } else {
            "Check the keyboard is connected on this transport, then retry."
        };
        ui::error(&us, msg, next, "transport-failure", EXIT_TRANSPORT);
    }
    EXIT_TRANSPORT
}

fn usage_fail(msg: &str, opts: &Opts, st: &ui::Style) -> i32 {
    if opts.json {
        ui::error_json(msg, "invalid-input");
    } else {
        let us = *st;
        ui::error(&us, msg, "Fix the file or the arguments, then retry.", "invalid-input", EXIT_USAGE);
    }
    EXIT_USAGE
}

fn backup_fail(msg: &str, opts: &Opts, st: &ui::Style) -> i32 {
    if opts.json {
        ui::error_json(msg, "backup-invalid");
    } else {
        let us = *st;
        ui::error(&us, msg, "Use a file produced by `clevertuna export`.", "backup-invalid", EXIT_BACKUP);
    }
    EXIT_BACKUP
}

// ─────────────────────────── gallery + themes ────────────────────────────

fn cmd_profile(opts: &Opts, st: &ui::Style, sub: Option<String>, name: Option<String>, third: Option<String>) -> i32 {
    let us = *st;
    match sub.as_deref() {
        Some("list") | None => {
            let items = gallery::list();
            if opts.json {
                println!("{}", json::to_string_pretty(&gallery::to_json(&items)));
                return EXIT_OK;
            }
            if items.is_empty() {
                ui::say(&us, "EMPTY", &format!("no saved profiles yet in {}", gallery::dir().display()));
                return EXIT_OK;
            }
            for e in items {
                let zones: Vec<&str> = e.zones.iter().map(|z| ui::zone_label(z)).collect();
                ui::say(&us, "PROFILE", &format!("{}  ({})", e.name, zones.join(", ")));
            }
            EXIT_OK
        }
        Some("rename") => {
            let from = match name {
                Some(n) => n,
                None => { ui::error(&us, "profile rename needs the current name and the new one", "Try: clevertuna profile rename \"Look 2026-08-23 0715\" \"Reef\"", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
            };
            let to = match third {
                Some(n) => n,
                None => { ui::error(&us, "profile rename needs a new name too", "Try: clevertuna profile rename <old> <new>", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
            };
            match gallery::rename(&from, &to) {
                Ok(p) => { ui::say(&us, "RENAMED", &format!("{} → {}", from, p.display())); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "Pick another name.", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("save") => {
            let n = match name { Some(n) => n, None => { ui::error(&us, "profile save needs a name", "Try: clevertuna profile save \"Deep Sea\"", "invalid-input", EXIT_USAGE); return EXIT_USAGE } };
            // A scheme someone shared is a file, not a keyboard: taking one
            // into the gallery must not need the hardware attached.
            if let Some(src) = opts.from.as_deref() {
                if src != "usb" && src != "ble" {
                    let text = match std::fs::read_to_string(src) {
                        Ok(t) => t,
                        Err(e) => { ui::error(&us, &format!("cannot read {}: {}", src, e), "Check the path.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
                    };
                    let doc = match json::parse(&text) {
                        Ok(d) => d,
                        Err(e) => { ui::error(&us, &format!("{} is not a scheme this tool understands: {}", src, e), "Export one with: clevertuna export scheme.json", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
                    };
                    let doc = match gallery::select_zones(&doc, &opts.only) {
                        Ok(d) => d,
                        Err(e) => { ui::error(&us, &e, "Check the zone names.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
                    };
                    return match gallery::save(&n, &doc) {
                        Ok(p) => { ui::say(&us, "SAVED", &format!("{} → {}", n, p.display())); EXIT_OK }
                        Err(e) => { ui::error(&us, &e, "Pick another name.", "invalid-input", EXIT_USAGE); EXIT_USAGE }
                    };
                }
            }
            with_device(opts, st, |dev| match service::get_backlight_json(dev) {
                Ok(doc) => {
                    let doc = match gallery::select_zones(&doc, &opts.only) {
                        Ok(d) => d,
                        Err(e) => { ui::error(&us, &e, "Check the zone names.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
                    };
                    match gallery::save(&n, &doc) {
                        Ok(p) => { ui::say(&us, "SAVED", &format!("{} → {}", n, p.display())); EXIT_OK }
                        Err(e) => { ui::error(&us, &e, "Pick another name.", "invalid-input", EXIT_USAGE); EXIT_USAGE }
                    }
                }
                Err(e) => fail(&e.to_string(), opts, st),
            })
        }
        Some("apply") => {
            let n = match name { Some(n) => n, None => { ui::error(&us, "profile apply needs a name", "Try: clevertuna profile list", "invalid-input", EXIT_USAGE); return EXIT_USAGE } };
            let doc = match gallery::load(&n) {
                Ok(d) => d,
                Err(e) => { ui::error(&us, &e, "Try: clevertuna profile list", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
            };
            let doc = match gallery::select_zones(&doc, &opts.only) {
                Ok(d) => d,
                Err(e) => { ui::error(&us, &e, "Check the zone names.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
            };
            if opts.dry_run {
                println!("{}", json::to_string_pretty(&doc));
                return EXIT_OK;
            }
            with_device(opts, st, |dev| match service::set_backlight_verified(dev, &doc) {
                Ok(out) => report_write(&out, dev.kind, opts, st),
                Err(e) => fail(&e.to_string(), opts, st),
            })
        }
        Some("delete") => {
            let n = match name { Some(n) => n, None => { ui::error(&us, "profile delete needs a name", "Try: clevertuna profile list", "invalid-input", EXIT_USAGE); return EXIT_USAGE } };
            match gallery::delete(&n) {
                Ok(p) => { ui::say(&us, "DELETED", &format!("{}", p.display())); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "Try: clevertuna profile list", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some(other) => {
            ui::error(&us, &format!("unknown profile command '{}'", other),
                      "Use list, save, apply or delete.", "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

/// Let the clock change the theme.
fn cmd_rotate(opts: &Opts, st: &ui::Style, sub: Option<String>, rest: &[String]) -> i32 {
    let us = *st;
    let mut plan = rotate::load();

    let announce = |plan: &rotate::Plan| {
        ui::say(&us, "ROTATE", &plan.describe());
        if plan.mode != rotate::Mode::Off {
            let n = plan.every.writes_per_day();
            if n > 100 {
                // Applying a theme is a flash write, and flash wears out. This
                // is a decision worth making on purpose.
                ui::say(&us, "NOTE", &format!(
                    "that is about {} writes to the keyboard's flash a day; hourly is {}",
                    n, rotate::Every::Hour.writes_per_day()
                ));
            }
        }
    };

    match sub.as_deref() {
        None | Some("status") => {
            if opts.json {
                // The plan, plus the vocabulary a window needs to offer it —
                // so no other language keeps its own list of cadences.
                let mut v = plan.to_json();
                if let Json::Obj(fields) = &mut v {
                    fields.insert(
                        "cadences".into(),
                        Json::Arr(rotate::Every::all().iter().map(|e| Json::Str(e.key())).collect()),
                    );
                    // Named here too, so a window does not turn "5m" into
                    // "Every 5 minutes" with a formatter of its own.
                    fields.insert(
                        "cadenceLabels".into(),
                        Json::Arr(rotate::Every::all().iter().map(|e| Json::Str(e.label())).collect()),
                    );
                    fields.insert(
                        "modes".into(),
                        Json::Arr(
                            [rotate::Mode::Off, rotate::Mode::Cycle, rotate::Mode::Random, rotate::Mode::DayNight]
                                .iter()
                                .map(|m| Json::Str(m.key().into()))
                                .collect(),
                        ),
                    );
                    fields.insert("describe".into(), Json::Str(plan.describe()));
                    fields.insert(
                        "writesPerDay".into(),
                        Json::Num(plan.every.writes_per_day() as f64),
                    );
                }
                println!("{}", json::to_string_pretty(&v));
                return EXIT_OK;
            }
            announce(&plan);
            if let Some(t) = &plan.last_theme {
                ui::say(&us, "LAST", t);
            }
            EXIT_OK
        }
        Some("off") => {
            plan.mode = rotate::Mode::Off;
            match rotate::save(&plan) {
                Ok(_) => { ui::say(&us, "ROTATE", "off"); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("every") => {
            let every = match rest.first().and_then(|s| rotate::Every::parse(s)) {
                Some(e) => e,
                None => {
                    ui::error(&us, "rotate every needs a cadence",
                              "One of: minute, hour, day, week, month.", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            let picks: Vec<String> = rest[1..].to_vec();
            if picks.is_empty() {
                ui::error(&us, "rotate every needs at least one theme",
                          "Try: clevertuna rotate every hour reef spectrum random", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
            // Refuse a plan that names something that does not exist, rather
            // than discovering it in an hour's time with nobody watching.
            for p in &picks {
                if let Err(e) = resolve_theme(p, opts) {
                    ui::error(&us, &format!("'{}' is not a theme: {}", p, e),
                              "Run: clevertuna theme list", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            }
            plan.mode = if picks.len() == 1 && picks[0] == "random" {
                rotate::Mode::Random
            } else {
                rotate::Mode::Cycle
            };
            plan.every = every;
            plan.picks = picks;
            plan.last_slot = None;
            plan.last_theme = None;
            plan.utc_offset_minutes = local_utc_offset_minutes();
            match rotate::save(&plan) {
                Ok(_) => { announce(&plan); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("slots") => {
            let on = match rest.first().map(|s| s.as_str()) {
                Some("on") | Some("sync") | None => true,
                Some("off") => false,
                Some(other) => {
                    ui::error(&us, &format!("'{}' is not on or off", other), "", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            plan.follow_slots = on;
            match rotate::save(&plan) {
                Ok(_) => {
                    ui::say(&us, "SLOTS", if on {
                        "every slot will be put back to the theme in use"
                    } else {
                        "each slot keeps whatever it was last given"
                    });
                    EXIT_OK
                }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("day-night") => {
            let day = rest.first().cloned().unwrap_or_else(|| "deep-current".into());
            let night = rest.get(1).cloned().unwrap_or_else(|| "nightshift".into());
            for p in [&day, &night] {
                if let Err(e) = resolve_theme(p, opts) {
                    ui::error(&us, &format!("'{}' is not a theme: {}", p, e),
                              "Run: clevertuna theme list", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            }
            plan.mode = rotate::Mode::DayNight;
            plan.day = day;
            plan.night = night;
            plan.day_from = rest.get(2).and_then(|h| h.parse().ok()).unwrap_or(rotate::DAY_FROM).min(23);
            plan.night_from = rest.get(3).and_then(|h| h.parse().ok()).unwrap_or(rotate::NIGHT_FROM).min(23);
            plan.every = rotate::Every::Hour;
            plan.last_slot = None;
            plan.last_theme = None;
            plan.utc_offset_minutes = local_utc_offset_minutes();
            match rotate::save(&plan) {
                Ok(_) => { announce(&plan); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        // The heartbeat. Almost always this decides there is nothing to do and
        // never opens the keyboard — which is what makes it safe to call often.
        Some("tick") => {
            let now = unix_now();

            // Before the clock: has the picture the current theme was built
            // from changed? Following it only while it is actually the theme in
            // use is what stops this fighting whatever you chose afterwards.
            let current = rotate::load_current();
            if current.source == "wallpaper" {
                let now_fp = opts
                    .wallpaper
                    .clone()
                    .map(std::path::PathBuf::from)
                    .or_else(wallpaper::current_wallpaper)
                    .map(|p| rotate::wallpaper_fingerprint(&p))
                    .unwrap_or_default();
                if !now_fp.is_empty() && now_fp != current.wallpaper {
                    return match resolve_theme("wallpaper", opts) {
                        Ok(doc) => {
                            if !opts.quiet && !opts.json {
                                ui::say(&us, "ROTATE", "the desktop picture changed");
                            }
                            apply_source(opts, st, &doc, "wallpaper")
                        }
                        Err(e) => {
                            ui::error(&us, &e, "", "wallpaper-unusable", EXIT_USAGE);
                            EXIT_USAGE
                        }
                    };
                }
            }

            // Reached over a different slot than the theme was written on?
            //
            // The protocol has no slot field; the slot is the connection you
            // arrived on, and each Bluetooth channel is a separate pairing with
            // its own identifier. So this is answerable, and answering it is
            // what makes one theme follow you across the cable and the three
            // channels instead of each keeping whatever it was last given.
            if plan.follow_slots && !current.source.is_empty() {
                let live = transport::live_slot_id();
                if !live.is_empty() && live != current.slot {
                    return match resolve_theme(&current.source, opts) {
                        Ok(doc) => {
                            if !opts.quiet && !opts.json {
                                ui::say(&us, "ROTATE", "a different slot — putting the theme back on");
                            }
                            apply_source(opts, st, &doc, &current.source.clone())
                        }
                        Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
                    };
                }
            }

            let want = match plan.due(now) {
                Some(w) => w,
                None => {
                    if !opts.quiet && !opts.json {
                        ui::say(&us, "ROTATE", "nothing due");
                    }
                    return EXIT_OK;
                }
            };
            let doc = match resolve_theme(&want, opts) {
                Ok(d) => d,
                Err(e) => {
                    ui::error(&us, &format!("rotation wanted '{}': {}", want, e),
                              "Fix the plan with: clevertuna rotate every <cadence> <themes…>",
                              "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            let code = apply_source(opts, st, &doc, &want);
            if code == EXIT_OK {
                // Only remembered once it landed, so a failed tick is retried
                // rather than silently counted as done.
                plan.mark(now, &want);
                let _ = rotate::save(&plan);
                if !opts.quiet && !opts.json {
                    ui::say(&us, "ROTATE", &format!("applied {}", want));
                }
            }
            code
        }
        Some(other) => {
            ui::error(&us, &format!("'{}' is not something rotate does", other),
                      "One of: status, off, every, day-night, slots, tick.", "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Minutes east of UTC, asked of the system once and then stored in the plan.
///
/// There is no timezone database in the standard library and this project has
/// no crates, so the operating system is asked the one question that matters.
/// Storing the answer means a tick never has to.
fn local_utc_offset_minutes() -> i32 {
    let out = if cfg!(target_os = "windows") {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-Date -UFormat %Z00)"])
            .output()
    } else {
        std::process::Command::new("date").arg("+%z").output()
    };
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => return 0,
    };
    // ±HHMM
    let sign = if text.starts_with('-') { -1 } else { 1 };
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return 0;
    }
    let h: i32 = digits[..2].parse().unwrap_or(0);
    let m: i32 = digits[2..4].parse().unwrap_or(0);
    sign * (h * 60 + m)
}

/// The five themes that hold a key.
fn cmd_favourites(opts: &Opts, st: &ui::Style, sub: Option<String>, arg: Option<String>) -> i32 {
    let us = *st;
    let mut f = favourites::load();
    match sub.as_deref() {
        None | Some("list") => {
            if opts.json {
                println!("{}", json::to_string_pretty(&favourites::to_json(&f)));
                return EXIT_OK;
            }
            for (i, theme) in f.iter().enumerate() {
                let what = if theme.is_empty() { us.dim("not set") } else { theme.clone() };
                ui::say(&us, "KEY", &format!("⌃⌥{}  {}", i + 1, what));
            }
            EXIT_OK
        }
        Some("set") => {
            let slot: usize = match arg.as_deref().and_then(|a| a.parse::<usize>().ok()) {
                Some(n) if (1..=favourites::SLOTS).contains(&n) => n - 1,
                _ => {
                    ui::error(&us, &format!("set needs a key number, 1 to {}", favourites::SLOTS),
                              "Try: clevertuna favourites set 1 theme:magma", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            let theme = match opts.from.clone() {
                Some(t) => t,
                None => {
                    ui::error(&us, "set needs a theme too",
                              "Try: clevertuna favourites set 1 --from theme:magma", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            // Refuse a name that does not resolve, rather than binding a key to
            // a mistake that only shows up when it is pressed.
            if let Err(e) = resolve_theme(&theme, opts) {
                ui::error(&us, &format!("'{}' is not a theme: {}", theme, e),
                          "Run: clevertuna theme list", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
            favourites::assign(&mut f, slot, &theme);
            match favourites::save(&f) {
                Ok(_) => { ui::say(&us, "KEY", &format!("⌃⌥{}  {}", slot + 1, theme)); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("clear") => {
            let slot: usize = match arg.as_deref().and_then(|a| a.parse::<usize>().ok()) {
                Some(n) if (1..=favourites::SLOTS).contains(&n) => n - 1,
                _ => {
                    ui::error(&us, "clear needs a key number", "", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            favourites::clear(&mut f, slot);
            match favourites::save(&f) {
                Ok(_) => { ui::say(&us, "KEY", &format!("⌃⌥{}  cleared", slot + 1)); EXIT_OK }
                Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some(other) => {
            ui::error(&us, &format!("'{}' is not something favourites does", other),
                      "One of: list, set, clear.", "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

/// The keyboard as a device: which OS it thinks it is on, and how to put it
/// back to new.
fn cmd_device(opts: &Opts, st: &ui::Style, sub: Option<String>, arg: Option<String>) -> i32 {
    let us = *st;
    match sub.as_deref() {
        Some("os") => {
            let mode = match arg.as_deref().map(|m| m.trim().to_ascii_lowercase()) {
                Some(ref m) if m == "mac" || m == "macos" => service::OS_MAC,
                Some(ref m) if m == "windows" || m == "win" => service::OS_WINDOWS,
                Some(ref m) if m == "linux" => service::OS_LINUX,
                _ => {
                    ui::error(&us, "os needs mac, windows or linux",
                              "It changes what the modifier and media keys do.", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            with_device(opts, st, |dev| match service::set_os_mode(dev, mode) {
                Ok(_) => {
                    ui::say(&us, "OS", arg.as_deref().unwrap_or("set"));
                    EXIT_OK
                }
                Err(e) => fail(&e.to_string(), opts, st),
            })
        }
        // The factory settings, fetched rather than fired.
        //
        // Writing them through the ordinary verified path means "put it back"
        // is the same operation as any other write — it can be previewed, it
        // reports what landed, and a backup taken first restores from it.
        Some("defaults") => with_device(opts, st, |dev| {
            let defaults = match service::get_default_settings(dev) {
                Ok(d) => d,
                Err(e) => return fail(&e.to_string(), opts, st),
            };
            if opts.dry_run || opts.json {
                match backlight::extract(&defaults).and_then(|b| backlight::to_json(&b)) {
                    Some(doc) => println!("{}", json::to_string_pretty(&doc)),
                    None => println!("{{}}"),
                }
                return EXIT_OK;
            }
            if arg.as_deref() != Some("apply") {
                let t = power::read(&defaults).unwrap_or_default();
                ui::say(&us, "DEFAULTS", &format!("{} bytes — {}", defaults.len(), t.describe()));
                ui::say(&us, "NEXT", "clevertuna device defaults apply — after: clevertuna export backup.clvx");
                return EXIT_OK;
            }
            match service::set_settings(dev, &defaults) {
                Ok(0) => { ui::say(&us, "RESTORED", "the keyboard is back to its factory settings"); EXIT_OK }
                Ok(status) => fail(&format!("the device refused the defaults (status {})", status), opts, st),
                Err(e) => fail(&e.to_string(), opts, st),
            }
        }),
        // Keeps everything; only rebuilds the link. The thing to try first
        // when the keyboard answers reads and refuses writes.
        Some("restart") => with_device(opts, st, |dev| match service::perform_restart(dev) {
            Ok(_) => { ui::say(&us, "RESTART", "asked the keyboard to reboot — give it a few seconds"); EXIT_OK }
            Err(e) => fail(&e.to_string(), opts, st),
        }),
        // The blunt one. Only ever on an explicit word, because unlike every
        // other action here it cannot be undone by the next click.
        Some("reset") => {
            if arg.as_deref() != Some("--yes") {
                ui::error(&us, "a full reset erases everything on the keyboard",
                          "Back up first: clevertuna export backup.clvx — then: clevertuna device reset --yes",
                          "confirm-required", EXIT_USAGE);
                return EXIT_USAGE;
            }
            with_device(opts, st, |dev| match service::perform_full_reset(dev) {
                Ok(_) => { ui::say(&us, "RESET", "the keyboard has been returned to factory settings"); EXIT_OK }
                Err(e) => fail(&e.to_string(), opts, st),
            })
        }
        _ => {
            ui::error(&us, "device needs os, defaults or reset",
                      "clevertuna device os mac · device defaults · device restart · device reset --yes",
                      "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

/// Everything about the keyboard that is not a colour.
/// What each function key sends, and setting it.
///
/// Writes go through the same read → modify → `SET_SETTINGS` → read back →
/// compare that every other setting uses, because remapping is an ordinary
/// field of the same message and deserves neither more ceremony nor less.
fn cmd_keys(opts: &Opts, st: &ui::Style, which: Option<String>, action: Option<String>) -> i32 {
    let us = *st;
    with_device(opts, st, |dev| {
        let blob = match service::get_settings(dev) {
            Ok(s) => s,
            Err(e) => return fail(&e.to_string(), opts, st),
        };

        let map = keymap::read(&blob);
        if map.is_empty() {
            ui::error(&us, "this keyboard has no remappable function row",
                      "Only some models in the family carry one.", "invalid-input", EXIT_USAGE);
            return EXIT_USAGE;
        }

        let (which, action) = match (&which, &action) {
            (Some(k), Some(a)) => (k.clone(), a.clone()),
            _ => {
                if opts.json {
                    println!("{}", json::to_string_pretty(&keymap::model(&blob)));
                    return EXIT_OK;
                }
                for (n, b) in &map {
                    ui::say(&us, "KEY", &format!("{:<5} {}", format!("F{}", n), b.label()));
                }
                ui::say(&us, "NEXT", &us.dim("clevertuna keys f5 mute — or `nothing` to clear"));
                return EXIT_OK;
            }
        };

        let n: u32 = match which.trim_start_matches(['f', 'F']).parse() {
            Ok(n) if (1..=12).contains(&n) => n,
            _ => {
                ui::error(&us, &format!("'{}' is not a function key", which),
                          "One of f1 … f12.", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
        };
        let wanted = match keymap::action(&action) {
            Some(a) => a,
            None => {
                let names: Vec<&str> = keymap::ACTIONS.iter().map(|a| a.id).collect();
                ui::error(&us, &format!("'{}' is not something a key can do", action),
                          &format!("One of: {}", names.join(", ")), "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
        };

        let updated = match keymap::write(&blob, n, &wanted.binding()) {
            Ok(u) => u,
            Err(e) => return fail(&e, opts, st),
        };
        if opts.dry_run {
            ui::say(&us, "DRY RUN", &format!("F{} would become {}", n, wanted.name));
            return EXIT_OK;
        }
        match service::set_settings(dev, &updated) {
            Ok(0) => {}
            Ok(status) => return fail(&format!("the device did not accept it (status {})", status), opts, st),
            Err(e) => return fail(&e.to_string(), opts, st),
        }
        std::thread::sleep(transport::SETTLE);
        // Read back and compare: done means the keyboard agrees.
        match service::get_settings(dev) {
            Ok(after) => {
                let got = keymap::read(&after).into_iter().find(|(k, _)| *k == n);
                match got {
                    Some((_, b)) if b.slots == wanted.binding().slots => {
                        ui::say(&us, "VERIFIED", &format!("F{} is now {}", n, wanted.name));
                        EXIT_OK
                    }
                    Some((_, b)) => {
                        ui::error(&us, &format!("F{} reads back as {}", n, b.label()),
                                  "The keyboard took the write and stored something else.",
                                  "mismatch", EXIT_TRANSPORT);
                        EXIT_TRANSPORT
                    }
                    None => fail("the function row vanished after the write", opts, st),
                }
            }
            Err(e) => fail(&e.to_string(), opts, st),
        }
    })
}

fn cmd_settings(opts: &Opts, st: &ui::Style, key: Option<String>, value: Option<String>) -> i32 {
    let us = *st;
    with_device(opts, st, |dev| {
        let blob = match service::get_settings(dev) {
            Ok(s) => s,
            Err(e) => return fail(&e.to_string(), opts, st),
        };

        let (key, value) = match (&key, &value) {
            (Some(k), Some(v)) => (k.clone(), v.clone()),
            _ => {
                // Nothing named: show everything.
                if opts.json {
                    println!("{}", json::to_string_pretty(&settings::model(&blob)));
                    return EXIT_OK;
                }
                for group in settings::groups() {
                    ui::say(&us, "GROUP", &us.bold(group));
                    for s in settings::all().iter().filter(|s| s.group == group) {
                        let shown = match settings::read_one(&blob, s) {
                            Some(v) => settings::describe(s, v),
                            None => us.dim("not on this keyboard"),
                        };
                        ui::say(&us, "SET", &format!("{:<26} {:<22} {}", s.key, shown, us.dim(s.label)));
                    }
                }
                return EXIT_OK;
            }
        };

        let setting = match settings::find(&key) {
            Some(s) => s,
            None => {
                ui::error(&us, &format!("'{}' is not a setting", key),
                          "Run: clevertuna settings", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
        };
        let wanted = match settings::parse_value(&setting, &value) {
            Some(v) => v,
            None => {
                ui::error(&us, &format!("'{}' is not a value for {}", value, key),
                          "Run: clevertuna settings", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
        };
        if settings::read_one(&blob, &setting).is_none() {
            ui::error(&us, &format!("this keyboard does not carry '{}'", key),
                      "Run: clevertuna settings to see what it does carry.", "invalid-input", EXIT_USAGE);
            return EXIT_USAGE;
        }

        let mut updated = match settings::write_one(&blob, &setting, wanted) {
            Ok(u) => u,
            Err(e) => return fail(&e, opts, st),
        };
        // The two timeouts constrain each other, and the rule belongs to the
        // pair rather than to either one.
        if setting.key == "backlight-timeout" || setting.key == "idle-timeout" {
            if let Some(t) = power::read(&updated) {
                let fixed = t.coherent();
                if fixed != t {
                    updated = match power::write(&updated, fixed) {
                        Ok(u) => u,
                        Err(e) => return fail(&e, opts, st),
                    };
                }
            }
        }

        match service::set_settings(dev, &updated) {
            Ok(0) => {}
            Ok(status) => return fail(&format!("the device did not accept it (status {})", status), opts, st),
            Err(e) => return fail(&e.to_string(), opts, st),
        }
        std::thread::sleep(transport::SETTLE);
        // Read back, because that is what done means here as everywhere else.
        match service::get_settings(dev).ok().and_then(|s| settings::read_one(&s, &setting).map(|v| (s, v))) {
            Some((_, got)) if got == settings::coerce(&setting, wanted) => {
                ui::say(&us, "SET", &format!("{} → {}", setting.key, settings::describe(&setting, got)));
                EXIT_OK
            }
            Some((_, got)) => {
                ui::say(&us, "SET", &format!("{} → {}", setting.key, settings::describe(&setting, got)));
                ui::error(&us, "the keyboard kept a different value", "", "mismatch", EXIT_MISMATCH);
                EXIT_MISMATCH
            }
            None => fail("could not read the setting back", opts, st),
        }
    })
}

/// When the light goes out.
fn cmd_timeout(opts: &Opts, st: &ui::Style, arg: Option<String>, second: Option<String>) -> i32 {
    let us = *st;
    let show = |t: power::Timeouts, opts: &Opts| {
        if opts.json {
            println!("{}", json::to_string_pretty(&Json::obj(vec![
                ("idle", Json::Num(t.idle as f64)),
                ("backlight", Json::Num(t.backlight as f64)),
                ("describe", Json::Str(t.describe())),
                ("idleChoices", Json::Arr(power::IDLE_CHOICES.iter().map(|c| Json::Num(*c as f64)).collect())),
                ("backlightChoices", Json::Arr(power::BACKLIGHT_CHOICES.iter().map(|c| Json::Num(*c as f64)).collect())),
            ])));
        } else {
            ui::say(&us, "BACKLIGHT", &format!("off after {}", power::describe(t.backlight, "never — always on")));
            ui::say(&us, "IDLE", &format!("dims after {}", power::describe(t.idle, "never")));
        }
    };

    let wanted = arg.as_deref().map(|a| power::parse_seconds(a));
    if let Some(None) = wanted {
        ui::error(&us, &format!("'{}' is not a duration", arg.unwrap_or_default()),
                  "Try: off, 5m, 30m, 1h — or the number of seconds.", "invalid-input", EXIT_USAGE);
        return EXIT_USAGE;
    }

    with_device(opts, st, |dev| {
        let settings = match service::get_settings(dev) {
            Ok(s) => s,
            Err(e) => return fail(&e.to_string(), opts, st),
        };
        let current = power::read(&settings).unwrap_or_default();
        let backlight = match wanted {
            Some(Some(v)) => v,
            _ => return { show(current, opts); EXIT_OK },
        };
        let idle = second
            .as_deref()
            .and_then(power::parse_seconds)
            .unwrap_or(current.idle);
        let want = power::Timeouts { idle, backlight }.coherent();
        if want == current {
            show(current, opts);
            return EXIT_OK;
        }
        let spliced = match power::write(&settings, want) {
            Ok(s) => s,
            Err(e) => return fail(&e, opts, st),
        };
        match service::set_settings(dev, &spliced) {
            Ok(0) => {}
            Ok(status) => return fail(&format!("the device did not accept it (status {})", status), opts, st),
            Err(e) => return fail(&e.to_string(), opts, st),
        }
        // Read back, because that is what "done" means here as everywhere else.
        std::thread::sleep(transport::SETTLE);
        match service::get_settings(dev).ok().and_then(|s| power::read(&s)) {
            Some(after) if after == want => { show(after, opts); EXIT_OK }
            Some(after) => { show(after, opts); ui::error(&us, "the keyboard kept different values", "", "mismatch", EXIT_MISMATCH); EXIT_MISMATCH }
            None => fail("could not read the settings back", opts, st),
        }
    })
}

/// A scheme built from the desktop picture, or the reason there is not one.
///
/// Factored out of the command so rotation can ask for the same thing without
/// going through a terminal interface it does not have.
fn wallpaper_doc(opts: &Opts) -> Result<(std::path::PathBuf, Json), String> {
    let path = opts
        .wallpaper
        .clone()
        .map(std::path::PathBuf::from)
        .or_else(wallpaper::current_wallpaper)
        .ok_or("could not find the current wallpaper")?;
    let img = wallpaper::load_image(&path)?;
    let zones: Vec<String> = if opts.only.is_empty() {
        backlight::ZONES.iter().map(|(n, _)| n.to_string()).collect()
    } else {
        opts.only
            .iter()
            .map(|z| gallery::canonical_zone(z).map(|s| s.to_string()).ok_or_else(|| format!("'{}' is not a zone", z)))
            .collect::<Result<Vec<_>, _>>()?
    };
    let doc = wallpaper::scheme_from_image(&img, &zones)?;
    Ok((path, doc))
}

/// Turn a theme reference into a scheme.
///
/// One vocabulary for every surface that names a theme — the menu, the
/// rotation plan, the builder: a preset id, a saved name, `wallpaper`, or
/// `random` (optionally `random:<n>` to name the roll).
fn resolve_theme(name: &str, opts: &Opts) -> Result<Json, String> {
    // The menu and the "what is on right now" record both spell a theme with
    // its kind in front — `theme:reef`, `profile:Mine`. Understanding those
    // here is what makes one vocabulary rather than two that must agree.
    if let Some(id) = name.strip_prefix("theme:") {
        return themes::find(id)
            .map(|p| p.scheme().to_doc())
            .ok_or_else(|| format!("no theme called '{}'", id));
    }
    if let Some(id) = name.strip_prefix("profile:") {
        return gallery::load(id);
    }
    if name == "random" {
        return Ok(themes::random_scheme(themes::seed_now()).1.to_doc());
    }
    if let Some(seed) = name.strip_prefix("random:") {
        let seed = seed.parse::<u64>().map_err(|_| format!("'{}' is not a roll", name))?;
        return Ok(themes::random_scheme(seed).1.to_doc());
    }
    if name == "wallpaper" || name == "match-wallpaper" {
        return wallpaper_doc(opts).map(|(_, d)| d);
    }
    if let Some(p) = themes::find(name) {
        return Ok(p.scheme().to_doc());
    }
    gallery::load(name)
}

fn cmd_match_wallpaper(opts: &Opts, st: &ui::Style) -> i32 {
    let us = *st;
    let path = match opts.wallpaper.clone().map(std::path::PathBuf::from).or_else(wallpaper::current_wallpaper) {
        Some(p) => p,
        None => {
            ui::error(&us, "could not find the current wallpaper",
                      "Pass one explicitly with --wallpaper <file>.", "wallpaper-not-found", EXIT_USAGE);
            return EXIT_USAGE;
        }
    };
    let img = match wallpaper::load_image(&path) {
        Ok(i) => i,
        Err(e) => { ui::error(&us, &e, "Try --wallpaper with a PNG.", "wallpaper-unreadable", EXIT_USAGE); return EXIT_USAGE }
    };
    let zones: Vec<String> = if opts.only.is_empty() {
        backlight::ZONES.iter().map(|(n, _)| n.to_string()).collect()
    } else {
        match opts.only.iter().map(|z| gallery::canonical_zone(z).map(|s| s.to_string())
                .ok_or_else(|| format!("'{}' is not a zone", z))).collect::<Result<Vec<_>, _>>() {
            Ok(v) => v,
            Err(e) => { ui::error(&us, &e, "Use keyboard, touchpad, left-slider or right-slider.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
        }
    };
    let doc = match wallpaper::scheme_from_image(&img, &zones) {
        Ok(d) => d,
        Err(e) => { ui::error(&us, &e, "Try a more colourful wallpaper.", "wallpaper-unusable", EXIT_USAGE); return EXIT_USAGE }
    };
    if !opts.quiet && !opts.json {
        let stops: Vec<String> = doc.get("backlight").and_then(|b| b.get(&zones[0]))
            .and_then(|z| z.get("colorWave"))
            .and_then(|w| w.get("colorLinePicker"))
            .and_then(|p| p.get("markersArray"))
            .and_then(|a| a.as_array())
            .map(|a| a.iter().map(|m| ui::hex(m.get("color").unwrap_or(&Json::Null))).collect())
            .unwrap_or_default();
        // The last stop repeats the first to close the loop; reporting it would
        // read as a duplicate rather than as the palette that was found.
        let mut stops: Vec<String> = stops;
        if stops.len() > 1 && stops.first() == stops.last() {
            stops.pop();
        }
        ui::say(&us, "MATCHED", &format!("{}  →  {}", path.display(), stops.join("  ")));
    }
    if opts.dry_run || opts.json {
        println!("{}", json::to_string_pretty(&doc));
        return EXIT_OK;
    }
    apply_source(opts, st, &doc, "wallpaper")
}

fn cmd_copy(opts: &Opts, st: &ui::Style) -> i32 {
    let us = *st;
    let parse = |s: &Option<String>| -> Option<bool> {
        match s.as_deref().map(|x| x.trim().to_ascii_lowercase()) {
            Some(ref v) if v == "usb" => Some(false),
            Some(ref v) if v == "ble" || v == "bluetooth" => Some(true),
            _ => None,
        }
    };
    let (from_ble, to_ble) = match (parse(&opts.from), parse(&opts.to)) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            ui::error(&us, "copy needs --from and --to", "Both must be usb or ble.", "invalid-input", EXIT_USAGE);
            return EXIT_USAGE;
        }
    };
    if from_ble == to_ble {
        ui::error(&us, "--from and --to are the same transport",
                  "The keyboard holds one connection at a time; copy moves a scheme between slots.",
                  "invalid-input", EXIT_USAGE);
        return EXIT_USAGE;
    }
    // Read from the source transport.
    let mut src_opts = Opts { ble: from_ble, ..clone_opts(opts) };
    src_opts.ble = from_ble;
    let doc = {
        let mut got: Option<Json> = None;
        let code = with_device(&src_opts, st, |dev| match service::get_backlight_json(dev) {
            Ok(d) => { got = Some(d); EXIT_OK }
            Err(e) => fail(&e.to_string(), &src_opts, st),
        });
        if code != EXIT_OK { return code }
        match got { Some(d) => d, None => return EXIT_TRANSPORT }
    };
    let doc = match gallery::select_zones(&doc, &opts.only) {
        Ok(d) => d,
        Err(e) => { ui::error(&us, &e, "Check the zone names.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
    };
    ui::say(&us, "READ BACK", &format!("scheme from {}", if from_ble { "BLUETOOTH" } else { "USB" }));
    if opts.dry_run {
        println!("{}", json::to_string_pretty(&doc));
        return EXIT_OK;
    }
    ui::say(&us, "NEXT", &format!(
        "switch the keyboard to the {} slot, then press enter",
        if to_ble { "Bluetooth" } else { "USB" }));
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let dst_opts = Opts { ble: to_ble, ..clone_opts(opts) };
    with_device(&dst_opts, st, |dev| match service::set_backlight_verified(dev, &doc) {
        Ok(out) => report_write(&out, dev.kind, opts, st),
        Err(e) => fail(&e.to_string(), opts, st),
    })
}

/// Copy the options for the other transport.
///
/// The device path is deliberately dropped: a clone exists to open a *different*
/// transport, so carrying the old path over would defeat the point. Everything
/// else follows the original, which is why this uses `..` — listing the fields
/// by hand meant every new option had to be remembered here too.
fn clone_opts(o: &Opts) -> Opts {
    Opts { device: None, ..o.clone() }
}

fn cmd_menu(opts: &Opts) -> i32 {
    let t = menu::detect_transport();
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "clevertuna".into());
    let out = match opts.format.as_str() {
        "waybar" => menu::render_waybar(t.as_deref(), gallery::list().len()),
        "swiftbar" | "xbar" => menu::render_swiftbar(t.as_deref(), &exe),
        "json" => menu::render_json(t.as_deref()),
        _ => menu::render_picker(t.as_deref()),
    };
    println!("{}", out);
    EXIT_OK
}

/// Write a scheme, prove it landed, and remember what it was.
///
/// Only a theme that landed is recorded, and only its *name* — enough to answer
/// "is the wallpaper theme still the one in use?" the next time the desktop
/// picture changes, and to stop following it the moment something else is
/// chosen.
fn apply_source(opts: &Opts, st: &ui::Style, doc: &Json, source: &str) -> i32 {
    let mut slot = String::new();
    let code = with_device(opts, st, |dev| {
        slot = dev.slot_id();
        match service::set_backlight_verified(dev, doc) {
            Ok(out) => report_write(&out, dev.kind, opts, st),
            Err(e) => fail(&e.to_string(), opts, st),
        }
    });
    if code == EXIT_OK {
        let wallpaper = if source == "wallpaper" {
            opts.wallpaper
                .clone()
                .map(std::path::PathBuf::from)
                .or_else(wallpaper::current_wallpaper)
                .map(|p| rotate::wallpaper_fingerprint(&p))
                .unwrap_or_default()
        } else {
            String::new()
        };
        rotate::save_current(&rotate::Current { source: source.to_string(), wallpaper, slot });
    }
    code
}

/// Run one menu action. The bar only ever passes an id, and a bar click has no
/// terminal, so nothing here prompts.
fn cmd_do(opts: &Opts, st: &ui::Style, id: &str) -> i32 {
    let us = *st;

    // A saved scheme. `apply:` is the spelling earlier versions wrote into
    // bars; it still works, because a bar config outlives a release.
    if let Some(name) = id.strip_prefix("profile:").or_else(|| id.strip_prefix("apply:")) {
        let doc = match gallery::load(name) {
            Ok(d) => d,
            Err(e) => { ui::error(&us, &e, "Try: clevertuna profile list", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
        };
        return apply_source(opts, st, &doc, &format!("profile:{}", name));
    }

    // A theme that ships with the tool.
    if let Some(theme_id) = id.strip_prefix("theme:") {
        let preset = match themes::find(theme_id) {
            Some(p) => p,
            None => {
                ui::error(&us, &format!("no theme called '{}'", theme_id),
                          "Run: clevertuna theme list", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
        };
        if !opts.quiet && !opts.json {
            ui::say(&us, "THEME", &format!("{} — {}", preset.name, preset.blurb));
        }
        return apply_source(opts, st, &preset.scheme().to_doc(), &format!("theme:{}", theme_id));
    }

    match id {
        "match-wallpaper" => cmd_match_wallpaper(opts, st),
        "random" => cmd_random(opts, st, None),
        "builder" => cmd_builder(opts, st),
        // Handled inside the menu-bar app, which opens its own window. From a
        // terminal the gallery commands are the same job with a keyboard.
        // Both open a window in the menu-bar app; from a terminal the same
        // commands do the same job.
        "timeout" => cmd_timeout(opts, st, None, None),
        "settings" => cmd_settings(opts, st, None, None),
        "open-app" => match gallery::open_vendor_app() {
            Ok(_) => { ui::say(&us, "OPENED", "TouchOnKeys"); EXIT_OK }
            Err(e) => { ui::error(&us, &e, "Install the vendor app.", "app-not-found", EXIT_USAGE); EXIT_USAGE }
        },
        "save" => {
            let name = format!("Look {}", now_stamp());
            with_device(opts, st, |dev| match service::get_backlight_json(dev) {
                Ok(doc) => match gallery::save(&name, &doc) {
                    Ok(p) => { ui::say(&us, "SAVED", &format!("{} → {}", name, p.display())); EXIT_OK }
                    Err(e) => { ui::error(&us, &e, "", "invalid-input", EXIT_USAGE); EXIT_USAGE }
                },
                Err(e) => fail(&e.to_string(), opts, st),
            })
        }
        "export" => {
            let file = gallery::export_dir().join(format!("clevertuna-{}.clvx", now_stamp()));
            if let Some(parent) = file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let file = file.to_string_lossy().to_string();
            with_device(opts, st, |dev| cmd_export(dev, &file, opts, st))
        }
        other => {
            ui::error(&us, &format!("unknown action '{}'", other),
                      "Run: clevertuna menu --format json", "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

/// The controls a visual builder edits, and the way back.
///
/// The builder is a window in another language, and the one thing it must not
/// do is learn the arithmetic: the moment Swift decides for itself what "speed"
/// means, there are two answers to a question with one. So it reads this flat
/// model, moves sliders in it, and hands the same shape back to be encoded
/// here.
///
///   clevertuna look                 what the keyboard is doing now
///   clevertuna look default         a starting point, with no keyboard needed
///   clevertuna look random          a rolled one, printed rather than applied
///   clevertuna look apply <file>    encode a model and write it, verified
///   clevertuna look preview <file>  the scheme it would write, and no write
fn cmd_look(opts: &Opts, st: &ui::Style, sub: Option<String>, arg: Option<String>) -> i32 {
    let us = *st;
    let print = |scheme: &effects::Scheme| {
        println!("{}", json::to_string_pretty(&scheme.to_model()));
        EXIT_OK
    };
    let read_model = |path: &str| -> Result<effects::Scheme, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        let v = json::parse(&text).map_err(|e| format!("{} is not a model this tool understands: {}", path, e))?;
        Ok(effects::Scheme::from_model(&v))
    };

    match sub.as_deref() {
        None => {
            let mut got: Option<Json> = None;
            let code = with_device(opts, st, |dev| match service::get_backlight_json(dev) {
                Ok(d) => { got = Some(d); EXIT_OK }
                Err(e) => fail(&e.to_string(), opts, st),
            });
            if code != EXIT_OK {
                return code;
            }
            match got {
                Some(d) => print(&effects::Scheme::from_doc(&d)),
                None => EXIT_TRANSPORT,
            }
        }
        Some("default") => print(&themes::find("deep-current").expect("shipped").scheme()),
        Some("random") => {
            let seed = opts.seed.unwrap_or_else(themes::seed_now);
            print(&themes::random_scheme(seed).1)
        }
        // Hand a model back with everything derived from it recomputed: the
        // swatch, and the resolved colours a preview paints. A builder that
        // worked those out for itself would be interpolating gradients in a
        // second language.
        Some("reread") => {
            let path = match arg {
                Some(p) => p,
                None => {
                    ui::error(&us, "look reread needs a file", "", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            match read_model(&path) {
                Ok(s) => print(&s),
                Err(e) => { ui::error(&us, &e, "Check the path.", "invalid-input", EXIT_USAGE); EXIT_USAGE }
            }
        }
        Some("preview") | Some("apply") => {
            let path = match arg {
                Some(p) => p,
                None => {
                    ui::error(&us, "look apply needs a file", "Write the model to a file first.", "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            let scheme = match read_model(&path) {
                Ok(s) => s,
                Err(e) => { ui::error(&us, &e, "Check the path.", "invalid-input", EXIT_USAGE); return EXIT_USAGE }
            };
            let doc = scheme.to_doc();
            // Validate before touching the keyboard: a builder should hear
            // "that palette is too long" rather than "the write failed".
            if let Err(e) = backlight::from_json(&doc) {
                ui::error(&us, &e, "Adjust the theme and try again.", "invalid-input", EXIT_USAGE);
                return EXIT_USAGE;
            }
            if sub.as_deref() == Some("preview") || opts.dry_run {
                println!("{}", json::to_string_pretty(&doc));
                return EXIT_OK;
            }
            apply_source(opts, st, &doc, "builder")
        }
        // What a named source would look like, without putting it on the
        // keyboard. A picker that shows a theme as a row of swatches is asking
        // a person to imagine the animation; this is how it can show it
        // instead. It reaches nothing: a theme we ship is compiled in and a
        // saved one is a file, so this answers with the keyboard unplugged.
        Some("of") => {
            let name = match arg {
                Some(n) => n,
                None => {
                    ui::error(&us, "look of needs a theme",
                              "A preset id, a saved name, wallpaper, or random.",
                              "invalid-input", EXIT_USAGE);
                    return EXIT_USAGE;
                }
            };
            match resolve_theme(&name, opts) {
                Ok(doc) => print(&effects::Scheme::from_doc(&doc)),
                Err(e) => {
                    ui::error(&us, &e, "Try `theme list`.", "invalid-input", EXIT_USAGE);
                    EXIT_USAGE
                }
            }
        }
        Some(other) => {
            ui::error(&us, &format!("'{}' is not something look does", other),
                      "One of: default, random, of, reread, preview, apply.", "invalid-input", EXIT_USAGE);
            EXIT_USAGE
        }
    }
}

/// Open the visual theme builder.
///
/// The builder is a window, and the only window this project ships is the macOS
/// app — so the menu offers this row only where it can open, and the command
/// says where the builder is rather than failing silently.
#[cfg(target_os = "macos")]
fn cmd_builder(opts: &Opts, st: &ui::Style) -> i32 {
    let us = *st;
    let _ = opts;
    // By bundle id first, so it works wherever the app was installed; by name
    // second, for a build sitting in `dist/` that Launch Services has seen.
    // The URL first: macOS hands it to the instance already in the menu bar,
    // which is where the app almost always is. `open --args` cannot do this —
    // launch arguments are dropped for an app that is already running, so the
    // command looked like it worked and opened nothing.
    let attempts: [(&str, Vec<&str>); 3] = [
        ("open", vec!["clevertuna://builder"]),
        ("open", vec!["-b", "tech.hartle.clevertuna", "--args", "--builder"]),
        ("open", vec!["-a", "Clevertuna", "--args", "--builder"]),
    ];
    for (cmd, args) in attempts {
        if let Ok(status) = std::process::Command::new(cmd).args(&args).status() {
            if status.success() {
                ui::say(&us, "OPENED", "the theme builder");
                return EXIT_OK;
            }
        }
    }
    ui::error(&us, "could not open Clevertuna.app",
              "Build it with scripts/build-macos-app.sh, then open it once.",
              "app-not-found", EXIT_USAGE);
    EXIT_USAGE
}

#[cfg(not(target_os = "macos"))]
fn cmd_builder(opts: &Opts, st: &ui::Style) -> i32 {
    let _ = opts;
    let us = *st;
    ui::error(&us, "the visual builder is part of the macOS app",
              "On this platform, build a theme with: clevertuna tui",
              "not-available", EXIT_USAGE);
    EXIT_USAGE
}

/// Roll a theme and put it on.
fn cmd_random(opts: &Opts, st: &ui::Style, seed: Option<u64>) -> i32 {
    let us = *st;
    let seed = seed.unwrap_or_else(themes::seed_now);
    let (name, scheme) = themes::random_scheme(seed);
    let doc = scheme.to_doc();
    if opts.json || opts.dry_run {
        println!("{}", json::to_string_pretty(&doc));
        return EXIT_OK;
    }
    if !opts.quiet {
        let swatch: Vec<String> = scheme.keyboard.palette().iter().map(|c| ui::block(&us, *c)).collect();
        ui::say(&us, "ROLLED", &format!("{}  {}", name, swatch.join("")));
        ui::say(&us, "SEED", &format!("{} — repeat with: clevertuna random --seed {}", seed, seed));
    }
    apply_source(opts, st, &doc, "random")
}

/// The built-in themes, as a list or as one applied scheme.
fn cmd_theme(opts: &Opts, st: &ui::Style, arg: Option<String>) -> i32 {
    let us = *st;
    let name = match arg.as_deref() {
        None | Some("list") => {
            if opts.json {
                let arr: Vec<Json> = themes::all()
                    .iter()
                    .map(|p| {
                        Json::obj(vec![
                            ("id", Json::Str(p.id.into())),
                            ("name", Json::Str(p.name.into())),
                            ("group", Json::Str(p.group.label().into())),
                            ("blurb", Json::Str(p.blurb.into())),
                            ("colors", Json::Arr(p.swatch().into_iter().map(Json::Str).collect())),
                        ])
                    })
                    .collect();
                println!("{}", json::to_string_pretty(&Json::Arr(arr)));
                return EXIT_OK;
            }
            for group in themes::Group::all() {
                ui::say(&us, "GROUP", &us.bold(group.label()));
                for p in themes::in_group(group) {
                    let swatch: Vec<String> = p
                        .scheme()
                        .keyboard
                        .palette()
                        .iter()
                        .map(|c| ui::block(&us, *c))
                        .collect();
                    ui::say(&us, "THEME", &format!("{:<16} {}  {}", p.id, swatch.join(""), us.dim(p.blurb)));
                }
            }
            return EXIT_OK;
        }
        Some(n) => n.to_string(),
    };
    cmd_do(opts, st, &format!("theme:{}", name))
}

/// A timestamp a person can read, from the clock alone.
///
/// The obvious arithmetic — days since the epoch, printed as a number — gives
/// names like `20688-0515`, which say nothing and sort strangely once the day
/// count rolls a digit. This is the civil-from-days conversion, which is a
/// dozen lines and gives `2026-08-23 0715`.
fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Howard Hinnant's civil-from-days, which is the standard way to do this
    // without a calendar library: shift the epoch to March 1st so the leap day
    // lands at the end of the year and every month has a fixed length.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // 0..=146096
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{:04}-{:02}-{:02} {:02}{:02}", y, m, d, rem / 3600, (rem % 3600) / 60)
}
