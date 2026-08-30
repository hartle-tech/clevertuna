#!/usr/bin/env bash
# Clevertuna picker for Wayland bars (waybar, Hyprland, sway…).
#
# Bind this to the bar's on-click. It shows the gallery plus the actions, runs
# whatever is chosen, and reports the result — so changing the keyboard's look
# never needs the vendor app.
#
# Needs one of: fuzzel, wofi, rofi, dmenu.
set -euo pipefail
CLEVERTUNA="${CLEVERTUNA:-clevertuna}"

pick() {
  if   command -v fuzzel >/dev/null; then fuzzel --dmenu --prompt "keyboard > "
  elif command -v wofi   >/dev/null; then wofi --dmenu --prompt keyboard
  elif command -v rofi   >/dev/null; then rofi -dmenu -p keyboard
  elif command -v dmenu  >/dev/null; then dmenu -p keyboard
  else cat >/dev/null; echo ""; fi
}

notify() {
  if command -v notify-send >/dev/null; then
    notify-send -a Clevertuna "$1" "${2:-}"
  else
    printf '%s %s\n' "$1" "${2:-}"
  fi
}

# The menu prints "id<TAB>label — detail". Show the label, keep the id.
choice=$("$CLEVERTUNA" menu --format picker \
  | awk -F'\t' '{print $2"\t"$1}' \
  | pick \
  | awk -F'\t' '{print $2}')

[ -z "${choice:-}" ] && exit 0

# copy needs two transports and a slot switch in between, so it is not a
# one-click action — say so rather than half-doing it.
if [ "$choice" = "copy" ]; then
  notify "Copy between slots" "run: clevertuna copy --from usb --to ble"
  exit 0
fi

if out=$("$CLEVERTUNA" --no-color do "$choice" 2>&1); then
  notify "Clevertuna" "$(printf '%s' "$out" | tail -1)"
else
  notify "Clevertuna failed" "$(printf '%s' "$out" | head -2 | tr '\n' ' ')"
  exit 1
fi
