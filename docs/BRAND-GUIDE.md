# Clevertuna brand guide

## 1. Positioning

Clevertuna makes an opaque keyboard protocol feel observable, portable, and
safe. It is technically serious without acting solemn. The brand joke is not
that the product is careless; the joke is that its tuna is impossibly
self-assured.

**Promise:** understand the keyboard, change one thing, and prove what stuck.

**Personality:** clever, dry, calm, exact, faintly pretentious.

**Primary line:** **Read the current.** It connects sea current, electric
current, and current device state.

**Supporting lines:**

- A rather cultivated keyboard configurator.
- Profiles without the fishing expedition.
- No fishy state.

Use one joke per surface at most. Operational labels, warnings, confirmation
copy, and errors stay literal.

## 2. Origin and visual inheritance

The identity deliberately retains three signals from the product context:

1. A deep indigo shell and bright blue interaction color from the current
   Clevetura web presentation.
2. Cyan, white, magenta, coral, and blue from the saved CLVX S `Color Wave`
   lighting profile in this repository.
3. Rounded, low-profile geometry that feels like the keyboard's chassis and
   keycaps.

Clevertuna changes the typography, mark, mascot, voice, and composition. Never
reuse the Clevetura wordmark, logo geometry, product photography, or claim an
official relationship.

## 3. Logo system

### Mark

The production mark combines a tuna silhouette with a low-profile keycap body,
an angle-bracket tail, a monocle-like lens, and a discreet moustache. It should
read as a fish first and a technical pun second.

- Minimum digital size: 20 px high.
- Clear space: one eye diameter on every side.
- At 20–31 px, use `clevertuna-app-icon.svg` or the mark without moustache
  detail.
- Do not rotate, outline again, recolor individual parts, or add a container
  behind the transparent mark.

### Wordmark and lockup

The wordmark separates **CLEVER** and **TUNA** by color, not a space. Preserve
that single word in accessible names and copy.

- On light surfaces: Abyss `#1C1949` + Trench `#FF00E8`.
- On dark surfaces: white + Current Cyan `#00C8FF`.
- Use the lockup at 220 px wide or larger.
- Use the mark alone in launcher icons, favicons, terminal prompts, and narrow
  navigation.

### Mascot: Sharklock Holmes

Sharklock Holmes is a tuna despite the badge. The contradiction is part of the
joke. He has round glasses, an over-groomed side part, a tilted bowler, a
Nietzsche moustache, an oversized pipe, a magenta trench coat, and exactly the
confidence of someone who read the wire protocol.

Use the mascot for:

- onboarding and About;
- a first-run or no-device empty state;
- release notes and documentation covers;
- a success state after a verified readback.

Do not use it:

- below 96 px;
- inside destructive confirmation dialogs;
- as a busy repeating background;
- as a replacement for status or device icons.

The approved raster has a solid Foam background. Crop from the square source;
do not use automatic background removal against the dark outlines.

## 4. Color

### Core palette

| Token | Hex | Role |
|---|---|---|
| Abyss | `#1C1949` | Shell, logo outline, dark surfaces |
| Ink | `#07101F` | Text on light and bright colors |
| Reef | `#0096FF` | Links, selection support, data |
| Current | `#00C8FF` | Primary action and active state |
| Tuna Mint | `#36F0B1` | Connected/ready/verified accent |
| Trench | `#FF00E8` | Mascot, focus ring, playful accent |
| Foam | `#F4F7FB` | Canvas and mascot background |
| White | `#FFFFFF` | Cards and inverse text |

### Functional spectrum

| Token | Hex | Meaning |
|---|---|---|
| Coral | `#FF5353` | Error or destructive risk |
| Amber | `#FFB100` | Warning or unverified write |
| Mist | `#D7DDE3` | Border and disabled surface |
| Muted | `#8A8B8B` | Secondary metadata |

The spectral gradient is reserved for lighting previews and the thinnest brand
moments. Do not use a rainbow gradient for ordinary buttons, text, or charts.

### Accessible pairings

- Ink text on Current, Mint, Amber, Foam, or White.
- White text on Abyss or Ink.
- Do not set white body text on Reef or Current.
- Trench is a focus/accent color; use Ink text if it becomes a surface.
- Every ready/warning/error treatment includes icon + label + color.

## 5. Typography

| Role | Family | Weight | Notes |
|---|---|---:|---|
| Display | Sora | 600–700 | Rounded, geometric, compact |
| UI/body | Inter | 400–650 | Dense controls and long copy |
| TUI/CLI | JetBrains Mono | 400–700 | Tabular values and command output |

Fallbacks are in `tokens/tokens.css`. Headings are sentence case. Button labels
use sentence case. The logo alone is uppercase with modest tracking.

## 6. Iconography

The icon grid is 24 × 24 with a 1.8 px round stroke. Use open geometry, minimal
fill, and one decisive signal dot or wave. The reusable sprite contains:

- `device`, `keyboard`, `touchpad`, `wave`, `solid`, `backup`, `restore`,
  `usb`, `bluetooth`, `verified`, `warning`, and `tuna`.

Default icons inherit `currentColor`. Status dots may use semantic tokens, but
the parent label must state the status.

## 7. Layout and surfaces

- 4 px base grid; major spacing steps are 16, 24, 32, and 48 px.
- Cards: 14 px radius, 1 px Mist border, almost no shadow.
- Main panels: maximum 1180 px content width; dense editors may go to 1320 px.
- Keep the shell quiet. The keyboard-light preview carries the color.
- Use a 22 px radius only for hero/onboarding surfaces.
- On dark UI, use Abyss as the canvas and `#25215D` as the raised panel; do not
  turn every panel into a gradient.

## 8. Motion

- 120 ms for hover/pressed, 180 ms for panel/control changes, 280 ms for a
  device-state transition.
- The lighting preview can drift slowly in its configured direction.
- A verified write may send one cyan-to-mint current sweep across the affected
  zone.
- Respect `prefers-reduced-motion`; replace sweeps with a static highlight.
- Do not animate the moustache, pipe smoke, or mascot during operational work.

## 9. Voice

### Do

- “Connected over USB.”
- “Backlight sent. Reading it back…”
- “Verified on device.”
- “No keyboard found. Connect USB, or choose BLE.”
- “Sharklock has no leads yet.” (empty state only)

### Avoid

- “Success!” before readback.
- “Something went wrong.” without the failed step and recovery.
- Fish puns inside errors, permissions guidance, or destructive confirmations.
- “Official,” “supported by Clevetura,” or any affiliation language.

## 10. Production checklist

- [ ] The mark remains legible at 20 px.
- [ ] Mascot is used at 96 px or larger.
- [ ] Focus is visible at 200% zoom and without relying on color alone.
- [ ] UI, TUI, and CLI use the same state vocabulary.
- [ ] Writes distinguish sent, read back, and verified.
- [ ] Export previews omit serial, chip UID, host identifiers, and telemetry.
- [ ] The independent-project notice is visible in About and documentation.
