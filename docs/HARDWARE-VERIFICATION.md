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

## Not verified Implemented from the same protocol description, but
  the keyboard accepts only one connection at a time, so the USB cable must be
  unplugged before Bluetooth can be exercised.
- macOS and Windows. The HID backend is Linux `hidraw`; other platforms need a
  backend written and tested.
- Whether the keyboard stores lighting per Bluetooth slot or globally.
