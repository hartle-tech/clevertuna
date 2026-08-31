# Hardware verification log

What has actually been run against a physical keyboard, and what has not.

## 2026-08-22 — CLVX S over USB

Device: CLVX S, vendor `0x36F7`, product `0x5755`, configuration interface at
usage page `0xFF00` / usage 1.

| Gate | Result |
|---|---|
| `list` finds the configuration interface | pass |
| `info` returns device fields, serial redacted by default | pass |
| `export` full settings backup | pass (1103 bytes) |
| `get-backlight` decodes all four zones | pass |
| `set-backlight` applies and self-verifies | pass, stage `verified` |
| independent read-back confirms the change | pass |
| restore original scheme | pass, stage `verified` |
| restored scheme equals the pre-change scheme | pass, exact |
| exit code 0 on verified write | pass |
| `--json` output | pass |

| Gate | Result |
|---|---|
| TUI renders, navigates zones, help, clean exit (120×32, ASCII) | pass |
| UI binds 127.0.0.1 only | pass |
| UI `GET /` serves the embedded page | pass (9582 bytes) |
| UI `GET /api/state` returns live device state | pass |
| UI `POST /api/apply` performs a verified write | pass, stage `verified` |
| UI rejects an oversized body | pass (413) |
| UI rejects unknown endpoints | pass (404) |
| CLI grammar `SENT` / `READ BACK` / `VERIFIED`, column aligned | pass |
| CLI error grammar `ERROR` / `NEXT` / `CODE` | pass |
| restore the original scheme after all testing | pass, exact |

Unit tests: 42 passed, 0 failed (`cargo test`).

## 2026-08-22 — CLVX S over Bluetooth (`--ble`)

Keyboard on a Bluetooth channel, USB cable unplugged.

| Gate | Result |
|---|---|
| `list` shows the GATT endpoint and no USB interface | pass |
| `--ble info` identifies the keyboard, serial redacted | pass |
| `--ble get-backlight` reads all four zones | pass |
| `--ble export` full backup | pass (822 bytes) |
| `--ble set-backlight` applies and self-verifies | pass, stage `verified` |
| independent read-back confirms the change | pass |
| restore the original scheme over Bluetooth | pass, `verified`, all four zones |

Getting here required replacing `busctl` polling with a persistent D-Bus
connection: BlueZ disables notifications when the client that called
`StartNotify` exits, and `ReadValue` polling returns a cycling buffer that
never empties — 30 KB across 600 reads, with duplicated responses and a CRC
that never matched.

## 2026-08-30 — CLVX S, key remapping

| Gate | Result |
|---|---|
| `keys` decodes the factory function row to the printed glyphs | pass |
| `keys f9 volume-up` writes and reads back | pass, stage `verified` |
| independent re-read after replug | pass |
| original binding restored | pass, exact |

## 2026-08-31 — summary

Against a **CLVX S**, over **both** transports.

Over USB: `list`, `info`, full `export`, `import`, per-zone backlight read and
write, a scheme moved from another machine and read back matching on all four
zones, a backup/apply/restore cycle that restored exactly, and the function row
remapped and put back.

Over Bluetooth, on Linux and on macOS: `info`, an 822-byte export, a verified
profile write, `match-wallpaper`, and a restore that came back byte-identical
apart from the device's own write counter. The cable has to come out first —
the keyboard grants one connection at a time.

109 unit tests cover the codec, splicing, validation, redaction, the menu
model, every shipped theme, and the conversions between a control and the
number the device stores. Every theme and two hundred consecutive rolls of the
randomiser are checked against the same encoder a real write uses, so a theme
that the firmware would refuse fails the suite rather than the keyboard.

## Not met by hardware

- The **macOS USB/HID** path — only its Bluetooth path has been exercised.
- Everything on **Windows**. It cross-compiles and imports only system DLLs,
  but has never been executed.
- The **theme builder's Apply**. The window and its preview are checked by
  rendering them headlessly, and the write underneath is the same verified path
  every other surface uses, but no keyboard was attached when it was built.
- Whether the keyboard stores lighting per Bluetooth slot or globally.
