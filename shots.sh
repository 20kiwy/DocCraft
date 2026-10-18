#!/usr/bin/env bash
# Headless screenshots of DocCraft 3.0 — launcher + every craft view.
# Xvfb + software GL + xdotool hub clicks + ctypes X11 frame grab.
set -uo pipefail
cd "$(dirname "$0")"

export PATH=/home/z/local-libs/bin:$HOME/.cargo/bin:$PATH
export LD_LIBRARY_PATH=/home/z/local-libs/lib/x86_64-linux-gnu:/home/z/local-libs/x86_64-linux-gnu:/home/z/local-libs/lib
export WGPU_BACKEND=gl
export LIBGL_ALWAYS_SOFTWARE=1
BIN=./target/fast/doccraft
OUT=${1:-shots}
mkdir -p "$OUT"

pkill -f 'Xvfb :99' 2>/dev/null; sleep 0.5
Xvfb :99 -screen 0 1440x920x24 >/dev/null 2>&1 &
XVFB_PID=$!
sleep 1.5
export DISPLAY=:99

grab() {
  xwd -root -silent > /tmp/_shot.xwd
  python3 tools/xwd2png.py /tmp/_shot.xwd "$OUT/$1.png"
}

"$BIN" ${DC_ARGS:-} &
APP_PID=$!
sleep 12
grab 00-launcher

# hub strip buttons along the bottom (y≈906): Home Writer Sheets Slides Reader Draw Markdown Typeset
Y=906
click() { xdotool mousemove "$1" "$Y" click 1; }

click 170; sleep 18; grab 01-wordcraft
click 224; sleep 16; grab 02-gridcraft
click 276; sleep 16; grab 03-deckcraft
click 330; sleep 16; grab 04-pdfcraft
click 383; sleep 8;  grab 05-draw
click 442; sleep 6;  grab 06-markdown
click 513; sleep 6;  grab 07-typeset
# back home for the full-suite card wall shot
click 110; sleep 4; grab 08-home

kill $APP_PID 2>/dev/null
kill $XVFB_PID 2>/dev/null
echo "screenshots in $OUT:"
ls "$OUT"
