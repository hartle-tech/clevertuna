#!/usr/bin/env bash
# Build the native SwiftUI app.
#
# No Xcode project: swiftc straight to a bundle, the same way the AppKit build
# works, so the tree stays readable and the build stays scriptable.
#
# The Rust core is bundled beside the app binary as the phase-1 device layer —
# see wiki/decisions/0001-one-language-per-platform.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP="dist/Clevertuna.app"
BUNDLE_ID="tech.hartle.clevertuna"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
MIN_MACOS="26.0"

note() { printf '  \033[2m%s\033[0m\n' "$*"; }

echo "Clevertuna — native macOS app"

# The Rust core: the Linux product, and phase 1's device layer here.
note "building the core"
cargo build --release --quiet

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

note "compiling Swift ($(swift --version 2>/dev/null | head -1 | sed 's/.*version //;s/ .*//'))"
xcrun swiftc \
  -swift-version 6 \
  -target "arm64-apple-macos${MIN_MACOS}" \
  -O -whole-module-optimization \
  -framework SwiftUI -framework AppKit \
  -o "$APP/Contents/MacOS/Clevertuna" \
  macos/Clevertuna/Sources/Clevertuna/*.swift

# NOT "clevertuna": macOS is case-insensitive by default, so that name is the
# same file as the app binary above and silently replaces it.
cp target/release/clevertuna "$APP/Contents/MacOS/clevertuna-core"
cp assets/clvx-s-layout.json "$APP/Contents/Resources/clvx-s-layout.json"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>            <string>Clevertuna</string>
  <key>CFBundleDisplayName</key>     <string>Clevertuna</string>
  <key>CFBundleIdentifier</key>      <string>$BUNDLE_ID</string>
  <key>CFBundleVersion</key>         <string>$VERSION</string>
  <key>CFBundleShortVersionString</key> <string>$VERSION</string>
  <key>CFBundleExecutable</key>      <string>Clevertuna</string>
  <key>CFBundlePackageType</key>     <string>APPL</string>
  <key>LSMinimumSystemVersion</key>  <string>$MIN_MACOS</string>
  <!-- Menu bar only: a keyboard app has no dock icon. -->
  <key>LSUIElement</key>             <true/>
  <!-- How the CLI reaches a running app. \`open --args\` cannot: macOS drops
       launch arguments for an app that is already up, which for a menu bar app
       is nearly always. A URL is delivered to the running instance. -->
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key>    <string>$BUNDLE_ID</string>
      <key>CFBundleURLSchemes</key> <array><string>clevertuna</string></array>
    </dict>
  </array>
  <key>NSAppleEventsUsageDescription</key>
  <string>Clevertuna reads the name of your current desktop picture so it can match the keyboard's colours to it.</string>
  <key>NSBluetoothAlwaysUsageDescription</key>
  <string>Clevertuna talks to your keyboard over Bluetooth to read and change its lighting.</string>
  <key>NSHumanReadableCopyright</key>
  <string>HARTLE.TECH — Apache-2.0. Not affiliated with Clevetura.</string>
</dict>
</plist>
PLIST

note "Info.plist ($BUNDLE_ID, version $VERSION)"

# Signed with a stable identity, because macOS pins a TCC grant to a signing
# requirement rather than to a path. Unsigned, every rebuild is a different
# application as far as Bluetooth is concerned, and the keyboard becomes
# unreachable again with no prompt to answer.
IDENTITY="${CLEVERTUNA_SIGN_IDENTITY:-Developer ID Application: HARTLE.TECH, UNIPESSOAL LDA (B3H4TRR7J3)}"
if security find-identity -v -p codesigning 2>/dev/null | grep -qF "$IDENTITY"; then
  # Inside out: the helper first, then the bundle that contains it.
  codesign --force --timestamp=none --sign "$IDENTITY" \
    "$APP/Contents/MacOS/clevertuna-core" >/dev/null 2>&1
  codesign --force --timestamp=none --sign "$IDENTITY" \
    --entitlements /dev/stdin "$APP" >/dev/null 2>&1 <<'ENTITLEMENTS'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.device.bluetooth</key> <true/>
</dict>
</plist>
ENTITLEMENTS
  if codesign --verify --deep --strict "$APP" 2>/dev/null; then
    note "signed ($(echo "$IDENTITY" | sed 's/Developer ID Application: //'))"
  else
    note "WARNING: the signature did not verify"
  fi
else
  note "WARNING: unsigned — Bluetooth grants will not survive a rebuild"
fi

# Install it, unless told not to. An app that only exists in dist/ is an app
# nobody can test: it is not in Launch Services by the name a person types, not
# in the menu bar after a reboot, and every check of it is really a check of a
# build directory. `--no-install` is there for CI.
if [[ "${1:-}" != "--no-install" ]]; then
  DEST="/Applications/Clevertuna.app"
  # Quit the running copy first: two instances mean two CoreBluetooth sessions
  # against a keyboard that grants exactly one connection.
  pkill -x Clevertuna 2>/dev/null || true
  sleep 1
  rm -rf "$DEST"
  cp -R "$APP" "$DEST"

  # Only the installed copy is registered. Registering the build directory too
  # leaves two bundles claiming one identifier, and `open -a Clevertuna` then
  # runs whichever Launch Services saw last — which is how a "fresh" launch
  # quietly ran an old build out of dist/.
  LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
  "$LSREG" -u "$ROOT/$APP" 2>/dev/null || true
  "$LSREG" -f "$DEST" 2>/dev/null || true
  note "installed to $DEST"
fi

printf '\nBuilt %s\n' "$APP"
if [[ "${1:-}" != "--no-install" ]]; then
  printf '  installed:     /Applications/Clevertuna.app\n'
  printf '  launch it:     open -a Clevertuna\n'
else
  printf '  open it with:  open %s\n' "$APP"
fi
du -sh "$APP" | awk '{printf "  %s on disk\n", $1}'
