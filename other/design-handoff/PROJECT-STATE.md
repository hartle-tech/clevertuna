# Clevertuna project state

Snapshot date: **2026-08-22** · supersedes the 2026-08-22 branding handoff.

The previous snapshot described a brand package with no implementation behind
it. This one describes a working product: the tool is written, it talks to real
hardware over both transports, and it has been verified against a physical
keyboard.

## Repository state

- Repository: `clevertuna.hartle.tech` (its own repo, Forgejo `hartle-tech/clevertuna.hartle.tech`, private)
- Branch: `main`
- Head commit: `{{COMMIT}}`
- Language: Rust, **zero dependencies** — the `[dependencies]` table is empty
- Source: ~6 700 lines across `src/`
- Licence: Apache-2.0

Protobuf, JSON, base64, CRC-32, DEFLATE, PNG decoding, D-Bus, HID over three
operating systems and argument parsing are all implemented in-tree. That is a
deliberate cost: the deliverable is one self-contained binary a stranger can
run without a toolchain, and every crate would have been a supply-chain
dependency on a tool that writes to firmware.

## What exists now

| Surface | State |
|---|---|
| CLI | `list info get-backlight set-backlight export import profile match-wallpaper copy open-app menu do tui ui version` |
| TUI | Full-screen, 80×24 and 120×32 layouts, ASCII fallback, no dependency (raw mode via `stty`) |
| macOS menu-bar app | `Clevertuna.app` — native, signed, no dock icon, no third-party host. Profiles show as colour swatches |
| Local web view | Loopback-only, one embedded page, no network access at all. A convenience, not the product's UI |
| Status bar / tray | waybar + dmenu-family picker (Linux), the native app (macOS), WinForms tray (Windows); a SwiftBar/xbar plugin stays as an alternative |
| Profile gallery | Save, list, apply, delete; per-platform config dir; `CLEVERTUNA_HOME` override |
| Wallpaper matching | Reads the current desktop picture and derives a five-stop scheme |

## Verified against real hardware

A Clevetura CLVX S, vendor `0x36F7`, product `0x5755`.

**Over USB**

- `list`, `info`, `export` (1 103 bytes of settings)
- Write → read back → compare → restore, byte-for-byte

**Over Bluetooth — on Linux *and* macOS**

- `info`, `get-backlight` across all four zones, `export`
- Write → independent read-back → restore, across all four zones
- On macOS this goes through CoreBluetooth rather than BlueZ. Verified on a Mac
  holding the keyboard as a bonded HID peripheral: `info`, a 1 103-byte export,
  a verified profile write, wallpaper matching, and a restore that came back
  byte-identical apart from the device's own write counter

**Wallpaper matching**

- Auto-detected the running compositor's wallpaper on Linux and applied the
  derived scheme to all four zones, verified by read-back
- Detects the desktop picture on macOS and converts a JPEG desktop through
  `sips`, verified end to end on this machine

**Status bar**

- The Linux picker was driven end to end: it listed the gallery, applied the
  chosen profile over Bluetooth, and reported `VERIFIED`
- The macOS app builds, signs and renders: it runs as an agent process with no
  dock icon, and its `--print-menu` and `--print-swatches` modes were checked
  against a real gallery

## Verified without hardware

- {{TESTS}} unit tests pass
- The TUI tests render real frames through the same code path the terminal
  uses and assert on that output
- The web UI page is checked for offline self-containment, a confirmation step
  before writing, visible focus styles, reduced-motion and dark-mode support,
  and for distinguishing transports without relying on colour alone

## Artefacts

| Platform | Artefact | State |
|---|---|---|
| macOS | `dist/clevertuna-macos-universal`, {{MACOS_BYTES}} bytes, `x86_64 + arm64` | **Signed** with the HARTLE.TECH Developer ID, hardened runtime, timestamped |
| Windows | `clevertuna.exe`, {{WINDOWS_BYTES}} bytes, PE32+ | Cross-compiled and linkage-verified; imports only system DLLs |
| Linux | built from source | Verified on the target machine |

## Not verified

- **No menu item in the macOS app has ever been clicked.** The app runs and
  renders, but the machine's terminal was fullscreen throughout, which hides the
  menu bar. In particular the first use of "Match the wallpaper" will ask for
  Automation permission, because from an app bundle the TCC subject is the
  bundle rather than the terminal that was already trusted.
- The Windows binary has never been **executed**. It compiles and links, and
  its imports are only system DLLs, but no Windows machine was available.
- The macOS binary is signed but **not notarised**, so Gatekeeper reports
  `rejected — Unnotarized Developer ID` on a machine that downloads it.
  Notarisation needs credentials that only the operator can create; the release
  script already takes `--notarize` once that profile exists.
- The macOS **HID** path (USB) has not been exercised against the keyboard —
  only macOS Bluetooth has. The Windows paths have touched nothing.
- Nothing has been published to a public forge or deployed.

## Remaining gates

1. Run the Windows binary on a Windows machine, with the keyboard.
2. Exercise the macOS HID path with the keyboard attached to a Mac.
3. Notarise the macOS artefact.
4. Decide the public-mirror question before any GitHub publication.
