# The Clevetura CLVX configuration protocol

Everything needed to write your own client. Determined by observing the wire
format and confirmed against a real CLVX S; see NOTICE for provenance.

Values below are **facts about the wire format** — report IDs, framing bytes,
protobuf field numbers — not anyone's source code.

---

## 1. Which interface

The keyboard exposes several HID interfaces. Configuration lives **only** on
the one whose first top-level collection is **usage page `0xFF00`, usage `1`**.

Over USB there are three interfaces:

```
0003:36F7:5755.*   usage 0x0001/2   pointer
0003:36F7:5755.*   usage 0x0001/6   keyboard
0003:36F7:5755.*   usage 0xFF00/1   ← configuration
```

Over Bluetooth the keyboard presents **seven** top-level collections —
GenericDesktop ×4, Consumer, Digitizer ×2 — and **none is `0xFF00`**. The
vendor page appears only nested inside a collection, so it is not a separate
interface. This is why no HID-based tool can configure the keyboard
wirelessly, the vendor's own app included.

IDs: vendor `0x36F7`; product `0x5755` (CLVX S / CLVX 1), `0x1313` (laptop).

## 2. Transport

### USB (HID reports, 64 bytes)

```
write   payload + 0x0A, split into 63-byte chunks,
        each sent as a report:  [0x23, ...chunk]

read    reports whose first byte is 0x24;
        drop that byte, accumulate the rest until 0x0A appears
```

Read with a short timeout (~50 ms) and give up after ~4 consecutive empty
reads. **Space requests out** — a request issued immediately after the
previous response is answered with `BAD_REQUEST`, while the same bytes after a
~250 ms pause are accepted.

### Bluetooth LE (GATT)

```
service          d0bf1500-c402-424a-80b0-bc7aeced077e
characteristic   d0bf0001-c402-424a-80b0-bc7aeced077e
                 flags: read, write, write-without-response, notify

write   payload + 0x0A, split into 56-byte chunks, written raw
        (no report-ID byte)
read    accumulate until 0x0A
```

Drain any pending bytes before writing.

## 3. Framing

The payload is **base64, as ASCII**, of a protobuf message:

```
USB   base64(Request)
BLE   '#' + base64(Request ‖ crc32le(Request))
```

The BLE CRC is standard CRC-32 (IEEE/zlib), appended little-endian, and
responses carry it too: strip the leading `'#'`, base64-decode, split off the
last four bytes, and check them against the CRC of the remainder.

## 4. Messages

Protobuf. Only the fields needed for settings are listed.

```proto
message Request {
  Type type                      = 1;   // omitted when 0, per proto3
  GetSettingsRequest getSettings  = 2;
  SetSettingsRequest setSettings  = 3;
  SetProfileSettingsRequest       = 4;
  GetDeviceInfoRequest            = 5;
  HeartBeat                       = 6;
  GetProfileSettingsRequest       = 7;
}

enum Request.Type {
  GET_SETTINGS = 0;  SET_SETTINGS = 1;  GET_DEVICE_INFO = 2;  HEARTBEAT = 3;
  SET_PROFILE_SETTINGS = 4;  GET_PROFILE_SETTINGS = 5;  CONTROL_AI = 6;
  GET_AI_STATE = 7;  SET_OS_MODE = 8;  GET_DEFAULT_SETTINGS = 9;
  PERFORM_FULL_RESET = 10;  GET_USER_AI_DATA = 11;  PERFORM_RESTART = 12;
  GET_DIAGNOSTICS = 13;
}

message Response {
  Type type                        = 1;
  GetSettingsResponse getSettings  = 2;   // { status = 1, AppSettings = 2 }
  SetSettingsResponse setSettings  = 3;   // { status = 1, AppSettings = 2 }
  GetDeviceInfoResponse            = 5;
  BadRequestResponse badRequest    = 7;   // { error = 1 }
}

enum Response.Type {           // note: NOT the same numbering as Request.Type
  GET_SETTINGS = 0;  SET_SETTINGS = 1;  GET_DEVICE_INFO = 2;  HEARTBEAT = 3;
  BAD_REQUEST = 4;   SET_PROFILE_SETTINGS = 5;  ...
}

enum BadRequestResponse.Error { CURRUPTED_REQUEST = 0; UNSUPPORTED_REQUEST = 1; }
enum SetSettingsResponse.Status { OK = 0; UNKNOWN_ERROR = 1; VALIDATION_ERROR = 2; }
```

Because proto3 omits zero values, a `GET_SETTINGS` request is just the empty
`getSettings` submessage: the two bytes `12 00`.

## 5. Settings

```proto
message AppSettings {
  GlobalSettings  global        = 1;
  ProfileSettings globalProfile = 2;
  uint32          counter       = 3;   // revision, increments on every write
}

message ProfileSettings {
  gestures  = 2;  touchZone = 3;  BacklightSettings backlight = 4;  keyboard = 5;
}

message BacklightSettings {
  Keyboard keyboard = 1;  Touchpad touchpad = 2;
  Slider leftSlider = 3;  Slider rightSlider = 4;
}
```

Each zone selects one effect:

```proto
message Zone {                       // Keyboard / Touchpad / Slider
  AutoEffect.SolidColor solidColor        = 1;
  InteractiveAnimation  interactiveAnimation = 2;
  AutoEffect.Breathing  breathing         = 4;
  AutoEffect.ColorCycle colorCycle        = 5;
  AutoEffect.ColorWave  colorWave         = 6;
  uint32                transparency      = 7;
  AutoEffect.Aurora     aurora            = 8;
}

message Color            { uint32 red = 1; green = 2; blue = 3; }
message ColorLineMarker  { Color color = 1; uint32 transparency = 2; position = 3; }
message ColorLinePicker  { uint32 markersNumber = 1; repeated ColorLineMarker markersArray = 2; }
message ColorWave        { ColorLinePicker colorLinePicker = 1; uint32 period = 2; direction = 3; length = 4; }
message SolidColor       { Color color = 1; }
message Breathing        { Color color = 1; uint32 period = 2; }
message ColorCycle       { ColorLinePicker colorLinePicker = 1; uint32 period = 2; }

// interactive animation differs per zone:
//   keyboard { Color color = 2; bool enable = 3; uint32 duration = 4; }
//   touchpad { Color color = 2; bool enable = 3; uint32 trace    = 4; }
//   slider   {                  bool enable = 3; }
```

Up to **5** markers. `position` is 0–100, `direction` degrees, `period`
milliseconds, `length` in mm corresponding to 100%.

### Request field numbers past the documented seven

`Request` carries more submessages than §4 lists, and they are numbered
`type + 2`: SET_OS_MODE (8) is field 10, GET_DEFAULT_SETTINGS (9) is 11,
PERFORM_FULL_RESET (10) is 12, PERFORM_RESTART (12) is 14. Four for four, and
it predicted the fifth — see below.

### The AI key answers

`GET_AI_STATE` (type 7, request field **9**) returns a response with
`type = 8` and its payload in field **10**:

```
0a 0a  08 00  10 00  18 00  20 00  40 00
└ field 1, 10 bytes
        └ five varints: fields 1, 2, 3, 4 and 8
```

So the AI key's state is five values in a submessage, and on a keyboard whose
AI has never been configured every one of them is zero. **What each means is
not established** — all-zeros carries no signal, and naming them from a guess
is how the aurora's colours got invented. Establishing it needs the vendor's
application driving the feature while the traffic is watched.

What *is* established: the request type is right, the keyboard accepts it, and
the shape is a submessage of five fields. `CONTROL_AI` (6) would be field 8 and
`GET_USER_AI_DATA` (11) field 13 by the same rule; neither has been sent.

## 6. Writing settings safely

`SET_SETTINGS` replaces the whole `AppSettings`, so read it first, replace only
what you mean to change, and send the rest back untouched. Preserve fields you
do not understand — that is what keeps a client working across firmware
revisions.

Two behaviours worth knowing:

- The device **accepts its own settings back verbatim**; re-sending an
  unmodified blob returns `status = OK` and only bumps `counter`. That makes a
  no-op round-trip a safe way to test a client.
- Requests sent back-to-back are rejected with `UNSUPPORTED_REQUEST` even when
  the bytes are valid. It is timing, not content: the same edit succeeds with a
  short pause and fails without one, and a one-byte change fails just as
  readily as a 300-byte one.
- **The reply is the fragile half of the exchange, and a lost reply is not a
  failed write.** Over Bluetooth an acknowledgement sometimes does not arrive,
  or arrives as the answer to the *previous* request — which decodes as a
  `BAD_REQUEST` with no status inside it. The settings have usually landed. So
  never treat a missing acknowledgement as a refusal and never re-send on one:
  read the settings back and compare, because the read-back is the only thing
  that can actually answer "did it land".

## Colour stops

A `colorWave` zone carries **exactly five** markers in `markersArray`. This is
not a convention — a write with four is refused with `BAD_REQUEST (1)`, the
same status the device returns for a malformed request, and every zone of a
stock device is populated with five.

A palette of fewer than five colours therefore has to fill the quota. Repeating
the first colour in the last slot is the useful way to do it: the wave returns
to where it started instead of jumping when the cycle comes round.

## 7. What the numbers mean

The field numbers above say where a value goes. They do not say what it *is*,
and three of them are counted from a different zero than any control a person
touches. A client that offers sliders needs all six of these, or it writes
schemes the firmware refuses and reports the refusal as a timing fault.

| Control | Range | Stored as | Conversion |
|---|---|---|---|
| Opacity | 0–100, 100 = fully lit | `transparency` | `transparency = 100 − opacity` |
| Speed | 500–10000, higher is faster | `period` (ms) | `period = 10500 − speed`, and the same the other way |
| Stretch | 100–1000 | `length` | written through unchanged |
| Spread | 0–359°, 0° = to the right | `direction` | `direction = (180 − angle) mod 360` |
| Duration (keys) | 1–3 — low, medium, high | `interactiveAnimation.duration` | unchanged |
| Trace (touchpad) | 1–5 — short … long | `interactiveAnimation.trace` | unchanged |

Three consequences are easy to get wrong:

- **Opacity and speed both invert.** A scheme that looks right and lights
  nothing usually has one of them the wrong way round.
- **Spread is a mirror, not an offset.** The angle and the stored direction
  turn *opposite ways*, so no `angle + k` ever fits. Measured on a CLVX S by
  setting each cardinal and watching which way the light ran:

  | the control says | the keyboard does |
  |---|---|
  | 0° right | down |
  | 90° up | left |
  | 180° left | up |
  | 270° down | right |

  Right↔down with up↔left is a reflection. Two offsets were tried first — 90,
  then 270 — and each merely rotated the reflection somewhere else. **One spot
  check cannot tell a half turn from a mirror; all four cardinals can.** The
  relation is its own inverse, so one function encodes and decodes.

- **A slider's direction is a token, not a bearing.** It holds only 90 or 270,
  meaning "along it, one way" and "along it, the other". Running the mirror over
  a strip's angle writes 180 or 0, which it does not accept — so encode and
  decode a slider by that explicit pairing instead. Measured on a CLVX S,
  2026-08-30.
- **`markersNumber` counts the real stops, not the array.** The array is always
  five long (see *Colour stops*), and the padding entries are copies. A client
  that sets `markersNumber = 5` after padding will read back three duplicate
  stops next time — including in the vendor's own application.

There is **no brightness field**. The device has one dial and it is opacity;
anything offering brightness as well is scaling the colours before it sends
them, which is what Clevertuna does.

### Per-zone limits

- The touch sliders take no `aurora` and no `interactiveAnimation`.
- A slider's `direction` is only ever 90 or 270 — it is a strip, and a wave can
  only run one way along it or the other.
- `colorCycle` has no direction: every LED shows the same colour at once.

## 8. When the light goes out

Two fields in `GlobalSettings` — not in a profile, because they are how the
keyboard behaves rather than how it looks, and they survive a change of theme.

| Field | Name | Unit | Values offered | 0 means |
|---|---|---|---|---|
| **20** | idle timeout | seconds | 0, 30, 60, 180, 300, 600, 1800 | never dim |
| **21** | backlight timeout | seconds | 0, 300, 600, 1800, 3600 | always on |

The rule the vendor application enforces, and the reason it does: **the idle
timeout is never longer than the backlight one**, because a dim that is
scheduled after the light has already gone out can never happen.

Identified by reading a stock CLVX S: field 20 held 180 and field 21 held 300,
which are exactly two of the values above and satisfy that rule. Confirmed by
writing 600/60, reading it back independently, and restoring.

## 9. Everything that is not a colour

The vendor's Electron bundle publishes its generated protobuf definitions in
its source maps (`@clevetura/clv-firmware-clvx/app-settings.ts`), so the rest of
the settings tree is specification rather than inference.

`AppSettings.global` — one set for the whole keyboard:

| Field | Name | Type |
|---|---|---|
| 2 | `tap1fEnable` | bool |
| 3 | `tap2fEnable` | bool |
| 4 | `holdEnable` | bool |
| 5 | `swapClickButtons` | bool |
| 6 | `currentAILevel` | uint32 |
| 7 | `newbieModeEnable` | bool |
| 8 | `touchActivationAfterLiftOff` | bool |
| 9 | `fnLock` | bool |
| 11 | `autoBrightnessEnable` | bool |
| 12 | `dominantHand` | enum — 0 unselected, 1 right, 2 left |
| 14 | `batterySavingModeEnable` | bool |
| 15 | `keySuppressorEnable` | bool |
| 16 | `holdDelayOnBorderEnable` | bool |
| 17 | `swapFnCtrl` | bool |
| 20 | `idleBacklightTime` | uint32, seconds |
| 21 | `backlightTime` | uint32, seconds |
| 22 | `autoUsbSwitchEnable` | bool |

`AppSettings.profile` — per profile:

| Field | Name |
|---|---|
| 1 | `id` |
| 2 | `gesture` — `threeFinger`(3) / `fourFinger`(4), each `swipe`(1) / `tap`(2) |
| 3 | `touchZone` — `touchpad`(1) / `slider`(2) |
| 4 | `backlight` — §5 |
| 5 | `keyboard` — `fKeys`(1), then F1…F12 as fields 13…24 |

### What a remapped key holds

Read off a CLVX S, 2026-08-30. Each of the F-key fields carries a field `5`
holding **four slots**, and each slot is `{1: type, 2: usage}`:

- `type = 0` — HID **Keyboard** page (0x07)
- `type = 1` — HID **Consumer** page (0x0C)

Four slots means a chord: the stock F12 is three of them at once. Empty slots
are `{1:0, 2:0}`. The factory function row decodes cleanly, which is what
confirms the reading:

| Field | Key | Slots | What it is |
|---|---|---|---|
| 18 | F6 | `1/0x70` | Display Brightness Decrement |
| 19 | F7 | `1/0x6F` | Display Brightness Increment |
| 20 | F8 | `1/0xB6` | Scan Previous Track |
| 21 | F9 | `1/0xCD` | Play/Pause |
| 22 | F10 | `1/0xB5` | Scan Next Track |
| 23 | F11 | `1/0xE2` | Mute |
| 24 | F12 | `0/0xE1`, `0/0xE3`, `0/0x21` | Left Shift + Left GUI + `4` — the macOS screenshot chord |

So **remapping is a solved problem on the wire**: it is an ordinary field of
`ProfileSettings`, it round-trips through the same `SET_SETTINGS` that writes
the backlight, and the values are standard HID usages rather than a private
table. Clevertuna does not implement it yet; nothing about the protocol is in
the way.

`touchZone.touchpad`: 1 `enable`, 2 `oneFingerEnable`, 3 `twoFingerEnable`.
`touchZone.slider`: 3 `left`, 4 `right`; each carries 2 `sensitivity` plus a
choice of action — 11 `customShortcut`, 12 `custom`, 13 `nothing`,
14 `asGlobal`, 15 `predefinedShortcut`.

⚠️ **`sensitivity` is 1–4, not 1–5.** The vendor's control offers five labels,
and a CLVX S refuses a 5 with `status 2` while accepting 1 through 4. A
published option list is not the range the firmware will take — measure it.
