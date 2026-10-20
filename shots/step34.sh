#!/bin/bash
# Sequential foreground shot runner — one craft per call, launch under
# `timeout`, screenshot, kill. Skips Design (figcraft hangs under --craft= at
# startup in this headless env; its existing screenshot is still accurate).
export PATH="/home/z/local-libs/usr/bin:$PATH"
export DISPLAY=:99
export WGPU_BACKEND=gl
export LD_LIBRARY_PATH="/home/z/local-libs/usr/lib/x86_64-linux-gnu"
export __EGL_VENDOR_LIBRARY_DIRS=/home/z/local-libs/glvnd/egl_vendor.d
cd /home/z/doccraft-suite

shot() { ffmpeg -loglevel error -f x11grab -video_size 1500x980 -i :99 -frames:v 1 -y "shots/$1.png" && echo "shot $1"; }
run() {
  pkill -f "target/fast/doccraft" 2>/dev/null; sleep 0.6
  timeout 40 ./target/fast/doccraft "$@" > /tmp/doccraft-run.log 2>&1 &
  RUNPID=$!
  sleep 9
}
end() { kill $RUNPID 2>/dev/null; pkill -f "target/fast/doccraft" 2>/dev/null; sleep 0.6; }

"$@"
