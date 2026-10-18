#!/bin/bash
# Xvfb screenshot run for DocCraft 3.1 — one craft per shot via hub clicks.
export PATH="/home/z/local-libs/bin:$PATH"
export DISPLAY=:99
export WGPU_BACKEND=gl
export LD_LIBRARY_PATH="/home/z/local-libs:/home/z/local-libs/lib:/home/z/local-libs/lib/x86_64-linux-gnu"
export __EGL_VENDOR_LIBRARY_DIRS=/home/z/local-libs/glvnd/egl_vendor.d
cd /home/z/doccraft-suite
mkdir -p shots
pkill -f "Xvfb :99" 2>/dev/null; sleep 0.5
Xvfb :99 -screen 0 1500x980x24 &
sleep 2
./target/fast/doccraft "$@" > /tmp/doccraft-run.log 2>&1 &
sleep 10
shot() {
  xwd -root -silent > /tmp/shot.xwd
  ffmpeg -y -loglevel error -i /tmp/shot.xwd "shots/$1.png"
  echo "$1"
}
click() {
  xdotool mousemove "$1" "$2" click 1
  sleep 1.8
}
shot "launcher"
click 197 905; shot "suite-writer"
click 267 905; shot "suite-sheets"
click 337 905; shot "suite-slides"
click 407 905; shot "suite-reader"
click 477 905; shot "suite-design"
click 547 905; shot "suite-draw"
click 100 220; sleep 1.2; shot "suite-draw-shapes"
click 619 905; shot "suite-markdown"
click 691 905; shot "suite-typeset"
pkill -f "target/fast/doccraft" 2>/dev/null
pkill -f "Xvfb :99" 2>/dev/null
echo DONE
