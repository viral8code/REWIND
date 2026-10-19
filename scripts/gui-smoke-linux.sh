#!/usr/bin/env bash
# Linux/X11 smoke test of the native GUI (also works under WSLg).
# Opens examples/notes in a real X11 window, types two lines with XTEST key
# events, presses Save with an XTEST mouse click and checks the saved file.
# Needs DISPLAY, python3, libX11 and libXtst (no extra packages).
#
# Usage: scripts/gui-smoke-linux.sh [path/to/rewind]
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rewind="${1:-$root/target/release/rewind}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$root/examples/notes/main.rw" "$work/main.rw"
python3 "$root/scripts/gui-drive-x11.py" --rewind "$rewind" --source "$work/main.rw" \
  --title "REWIND notes" --effects gui,fileRead,fileWrite \
  --steps "clickf:0.5,0.45;keys:hello{ENTER}world;shot:$work/typed.png;clickf:0.25,0.92;wait:800"
[ -f "$work/notes.txt" ] || { echo "FAIL: notes.txt was not written"; exit 1; }
if [ "$(cat "$work/notes.txt")" != $'hello\nworld' ]; then
  echo "FAIL: unexpected file content:"; cat "$work/notes.txt"; exit 1
fi
[ -s "$work/typed.png" ] || { echo "FAIL: no screenshot"; exit 1; }
echo "OK: typed text was saved"
