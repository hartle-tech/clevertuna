# Install

## macOS

[Download the app](https://github.com/hartle-tech/clevertuna/releases/latest/download/Clevertuna-macOS.zip)
— a menu bar app, a theme builder and a gallery. **macOS 26 or later.**

Signed with a HARTLE.TECH Developer ID but **not notarised**, so the first
launch needs **right-click → Open**; Gatekeeper refuses a double-click.

To build it yourself:

```bash
./scripts/build-macos-native.sh   # build, sign, and install to /Applications
open dist/Clevertuna.app
```

## Linux

No prebuilt binary yet. There is nothing to install alongside it and no crates
to fetch:

```bash
cargo build --release          # target/release/clevertuna
```

That gives you the CLI, the full-screen TUI (`clevertuna tui`) and the
status-bar picker. Bluetooth needs BlueZ.

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

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

`KERNELS=="*:36F7:*"` matches the HID name `BUS:VID:PID.N`, so it covers USB and
Bluetooth alike. The vendor's own script keys on `ATTRS{idVendor}`, which only
ever matches USB, and adds you to `plugdev`, which many distributions do not
have.

## Windows

It cross-compiles and imports only system DLLs, but has never been executed —
and Bluetooth is not implemented there. Use the cable, and expect rough edges.
