# Clevertuna — design handoff, return leg

This package answers the branding handoff of 2026-08-22. That one described a
product that did not exist yet. This one carries the product.

Clevertuna is an independent tool for the Clevetura CLVX keyboards. It is not
affiliated with Clevetura, and it uses none of their code — the protocol was
recovered from the wire and from the vendor application's own published source
maps, and is written up in `source-context/PROTOCOL.md`.

## Read in this order

1. **`PROJECT-STATE.md`** — what is built, what is verified against real
   hardware, and what is explicitly not verified. Read the "Not verified"
   section; it is not a formality.
2. **`IMPLEMENTATION-NOTES.md`** — the brief's acceptance checklist answered
   item by item, where the implementation departed from the brief and why, and
   the three surfaces that now exist without having been designed.
3. **`implemented-samples/`** — the product's real output. Every file was
   captured from the built binary by `scripts/design-handoff.sh`. None of it
   was written by hand, so what is here is what a user sees.
4. **`brand/`** — the brand source, carried forward unchanged.
5. **`source-context/`** — README, licence, protocol notes, the hardware
   verification log, and the status-bar integrations for the three platforms.

## What is most useful to look at

- `implemented-samples/tui-120col.txt` and `tui-80col.txt` — the terminal
  interface at both designed sizes, and `tui-80col-ascii.txt` for the fallback
  that carries no box-drawing characters.
- `implemented-samples/tui-no-device.txt` — the empty state.
- `implemented-samples/macos-menu.txt` and `macos-menu-swatches.png` — the
  native macOS menu-bar app, and the colour swatches it draws for each saved
  profile. Both come from the app binary itself.
- `implemented-samples/menu-*.txt|json` — the same menu model rendered four
  other ways. This is the newest surface and the least designed.
- `implemented-samples/cli-output.md` — the output grammar and the three-line
  error form.

## What is not here

- The web view page is not captured: it is served only while a keyboard is
  attached, and this package was built without one. It is a convenience rather
  than the product's UI — on macOS that is now `Clevertuna.app`.
- No mascot appears on any surface yet.
- There is no interactive colour-stop editor. Schemes are read, applied from
  files or the gallery, or derived from the wallpaper.
