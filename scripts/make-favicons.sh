#!/usr/bin/env bash
#
# make-favicons.sh — every icon the site serves, from one source.
#
# The first favicon was a 24-unit viewBox with two 1.5-unit strokes: an
# outlined rectangle with a thin wave inside it, on a pure black tile. At 16px
# — which is the only size a favicon is actually seen at — the strokes landed
# on half-pixels and it came out a teal smudge, and the black tile disappeared
# into a dark tab bar. So: one shape, not two; a stroke thick enough to survive
# rasterising; and a bright ground so it reads on light chrome and dark.
#
#   ./scripts/make-favicons.sh
#
# Needs rsvg-convert and ImageMagick (brew install librsvg imagemagick).

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WEB="$ROOT/web"
SRC="$WEB/mark.svg"

command -v rsvg-convert >/dev/null || { echo "need rsvg-convert" >&2; exit 1; }
command -v magick >/dev/null || { echo "need ImageMagick" >&2; exit 1; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# iOS masks the icon itself, so the touch icon is full-bleed: rounded corners
# baked in here would show as black notches inside Apple's own rounding.
sed 's/ rx="8"//' "$SRC" > "$TMP/square.svg"

echo "Rendering from $(basename "$SRC")"

for size in 16 32 48 180 192 512; do
  case $size in
    180|192|512) from="$TMP/square.svg" ;;
    *)           from="$SRC" ;;
  esac
  rsvg-convert -w "$size" -h "$size" "$from" -o "$TMP/$size.png"
  printf '  %sx%s\n' "$size" "$size"
done

cp "$TMP/180.png" "$WEB/apple-touch-icon.png"
cp "$TMP/192.png" "$WEB/icon-192.png"
cp "$TMP/512.png" "$WEB/icon-512.png"
cp "$TMP/32.png"  "$WEB/favicon-32.png"
cp "$TMP/16.png"  "$WEB/favicon-16.png"

# One .ico carrying all three legacy sizes, for the browsers that still ask for
# /favicon.ico by name whatever the markup says.
magick "$TMP/16.png" "$TMP/32.png" "$TMP/48.png" "$WEB/favicon.ico"

printf '\nWrote:\n'
for f in favicon.ico favicon-16.png favicon-32.png apple-touch-icon.png icon-192.png icon-512.png; do
  printf '  %-24s %s\n' "$f" "$(du -h "$WEB/$f" | cut -f1 | tr -d ' ')"
done
