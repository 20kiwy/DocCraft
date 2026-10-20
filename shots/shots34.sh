#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# DocCraft 3.4 screenshot run — one craft per shot, UI3 unified design tokens.
# Crafts launch directly with --craft=<name> (and --light for the light theme),
# so no hub-coordinate guessing: each run is deterministic.
#
# Requires: Xvfb, ffmpeg, xdotool (from /home/z/local-libs), the fast build.
# Usage: bash shots/shots34.sh
# ─────────────────────────────────────────────────────────────────────────────
export PATH="/home/z/local-libs/usr/bin:$PATH"
export DISPLAY=:99
export WGPU_BACKEND=gl
export LD_LIBRARY_PATH="/home/z/local-libs/usr/lib/x86_64-linux-gnu"
export __EGL_VENDOR_LIBRARY_DIRS=/home/z/local-libs/glvnd/egl_vendor.d
cd "$(dirname "$0")/.."
mkdir -p shots

pkill -f "Xvfb :99" 2>/dev/null; sleep 0.5
Xvfb :99 -screen 0 1500x980x24 &
sleep 2

shot() { ffmpeg -loglevel error -f x11grab -video_size 1500x980 -i :99 -frames:v 1 -y "shots/$1.png" && echo "shot $1"; }
run() {
  pkill -f "target/fast/doccraft" 2>/dev/null; sleep 0.6
  ./target/fast/doccraft "$@" > /tmp/doccraft-run.log 2>&1 &
  sleep 9
}

run;                                        shot "00-launcher"
run --craft=writer;                         shot "suite-writer"
run --craft=sheets;                         shot "suite-sheets"
run --craft=slides;                         shot "suite-slides"
run --craft=reader;                         shot "suite-reader"
run --craft=design;                         shot "suite-design"

# Draw — full GraphEditor: main view, Arrange menu open, Diagram tab
run --craft=draw;                           shot "suite-draw"
xdotool mousemove 132 61 click 1; sleep 1.2; shot "17-draw-arrange-menu"
xdotool key Escape; sleep 0.6
xdotool mousemove 700 480 click 1; sleep 1.2; shot "18-draw-diagram-tab"
run --craft=draw --light;                   shot "suite-draw-light"

run --craft=markdown;                       shot "suite-markdown"
run --craft=markdown --light;               shot "suite-markdown-light"
run --craft=typeset;                        shot "suite-typeset"

# Notes — preview pane on by default, Ctrl+P toggles it off
run --craft=notes;                          shot "suite-notes"
xdotool key ctrl+p; sleep 1.2;              shot "19-notes-editor-only"

pkill -f "target/fast/doccraft" 2>/dev/null
pkill -f "Xvfb :99" 2>/dev/null
echo DONE
