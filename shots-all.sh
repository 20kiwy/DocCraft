#!/usr/bin/env bash
# All DocCraft 3.2 screenshots in ONE Xvfb session (sandbox reaps bg procs).
set -uo pipefail
cd /home/z/doccraft-suite
export PATH=/home/z/local-libs/bin:$HOME/.cargo/bin:$PATH
export LD_LIBRARY_PATH=/home/z/local-libs/lib/x86_64-linux-gnu:/home/z/local-libs/x86_64-linux-gnu:/home/z/local-libs/lib
export WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 DISPLAY=:99
export __EGL_VENDOR_LIBRARY_DIRS=/home/z/local-libs/glvnd/egl_vendor.d
BIN=./target/fast/doccraft
OUT=shots
Y=903
grab() { xwd -root -silent > /tmp/_s.xwd && python3 tools/xwd2png.py /tmp/_s.xwd "$OUT/$1.png" && echo "shot: $1"; }
click() { xdotool mousemove "$1" "$Y" click 1; }

pkill -f target/fast/doccraft 2>/dev/null; pkill -f 'Xvfb :99' 2>/dev/null; sleep 0.5
Xvfb :99 -screen 0 1440x920x24 >/dev/null 2>&1 &
sleep 2
"$BIN" >/tmp/app.log 2>&1 &
sleep 14
grab 00-launcher
# about window
xdotool mousemove 1394 24 click 1; sleep 3; grab 11-about
xdotool key Escape; sleep 1
# crafts via hub
click 209; sleep 16; grab 01-wordcraft
click 289; sleep 15; grab 02-gridcraft
click 369; sleep 15; grab 03-deckcraft
click 449; sleep 15; grab 04-pdfcraft
click 529; sleep 12; grab 05-figcraft
click 609; sleep 8;  grab 06-draw
click 689; sleep 6;  grab 07-vector
click 769; sleep 6;  grab 08-markdown
click 849; sleep 6;  grab 09-typeset
click 929; sleep 6;  grab 10-notes
echo ALL-DONE
