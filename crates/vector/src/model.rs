//! The Vector document model: shapes (rect / ellipse / line / poly / path),
//! freehand strokes and text items in one paint-ordered node list, plus
//! bezier flattening, hit tests and a sample document.
//!
//! Paint order is the order of [`Doc::nodes`] (last = topmost); layers
//! reordering moves nodes inside the vector.

use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke, Vec2};

pub const GRID_MINOR: f32 = 8.0;
/// Major grid every 8 minors (64 pt).
pub const GRID_MAJOR_EVERY: usize = 8;

// ---------------------------------------------------------------------------
// anchors + paths
// ---------------------------------------------------------------------------

/// One bezier anchor point with its two direction handles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub p: Pos2,
    /// Incoming direction handle (offset from `p`).
    pub h_in: Vec2,
    /// Outgoing direction handle (offset from `p`).
    pub h_out: Vec2,
}

impl Anchor {
    pub fn corner(p: Pos2) -> Self {
        Self { p, h_in: Vec2::ZERO, h_out: Vec2::ZERO }
    }
    pub fn mirrored(p: Pos2, h: Vec2) -> Self {
        Self { p, h_in: -h, h_out: h }
    }
}

/// Sample one cubic bezier segment into `steps` line points (excluding start).
pub fn cubic_points(p0: Pos2, c1: Pos2, c2: Pos2, p3: Pos2, steps: usize) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(steps);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let mt = 1.0 - t;
        let x = mt * mt * mt * p0.x + 3.0 * mt * mt * t * c1.x + 3.0 * mt * t * t * c2.x + t * t * t * p3.x;
        let y = mt * mt * mt * p0.y + 3.0 * mt * mt * t * c1.y + 3.0 * mt * t * t * c2.y + t * t * t * p3.y;
        out.push(pos2(x, y));
    }
    out
}

/// Flatten a full anchor list (open or closed) into a polyline.
pub fn flatten(anchors: &[Anchor], closed: bool) -> Vec<Pos2> {
    let mut pts = Vec::new();
    let Some(first) = anchors.first() else { return pts };
    pts.push(first.p);
    let segs = if closed { anchors.len() } else { anchors.len().saturating_sub(1) };
    for i in 0..segs {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % anchors.len()];
        if a.h_out == Vec2::ZERO && b.h_in == Vec2::ZERO {
            pts.push(b.p);
        } else {
            pts.extend(cubic_points(a.p, a.p + a.h_out, b.p + b.h_in, b.p, 12));
        }
    }
    pts
}

/// Regular polygon corner points inside `r` (flat bottom for 4+ sides).
pub fn poly_points(r: Rect, sides: usize) -> Vec<Pos2> {
    let n = sides.clamp(3, 24);
    let (c, rx, ry) = (r.center(), r.width() / 2.0, r.height() / 2.0);
    let off = if n % 2 == 0 { std::f32::consts::PI / n as f32 } else { std::f32::consts::FRAC_PI_2 };
    (0..n)
        .map(|i| {
            let a = off + i as f32 * 2.0 * std::f32::consts::PI / n as f32 as f32;
            pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// nodes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rect,
    Ellipse,
    Line,
    Poly,
    Path,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Rect => "Rectangle",
            Kind::Ellipse => "Ellipse",
            Kind::Line => "Line",
            Kind::Poly => "Polygon",
            Kind::Path => "Path",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub id: u64,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Corner points / bezier anchors for Line (2) and Path (n).
    pub anchors: Vec<Anchor>,
    pub closed: bool,
    /// Regular polygon side count for [`Kind::Poly`].
    pub sides: usize,
    pub fill: Color32,
    /// Optional two-stop vertical gradient (top, bottom).
    pub gradient: Option<(Color32, Color32)>,
    pub stroke: Color32,
    pub stroke_w: f32,
    pub opacity: f32,
    pub hidden: bool,
    pub locked: bool,
}

impl Shape {
    pub fn new(id: u64, kind: Kind, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            id,
            kind,
            x,
            y,
            w,
            h,
            anchors: Vec::new(),
            closed: false,
            sides: 6,
            fill: Color32::WHITE,
            gradient: None,
            stroke: Color32::from_rgb(0x26, 0x26, 0x26),
            stroke_w: 1.0,
            opacity: 1.0,
            hidden: false,
            locked: false,
        }
    }

    pub fn rect(&self) -> Rect {
        match self.kind {
            Kind::Line | Kind::Path if !self.anchors.is_empty() => {
                let mut r = Rect::from_min_max(self.anchors[0].p, self.anchors[0].p);
                for a in &self.anchors {
                    r.extend_with(a.p);
                }
                r
            }
            _ => Rect::from_min_size(pos2(self.x, self.y), vec2(self.w.max(0.0), self.h.max(0.0))),
        }
    }

    /// Re-derive x/y/w/h from the anchor list (Line / Path).
    pub fn sync_bounds(&mut self) {
        if self.anchors.is_empty() {
            return;
        }
        let r = self.rect();
        self.x = r.min.x;
        self.y = r.min.y;
        self.w = r.width();
        self.h = r.height();
    }

    pub fn set_rect(&mut self, r: Rect) {
        if matches!(self.kind, Kind::Line | Kind::Path) && !self.anchors.is_empty() {
            let old = self.rect();
            let sx = if old.width() > 0.01 { r.width() / old.width() } else { 1.0 };
            let sy = if old.height() > 0.01 { r.height() / old.height() } else { 1.0 };
            for a in &mut self.anchors {
                a.p = pos2(r.min.x + (a.p.x - old.min.x) * sx, r.min.y + (a.p.y - old.min.y) * sy);
                a.h_in *= sx;
                a.h_out *= sy;
            }
            self.sync_bounds();
        } else {
            self.x = r.min.x;
            self.y = r.min.y;
            self.w = r.width().max(0.0);
            self.h = r.height().max(0.0);
        }
    }

    pub fn translate(&mut self, d: Vec2) {
        self.x += d.x;
        self.y += d.y;
        for a in &mut self.anchors {
            a.p += d;
        }
    }

    /// Paint a shape into the screen-space `rect` (path kinds map anchors
    /// through `xform`).
    pub fn paint(&self, p: &egui::Painter, screen: Rect, zoom: f32, xform: impl Fn(Pos2) -> Pos2) {
        let op = self.opacity.clamp(0.0, 1.0);
        let a = |c: Color32| alpha(c, op);
        let stroke = Stroke::new((self.stroke_w * zoom).max(0.5), a(self.stroke));
        match self.kind {
            Kind::Rect => {
                if let Some((top, bot)) = self.gradient {
                    gradient_fill(p, screen, a(top), a(bot));
                } else {
                    p.rect_filled(screen, 0.0, a(self.fill));
                }
                p.rect_stroke(screen, 0.0, stroke, egui::StrokeKind::Middle);
            }
            Kind::Ellipse => {
                let rad = vec2(screen.width() / 2.0, screen.height() / 2.0);
                if let Some((top, bot)) = self.gradient {
                    p.add(egui::Shape::ellipse_filled(screen.center(), rad, a(top)));
                } else {
                    p.add(egui::Shape::ellipse_filled(screen.center(), rad, a(self.fill)));
                }
                p.add(egui::Shape::ellipse_stroke(screen.center(), rad, stroke));
            }
            Kind::Poly => {
                let pts = poly_points(screen, self.sides);
                p.add(egui::Shape::convex_polygon(pts, a(self.fill), stroke));
            }
            Kind::Line | Kind::Path => {
                // anchors are in document coordinates
                let _ = screen;
                self.paint_path(p, xform, a(self.fill), stroke);
            }
        }
    }

    /// Paint a line / path node: anchors are mapped through `xform`.
    pub fn paint_path(&self, p: &egui::Painter, xform: impl Fn(Pos2) -> Pos2, fill: Color32, stroke: Stroke) {
        let pts: Vec<Pos2> = flatten(&self.anchors, self.closed);
        let spts: Vec<Pos2> = pts.iter().map(|q| xform(*q)).collect();
        if self.kind == Kind::Path {
            p.add(egui::Shape::Path(egui::epaint::PathShape {
                points: spts,
                closed: self.closed,
                fill,
                stroke: stroke.into(),
            }));
        } else {
            // lines / imported polylines: plain stroke, no fill
            p.add(egui::Shape::line(spts, stroke));
        }
    }

    /// Distance from a document point to this shape (for hit tests).
    pub fn hit(&self, dp: Pos2) -> bool {
        match self.kind {
            Kind::Rect | Kind::Poly => self.rect().contains(dp),
            Kind::Ellipse => {
                let r = self.rect();
                let c = r.center();
                let dx = (dp.x - c.x) / (r.width() / 2.0).max(0.001);
                let dy = (dp.y - c.y) / (r.height() / 2.0).max(0.001);
                if self.fill == Color32::TRANSPARENT {
                    // hollow: only the rim counts
                    let d = (dx * dx + dy * dy).sqrt();
                    (d - 1.0).abs() * r.width().min(r.height()) / 2.0 < (self.stroke_w + 3.0).max(6.0)
                } else {
                    dx * dx + dy * dy <= 1.0
                }
            }
            Kind::Line | Kind::Path => {
                let pts = flatten(&self.anchors, self.closed);
                if self.closed && self.fill != Color32::TRANSPARENT && pts.len() > 2 {
                    point_in_poly(dp, &pts)
                } else {
                    let tol = (self.stroke_w + 3.0).max(6.0);
                    pts.windows(2).any(|s| dist_point_seg(dp, s[0], s[1]) <= tol)
                }
            }
        }
    }
}

/// Freehand stroke (brush / pencil).
#[derive(Clone, Debug, PartialEq)]
pub struct FreeStroke {
    pub id: u64,
    pub pts: Vec<Pos2>,
    pub color: Color32,
    pub width: f32,
    pub opacity: f32,
    pub hidden: bool,
    pub locked: bool,
}

impl FreeStroke {
    pub fn bounds(&self) -> Rect {
        let Some(f) = self.pts.first() else { return Rect::NOTHING };
        let mut r = Rect::from_min_max(*f, *f);
        for q in &self.pts {
            r.extend_with(*q);
        }
        r
    }
    pub fn translate(&mut self, d: Vec2) {
        for q in &mut self.pts {
            *q += d;
        }
    }
    /// Offset relative to the bounding box so resize can re-place the points.
    pub fn set_bounds(&mut self, r: Rect) {
        let old = self.bounds();
        if old.width() < 0.01 || old.height() < 0.01 {
            let d = r.min - old.min;
            self.translate(d);
            return;
        }
        let sx = r.width() / old.width();
        let sy = r.height() / old.height();
        for q in &mut self.pts {
            *q = pos2(r.min.x + (q.x - old.min.x) * sx, r.min.y + (q.y - old.min.y) * sy);
        }
    }
    pub fn hit(&self, dp: Pos2) -> bool {
        let tol = (self.width + 3.0).max(6.0);
        self.pts.windows(2).any(|s| dist_point_seg(dp, s[0], s[1]) <= tol)
            || self.pts.first().is_some_and(|f| (*f - dp).length() <= tol)
    }
}

/// A text item.
#[derive(Clone, Debug, PartialEq)]
pub struct TextItem {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub text: String,
    pub size: f32,
    pub color: Color32,
    pub opacity: f32,
    pub hidden: bool,
    pub locked: bool,
}

impl TextItem {
    pub fn bounds(&self) -> Rect {
        let h = self.size * 1.4;
        Rect::from_min_size(pos2(self.x, self.y), vec2(self.w.max(24.0), h))
    }
    pub fn translate(&mut self, d: Vec2) {
        self.x += d.x;
        self.y += d.y;
    }
}

/// One document node — a shape, a freehand stroke or a text item.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Shape(Shape),
    Stroke(FreeStroke),
    Text(TextItem),
}

impl Node {
    pub fn id(&self) -> u64 {
        match self {
            Node::Shape(s) => s.id,
            Node::Stroke(s) => s.id,
            Node::Text(t) => t.id,
        }
    }
    pub fn hidden(&self) -> bool {
        match self {
            Node::Shape(s) => s.hidden,
            Node::Stroke(s) => s.hidden,
            Node::Text(t) => t.hidden,
        }
    }
    pub fn set_hidden(&mut self, v: bool) {
        match self {
            Node::Shape(s) => s.hidden = v,
            Node::Stroke(s) => s.hidden = v,
            Node::Text(t) => t.hidden = v,
        }
    }
    pub fn locked(&self) -> bool {
        match self {
            Node::Shape(s) => s.locked,
            Node::Stroke(s) => s.locked,
            Node::Text(t) => t.locked,
        }
    }
    pub fn bounds(&self) -> Rect {
        match self {
            Node::Shape(s) => s.rect(),
            Node::Stroke(s) => s.bounds(),
            Node::Text(t) => t.bounds(),
        }
    }
    pub fn set_bounds(&mut self, r: Rect) {
        match self {
            Node::Shape(s) => s.set_rect(r),
            Node::Stroke(s) => s.set_bounds(r),
            Node::Text(t) => {
                t.x = r.min.x;
                t.y = r.min.y;
                t.w = r.width();
                t.size = (r.height() / 1.4).max(6.0);
            }
        }
    }
    pub fn translate(&mut self, d: Vec2) {
        match self {
            Node::Shape(s) => s.translate(d),
            Node::Stroke(s) => s.translate(d),
            Node::Text(t) => t.translate(d),
        }
    }
    pub fn opacity(&self) -> f32 {
        match self {
            Node::Shape(s) => s.opacity,
            Node::Stroke(s) => s.opacity,
            Node::Text(t) => t.opacity,
        }
    }
    pub fn kind_label(&self) -> &'static str {
        match self {
            Node::Shape(s) => s.kind.label(),
            Node::Stroke(_) => "Stroke",
            Node::Text(_) => "Text",
        }
    }
    pub fn name(&self) -> String {
        match self {
            Node::Shape(s) => s.kind.label().to_string(),
            Node::Stroke(_) => "Freehand stroke".into(),
            Node::Text(t) => {
                let line = t.text.lines().next().unwrap_or("Text");
                if line.chars().count() > 22 {
                    format!("{}…", line.chars().take(22).collect::<String>())
                } else {
                    line.to_string()
                }
            }
        }
    }
    pub fn hit(&self, dp: Pos2) -> bool {
        match self {
            Node::Shape(s) => s.hit(dp),
            Node::Stroke(s) => s.hit(dp),
            Node::Text(t) => t.bounds().expand(3.0).contains(dp),
        }
    }
    pub fn paint(&self, p: &egui::Painter, zoom: f32, xform: impl Fn(Pos2) -> Pos2) {
        match self {
            Node::Shape(s) => match s.kind {
                Kind::Line | Kind::Path => {
                    let op = s.opacity.clamp(0.0, 1.0);
                    let stroke = Stroke::new((s.stroke_w * zoom).max(0.5), alpha(s.stroke, op));
                    let fill = if s.kind == Kind::Line || s.fill == Color32::TRANSPARENT || !s.closed {
                        Color32::TRANSPARENT
                    } else {
                        alpha(s.fill, op)
                    };
                    s.paint_path(p, &xform, fill, stroke);
                }
                _ => {
                    let r = Rect::from_min_max(xform(s.rect().min), xform(s.rect().max));
                    s.paint(p, r, zoom, &xform);
                }
            },
            Node::Stroke(s) => {
                let pts: Vec<Pos2> = s.pts.iter().map(|q| xform(*q)).collect();
                p.add(egui::Shape::line(pts, Stroke::new((s.width * zoom).max(0.5), alpha(s.color, s.opacity))));
            }
            Node::Text(t) => {
                let sp = xform(pos2(t.x, t.y));
                let fs = (t.size * zoom).max(4.0);
                let g = p.layout(
                    t.text.clone(),
                    egui::FontId::proportional(fs),
                    alpha(t.color, t.opacity),
                    t.w.max(24.0) * zoom,
                );
                p.galley(sp, g, alpha(t.color, t.opacity));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// document
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Doc {
    /// Artboard size in pt.
    pub artboard: Vec2,
    pub nodes: Vec<Node>,
    pub next_id: u64,
}

impl Doc {
    pub fn new(w: f32, h: f32) -> Self {
        Self { artboard: vec2(w, h), nodes: Vec::new(), next_id: 1 }
    }

    pub fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id() == id)
    }
    pub fn node_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id() == id)
    }

    /// Topmost visible, unlocked node under a document point.
    pub fn hit(&self, dp: Pos2) -> Option<u64> {
        for n in self.nodes.iter().rev() {
            if !n.hidden() && !n.locked() && n.hit(dp) {
                return Some(n.id());
            }
        }
        None
    }

    pub fn delete(&mut self, ids: &[u64]) {
        self.nodes.retain(|n| !ids.contains(&n.id()));
    }

    /// Duplicate: copies selected nodes above the originals, offset by (16, 16).
    pub fn duplicate(&mut self, ids: &[u64]) -> Vec<u64> {
        let mut fresh = Vec::new();
        let clones: Vec<Node> = self
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id()))
            .cloned()
            .collect();
        for mut n in clones {
            let old = n.id();
            let id = self.alloc();
            match &mut n {
                Node::Shape(s) => s.id = id,
                Node::Stroke(s) => s.id = id,
                Node::Text(t) => t.id = id,
            }
            n.translate(vec2(16.0, 16.0));
            fresh.push((old, id, n));
        }
        let out = fresh.iter().map(|(_, id, _)| *id).collect();
        let mut extra: Vec<Node> = fresh.into_iter().map(|(_, _, n)| n).collect();
        self.nodes.append(&mut extra);
        out
    }

    /// Bring to front / send to back by moving inside the node list.
    pub fn reorder(&mut self, id: u64, to_front: bool) {
        let Some(pos) = self.nodes.iter().position(|n| n.id() == id) else { return };
        let n = self.nodes.remove(pos);
        if to_front {
            self.nodes.push(n);
        } else {
            self.nodes.insert(0, n);
        }
    }

    pub fn selection_bounds(&self, sel: &[u64]) -> Option<Rect> {
        let mut r: Option<Rect> = None;
        for id in sel {
            if let Some(n) = self.node(*id) {
                let b = n.bounds();
                r = Some(match r {
                    Some(acc) => acc.union(b),
                    None => b,
                });
            }
        }
        r
    }

    // ── sample ───────────────────────────────────────────────────────────────

    pub fn sample() -> Self {
        let mut d = Doc::new(640.0, 480.0);
        let mut card = Shape::new(d.alloc(), Kind::Rect, 96.0, 88.0, 264.0, 176.0);
        card.fill = Color32::from_rgb(0x18, 0x5A, 0xBD);
        card.gradient = Some((Color32::from_rgb(0x2E, 0x9E, 0xCC), Color32::from_rgb(0x11, 0x2B, 0x49)));
        card.stroke = Color32::from_rgb(0x11, 0x2B, 0x49);
        card.stroke_w = 1.5;
        card.opacity = 1.0;
        d.nodes.push(Node::Shape(card));

        let mut sun = Shape::new(d.alloc(), Kind::Ellipse, 424.0, 72.0, 104.0, 104.0);
        sun.fill = Color32::from_rgb(0xF5, 0xC4, 0x42);
        sun.stroke = Color32::from_rgb(0xE8, 0x5D, 0x2A);
        sun.stroke_w = 2.0;
        d.nodes.push(Node::Shape(sun));

        let mut poly = Shape::new(d.alloc(), Kind::Poly, 392.0, 240.0, 144.0, 128.0);
        poly.sides = 6;
        poly.fill = Color32::from_rgb(0x33, 0xA0, 0x52);
        poly.stroke = Color32::from_rgb(0x0F, 0x6B, 0x3C);
        poly.stroke_w = 1.5;
        poly.opacity = 0.9;
        d.nodes.push(Node::Shape(poly));

        let mut line = Shape::new(d.alloc(), Kind::Line, 0.0, 0.0, 0.0, 0.0);
        line.anchors = vec![Anchor::corner(pos2(64.0, 400.0)), Anchor::corner(pos2(560.0, 352.0))];
        line.stroke = Color32::from_rgb(0x80, 0x80, 0x80);
        line.stroke_w = 1.0;
        line.sync_bounds();
        d.nodes.push(Node::Shape(line));

        let mut path = Shape::new(d.alloc(), Kind::Path, 0.0, 0.0, 0.0, 0.0);
        path.closed = true;
        path.anchors = vec![
            Anchor::mirrored(pos2(160.0, 320.0), vec2(-40.0, 0.0)),
            Anchor::mirrored(pos2(232.0, 384.0), vec2(0.0, 32.0)),
            Anchor::mirrored(pos2(304.0, 320.0), vec2(40.0, 0.0)),
        ];
        path.fill = Color32::from_rgb(0xC2, 0x4E, 0xA0);
        path.stroke = Color32::from_rgb(0x7A, 0x1F, 0x5C);
        path.stroke_w = 1.5;
        path.sync_bounds();
        d.nodes.push(Node::Shape(path));

        let sid = d.alloc();
        d.nodes.push(Node::Stroke(FreeStroke {
            id: sid,
            pts: (0..=24)
                .map(|i| pos2(472.0 + i as f32 * 3.0, 300.0 + (i as f32 * 0.7).sin() * 26.0))
                .collect(),
            color: Color32::from_rgb(0xE8, 0x5D, 0x2A),
            width: 2.5,
            opacity: 1.0,
            hidden: false,
            locked: false,
        }));

        let tid = d.alloc();
        d.nodes.push(Node::Text(TextItem {
            id: tid,
            x: 96.0,
            y: 40.0,
            w: 320.0,
            text: "Vector — sample artboard".into(),
            size: 20.0,
            color: Color32::from_rgb(0x33, 0x33, 0x33),
            opacity: 1.0,
            hidden: false,
            locked: false,
        }));
        d
    }
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

pub fn alpha(c: Color32, op: f32) -> Color32 {
    if op >= 0.999 {
        return c;
    }
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * op.clamp(0.0, 1.0)) as u8)
}

/// Vertical two-stop gradient fill, painted as strips (works for any rect).
pub fn gradient_fill(p: &egui::Painter, r: Rect, top: Color32, bot: Color32) {
    let steps = 24;
    let h = r.height() / steps as f32;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let c = lerp(top, bot, t);
        let y = r.top() + h * i as f32;
        p.rect_filled(Rect::from_min_max(pos2(r.left(), y), pos2(r.right(), y + h + 0.5)), 0.0, c);
    }
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()), mix(a.a(), b.a()))
}

pub fn dist_point_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    if ab.dot(ab) < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub fn point_in_poly(p: Pos2, pts: &[Pos2]) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for (i, a) in pts.iter().enumerate() {
        let b = pts[j];
        if (a.y > p.y) != (b.y > p.y)
            && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y + 1e-6) + a.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Snap a coordinate to the grid (when snapping is on).
pub fn snap(v: f32, on: bool) -> f32 {
    if on {
        (v / GRID_MINOR).round() * GRID_MINOR
    } else {
        v
    }
}

pub fn snap_p(p: Pos2, on: bool) -> Pos2 {
    pos2(snap(p.x, on), snap(p.y, on))
}
