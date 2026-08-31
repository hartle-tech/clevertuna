# Usage

Every command, flag and window. If you just want the thing running, the
[README](../README.md) is shorter.

- [Commands](#commands)
- [Flags](#flags)
- [Terminal interface](#terminal-interface)
- [The macOS menu bar](#the-macos-menu-bar)
- [The theme builder](#the-theme-builder)
- [Local web view](#local-web-view)
- [Rotation](#rotation)
- [Slots](#slots)
- [The keyboard as a device](#the-keyboard-as-a-device)
- [The rest of the keyboard](#the-rest-of-the-keyboard)
- [Backlight and power](#backlight-and-power)
- [Key remapping](#key-remapping)

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
keys [<key> <action>]    what each function key sends, and remap one
match-wallpaper          build a scheme from the desktop picture
rotate every <c> <t…>    let the clock change the theme
rotate day-night <d> <n> one theme by day, another by night
rotate slots on|off      keep the cable and the 3 channels alike
timeout <off> [<idle>]   when the backlight dims, and when it goes out
device os|defaults|reset the keyboard's own factory behaviour
settings [<k> <v>]       the 21 non-colour settings
```

Four surfaces, one domain layer — the CLI, the TUI, the menu bar and the local
web view all speak the same states, the same words and the same themes.

## Flags

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

## Terminal interface

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

## The macOS menu bar

`Clevertuna.app` puts the keyboard in the menu bar, so a look can be changed
without opening a terminal or the vendor app. Themes appear as the colours they
are rather than as filenames.

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

If you already run SwiftBar or xbar, `contrib/macos/clevertuna.5m.sh` is a
plugin that does the same job inside those.

## The theme builder

A window with the deck drawn in it — keys, touchpad, and the two touch sliders
lying horizontally along the top where they are on the hardware. Pick a zone,
pick an effect, set its colours, and move brightness, opacity, speed, stretch
and spread until it looks right.

```bash
clevertuna builder      # or: Theme Builder… in the menu
```

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

Nothing reaches the keyboard until **Apply**. A settings write is a flash
operation and takes longer over Bluetooth than a slider drag does, so
live-writing would queue writes behind a moving thumb and report failures for
writes still in flight.

The window does no arithmetic the device cares about. `clevertuna look` prints
a flat model of exactly those controls — with each one's range, and the resolved
colours a preview should paint — and the window hands the same shape back to
`clevertuna look apply`. Speed is never turned into a period in Swift; that
lives in `src/effects.rs`, once, next to the ranges it clamps to.

## Local web view

```bash
clevertuna ui        # http://127.0.0.1:7331
```

Bound to loopback, one page, three endpoints, one request at a time so two tabs
cannot interleave hardware exchanges. No accounts, no cloud, no telemetry, no
outbound requests — the page is embedded in the binary and references nothing
external. Request bodies are capped and the client never chooses a filesystem
path.

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

## Slots

The cable and the keyboard's three Bluetooth channels each hold their own
lighting, because the protocol has no slot field: **the slot is simply the
connection you arrived on**. Each channel is a separate pairing, so each has its
own identifier, which is enough to notice you have moved.

```bash
clevertuna rotate slots on    # whichever slot you reach it on gets the theme you are using
```

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

## Key remapping

The function row is stored in the keyboard, four slots per key, and Clevertuna
reads and writes it the same verified way it writes a colour.

```bash
clevertuna keys                # what each function key sends
clevertuna keys f5 mute        # remap one, written and read back to check
clevertuna keys f5 nothing     # clear it
clevertuna --json keys         # the whole row as a model
```

Actions come from two HID pages — the keyboard page for ordinary keys, the
consumer page for media and system controls. Name one that does not exist and
the error lists every one that does. Field numbers in the firmware say *where*
a slot is, not *what* it does; the decoding is in
[PROTOCOL.md](PROTOCOL.md).

Only some models in the family carry a remappable row; on the others the
command says so rather than writing anything.
