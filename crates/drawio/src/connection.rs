//! The connection engine: fixed connection points, perimeter math and
//! endpoint resolution exactly per the drawio/mxGraph spec, plus the
//! connection drag state machine used by [`crate::canvas`].
//!
//! drawio stores links as cell references (`source`/`target` ids) plus
//! optional fixed anchors (`exitX/exitY/entryX/entryY` in 0..1 shape coords)
//! and optional waypoints; endpoints are recomputed every render so linked
//! edges follow their shapes. Dangling ends carry absolute `sourcePoint` /
//! `targetPoint`s.

use crate::model::{Cell, Diagram, EdgeRoute};
use egui::{Pos2, Rect, Vec2};

// ── shape classes ────────────────────────────────────────────────────────────

/// Connection geometry class of a vertex (drives fixed points + perimeter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeClass {
    Rect,
    Ellipse,
    Rhombus,
    Triangle,
}

pub fn shape_class(style: &str) -> ShapeClass {
    let shape = crate::style::get(style, "shape").unwrap_or("");
    if shape == "rhombus" || shape == "diamond" || style.contains("rhombus") {
        ShapeClass::Rhombus
    } else if shape == "ellipse" || shape == "doubleEllipse" || shape == "ellipse" || style.contains("ellipse") {
        ShapeClass::Ellipse
    } else if shape == "triangle" || style.contains("triangle") {
        ShapeClass::Triangle
    } else {
        ShapeClass::Rect
    }
}

/// Triangle `direction` (default east); returns the apex direction.
fn triangle_dir(style: &str) -> u8 {
    // 0 east, 1 north, 2 west, 3 south
    match crate::style::get(style, "direction") {
        Some("north") => 1,
        Some("west") => 2,
        Some("south") => 3,
        _ => 0,
    }
}

/// Rotate shape-normalized (u, v) by a triangle direction.
fn rot_triangle(u: f32, v: f32, dir: u8) -> (f32, f32) {
    match dir {
        1 => (v, u),           // north: apex at top
        2 => (1.0 - u, 1.0 - v), // west: mirrored
        3 => (1.0 - v, u),     // south
        _ => (u, v),
    }
}

// ── fixed connection points (spec §3) ────────────────────────────────────────

/// Fixed connection points in shape-normalized coords for the shape class.
/// Rect: 16 · ellipse/rhombus: 8 · triangle: 6.
pub fn fixed_points(class: ShapeClass, style: &str) -> Vec<(f32, f32)> {
    match class {
        ShapeClass::Rect => {
            let mut pts = vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.0), (0.5, 1.0), (0.0, 0.5), (1.0, 0.5)];
            for t in [0.25, 0.75] {
                pts.push((t, 0.0));
                pts.push((t, 1.0));
                pts.push((0.0, t));
                pts.push((1.0, t));
            }
            pts
        }
        ShapeClass::Ellipse | ShapeClass::Rhombus => {
            vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.0), (0.5, 1.0), (0.0, 0.5), (1.0, 0.5)]
        }
        ShapeClass::Triangle => {
            let dir = triangle_dir(style);
            [(0.0, 0.25), (0.0, 0.5), (0.0, 0.75), (0.5, 0.0), (0.5, 1.0), (1.0, 0.5)]
                .iter()
                .map(|&(u, v)| rot_triangle(u, v, dir))
                .collect()
        }
    }
}

/// The 16 rect points (public for markers/tests).
pub const RECT_POINTS: [(f32, f32); 16] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (0.0, 1.0),
    (1.0, 1.0),
    (0.25, 0.0),
    (0.5, 0.0),
    (0.75, 0.0),
    (0.0, 0.25),
    (0.0, 0.5),
    (0.0, 0.75),
    (1.0, 0.25),
    (1.0, 0.5),
    (1.0, 0.75),
    (0.25, 1.0),
    (0.5, 1.0),
    (0.75, 1.0),
];

/// All fixed points of a vertex in world coordinates (ellipse/rhombus/triangle
/// points are projected onto the outline).
pub fn fixed_points_world(cell: &Cell) -> Vec<Pos2> {
    let r = cell.rect();
    let class = shape_class(&cell.style);
    fixed_points(class, &cell.style)
        .into_iter()
        .map(|(u, v)| {
            let p = Pos2::new(r.min.x + u * r.width(), r.min.y + v * r.height());
            match class {
                ShapeClass::Ellipse => ellipse_perimeter(r, p),
                ShapeClass::Rhombus => rhombus_perimeter(r, p),
                _ => p,
            }
        })
        .collect()
}

// ── perimeter math (spec §4, mxGraph mxPerimeter.js) ─────────────────────────

/// mxGraph `rectanglePerimeter`: pick the side via atan2 comparison with
/// `atan2(h, w)`; the orthogonal variant snaps onto `next.x`/`next.y` when
/// inside the rect's range.
pub fn rectangle_perimeter_ortho(r: Rect, next: Pos2, orthogonal: bool) -> Pos2 {
    let cx = r.center().x;
    let cy = r.center().y;
    let dx = next.x - cx;
    let dy = next.y - cy;
    let alpha = dy.atan2(dx);
    let pi = std::f32::consts::PI;
    let pi2 = pi / 2.0;
    let t = r.height().atan2(r.width().max(0.001));
    let p;
    if alpha < -pi + t || alpha > pi - t {
        // left edge
        p = Pos2::new(r.left(), cy - r.width() * alpha.tan() / 2.0);
    } else if alpha < -t {
        // top edge
        p = Pos2::new(cx - r.height() * (pi2 - alpha).tan() / 2.0, r.top());
    } else if alpha < t {
        // right edge
        p = Pos2::new(r.right(), cy + r.width() * alpha.tan() / 2.0);
    } else {
        // bottom edge
        p = Pos2::new(cx + r.height() * (pi2 - alpha).tan() / 2.0, r.bottom());
    }
    if orthogonal {
        let mut q = p;
        if next.x >= r.left() && next.x <= r.right() {
            q.x = next.x;
        } else if next.y >= r.top() && next.y <= r.bottom() {
            q.y = next.y;
        }
        q.x = q.x.clamp(r.left(), r.right());
        q.y = q.y.clamp(r.top(), r.bottom());
        q
    } else {
        p
    }
}

pub fn rectangle_perimeter(r: Rect, next: Pos2) -> Pos2 {
    rectangle_perimeter_ortho(r, next, false)
}

/// mxGraph `ellipsePerimeter` closed form: with (dx, dy) = next − center,
/// a = w/2, b = h/2 → `t = a*b / sqrt(b²dx² + a²dy²)`, point = center + t·(dx,dy).
pub fn ellipse_perimeter(r: Rect, next: Pos2) -> Pos2 {
    let c = r.center();
    let (a, b) = (r.width() / 2.0, r.height() / 2.0);
    let (dx, dy) = (next.x - c.x, next.y - c.y);
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return c;
    }
    let den = (b * b * dx * dx + a * a * dy * dy).sqrt();
    let t = a * b / den.max(1e-9);
    Pos2::new(c.x + t * dx, c.y + t * dy)
}

/// mxGraph `rhombusPerimeter`: ray center→next against the quadrant's edge;
/// corner special cases (px==cx → top/bottom, py==cy → left/right).
pub fn rhombus_perimeter(r: Rect, next: Pos2) -> Pos2 {
    let cx = r.center().x;
    let cy = r.center().y;
    let (px, py) = (next.x, next.y);
    if (px - cx).abs() < 1e-6 {
        return Pos2::new(cx, if py <= cy { r.top() } else { r.bottom() });
    }
    if (py - cy).abs() < 1e-6 {
        return Pos2::new(if px <= cx { r.left() } else { r.right() }, cy);
    }
    // quadrant edges: top (cx,top)→(right,cy), right (right,cy)→(cx,bottom), …
    let (a, b) = if px > cx && py < cy {
        (Pos2::new(cx, r.top()), Pos2::new(r.right(), cy))
    } else if px <= cx && py < cy {
        (Pos2::new(r.left(), cy), Pos2::new(cx, r.top()))
    } else if px <= cx {
        (Pos2::new(r.left(), cy), Pos2::new(cx, r.bottom()))
    } else {
        (Pos2::new(r.right(), cy), Pos2::new(cx, r.bottom()))
    };
    // intersect ray c + t·d with segment a..b
    let d = next - c_at(cx, cy);
    let s = b - a;
    let den = d.x * s.y - d.y * s.x;
    if den.abs() < 1e-9 {
        return a;
    }
    let qp = a - c_at(cx, cy);
    let t = (qp.x * s.y - qp.y * s.x) / den;
    Pos2::new(cx + d.x * t, cy + d.y * t)
}

fn c_at(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

/// Dispatch perimeter by shape class (triangle honors `direction`).
pub fn perimeter(style: &str, r: Rect, next: Pos2) -> Pos2 {
    match shape_class(style) {
        ShapeClass::Rect => rectangle_perimeter(r, next),
        ShapeClass::Ellipse => ellipse_perimeter(r, next),
        ShapeClass::Rhombus => rhombus_perimeter(r, next),
        ShapeClass::Triangle => triangle_perimeter(r, next, triangle_dir(style)),
    }
}

/// Triangle perimeter: the 3 edges of the (possibly rotated) triangle.
pub fn triangle_perimeter(r: Rect, next: Pos2, dir: u8) -> Pos2 {
    let c = r.center();
    let d = next - c;
    if d.length_sq() < 1e-9 {
        return c;
    }
    let corners: [Pos2; 3] = match dir {
        1 => [Pos2::new(r.left(), r.bottom()), Pos2::new(r.left(), r.top()), Pos2::new(r.right(), c.y)],
        2 => [Pos2::new(r.right(), r.top()), Pos2::new(r.right(), r.bottom()), Pos2::new(r.left(), c.y)],
        3 => [Pos2::new(r.left(), r.top()), Pos2::new(r.right(), r.top()), Pos2::new(c.x, r.bottom())],
        _ => [Pos2::new(r.left(), r.top()), Pos2::new(r.left(), r.bottom()), Pos2::new(r.right(), c.y)],
    };
    let mut best = (f32::MAX, corners[0]);
    for w in corners.windows(2) {
        if let Some(p) = ray_seg(c, d, w[0], w[1])
            && let t = p.distance(c)
            && t < best.0
        {
            best = (t, p);
        }
    }
    // closing edge 2→0
    if let Some(p) = ray_seg(c, d, corners[2], corners[0])
        && let t = p.distance(c)
        && t < best.0
    {
        best = (t, p);
    }
    best.1
}

fn ray_seg(c: Pos2, d: Vec2, a: Pos2, b: Pos2) -> Option<Pos2> {
    let s = b - a;
    let den = d.x * s.y - d.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let qp = a - c;
    let t = (qp.x * s.y - qp.y * s.x) / den;
    let u = (qp.x * d.y - qp.y * d.x) / den;
    (t > 0.0001 && (0.0..=1.0).contains(&u)).then(|| c + d * t)
}

// ── endpoint resolution (spec §5) ────────────────────────────────────────────

/// A resolved edge endpoint: world position + whether it is pinned to a fixed
/// constraint point (drives the × inside the endpoint handle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Endpt {
    pub pos: Pos2,
    pub pinned: bool,
}

impl Endpt {
    fn dangling(p: Pos2) -> Self {
        Endpt { pos: p, pinned: false }
    }
}

/// Resolve one edge endpoint per spec:
/// 1. linked cell + fixed constraint → constraint point (projected onto the
///    outline for curved perimeters), offset by perimeter spacing;
/// 2. linked cell without constraint → floating perimeter point along the
///    center→`toward` ray;
/// 3. no link → the dangling `sourcePoint`/`targetPoint`.
pub fn resolve_endpoint(page: &Diagram, edge: &Cell, source: bool, toward: Pos2) -> Option<Endpt> {
    let id = if source { edge.source.as_deref() } else { edge.target.as_deref() };
    let dangling = if source { edge.geom.source_pt } else { edge.geom.target_pt };
    match id.and_then(|id| page.cell(id)) {
        Some(cell) if cell.vertex => {
            let r = cell.rect();
            let class = shape_class(&cell.style);
            let constraint = if source { edge.exit() } else { edge.entry() };
            let spacing = crate::style::perimeter_spacing(&edge.style, source);
            match constraint {
                Some((fx, fy)) => {
                    let mut p = Pos2::new(r.min.x + fx * r.width(), r.min.y + fy * r.height());
                    if matches!(class, ShapeClass::Ellipse | ShapeClass::Rhombus | ShapeClass::Triangle) {
                        // project along center→point onto the outline
                        p = match class {
                            ShapeClass::Ellipse => ellipse_perimeter(r, p),
                            ShapeClass::Rhombus => rhombus_perimeter(r, p),
                            _ => triangle_perimeter(r, p, triangle_dir(&cell.style)),
                        };
                    }
                    if spacing != 0.0 {
                        let c = r.center();
                        let dir = Vec2::new(p.x - c.x, p.y - c.y);
                        let len = dir.length();
                        if len > 0.001 {
                            p += dir / len * spacing;
                        }
                    }
                    Some(Endpt { pos: p, pinned: true })
                }
                None => {
                    let mut p = perimeter(&cell.style, r, toward);
                    let spacing = crate::style::perimeter_spacing(&edge.style, source);
                    if spacing != 0.0 {
                        let c = r.center();
                        let dir = Vec2::new(p.x - c.x, p.y - c.y);
                        let len = dir.length();
                        if len > 0.001 {
                            p += dir / len * spacing;
                        }
                    }
                    Some(Endpt { pos: p, pinned: false })
                }
            }
        }
        _ => dangling.map(|(x, y)| Endpt::dangling(Pos2::new(x, y))),
    }
}

/// Exit/entry direction of a fixed constraint (unit vector pointing outward),
/// used for orthogonal jetty stubs.
pub fn constraint_dir(cell: &Cell, constraint: (f32, f32)) -> Vec2 {
    let (fx, fy) = constraint;
    let dir = match shape_class(&cell.style) {
        ShapeClass::Rect | ShapeClass::Triangle => {
            // dominant side of the constraint
            if fx <= 0.05 {
                Vec2::X * -1.0
            } else if fx >= 0.95 {
                Vec2::X
            } else if fy <= 0.05 {
                Vec2::Y * -1.0
            } else if fy >= 0.95 {
                Vec2::Y
            } else {
                Vec2::ZERO
            }
        }
        ShapeClass::Ellipse => {
            let (a, b) = (cell.geom.w / 2.0, cell.geom.h / 2.0);
            let u = (fx - 0.5) * 2.0 * a;
            let v = (fy - 0.5) * 2.0 * b;
            // outward normal of the ellipse ≈ (u/a², v/b²)
            let n = Vec2::new(u / (a * a).max(1.0), v / (b * b).max(1.0));
            if n.length_sq() < 1e-6 {
                Vec2::ZERO
            } else {
                n.normalized()
            }
        }
        ShapeClass::Rhombus => {
            let n = Vec2::new(fx - 0.5, fy - 0.5);
            if n.length_sq() < 1e-6 {
                Vec2::ZERO
            } else {
                n.normalized()
            }
        }
    };
    dir
}

// ── routing ──────────────────────────────────────────────────────────────────

/// mxGraph jetty length: with a marker on that end
/// `max(2, ceil((markerSize + 10) / 10)) * 10`, else 20 model units.
pub fn jetty_len(style: &str, source: bool) -> f32 {
    let (arrow, size) = if source {
        (
            crate::style::get(style, "startArrow").unwrap_or("none"),
            crate::style::get_f32(style, "startSize", 6.0),
        )
    } else {
        (
            crate::style::get(style, "endArrow").unwrap_or("classic"),
            crate::style::get_f32(style, "endSize", 6.0),
        )
    };
    if arrow == "none" {
        20.0
    } else {
        (2.0f32.max(((size + 10.0) / 10.0).ceil()) * 10.0).min(60.0)
    }
}

/// World-space routing polyline for one edge, computed through the connection
/// engine (so linked endpoints follow their shapes and constraints hold).
pub fn route_points(page: &Diagram, cell: &Cell) -> Vec<Pos2> {
    // reference ends for the "toward" rays (other end's center / last waypoint)
    let center_of = |id: &Option<String>, pt: &Option<(f32, f32)>| -> Option<Pos2> {
        id.as_deref().and_then(|id| page.cell(id)).map(|c| c.rect().center()).or_else(|| pt.map(Pos2::from))
    };
    let wps: Vec<Pos2> = cell.geom.points.iter().map(|w| Pos2::from(*w)).collect();
    let first_wp = wps.first().copied();
    let last_wp = wps.last().copied();

    let a_target = first_wp.or(center_of(&cell.target, &cell.geom.target_pt));
    let b_target = last_wp.or(center_of(&cell.source, &cell.geom.source_pt));
    let (Some(at), Some(bt)) = (a_target, b_target) else { return Vec::new() };

    let Some(a) = resolve_endpoint(page, cell, true, at) else { return Vec::new() };
    let Some(b) = resolve_endpoint(page, cell, false, bt) else { return Vec::new() };

    match crate::style::edge_route(&cell.style) {
        EdgeRoute::Straight => {
            let mut pts = vec![a.pos];
            pts.extend(wps);
            pts.push(b.pos);
            pts
        }
        EdgeRoute::Curved => {
            let mut pts = vec![a.pos];
            pts.extend(wps);
            pts.push(b.pos);
            pts
        }
        EdgeRoute::Orthogonal => orthogonal_chain(page, a, &wps, b, cell),
        EdgeRoute::ElbowH => {
            // horizontal middle segment (mxGraph elbowEdgeStyle;elbow=vertical)
            let m = (a.pos.y + b.pos.y) / 2.0;
            vec![a.pos, Pos2::new(a.pos.x, m), Pos2::new(b.pos.x, m), b.pos]
        }
        EdgeRoute::ElbowV => {
            // vertical middle segment (mxGraph elbowEdgeStyle default)
            let m = (a.pos.x + b.pos.x) / 2.0;
            vec![a.pos, Pos2::new(m, a.pos.y), Pos2::new(m, b.pos.y), b.pos]
        }
        EdgeRoute::EntityRelation => entity_route(a, b),
    }
}

/// Orthogonal polyline with mxGraph jetty stubs and waypoint control points.
fn orthogonal_chain(page: &Diagram, a: Endpt, wps: &[Pos2], b: Endpt, cell: &Cell) -> Vec<Pos2> {
    let style = cell.style.as_str();
    let ja = jetty_len(style, true);
    let jb = jetty_len(style, false);
    let mut pts = vec![a.pos];
    // source jetty: out along the exit direction (constraint side or toward
    // the first bend)
    let exit_dir = exit_direction(page, cell, true, a, wps.first().copied());
    if ja > 0.0 && exit_dir != Vec2::ZERO {
        pts.push(a.pos + exit_dir * ja);
    }
    let mut cur = *pts.last().unwrap();
    for w in wps {
        pts.extend(elbow_pts(cur, *w));
        pts.push(*w);
        cur = *w;
    }
    let enter_dir = exit_direction(page, cell, false, b, wps.last().copied());
    let target = if enter_dir != Vec2::ZERO && jb > 0.0 { b.pos - enter_dir * jb } else { b.pos };
    pts.extend(elbow_pts(cur, target));
    if enter_dir != Vec2::ZERO && jb > 0.0 {
        pts.push(target);
    }
    pts.push(b.pos);
    dedupe(pts)
}

/// Direction the edge leaves (or enters) an endpoint: fixed constraint → the
/// anchor's side normal; floating → dominant axis toward the next bend.
fn exit_direction(page: &Diagram, cell: &Cell, source: bool, end: Endpt, next: Option<Pos2>) -> Vec2 {
    let constraint = if source { cell.exit() } else { cell.entry() };
    if let Some(c) = constraint {
        let id = if source { cell.source.as_deref() } else { cell.target.as_deref() };
        if let Some(host) = id.and_then(|id| page.cell(id)) {
            let d = constraint_dir(host, c);
            if d != Vec2::ZERO {
                return d;
            }
        }
    }
    // floating: dominant axis toward the next point
    if let Some(n) = next {
        let d = n - end.pos;
        if d.x.abs() > d.y.abs() {
            Vec2::new(d.x.signum(), 0.0)
        } else if d.length_sq() > 0.001 {
            Vec2::new(0.0, d.y.signum())
        } else {
            Vec2::ZERO
        }
    } else {
        Vec2::ZERO
    }
}

fn elbow_pts(p: Pos2, q: Pos2) -> Vec<Pos2> {
    if (p.x - q.x).abs() < 0.5 || (p.y - q.y).abs() < 0.5 {
        Vec::new()
    } else if (q.x - p.x).abs() >= (q.y - p.y).abs() {
        vec![Pos2::new(q.x, p.y)]
    } else {
        vec![Pos2::new(p.x, q.y)]
    }
}

/// drawio entityRelationEdgeStyle: horizontal jetties from the side centers
/// joined by a vertical run (S-shaped when the ends differ in y).
fn entity_route(a: Endpt, b: Endpt) -> Vec<Pos2> {
    let j = 20.0;
    let (ax, ay) = (a.pos.x, a.pos.y);
    let (bx, by) = (b.pos.x, b.pos.y);
    if (by - ay).abs() < 0.5 {
        return vec![a.pos, b.pos];
    }
    let my = (ay + by) / 2.0;
    if bx >= ax {
        vec![a.pos, Pos2::new(ax + j, ay), Pos2::new(ax + j, my), Pos2::new(bx - j, my), Pos2::new(bx - j, by), b.pos]
    } else {
        vec![a.pos, Pos2::new(ax - j, ay), Pos2::new(ax - j, my), Pos2::new(bx + j, my), Pos2::new(bx + j, by), b.pos]
    }
}

fn dedupe(pts: Vec<Pos2>) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().map(|q: &Pos2| q.distance(p) > 0.5).unwrap_or(true) {
            out.push(p);
        }
    }
    out
}

/// Route a preview edge (connection drag) between two fixed endpoints with
/// the real edge style — jetties + elbow bends for orthogonal, single-bend
/// for the elbow flavors, S-shape for entity relation, straight otherwise.
pub fn preview_route(style: &str, a: Pos2, b: Pos2) -> Vec<Pos2> {
    match crate::style::edge_route(style) {
        EdgeRoute::Orthogonal => {
            let ja = jetty_len(style, true);
            let jb = jetty_len(style, false);
            let da = dominant_axis(a, b);
            let db = dominant_axis(b, a);
            let a2 = a + da * ja;
            let b2 = b - db * jb;
            let mut pts = vec![a, a2];
            pts.extend(elbow_pts(a2, b2));
            if jb > 0.0 && db != Vec2::ZERO {
                pts.push(b2);
            }
            pts.push(b);
            dedupe(pts)
        }
        EdgeRoute::ElbowH => {
            let m = (a.y + b.y) / 2.0;
            vec![a, Pos2::new(a.x, m), Pos2::new(b.x, m), b]
        }
        EdgeRoute::ElbowV => {
            let m = (a.x + b.x) / 2.0;
            vec![a, Pos2::new(m, a.y), Pos2::new(m, b.y), b]
        }
        EdgeRoute::EntityRelation => entity_route(Endpt { pos: a, pinned: false }, Endpt { pos: b, pinned: false }),
        EdgeRoute::Straight | EdgeRoute::Curved => vec![a, b],
    }
}

fn dominant_axis(from: Pos2, to: Pos2) -> Vec2 {
    let d = to - from;
    if d.length_sq() < 1e-6 {
        return Vec2::ZERO;
    }
    if d.x.abs() > d.y.abs() {
        Vec2::new(d.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, d.y.signum())
    }
}

// ── snap + drag state machine ────────────────────────────────────────────────

/// A snap hit during a connection drag: the shape and, when within tolerance
/// of a fixed point, its normalized constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct Snap {
    pub shape: String,
    pub constraint: Option<(f32, f32)>,
    /// World position of the snapped endpoint.
    pub pos: Pos2,
}

/// Connection drag state machine (spec §2/§3 + endpoint re-linking).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ConnectState {
    #[default]
    Idle,
    /// Dragging a new connection out of a shape's hover arrow.
    Connecting { source: String, side: Option<(f32, f32)>, cur: Pos2, snapped: Option<Snap> },
    /// Dragging an existing edge endpoint to re-link or dangle it.
    EndpointDrag { edge: String, is_source: bool, cur: Pos2, snapped: Option<Snap> },
}

impl ConnectState {
    pub fn active(&self) -> bool {
        !matches!(self, ConnectState::Idle)
    }

    pub fn cursor(&self) -> Option<Pos2> {
        match self {
            ConnectState::Connecting { cur, .. } | ConnectState::EndpointDrag { cur, .. } => Some(*cur),
            _ => None,
        }
    }

    pub fn set_cursor(&mut self, p: Pos2) {
        match self {
            ConnectState::Connecting { cur, snapped, .. } | ConnectState::EndpointDrag { cur, snapped, .. } => {
                *cur = p;
                let _ = snapped;
            }
            _ => {}
        }
    }
}

/// Find the snap target for a dragging cursor: the vertex under/next to the
/// cursor and, when within `tol` world units of one of its fixed points, that
/// point's constraint. `exclude` ids are skipped (the source shape may still
/// connect to itself in drawio; pass `None` to allow everything).
pub fn find_snap(page: &Diagram, cursor: Pos2, tol: f32, exclude: Option<&str>) -> Option<Snap> {
    // candidates: vertices whose bbox (padded) contains the cursor
    let pad = tol * 2.0;
    let mut best: Option<(f32, Snap)> = None;
    for c in page.cells.iter().rev() {
        if !c.vertex || !page.layer_visible(c) {
            continue;
        }
        if let Some(ex) = exclude
            && c.id == ex
        {
            continue;
        }
        let r = c.rect().expand(pad);
        if !r.contains(cursor) {
            continue;
        }
        // nearest fixed point
        let pts = fixed_points_world(c);
        let mut nearest: Option<(f32, Pos2, (f32, f32))> = None;
        let class = shape_class(&c.style);
        for (i, p) in pts.iter().enumerate() {
            let d = p.distance(cursor);
            if d <= tol && nearest.map(|(nd, _, _)| d < nd).unwrap_or(true) {
                let norm = fixed_points(class, &c.style)[i];
                nearest = Some((d, *p, norm));
            }
        }
        let snap = match nearest {
            Some((_, p, norm)) => Snap { shape: c.id.clone(), constraint: Some(norm), pos: p },
            None => Snap { shape: c.id.clone(), constraint: None, pos: cursor },
        };
        let d = cursor.distance(r.center());
        if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, snap));
        }
    }
    best.map(|(_, s)| s)
}

/// True when the cursor is over the given vertex (for the drop-target
/// highlight while connecting).
pub fn hovering_vertex(page: &Diagram, cursor: Pos2) -> Option<String> {
    page.cells
        .iter()
        .rev()
        .find(|c| c.vertex && page.layer_visible(c) && c.rect().contains(cursor))
        .map(|c| c.id.clone())
}
