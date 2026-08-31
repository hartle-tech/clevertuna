#!/usr/bin/env bash
# Build the Claude Design handoff package.
#
# The package is the return leg of the design handoff: it carries the brand
# source forward unchanged and adds what the implementation actually turned
# into, with every sample captured from the built binary rather than written
# out by hand. A sample that is typed by hand is a drawing of the product; one
# that is captured is the product.
#
#   ./scripts/design-handoff.sh
#
# Writes dist/clevertuna-design-handoff-<date>.zip and its .sha256.

set -euo pipefail

cd "$(dirname "$0")/.."
REPO="$PWD"
BRAND="${CLEVERTUNA_BRAND_DIR:-$HOME/Projects/aphros.hartle.tech/other/clvx/brand/clevertuna}"
DATE="$(date +%Y-%m-%d)"
NAME="clevertuna-design-handoff-$DATE"
OUT="$REPO/dist"
BIN="$REPO/target/release/clevertuna"

note() { printf '  %s\n' "$*"; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ -d "$BRAND" ]] || die "brand source not found at $BRAND (set CLEVERTUNA_BRAND_DIR)"

printf '\nBuilding the binary the samples come from\n'
cargo build --release >/dev/null 2>&1 || die "cargo build failed"
[[ -x "$BIN" ]] || die "no binary at $BIN"
note "$("$BIN" version)"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
ROOT="$STAGE/$NAME"
mkdir -p "$ROOT"/{source-context,implemented-samples,brand}

printf '\nCollecting state and notes\n'
cp other/design-handoff/START-HERE.md other/design-handoff/IMPLEMENTATION-NOTES.md "$ROOT/"

# The state document reports facts that change with every build, so they are
# stamped in here rather than kept in the file by hand, where they would rot.
COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
TESTS="$(cargo test 2>&1 | grep -m1 'test result' | sed 's/.*ok\. \([0-9]*\) passed.*/\1/' || echo '?')"
MACOS_BYTES="$(wc -c < dist/clevertuna-macos-universal 2>/dev/null | tr -d ' ' || echo 'not built')"
WINDOWS_BYTES="$(wc -c < target/x86_64-pc-windows-gnu/release/clevertuna.exe 2>/dev/null | tr -d ' ' || echo 'not built')"
sed -e "s|{{COMMIT}}|$COMMIT|" \
    -e "s|{{TESTS}}|$TESTS|" \
    -e "s|{{MACOS_BYTES}}|$MACOS_BYTES|" \
    -e "s|{{WINDOWS_BYTES}}|$WINDOWS_BYTES|" \
    other/design-handoff/PROJECT-STATE.md > "$ROOT/PROJECT-STATE.md"
note "START-HERE.md, PROJECT-STATE.md (stamped at ${COMMIT:0:8}), IMPLEMENTATION-NOTES.md"

printf '\nCollecting source context\n'
cp README.md LICENSE NOTICE CLAUDE.md "$ROOT/source-context/"
mkdir -p "$ROOT/source-context/macos"
cp macos/ClevertunaBar/main.swift "$ROOT/source-context/macos/"
cp docs/PROTOCOL.md docs/HARDWARE-VERIFICATION.md "$ROOT/source-context/"
mkdir -p "$ROOT/source-context/contrib"
cp -R contrib/. "$ROOT/source-context/contrib/"
note "README, licence, protocol, hardware verification, status-bar integrations"

printf '\nCapturing samples from the binary\n'
# An empty gallery would hide half of the menu, so the samples are captured
# against a temporary one that is thrown away with the staging directory.
export CLEVERTUNA_HOME="$STAGE/gallery"
mkdir -p "$CLEVERTUNA_HOME"

S="$ROOT/implemented-samples"
if "$BIN" match-wallpaper --dry-run --quiet >/dev/null 2>&1; then
  "$BIN" match-wallpaper --dry-run 2>/dev/null | tail -n +2 > "$S/scheme-from-wallpaper.json"
  note "scheme-from-wallpaper.json (this machine's desktop)"
else
  cp examples/*.json "$S/scheme-from-wallpaper.json" 2>/dev/null || true
  note "no wallpaper available; used a bundled example"
fi

if [[ -s "$S/scheme-from-wallpaper.json" ]]; then
  "$BIN" profile save "Desert Dusk" --from "$S/scheme-from-wallpaper.json" >/dev/null 2>&1 || true
  "$BIN" profile save "Keys Only" --from "$S/scheme-from-wallpaper.json" --only keyboard >/dev/null 2>&1 || true
fi

"$BIN" --no-color ui --print-frame --cols 120 --from "$S/scheme-from-wallpaper.json"        > "$S/tui-120col.txt" 2>/dev/null || true
"$BIN" --no-color ui --print-frame --cols 80  --from "$S/scheme-from-wallpaper.json"        > "$S/tui-80col.txt"  2>/dev/null || true
"$BIN" --no-color ui --print-frame --cols 80  --ascii --from "$S/scheme-from-wallpaper.json" > "$S/tui-80col-ascii.txt" 2>/dev/null || true
"$BIN" --no-color ui --print-frame --cols 120                                               > "$S/tui-no-device.txt" 2>/dev/null || true
note "TUI frames: 120col, 80col, ascii, no-device"

"$BIN" --no-color menu --format json     > "$S/menu.json"          2>/dev/null || true
"$BIN" --no-color menu --format swiftbar > "$S/menu-swiftbar.txt"  2>/dev/null || true
"$BIN" --no-color menu --format waybar   > "$S/menu-waybar.json"   2>/dev/null || true
"$BIN" --no-color menu --format picker   > "$S/menu-picker.txt"    2>/dev/null || true
note "menu in all four renderings"

# These commands report usage and error states on purpose, so a non-zero exit
# is the sample, not a failure. `set -e` must not treat it as one.
{
  echo "# Captured CLI output"
  echo
  echo "Every block below is real output, captured by scripts/design-handoff.sh."
  echo
  echo '## clevertuna --help'
  echo '```'; "$BIN" --no-color 2>&1 | head -60 || true; echo '```'
  echo
  echo '## clevertuna list, with no keyboard attached'
  echo '```'; "$BIN" --no-color list 2>&1 | head -10 || true; echo '```'
  echo
  echo '## The error grammar'
  echo '```'; "$BIN" --no-color match-wallpaper --wallpaper /nonexistent.png 2>&1 | head -6 || true; echo '```'
  echo
  echo '## The profile gallery'
  echo '```'; "$BIN" --no-color profile list 2>&1 | head -10 || true; echo '```'
} > "$S/cli-output.md"
note "cli-output.md"

# The macOS app renders its own menu and swatches, so the samples come from the
# app binary rather than from a description of it.
APP_BIN="dist/Clevertuna.app/Contents/MacOS/ClevertunaBar"
if [[ -x "$APP_BIN" ]]; then
  "$APP_BIN" --print-menu > "$S/macos-menu.txt" 2>/dev/null || true
  "$APP_BIN" --print-swatches "$S/macos-menu-swatches.png" >/dev/null 2>&1 || true
  note "macOS menu-bar app: menu + swatch strip"
else
  note "Clevertuna.app not built; run scripts/build-macos-app.sh for the macOS samples"
fi

# The web view is embedded in the binary; serve it once and keep the page.
PORT=8730
"$BIN" ui --port "$PORT" --quiet >/dev/null 2>&1 &
UIPID=$!
for _ in $(seq 1 20); do
  if curl -fsS "http://127.0.0.1:$PORT/" -o "$S/web-ui.html" 2>/dev/null; then break; fi
  sleep 0.2
done
kill "$UIPID" 2>/dev/null || true
wait "$UIPID" 2>/dev/null || true
if [[ -s "$S/web-ui.html" ]]; then note "web-ui.html"; else
  rm -f "$S/web-ui.html"
  note "web UI needs a device to serve; page not captured"
fi

printf '\nCarrying the brand source forward\n'
# The previous package shipped the same assets under both claude-design/ and
# shared/, which cost 1.5 MB twice for no benefit. They live in one place here.
for d in assets tokens; do
  [[ -d "$BRAND/$d" ]] && cp -R "$BRAND/$d" "$ROOT/brand/" && note "brand/$d/"
done
for f in BRAND-GUIDE.md CLAUDE-DESIGN-HANDOFF.md CLAUDE-CODE-HANDOFF.md preview.html README.md; do
  [[ -f "$BRAND/$f" ]] && cp "$BRAND/$f" "$ROOT/brand/" && note "brand/$f"
done
find "$ROOT" -name .DS_Store -delete

printf '\nPackaging\n'
mkdir -p "$OUT"
rm -f "$OUT/$NAME.zip" "$OUT/$NAME.zip.sha256"
(cd "$STAGE" && zip -qr "$OUT/$NAME.zip" "$NAME")
(cd "$OUT" && shasum -a 256 "$NAME.zip" > "$NAME.zip.sha256")

printf '\nBuilt %s\n' "dist/$NAME.zip"
note "$(cd "$OUT" && cat "$NAME.zip.sha256")"
note "$(unzip -l "$OUT/$NAME.zip" | tail -1 | tr -s ' ')"
