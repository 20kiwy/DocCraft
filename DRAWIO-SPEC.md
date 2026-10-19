# drawio connection & edge spec (extracted from jgraph/drawio + mxGraph sources)

This is the behavioral ground truth for the native Rust/egui drawio craft in this
suite. Implement to match drawio's UX exactly.

## 1. Colors (do NOT unify — three different "greens")
- `#29b6f2` — hover connection arrows (4 triangles), fixed-point "×" markers,
  selection handles, edge-endpoint handles (drawio light theme handle color).
- `#00a8ff` — valid drop-target highlight (shape outline stroke while connecting),
  vertex/edge selection color, width 5, opacity 50%.
- `#00FF00` — fixed-point snap ring (filled ellipse ~12px, 50% opacity) shown when
  the cursor snaps onto a fixed connection point; also outline-connect highlight.
- Guides `#0088cf`; edge preview = the REAL edge style (no green/red recolor).

## 2. Hover connection arrows (HoverIcons)
- On hovering a vertex, after ~140ms show 4 small triangles (up/down/left/right,
  fill #29b6f2, white stroke) just OUTSIDE the shape bounds + 2px spacing.
- Click (no drag) on an arrow = create a new vertex connected in that direction
  (default edge, defaultEdgeLength ~ 120 units).
- Drag from an arrow = start a connection from that shape (source highlighted).
- Arrows must not be drawn when they would overlap other cells.

## 3. Fixed connection points ("×" markers)
- Shown ONLY while a connection drag is active (not on plain hover).
- Rectangle-like shapes: 16 points — corners (0,0)(1,0)(0,1)(1,1), plus
  (0.25,0)(0.5,0)(0.75,0) and (0,0.25)(0,0.5)(0,0.75) and same for x=1 and y=1.
- Ellipse/rhombus: 8 points — 4 bbox corners + 4 edge midpoints, projected onto
  the ellipse/rhombus outline.
- Triangle: 6 points (0,0.25)(0,0.5)(0,0.75)(0.5,0)(0.5,1)(1,0.5).
- Marker: 5x5 px "×" cross, color #29b6f2, thin translucent white halo.
- Snap: cursor within ~4px (tolerance) of a point → snap + draw the #00FF00
  50%-opacity ellipse ring around it. The edge then pins to that constraint:
  store `exitX/exitY` (source) / `entryX/entryY` (target) in the edge style.
- Without snap: floating mode — endpoints computed by intersecting the
  center→other-end ray with the shape outline (perimeter), recomputed live.

## 4. Perimeter math (exact, for floating endpoints)
- rectanglePerimeter: ray center→next vs rect; pick side via atan2 comparison
  with atan2(h,w); orthogonal variant snaps to next.x/next.y when inside range.
- ellipsePerimeter: closed form `t = a*b / sqrt(b²*dx² + a²*dy²)` where
  (dx,dy) = next − center normalized direction, a = w/2, b = h/2;
  point = center + t*(dx,dy). (drawio vendored closed form)
- rhombusPerimeter: intersect ray with the 2 segments of the quadrant the
  direction points into; corner special cases (cx==px → top/bottom, cy==py → left/right).

## 5. Edge model (mxGraph-compatible)
- Edge stores: source cell id + target cell id (references, survive shape moves),
  optional fixed constraints exitX/exitY/entryX/entryY (0..1 floats),
  optional waypoints (absolute model points), or dangling sourcePoint/targetPoint.
- Endpoints recomputed every render from the model (linked = follows shapes).
- Edge-endpoint handles: filled circles r≈6, #29b6f2 fill, white stroke;
  if the endpoint is pinned to a fixed point, draw a small × inside.
- Dragging an endpoint onto another shape re-links it (snapToTerminals);
  dropping on empty canvas makes it a dangling endpoint at that point.

## 6. Arrow markers (Format panel "Line start"/"Line end") — EXACT list & style strings
Two dropdowns; each entry sets `<start|end>Arrow` + `<start|end>Fill`:
```
None            XArrow=none;XFill=0
Classic         XArrow=classic;XFill=1
Classic Thin    XArrow=classicThin;XFill=1
Open            XArrow=open;XFill=0
Open Thin       XArrow=openThin;XFill=0
Open Async      XArrow=openAsync;XFill=0
Block           XArrow=block;XFill=1
Block Thin      XArrow=blockThin;XFill=1
Async           XArrow=async;XFill=1
Oval            XArrow=oval;XFill=1
Diamond         XArrow=diamond;XFill=1
Diamond Thin    XArrow=diamondThin;XFill=1
Classic (open)  XArrow=classic;XFill=0
Classic Thin (open) XArrow=classicThin;XFill=0
Block (open)    XArrow=block;XFill=0
Block Thin (open) XArrow=blockThin;XFill=0
Async (open)    XArrow=async;XFill=0
Oval (open)     XArrow=oval;XFill=0
Diamond (open)  XArrow=diamond;XFill=0
Diamond Thin (open) XArrow=diamondThin;XFill=0
Box             XArrow=box;XFill=0
Half Circle     XArrow=halfCircle;XFill=0
Dash            XArrow=dash;XFill=0
Cross           XArrow=cross;XFill=0
Circle Plus     XArrow=circlePlus;XFill=0
Circle          XArrow=circle;XFill=1 (start) / 0 (end)
Base Dash       XArrow=baseDash;XFill=0
ER one          XArrow=ERone;XFill=0
ER mand one     XArrow=ERmandOne;XFill=0
ER many         XArrow=ERmany;XFill=0
ER one-to-many  XArrow=ERoneToMany;XFill=0
ER zero-to-one  XArrow=ERzeroToOne;XFill=0
ER zero-to-many XArrow=ERzeroToMany;XFill=0
Double Block    XArrow=doubleBlock;XFill=0|1
```
Default edge = `endArrow=classic;endFill=1`, marker size default 6 (startSize/endSize).
ER use-case mapping: 1:1 = ERone+ERone, 1:n = ERone+ERmany (start=end of the "one" side).

## 7. Edge Format panel (when an edge is selected) — must contain
- Line color (strokeColor) + Sharp/Rounded/Curved (rounded/curved)
- Pattern: solid / dashed 1-3 / dotted 1-3 (dashPattern strings "8 8", "12 12",
  "8 4 1 4", "1 1", "1 2", "1 4")
- Connection shape: Line / Link / Arrow / Simple Arrow / Tapered / Filled Edge
- Line width (strokeWidth)
- **Route** dropdown: Straight (no edgeStyle) / Orthogonal
  (edgeStyle=orthogonalEdgeStyle) / Horizontal Elbow (elbowEdgeStyle;elbow=vertical) /
  Vertical Elbow (elbowEdgeStyle) / Isometric H/V / Curved / Entity Relation
  (entityRelationEdgeStyle)
- **Line start** / **Line end** dropdowns (the marker list above)
- Size start / Size end (startSize/endSize), Offset start/end
  (sourcePerimeterSpacing/targetPerimeterSpacing)
- Line Jumps: jumpStyle = none|arc|gap|sharp|line, jumpSize default 6
- Opacity; Effects: Shadow, Flow animation, Sketch, Orthogonal Ends

## 8. Orthogonal routing essentials
- Jetty length: `auto` = with marker on that end: max(2, ceil((markerSize+10)/10))*10,
  else 20 model units. User waypoints act as control hints (L/Z/S polylines).
- Rounded corners when `rounded=1` (arcSize).

## 9. Export (File > Export As)
- XML (.drawio, uncompressed + compressed option), PNG, SVG, PDF, JPEG.
- PNG/SVG dialog controls: Zoom %, Selection only, Crop, Transparent background,
  Border width, Grid, Shadow. Implement zoom/border/transparent/crop at minimum.

## 10. Style keys to honor (subset used by the native model)
fillColor, strokeColor, strokeWidth, dashed, dashPattern, opacity, rounded,
arcSize, fillColor/strokeColor=none handling, direction, rotation, flipH/flipV,
fontSize, fontColor, fontStyle (1=bold,2=italic,4=underline), align,
verticalAlign, labelPosition, spacing, whiteSpace=wrap, exitX/exitY/exitDx/exitDy,
entryX/entryY/entryDx/entryDy, edgeStyle, elbow, jumpStyle, jumpSize,
startArrow/endArrow + startFill/endFill + startSize/endSize,
perimeterSpacing/sourcePerimeterSpacing/targetPerimeterSpacing.
