#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Automated drawio connection test — drawio HoverIcons spec parity (v32.5.5).
#
# Boots crates/drawio/examples/connect-demo under Xvfb, then drives xdotool
# through the real user flows:
#   1. hover the Start shape → the four blue #29b6f2 arrows appear (140 ms)
#   2. press the right arrow → drag to the End shape's right-middle fixed
#      point (green dot + "[100%, 50%]" hint) → release
#      → JSON must show an edge source=Start target=End exit/entry (1,0.5)
#   3. grab the new edge's target endpoint handle → drag to the sample
#      diamond → release
#      → JSON must show the edge re-linked target=diamond entry (1,0.5)
#
# Usage: shots/connect-test.sh [path-to-connect-demo]
# ─────────────────────────────────────────────────────────────────────────────
set -u
cd "$(dirname "$0")/.."
DEMO="${1:-target/fast/examples/connect-demo}"
OUT=/tmp/drawio-edges.json
export DISPLAY=:99
export WGPU_BACKEND=gl
export LD_LIBRARY_PATH=/home/z/local-libs/usr/lib/x86_64-linux-gnu
export __EGL_VENDOR_LIBRARY_DIRS=/home/z/local-libs/glvnd/egl_vendor.d
export LIBGL_DRIVERS_PATH=/usr/lib/x86_64-linux-gnu/dri
XD=/home/z/local-libs/usr/bin/xdotool
shot() { ffmpeg -loglevel error -f x11grab -video_size 1280x860 -i :99 -frames:v 1 -y "$1"; }

pkill -9 -f connect-demo 2>/dev/null; sleep 0.3
pkill -f "Xvfb :99" 2>/dev/null; sleep 0.5
# eframe persists the demo window size per app-id and restores it over the
# requested viewport — a stale 1100x760 entry used to park the edge endpoint
# under the format panel and break the endpoint drag. Start clean.
rm -rf ~/.local/share/drawioconnectdemo
(Xvfb :99 -screen 0 1280x860x24 &); sleep 1.5
rm -f "$OUT"

"$DEMO" & DEMO_PID=$!
for i in $(seq 1 120); do
  python3 -c "import json;json.load(open('$OUT'))" 2>/dev/null && break
  sleep 0.5
done
python3 -c "import json;json.load(open('$OUT'))" 2>/dev/null || { echo "FAIL: no state dump"; kill $DEMO_PID; exit 1; }

coords() {
python3 - << 'PY'
import json
d = json.load(open("/tmp/drawio-edges.json"))
vx, vy = d["view"]
s = next(s for s in d["shapes"] if s["x"] == 120.0)
e = next(s for s in d["shapes"] if s["x"] == 400.0)
dia = next(s for s in d["shapes"] if s["w"] == 160.0 and s["h"] == 80.0)
print(f"ARROW=\"{vx+s['x']+s['w']+2+13:.0f} {vy+s['y']+s['h']/2:.0f}\"")
print(f"HOVER=\"{vx+s['x']+s['w']/2:.0f} {vy+s['y']+s['h']/2:.0f}\"")
print(f"TARGET=\"{vx+e['x']+e['w']:.0f} {vy+e['y']+e['h']/2:.0f}\"")
print(f"DIAMOND=\"{vx+dia['x']+dia['w']:.0f} {vy+dia['y']+dia['h']/2:.0f}\"")
PY
}
eval $(coords)
echo "coords: arrow=$ARROW target=$TARGET diamond=$DIAMOND"

drag() { $XD mousemove $1 $2; sleep 0.7; $XD mousedown 1; sleep 0.3; for i in 1 2 3 4 5 6; do eval "CX=\$(( $1 + ( $3 - $1 ) * i / 6 ))"; eval "CY=\$(( $2 + ( $4 - $2 ) * i / 6 ))"; $XD mousemove $CX $CY; sleep 0.1; done; sleep 0.45; $XD mouseup 1; sleep 0.7; }

# 1) hover Start, then drag its right arrow onto End's right-middle point
$XD mousemove $HOVER; sleep 0.6
$XD mousemove $ARROW; sleep 0.35
drag $ARROW $TARGET
shot /tmp/drawio-test-1-connected.png

# 2) grab the edge's target endpoint (now at End's right-middle) and re-link
#    it to the sample diamond's right vertex
drag $TARGET $DIAMOND
shot /tmp/drawio-test-2-relinked.png

python3 - << 'PY'
import json, sys
d = json.load(open("/tmp/drawio-edges.json"))
dia = next(s["id"] for s in d["shapes"] if s["w"] == 160.0 and s["h"] == 80.0)
# 1) the arrow drag must have produced an edge anchored at fixed points (1, 0.5)
connected = [e for e in d["edges"] if e["exitX"] == 1 and e["exitY"] == 0.5 and e["entryX"] == 1 and e["entryY"] == 0.5]
# 2) the endpoint drag must have re-linked that edge onto the diamond's right vertex
relinked = any(e["target"] == dia and e["entryX"] == 1 and e["entryY"] == 0.5 for e in connected)
ok = bool(connected) and relinked
print("PASS: arrow drag connected at fixed points (1,0.5); endpoint drag re-linked target to the diamond (1,0.5)" if ok else "FAIL")
sys.exit(0 if ok else 1)
PY
RC=$?
kill $DEMO_PID 2>/dev/null
exit $RC
