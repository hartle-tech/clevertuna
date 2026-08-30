#!/usr/bin/env bash
# Build, sign and (optionally) notarise the macOS Clevertuna binary.
#
# Every macOS release is signed with the HARTLE.TECH Developer ID. An unsigned
# or ad-hoc-signed build will be quarantined on anyone else's Mac, so the
# signing step is not optional here — the script refuses to produce an
# unsigned artefact unless you ask for one explicitly.
#
#   ./scripts/release-macos.sh                 # build, sign, verify
#   ./scripts/release-macos.sh --notarize      # …and notarise + staple
#
# Notarisation needs a keychain profile created once:
#   xcrun notarytool store-credentials hartle-notary \
#     --apple-id <apple-id> --team-id B3H4TRR7J3 --password <app-specific-password>

set -euo pipefail

IDENTITY="${CLEVERTUNA_SIGN_IDENTITY:-Developer ID Application: HARTLE.TECH, UNIPESSOAL LDA (B3H4TRR7J3)}"
NOTARY_PROFILE="${CLEVERTUNA_NOTARY_PROFILE:-hartle-notary}"
NOTARIZE=0
ALLOW_UNSIGNED=0

for arg in "$@"; do
  case "$arg" in
    --notarize) NOTARIZE=1 ;;
    --allow-unsigned) ALLOW_UNSIGNED=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."
note() { printf '  %s\n' "$*"; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || die "this script builds the macOS artefact and must run on macOS"

printf '\nBuilding\n'
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

mkdir -p dist
OUT=dist/clevertuna-macos-universal
lipo -create -output "$OUT" \
  target/aarch64-apple-darwin/release/clevertuna \
  target/x86_64-apple-darwin/release/clevertuna
note "$(lipo -info "$OUT")"
note "$(du -h "$OUT" | cut -f1) universal binary"

printf '\nSigning\n'
if ! security find-identity -v -p codesigning | grep -q "$IDENTITY"; then
  if (( ALLOW_UNSIGNED )); then
    note "identity not found — continuing unsigned because --allow-unsigned was given"
    note "this artefact must NOT be published"
    exit 0
  fi
  die "signing identity not found: $IDENTITY
     install the HARTLE.TECH Developer ID, or pass --allow-unsigned for a local-only build"
fi

# --options runtime is required for notarisation; --timestamp makes the
# signature outlive the certificate.
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$OUT"
codesign --verify --strict --verbose=2 "$OUT" 2>&1 | sed 's/^/  /'
codesign -dv "$OUT" 2>&1 | grep -E "Authority|TeamIdentifier|Timestamp" | sed 's/^/  /'

if (( NOTARIZE )); then
  printf '\nNotarising\n'
  ZIP="dist/clevertuna-macos-universal.zip"
  ditto -c -k --keepParent "$OUT" "$ZIP"
  xcrun notarytool submit "$ZIP" --keychain-profile "$NOTARY_PROFILE" --wait
  # a bare binary cannot be stapled; the ticket is looked up online, so verify
  # the way Gatekeeper will
  spctl -a -vvv -t install "$OUT" 2>&1 | sed 's/^/  /' || \
    note "spctl assessment is advisory for a bare binary"
  note "notarised; ship $ZIP"
fi

printf '\nBuilt %s\n' "$OUT"
shasum -a 256 "$OUT" | sed 's/^/  /'
