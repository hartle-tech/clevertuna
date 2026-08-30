<div align="center">

<img src="assets/brand/clevertuna-lockup.svg" alt="Clevertuna" width="380">

### Read the current.

A rather cultivated configurator for **Clevetura CLVX** keyboards.<br>
One self-contained binary. No daemon, no account, no telemetry.

<p>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/Apache--2.0-00C8FF?style=for-the-badge&labelColor=07101F"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-00C8FF?style=for-the-badge&logo=rust&logoColor=white&labelColor=07101F">
  <img alt="Dependencies" src="https://img.shields.io/badge/dependencies-zero-36F0B1?style=for-the-badge&labelColor=07101F">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-x86__64-FFB100?style=for-the-badge&logo=linux&logoColor=white&labelColor=07101F">
</p>

</div>

---

Your colour scheme lives in the keyboard, and the vendor app has no export, no
import, and no way to hand it to anyone else. Worse, it **overwrites its own
config from the keyboard every time it connects**, so copying its settings file
between machines achieves precisely nothing.

Clevertuna makes the scheme a file.

```bash
clevertuna get-backlight mine.json     # take what the keyboard has
#  … send mine.json to a friend …
clevertuna set-backlight mine.json     # and it looks like yours
```

## Install

Grab a release binary, or build it — there is nothing to install alongside it.

```bash
cargo build --release          # target/release/clevertuna
```

Zero dependencies, on purpose: a tiny protobuf codec, base64, CRC-32, JSON and
argument parsing are all small enough to own, and the payoff is a binary that
builds offline and has no supply chain to audit.

### Permissions

Run it with `sudo`, or grant your own session access once:

```
# /etc/udev/rules.d/99-clevetura.rules
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", KERNELS=="*:36F7:*", \
    TAG+="uaccess", MODE="0660", GROUP="input"
```

`KERNELS=="*:36F7:*"` matches the HID name `BUS:VID:PID.N`, so it covers USB and
Bluetooth alike. The vendor's own script keys on `ATTRS{idVendor}`, which only
ever matches USB, and adds you to `plugdev`, which many distributions do not
have.

### Save a look, or back the keyboard up?

They sound alike and are not, so each says which it is:

| | What it writes | What it is for |
|---|---|---|
| **Save This Look** | the lighting only, named, into your gallery | picking it again from Yours, or sending it to someone |
| **Export a Backup** | **every** setting, verbatim — gestures, touch zones, key maps, the lot | `clevertuna import` before you experiment |

A saved theme is a small file describing four zones. A backup is the keyboard's
entire configuration, about a kilobyte of it, and restoring one puts back
things this tool does not otherwise touch.

## Themes

Fifteen ship with the tool, five each in three groups — steady, breathing and
moving — so the keyboard has a look before you have configured anything. Five,
not fifty: a picker is for choosing, and the ones that survived are the ones
that do something the others do not rather than the same idea in another
colour.

```bash
clevertuna theme list        # names, colours and one line each
clevertuna theme reef        # shallow water, moving slowly
clevertuna random            # roll one on the spot
clevertuna random --seed 8   # …and roll that exact one again
```

A roll is seeded, and the seed is printed and carried in the theme's name
(`Teal Wave 6f2a`), so a good one is never lost to the next click.

It rolls for movement: about 40% waves, 22% cycles, 16% breaths, 15% auroras
and only 7% steady colours — nobody presses a dice button hoping for a still
one. Palettes are built by hue family and no two members come within 45° of
each other, because three colours inside 64° read as one colour that went wrong
rather than as a scheme. Roughly a third of rolls give the touchpad or the
sliders their own treatment, in the complement of the palette's own base.

## Commands

```
list                     the keyboards this machine can configure
info                     identify the connected keyboard
get-backlight [file]     read the colour scheme        (stdout if no file)
set-backlight <file>     apply a scheme, then prove it landed
export <file>            back up every setting, verbatim
import <file>            restore such a backup
tui [file]               keyboard-first terminal interface
ui                       local interface on http://127.0.0.1:7331
theme [list|<id>]        the 15 themes that ship with Clevertuna
random [--seed <n>]      roll a new theme and put it on
builder                  open the visual theme builder (macOS)
profile save/rename/…    your own schemes, by name
match-wallpaper          build a scheme from the desktop picture
rotate every <c> <t…>    let the clock change the theme
rotate day-night <d> <n> one theme by day, another by night
rotate slots on|off      keep the cable and the 3 channels alike
timeout <off> [<idle>]   when the backlight dims, and when it goes out
```

Four surfaces, one domain layer — the CLI, the TUI, the menu bar and the local
web view all speak the same states, the same words and the same themes.

```
--ble                    talk over Bluetooth GATT instead of USB
--device <path>          use a specific interface
--json                   machine-readable output
--quiet                  only the payload
--no-color               never emit colour (NO_COLOR honoured too)
--show-identifiers       include serial numbers and similar
--ascii                  no box drawing or colour blocks (TUI)
--port <n>               port for `ui` (default 7331, loopback only)
--seed <n>               repeat a particular roll of `random`
--print-menu             (Clevertuna.app) print the menu instead of showing it
--print-swatches <file>  (Clevertuna.app) draw the menu's rows to a PNG
--print-builder <file>   (Clevertuna.app) draw the theme builder to a PNG
```

### Terminal interface

```bash
clevertuna tui                    # or: clevertuna --ascii tui
```

```text
 CLEVERTUNA  Read the current.                          USB  ● connected
 ┌ Devices ────────┐ ┌ Lighting / keyboard ───────────────────┐
 │ › CLVX S        │ │ Effect       Colour wave                │
 │   BLE candidate │ │ Stops        ■ #FF5353  ■ #00C8FF       │
 │                 │ │ Direction    270°     Period  3000 ms   │
 │ [r] refresh     │ │ Length       1000      Interactive  on  │
 └─────────────────┘ ├─────────────────────────────────────────┤
                     │  [keyboard]  touchpad  left slider      │
                     └─────────────────────────────────────────┘
 Status: VERIFIED · device matches the scheme · 14:32:08
```

Arrows or `h/j/k/l`, `[` and `]` for zones, `r` refresh, `p` preview, `s` send,
`b` backup, `?` help, `q` quit. Works at 80×24 and 120×32, falls back to ASCII
without box drawing, and **sending asks for confirmation**. Restore is
deliberately *not* bound to a key — it lives in `import` so it cannot happen by
accident.

### The macOS menu bar

`Clevertuna.app` puts the keyboard in the menu bar, so a look can be changed
without opening a terminal or the vendor app. Themes appear as the colours they
are rather than as filenames.

```bash
./scripts/build-macos-native.sh   # build, sign, and install to /Applications
open dist/Clevertuna.app
```

```
🎛 Theme Builder…
   Steady · Breathing · Moving — five each, with their colours
   Smart — 🎲 Random · 🖼 Wallpaper
   My Themes — yours
💾 Save This Look…
⚙ Settings ▸       🎛 Touch & Keyboard…   ☾ Backlight & Power…
                   📤 Export a Backup…    ↗ Open TouchOnKeys
```

**The themes are the menu.** Behind a submenu they cost a hover before anything
could be picked, which is the whole job; everything that is *not* a theme went
the other way, into Settings. Every row carries the colours it stands for,
including Random (a spectrum, because it could be anything) and Wallpaper (your
desktop picture's actual palette). Renaming and deleting is the theme manager's
job — the builder's **My themes…** button — rather than a row sitting among the
themes it would delete.

**Shortcuts** work while another app is in front. **⌃⌥1–5** put on whichever
five themes *you* chose — nothing is bound until you choose, because a key that
puts on a theme you never picked is worse than a key that does nothing. Assign
them in the theme manager (**My themes…** in the builder, or `clevertuna
favourites`). **⌃⌥B** opens the builder and **⌃⌥T** the vendor app.

They are registered through Carbon rather than an event monitor, so Clevertuna
asks for no Accessibility permission and is told only about those combinations.
One another app already holds is skipped quietly, and the menu labels only the
keys that actually registered.

> **Only one app can configure the keyboard at a time.** While TouchOnKeys is
> open, Clevertuna's reads still work and its writes are refused — so if a write
> fails, that is the first thing to check. Clevertuna says so when it notices.

It is a native AppKit app with no dock icon, and it needs no third-party
menu-bar host. The CLI is inside the bundle, so the app and the tool are always
the same build.

**Nothing in the bar asks first.** Everything it does is lighting, which the
next click undoes, and a theme picker that asks whether you meant it is not a
picker. The terminal interface still confirms, because there the same keystroke
can restore a whole backup. And nothing appears in the bar that cannot be done:
the connection state lives on the icon and in its tooltip rather than spending
a row, and copying a scheme between transports — which needs two connections to
a keyboard that grants one — is a terminal command, not a menu entry.

The app renders the same menu model every other bar renders — `clevertuna menu
--format json` — so there is one answer to "what are the actions" and several
renderings of it. Three flags let it be checked without clicking, which matters
because a fullscreen window hides the menu bar completely:

```bash
app=/Applications/Clevertuna.app/Contents/MacOS/Clevertuna
$app --print-builder builder.png
$app --print-builder pad.png     --zone pad
$app --print-builder themes.png  --surface themes --theme "Lemon Pop"
$app --print-builder save.png    --surface prompt --prompt save
$app --selftest                  # eight checks, against the keyboard

# What a surface costs to keep on screen, from getrusage — the same number
# `ps -Ao pcpu` reports. `--roll --seed <n>` renders a repeatable look, which
# needs no keyboard, and a look that does not animate is refused rather than
# measured as a paused clock.
$app --print-builder deck.png --roll --seed 7 --bench 12
```

## Rotation

The keyboard can change its own mind on a clock.

```bash
clevertuna rotate every hour hartle spectrum magma   # step through a list
clevertuna rotate every day random                   # a new one each day
clevertuna rotate day-night deep-current nightshift  # bright by day, dim by night
clevertuna rotate status
clevertuna rotate off
```

Three things worth knowing:

- **It is a tick, not a daemon.** A resident process would hold the one
  connection the keyboard grants and lock every other surface out. Instead
  something already running asks "is anything due?" — almost always the answer
  is no, and nothing is opened. On macOS the menu-bar app asks every thirty
  seconds; elsewhere a cron line does: `* * * * * clevertuna --quiet rotate tick`.
- **Slots are anchored to the clock**, so "every hour" changes on the hour and
  two machines given the same plan agree without talking. A random rotation
  seeds itself from the slot number, so the theme is a fact about the hour and
  "what *was* that one?" has an answer.
- **It says what it costs.** Applying a theme is a flash write. A minute
  cadence is about 1440 of them a day against 24 for hourly, and flash wears
  out, so that is a decision to make on purpose.

`wallpaper` and `random` are themes here too. And the wallpaper theme keeps
following: while it is the one in use, changing your desktop picture rebuilds
it. Choosing any other theme stops that.

The cadence floor is **five minutes** — 288 flash writes a day rather than the
1440 a one-minute cadence would cost — with 15 and 30 above it.

### Slots

The cable and the keyboard's three Bluetooth channels each hold their own
lighting, because the protocol has no slot field: **the slot is simply the
connection you arrived on**. Each channel is a separate pairing, so each has its
own identifier, which is enough to notice you have moved.

```bash
clevertuna rotate slots on    # whichever slot you reach it on gets the theme you are using
```

## The keyboard as a device

```bash
clevertuna device os mac          # what the modifier and media keys should do
clevertuna device defaults        # the factory settings, fetched not fired
clevertuna device defaults apply  # …and written through the verified path
clevertuna device reset --yes     # the firmware's own full reset
```

`defaults` is the safe half of a reset: the factory settings are *read*, can be
looked at, and are then written like any other write — previewable, verified,
and restorable from a backup taken first. `reset` is the blunt one, and it is
the only command here that insists on an explicit word.

## The rest of the keyboard

Everything that is not a colour: 21 settings across power, touch, multi-touch
and the keys, read from and written to the device.

```bash
clevertuna settings                        # all of them, grouped
clevertuna settings two-finger-tap off
clevertuna settings left-slider-sensitivity 4
clevertuna settings dominant-hand Left
```

The table lives in one place, so the window draws itself from what the CLI
prints — kind, options and current value per row — and adding a setting is a
change in one Rust file rather than in two languages that have to agree.

A field the firmware does not carry reads as **absent**, not as off, and the
window greys it and says so. Those are different facts.

## Backlight and power

The keyboard's own two timeouts, the same ones the vendor app sets:

```bash
clevertuna timeout            # what it is doing now
clevertuna timeout 10m 1m     # off after 10 minutes, dim after 1
clevertuna timeout off        # always on
```

The idle timeout is clamped to the backlight one, because a dim scheduled after
the light has gone out can never happen. An unusual value snaps to the nearest
one the device is offered — a value the vendor app cannot display is a setting
its owner can no longer see.

### The theme builder

A window with the deck drawn in it — keys, touchpad, and the two touch sliders
lying horizontally along the top where they are on the hardware. Pick a zone,
pick an effect, set its colours, and move brightness, opacity, speed, stretch
and spread until it looks right.

Direction is a dial for the two areas, because the device stores a full circle
for them, and two buttons for the touch sliders, because a strip can only run
one way along itself or the other — a dial that snapped to opposite ends of
itself would be a lie about the hardware. **Copy zone to…** puts the zone you
have just set onto the others — or drag one zone onto another in the picture,
which does the same thing and converts whatever the target cannot hold: a
diagonal wave dropped on a touch slider becomes a wave along it, and an aurora
becomes the nearest effect that strip can actually show. **Rotate…** opens the
clock, **My themes…** renames and deletes.

The preview animates, so a speed slider shows a speed, and **brightness,
opacity and stretch redraw as the thumb moves** — the CLI resolves the palette
at full strength precisely so those three cost nothing to change. Controls an
effect does not read are greyed rather than hidden, and the model says which
those are, so the aurora shows no colour well and no speed instead of offering
two settings the firmware has no field for.

```bash
clevertuna builder      # or: Theme Builder… in the menu
```

Nothing reaches the keyboard until **Apply**. A settings write is a flash
operation and takes longer over Bluetooth than a slider drag does, so
live-writing would queue writes behind a moving thumb and report failures for
writes still in flight.

The window does no arithmetic the device cares about. `clevertuna look` prints
a flat model of exactly those controls — with each one's range, and the resolved
colours a preview should paint — and the window hands the same shape back to
`clevertuna look apply`. Speed is never turned into a period in Swift; that
lives in `src/effects.rs`, once, next to the ranges it clamps to.

If you already run SwiftBar or xbar, `contrib/macos/clevertuna.5m.sh` is a
plugin that does the same job inside those.

### Local web view

```bash
clevertuna ui        # http://127.0.0.1:7331
```

Bound to loopback, one page, three endpoints, one request at a time so two tabs
cannot interleave hardware exchanges. No accounts, no cloud, no telemetry, no
outbound requests — the page is embedded in the binary and references nothing
external. Request bodies are capped and the client never chooses a filesystem
path.

## "Verified" means verified

A command that returned zero is not proof that anything happened. Every write
walks the same ladder, and each rung is reportable:

```
validated → sent → acknowledged → read back → compared → verified
```

**Acknowledged** only means the keyboard liked the request. **Verified** means
Clevertuna asked again afterwards and the answer matched what you asked for. If
it does not match, that is a `mismatch` — a first-class result with expected and
actual values, not a success with a shrug.

| Exit | Meaning |
|---:|---|
| 0 | read completed, or write verified |
| 2 | usage or validation error |
| 3 | no device found |
| 4 | transport or protocol failure |
| 5 | write accepted but readback differs |
| 6 | backup file rejected |

Comparison is deliberately limited to what your scheme actually named. The
keyboard fills in fields you left out, and reporting those as a mismatch would
be noise.

## Which slot am I writing?

The keyboard holds **one live connection at a time** — plugging the cable in
drops the Bluetooth link. So the transport you reach it on *is* the slot you
configure:

```bash
clevertuna set-backlight mine.json          # the USB connection
clevertuna --ble set-backlight mine.json    # whichever Bluetooth channel is live
```

Note that the **configuration interface is only exposed over USB as HID**. Over
Bluetooth the keyboard presents ordinary keyboard and pointer collections and
nothing vendor-defined — the same on Linux and on macOS — which is why no HID
tool, the vendor app included, can configure it wirelessly. Bluetooth goes
through a GATT characteristic instead, and that is what `--ble` uses: BlueZ on
Linux, CoreBluetooth on macOS, both talking to the same characteristic.

Bluetooth is not implemented on Windows yet; there, use the cable.

## The scheme file

Plain JSON, one object per zone, one effect per zone:

```json
{
  "clevertuna_backlight": 1,
  "backlight": {
    "keyboard": {
      "colorWave": {
        "colorLinePicker": {
          "markersNumber": 5,
          "markersArray": [
            { "color": { "red": 255, "green": 83, "blue": 83 }, "position": 5 },
            { "color": { "red": 0, "green": 200, "blue": 255 }, "position": 29 }
          ]
        },
        "period": 3000, "direction": 270, "length": 1000
      },
      "interactiveAnimation": { "enable": true, "duration": 3 },
      "transparency": 0
    }
  }
}
```

Zones are `keyboard`, `touchpad`, `leftSlider`, `rightSlider`. Effects are
`solidColor`, `breathing`, `colorCycle`, `colorWave`, `aurora`. Up to five
markers; positions 0–100; `direction` in degrees; `period` in milliseconds.
Sliders take only `enable` on their interactive animation.

There is a worked example in [`examples/`](examples/).

## Safety

- `set-backlight` rewrites **only** the backlight. Gestures, touch zones, key
  mappings and any field this tool does not model are carried through
  byte-for-byte, so a colour change cannot quietly rewrite something else.
- `import` is a full restore and is broader. It validates the file, then asks.
- Schemes are validated **before** the device is opened: ranges, marker counts,
  one-effect-per-zone, and schema version.
- Serial numbers and similar identifiers are hidden unless you pass
  `--show-identifiers`, so a pasted terminal is safe by default.
- Nothing phones home. There is no update check, no analytics, no network code.

## How it talks to the keyboard

The full wire format is written up in [`docs/PROTOCOL.md`](docs/PROTOCOL.md) —
report IDs, framing, protobuf field numbers, the GATT UUIDs, and the two device
behaviours that will otherwise waste your afternoon:

- the keyboard **rejects requests sent back-to-back** with `UNSUPPORTED_REQUEST`
  even when the bytes are perfectly valid — it is timing, not size, and a
  one-byte edit fails just as readily as a large one;
- it **accepts its own settings back verbatim**, which makes a no-op round trip
  a safe way to test a client;
- over Bluetooth **the acknowledgement is the fragile half** — it sometimes does
  not arrive, or arrives as the answer to the previous request. A lost
  acknowledgement is not a failed write, so Clevertuna settles the question by
  reading the keyboard back rather than by believing the reply.

## Verified against hardware

Against a **CLVX S**, over **both** transports.

Over USB: `list`, `info`, full `export`, `import`, per-zone backlight read and
write, a scheme moved from another machine and read back matching on all four
zones, and a backup/apply/restore cycle that restored exactly.

Over Bluetooth, on Linux and on macOS: `info`, a 1103-byte export, a verified
profile write, `match-wallpaper`, and a restore that came back byte-identical
apart from the device's own write counter. The cable has to come out first —
the keyboard grants one connection at a time.

109 unit tests cover the codec, splicing, validation, redaction, the menu
model, every shipped theme, and the conversions between a control and the
number the device stores. Every theme and two hundred consecutive rolls of the
randomiser are checked against the same encoder a real write uses, so a theme
that the firmware would refuse fails the suite rather than the keyboard.

Not yet met by hardware: the **macOS USB/HID** path (only its Bluetooth path
has), everything on **Windows** (it cross-compiles and imports only system
DLLs, but has never been executed), and the **theme builder's Apply** — the
window and its preview are checked by rendering them headlessly, and the write
underneath is the same verified path every other surface uses, but no keyboard
was attached when it was built.

## Support this work

<p align="center">
  <a href="https://github.com/sponsors/code-hartle-tech"><img alt="Sponsor" src="assets/badges/sponsor.svg"></a>
  <a href="https://patreon.com/HARTLETECH"><img alt="Patreon" src="assets/badges/patreon.svg"></a>
  <a href="https://liberapay.com/hartle.tech/donate"><img alt="Liberapay" src="assets/badges/liberapay.svg"></a>
  <a href="https://ko-fi.com/hartletech"><img alt="Ko-fi" src="assets/badges/kofi.svg"></a>
</p>
<p align="center">
  <a href="https://wise.com/pay/business/hartletechunipessoallda"><img alt="Wise" src="assets/badges/wise.svg"></a>
  <a href="https://paypal.me/hartletech"><img alt="PayPal" src="assets/badges/paypal.svg"></a>
  <a href="https://buy.stripe.com/5kQ8wR3Wm1sjbKW15E9fW01"><img alt="Stripe" src="assets/badges/stripe.svg"></a>
</p>

## Licence and provenance

Apache-2.0. [`NOTICE`](NOTICE) records how the wire format was determined: this
is an independent implementation of a protocol for interoperability, so that
people who own the hardware can move their own settings between their own
machines. No vendor code is included or redistributed.

Clevertuna is **not affiliated with, endorsed by, or supported by Clevetura**.
"Clevetura", "CLVX" and "TouchOnKeys" belong to their owner.
