# What was built, against what the brief asked for

This answers the 2026-08-22 Claude Design brief from the implementation side.
It is the return leg of the handoff: the brief described the product, and this
records what the product turned out to be, so the next round of design work
starts from the real thing rather than from the proposal.

Live samples of every surface named here are in `implemented-samples/`, all
captured from the built binary, none redrawn by hand.

## Acceptance checklist, answered

| Checklist item | State |
|---|---|
| One product across GUI, TUI, CLI | Held. All three render from the same state and the same `<STATE>  <detail>` grammar. |
| No vendor logo or implied affiliation | Held. `NOTICE` states the marks belong to their owners and that this is unaffiliated. |
| Mascot crisp and occasional | Not applicable yet — no surface ships the mascot. |
| Palette recognisable, UI calm | Held in the CLI and TUI, which use five brand colours and no others. |
| Keyboard-only navigation complete and visible | Held in the TUI. Every key is listed on the frame, always. |
| USB / BLE / disconnected distinct without colour | Held everywhere. The word is always present: `USB connected`, `BLUETOOTH connected`, `No keyboard`. The waybar module also sets a CSS class, and its stylesheet says in a comment that colour is a hint, not the message. |
| Sent / read-back / verified separate | Held, and enforced in code: `Stage` is an enum, and `VERIFIED` is only reachable after an independent read-back compares equal. |
| Restore riskier than applying a scheme | Held. Restore is bound to no key in the TUI at all — it can only be asked for explicitly on the command line. |
| Shareable views omit identifiers | Held. Serial numbers are redacted unless `--show-identifiers` is passed. |
| Reduced motion has a static equivalent | Held in the web UI, and asserted by a test. |

## Where the implementation departed from the brief

- **The desktop GUI is a menu-bar app, not a window.** The brief drew an
  application with sliders and dialogs. What shipped on macOS is
  `Clevertuna.app`: native AppKit, signed, no dock icon, and a menu whose
  profile entries are drawn as colour swatches rather than listed as filenames.
  It renders the same menu model the Windows tray and the waybar picker render,
  so there is one answer to "what are the actions" and three renderings of it.

  A full window was considered and dropped: everything the brief's editor did
  except editing colour stops is reachable in two clicks from the bar, and a
  window would have been a second place to maintain the same state.

  There is also a loopback web view (`clevertuna ui`). It predates the menu-bar
  app and is a convenience, not the product's interface — treat the menu-bar app
  as the macOS UI.

- **The lighting editor is not an editor yet.** The web UI and the TUI show the
  current scheme and apply schemes from files or the gallery. Editing colour
  stops interactively is designed but not built.

- **Nothing animates.** No progress strip, no toast. The CLI grammar prints one
  line per state transition and the TUI redraws its status line. This is the
  calm reading of the brief, and it is also what a tool with no async runtime
  can honestly do.

## Surfaces the brief did not cover, which now need design

These came from the operator after the brief was written. They exist and work,
but they were built to be correct rather than to be beautiful.

1. **Status bar and tray, on all three platforms.** The point is to change the
   keyboard's look without opening the vendor app. One menu model is rendered
   three ways: waybar plus a dmenu-family picker on Linux, a native signed app
   on macOS, a WinForms tray on Windows. A SwiftBar/xbar plugin remains for
   people who already run one of those. Every entry carries a stable id and a
   `writes` flag; the shells confirm before anything that writes.

   Open questions for design: what the bar shows when no keyboard is present,
   how a profile's colours preview on the two platforms whose menus are still
   text-only (macOS now draws real swatches — see
   `implemented-samples/macos-menu-swatches.png`), and how far the gallery should be allowed to grow before it
   needs its own window.

2. **The profile gallery.** Named schemes on disk, in a per-platform config
   directory. This is the *share your look with a friend* path, so a shared
   file can be taken into the gallery with no keyboard attached.

   Open questions: naming and collisions, and whether a profile should record
   which zones it covers in a way a person can see before applying it.

3. **Wallpaper matching.** Reads the current desktop picture, discards
   near-black and near-white, weights by saturation, and derives five stops.

   Open questions: whether the five stops should be orderable by hue rather
   than by frequency, and what to show when an image yields no usable colour —
   the tool currently refuses, which is honest but abrupt.

## The output grammar, as built

Every non-JSON line is `<STATE>  <detail>`, state left-aligned in nine columns.
Errors are three lines — what happened, what to do next, and a stable code:

```
ERROR      could not find the current wallpaper
NEXT       Pass one explicitly with --wallpaper <file>.
CODE       wallpaper-not-found (exit 2)
```

The codes are stable and meant to be matched by scripts. Exit codes are
`0` success, `2` usage or input, `3` no device, `4` transport, `5` protocol,
`6` verification mismatch.
