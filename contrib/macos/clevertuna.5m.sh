#!/usr/bin/env bash
# Clevertuna in the macOS menu bar, via SwiftBar or xbar.
#
# Copy into the SwiftBar (or xbar) plugin folder and make it executable. The
# filename sets the refresh interval — 5m here.
#
# The binary renders the whole menu, gallery included, so this file stays a
# launcher and never has to learn what the actions are.
export PATH="/usr/local/bin:/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"
CLEVERTUNA="${CLEVERTUNA:-$(command -v clevertuna || echo /usr/local/bin/clevertuna)}"

if [ ! -x "$CLEVERTUNA" ]; then
  echo "CLVX"
  echo "---"
  echo "clevertuna is not installed | color=#FF5353"
  exit 0
fi

exec "$CLEVERTUNA" menu --format swiftbar
