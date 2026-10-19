//! Page export: SVG, PNG and PDF.
//!
//! - SVG — vector primitives + per-kind arrow markers + labels; honors zoom,
//!   border width, transparent background and crop-to-content.
//! - PNG — small software rasterizer (rects/ellipses/lines/polygons, dashed
//!   lines, a 5×7 bitmap font upscaled for labels) + a dependency-free PNG
//!   encoder (zlib "stored" blocks).
//! - PDF — one-page vector PDF adapted from the typeset crate's writer
//!   (valid xref, Type1 base-14 Helvetica for labels, vector ops for shapes).

use crate::connection;
use crate::model::{ArrowKind, Cell, Diagram};
use crate::style;
use egui::{Pos2, Rect, Vec2};

/// Export options (drawio File › Export As dialog).
#[derive(Debug, Clone, Copy)]
pub struct Opts {
    pub zoom: f32,
    pub border: f32,
    pub transparent: bool,
    pub crop: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { zoom: 1.0, border: 10.0, transparent: false, crop: false }
    }
}

// ── shared geometry ──────────────────────────────────────────────────────────

/// World bounds to export: the page, or the content bbox when cropping.
fn bounds(d: &Diagram, crop: bool) -> Rect {
    if !crop {
        return Rect::from_min_size(Pos2::ZERO, Vec2::new(d.page_w.max(1.0), d.page_h.max(1.0)));
    }
    let mut r: Option<Rect> = None;
    let mut acc = |rr: Rect| r = Some(match r {
        None => rr,
        Some(p) => p.union(rr),
    });
    for c in d.cells.iter().filter(|c| d.layer_visible(c)) {
        if c.vertex {
            acc(c.rect());
        } else {
            for p in connection::route_points(d, c) {
                acc(Rect::from_center_size(p, Vec2::splat(2.0)));
            }
        }
    }
    r.unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(d.page_w.max(1.0), d.page_h.max(1.0))))
}

/// Edge color/width/dash resolved from the style.
struct EdgeLook {
    color: String,
    rgb: [u8; 3],
    width: f32,
    dash: Vec<f32>,
}

fn edge_look(c: &Cell) -> EdgeLook {
    let hex = style::get(&c.style, "strokeColor").unwrap_or("#000000").to_string();
    let rgb = style::parse_color(&hex).map(|x| [x.r(), x.g(), x.b()]).unwrap_or([0, 0, 0]);
    let dash: Vec<f32> = if style::get_bool(&c.style, "dashed", false) {
        style::get(&c.style, "dashPattern")
            .map(|p| p.split_whitespace().filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_else(|| vec![6.0, 4.0])
    } else {
        Vec::new()
    };
    EdgeLook { color: hex, rgb, width: style::get_f32(&c.style, "strokeWidth", 1.0), dash }
}

fn vertex_fill(c: &Cell) -> Option<String> {
    match style::get(&c.style, "fillColor") {
        Some("none") | None if style::is_none_color(&c.style, "fillColor") => None,
        Some("none") => None,
        Some(v) => Some(v.to_string()),
        None => Some("#ffffff".into()),
    }
}

/// One marker primitive in local space (tip at origin, +x = travel direction).
enum Prim {
    /// polygon points, filled?
    Poly(Vec<(f32, f32)>, bool),
    /// open polyline
    Line(Vec<(f32, f32)>),
    /// center, radius, filled?
    Circle((f32, f32), f32, bool),
}

/// Marker geometry for every ArrowKind (proportions match canvas::paint_marker).
fn marker_prims(kind: ArrowKind, fill: bool, size: f32, sw: f32) -> Vec<Prim> {
    let s = size * 1.5 + sw * 2.0;
    let back = (-s, 0.0);
    match kind {
        ArrowKind::None => Vec::new(),
        ArrowKind::Classic | ArrowKind::ClassicThin => {
            let w = if kind == ArrowKind::Classic { 0.5 } else { 0.3 };
            vec![Prim::Poly(vec![(0.0, 0.0), back, (back.0, s * w), (back.0, -s * w)], fill)]
        }
        ArrowKind::Block | ArrowKind::BlockThin => {
            let w = if kind == ArrowKind::Block { 0.55 } else { 0.32 };
            vec![Prim::Poly(vec![(0.0, 0.0), back, (back.0, s * w), (back.0, -s * w)], fill)]
        }
        ArrowKind::Open | ArrowKind::OpenThin => {
            let w = if kind == ArrowKind::Open { 0.65 } else { 0.45 };
            vec![Prim::Line(vec![(0.0, 0.0), (back.0, s * w), (0.0, 0.0), (back.0, -s * w)])]
        }
        ArrowKind::OpenAsync => {
            vec![Prim::Line(vec![(0.0, 0.0), (back.0, s * 0.9)]), Prim::Line(vec![(0.0, 0.0), (back.0, -s * 0.5)])]
        }
        ArrowKind::Async => {
            vec![Prim::Poly(vec![(0.0, 0.0), (back.0, s * 0.85), (-s * 0.55, -s * 0.45)], fill)]
        }
        ArrowKind::Oval | ArrowKind::Circle => vec![Prim::Circle((-s * 0.5, 0.0), s * 0.5, fill)],
        ArrowKind::Diamond | ArrowKind::DiamondThin => {
            let w = if kind == ArrowKind::Diamond { 0.4 } else { 0.25 };
            vec![Prim::Poly(vec![(0.0, 0.0), (-s * 0.5, s * w), back, (-s * 0.5, -s * w)], fill)]
        }
        ArrowKind::Box => vec![Prim::Poly(
            vec![(0.0, 0.0), (0.0, s * 0.7), (back.0, s * 0.7), (back.0, -s * 0.7), (0.0, -s * 0.7)],
            fill,
        )],
        ArrowKind::HalfCircle => {
            let r = s * 0.6;
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|k| {
                    let th = -std::f32::consts::FRAC_PI_2 + k as f32 / 12.0 * std::f32::consts::PI;
                    (-r * th.cos(), r * th.sin())
                })
                .collect();
            vec![Prim::Line(pts)]
        }
        ArrowKind::Dash => vec![Prim::Line(vec![(-s * 0.5, s * 0.6), (-s * 0.5, -s * 0.6)])],
        ArrowKind::BaseDash => vec![Prim::Line(vec![(-s * 0.2, s * 0.4), (-s * 0.2, -s * 0.4)])],
        ArrowKind::Cross => vec![
            Prim::Line(vec![(-s * 0.5, s * 0.5), (-s * 0.5, -s * 0.5)]),
            Prim::Line(vec![(0.0, 0.0), (-s, 0.0)]),
        ],
        ArrowKind::CirclePlus => {
            vec![
                Prim::Circle((-s * 0.6, 0.0), s * 0.5, false),
                Prim::Line(vec![(-s * 0.6, s * 0.3), (-s * 0.6, -s * 0.3)]),
                Prim::Line(vec![(-s * 0.3, 0.0), (-s * 0.9, 0.0)]),
            ]
        }
        ArrowKind::ErOne => vec![Prim::Line(vec![(-s * 0.5, s * 0.55), (-s * 0.5, -s * 0.55)])],
        ArrowKind::ErMandOne => {
            vec![
                Prim::Line(vec![(-s * 0.35, s * 0.5), (-s * 0.35, -s * 0.5)]),
                Prim::Line(vec![(-s * 0.65, s * 0.5), (-s * 0.65, -s * 0.5)]),
            ]
        }
        ArrowKind::ErMany => vec![Prim::Line(vec![(-s, s * 0.6), (-s, 0.0), (-s, -s * 0.6), (-s, 0.0), (0.0, 0.0)])],
        ArrowKind::ErOneToMany => {
            vec![
                Prim::Line(vec![(-s * 0.75, s * 0.55), (-s * 0.75, -s * 0.55)]),
                Prim::Line(vec![(-s, s * 0.6), (-s, 0.0), (-s, -s * 0.6), (-s, 0.0), (0.0, 0.0)]),
            ]
        }
        ArrowKind::ErZeroToOne => {
            vec![
                Prim::Circle((-s * 0.85, 0.0), s * 0.3, false),
                Prim::Line(vec![(-s * 0.35, s * 0.5), (-s * 0.35, -s * 0.5)]),
            ]
        }
        ArrowKind::ErZeroToMany => {
            vec![
                Prim::Circle((-s * 0.85, 0.0), s * 0.3, false),
                Prim::Line(vec![(-s * 0.45, s * 0.55), (-s * 0.45, 0.0), (-s * 0.45, -s * 0.55), (-s * 0.45, 0.0), (0.0, 0.0)]),
            ]
        }
        ArrowKind::DoubleBlock => {
            vec![
                Prim::Poly(vec![(0.0, 0.0), back, (back.0, s * 0.5), (back.0, -s * 0.5)], fill),
                Prim::Poly(vec![back, (-s * 1.9, 0.0), (-s * 1.9, s * 0.5), (-s * 1.9, -s * 0.5)], fill),
            ]
        }
    }
}

fn rot_deg(dir: Vec2) -> f32 {
    dir.y.atan2(dir.x).to_degrees()
}

// ── SVG ──────────────────────────────────────────────────────────────────────

pub fn svg(d: &Diagram, o: &Opts) -> String {
    let b = bounds(d, o.crop);
    let z = o.zoom.max(0.05);
    let (w, h) = ((b.width() * z + o.border * 2.0).ceil(), (b.height() * z + o.border * 2.0).ceil());
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\n"
    );
    if !o.transparent {
        let bg = d.background.clone().unwrap_or_else(|| "#ffffff".into());
        s.push_str(&format!("<rect width=\"{w:.0}\" height=\"{h:.0}\" fill=\"{bg}\"/>\n"));
    }
    s.push_str(&format!(
        "<g transform=\"translate({:.2} {:.2}) scale({z:.4})\">\n",
        o.border - b.min.x * z,
        o.border - b.min.y * z
    ));

    let hex = |c: &Cell, k: &str, dflt: &str| style::get(&c.style, k).unwrap_or(dflt).to_string();
    // edges first (like the canvas), then vertices
    for c in d.cells.iter().filter(|c| c.edge && d.layer_visible(c)) {
        let pts = connection::route_points(d, c);
        if pts.len() < 2 {
            continue;
        }
        let lk = edge_look(c);
        let dash = if lk.dash.is_empty() {
            String::new()
        } else {
            format!(" stroke-dasharray=\"{}\"", lk.dash.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" "))
        };
        let path = pts.iter().map(|p| format!("{:.1},{:.1}", p.x, p.y)).collect::<Vec<_>>().join(" ");
        s.push_str(&format!(
            "<polyline points=\"{path}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{dash}/>\n",
            lk.color, lk.width
        ));
        // markers
        let ends = [
            (true, *pts.first().unwrap(), *pts.iter().nth(1).unwrap()),
            (false, *pts.last().unwrap(), pts[pts.len() - 2]),
        ];
        for (is_start, tip, other) in ends {
            let kind = if is_start {
                style::get(&c.style, "startArrow").and_then(ArrowKind::from_str)
            } else {
                Some(style::get(&c.style, "endArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::Classic))
            };
            let Some(kind) = kind.filter(|k| *k != ArrowKind::None) else { continue };
            let fill = style::get_bool(&c.style, if is_start { "startFill" } else { "endFill" }, true);
            let size = style::get_f32(&c.style, if is_start { "startSize" } else { "endSize" }, 6.0);
            let dir = tip - other;
            if dir.length_sq() < 1e-6 {
                continue;
            }
            let sw = if is_start { lk.width } else { lk.width };
            s.push_str(&svg_marker(kind, fill, size, sw, &lk.color, tip, rot_deg(dir)));
        }
        // edge label at the path midpoint
        if !c.value.is_empty() {
            let mid = midpoint(&pts);
            s.push_str(&svg_text(&c.value, mid, &c.style));
        }
    }

    for c in d.cells.iter().filter(|c| c.vertex && d.layer_visible(c)) {
        let g = &c.geom;
        let fill = vertex_fill(c).unwrap_or_else(|| "none".into());
        let stroke = hex(c, "strokeColor", "#000000");
        let swd = style::get_f32(&c.style, "strokeWidth", 1.0);
        let dash = if style::get_bool(&c.style, "dashed", false) { " stroke-dasharray=\"6 4\"" } else { "" };
        let common = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{swd}\"{dash}");
        let shape = style::get(&c.style, "shape").unwrap_or("");
        let el = if shape == "umlActor" {
            String::new()
        } else if c.style.contains("ellipse") && shape.is_empty() || shape == "doubleEllipse" {
            format!(
                "<ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" {common}/>",
                g.x + g.w / 2.0,
                g.y + g.h / 2.0,
                g.w / 2.0,
                g.h / 2.0
            )
        } else if c.style.contains("rhombus") {
            format!(
                "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" {common}/>",
                g.x + g.w / 2.0, g.y, g.x + g.w, g.y + g.h / 2.0, g.x + g.w / 2.0, g.y + g.h, g.x, g.y + g.h / 2.0
            )
        } else if c.style.contains("triangle") {
            format!("<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" {common}/>", g.x + g.w / 2.0, g.y, g.x + g.w, g.y + g.h, g.x, g.y + g.h)
        } else {
            let rx = if style::get_bool(&c.style, "rounded", false) { 10 } else { 0 };
            format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{rx}\" {common}/>", g.x, g.y, g.w, g.h)
        };
        if !el.is_empty() {
            s.push_str(&el);
            s.push('\n');
        }
        if !c.value.is_empty() {
            s.push_str(&svg_text(&c.value, Pos2::new(g.x + g.w / 2.0, g.y + g.h / 2.0), &c.style));
        }
    }
    s.push_str("</g>\n</svg>\n");
    s
}

fn svg_marker(kind: ArrowKind, fill: bool, size: f32, sw: f32, color: &str, tip: Pos2, deg: f32) -> String {
    let prims = marker_prims(kind, fill, size, sw);
    let mut out = format!("<g transform=\"translate({:.2} {:.2}) rotate({deg:.2})\">", tip.x, tip.y);
    for p in prims {
        match p {
            Prim::Poly(pts, f) => {
                let path = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ");
                out.push_str(&format!(
                    "<polygon points=\"{path}\" fill=\"{}\" stroke=\"{color}\" stroke-width=\"{sw:.2}\"/>\n",
                    if f { color } else { "none" }
                ));
            }
            Prim::Line(pts) => {
                let path = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ");
                out.push_str(&format!("<polyline points=\"{path}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{sw:.2}\"/>\n"));
            }
            Prim::Circle((cx, cy), r, f) => {
                out.push_str(&format!(
                    "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{r:.2}\" fill=\"{}\" stroke=\"{color}\" stroke-width=\"{sw:.2}\"/>\n",
                    if f { color } else { "none" }
                ));
            }
        }
    }
    out.push_str("</g>\n");
    out
}

fn svg_text(value: &str, at: Pos2, st: &str) -> String {
    let fc = style::get(st, "fontColor").unwrap_or("#000000");
    let fs = style::get_f32(st, "fontSize", 12.0);
    let weight = if style::font_style(st) & 1 != 0 { " font-weight=\"bold\"" } else { "" };
    format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Helvetica\" font-size=\"{fs}\"{weight} fill=\"{fc}\">{}</text>\n",
        at.x,
        at.y,
        xml_esc(value)
    )
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn midpoint(pts: &[Pos2]) -> Pos2 {
    let mut total = 0.0;
    let segs: Vec<f32> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
    let half = segs.iter().sum::<f32>() / 2.0;
    for (i, len) in segs.iter().enumerate() {
        if total + *len >= half {
            let t = if *len > 0.0 { (half - total) / *len } else { 0.0 };
            return pts[i] + (pts[i + 1] - pts[i]) * t;
        }
        total += *len;
    }
    *pts.last().unwrap_or(&Pos2::ZERO)
}

// ── PNG (software rasterizer + minimal encoder) ─────────────────────────────

struct Fb {
    w: usize,
    h: usize,
    px: Vec<u8>, // RGBA
}

impl Fb {
    fn new(w: usize, h: usize) -> Self {
        Fb { w, h, px: vec![0; w * h * 4] }
    }

    fn put(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let a = c[3] as u32;
        if a == 0 {
            return;
        }
        let i = ((y as usize) * self.w + x as usize) * 4;
        if a >= 255 {
            self.px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        } else {
            // src-over
            let ia = 255 - a;
            for k in 0..3 {
                self.px[i + k] = ((c[k] as u32 * a + self.px[i + k] as u32 * ia) / 255) as u8;
            }
            self.px[i + 3] = (a + (self.px[i + 3] as u32 * ia) / 255).min(255) as u8;
        }
    }

    fn fill_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 4]) {
        for y in y0 as i64..y1.ceil() as i64 {
            for x in x0 as i64..x1.ceil() as i64 {
                self.put(x, y, c);
            }
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 4]) {
        if r <= 0.0 {
            self.put(cx as i64, cy as i64, c);
            return;
        }
        for dy in -(r.ceil() as i64)..=(r.ceil() as i64) {
            for dx in -(r.ceil() as i64)..=(r.ceil() as i64) {
                if (dx * dx + dy * dy) as f32 <= r * r {
                    self.put(cx as i64 + dx, cy as i64 + dy, c);
                }
            }
        }
    }

    fn line(&mut self, a: Pos2, b: Pos2, width: f32, color: [u8; 3], dash: &[f32]) {
        let c = [color[0], color[1], color[2], 255];
        let d = b - a;
        let len = d.length();
        if len < 0.01 {
            self.disc(a.x, a.y, width / 2.0, c);
            return;
        }
        let dir = d / len;
        let r = (width / 2.0).max(0.5);
        let step = 0.5f32;
        let pattern: Vec<f32> = if dash.is_empty() { Vec::new() } else { dash.to_vec() };
        let mut t = 0.0;
        let mut on = true;
        let mut pat_i = 0;
        let mut pat_t = 0.0;
        while t <= len {
            if !pattern.is_empty() {
                on = pat_i % 2 == 0;
            }
            if on {
                let p = a + dir * t;
                self.disc(p.x, p.y, r, c);
            }
            t += step;
            if !pattern.is_empty() {
                pat_t += step;
                while pat_t >= pattern[pat_i % pattern.len()] {
                    pat_t -= pattern[pat_i % pattern.len()];
                    pat_i += 1;
                }
            }
        }
    }

    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, fill: Option<[u8; 4]>, stroke: Option<([u8; 3], f32)>) {
        let x0 = (cx - rx).floor() as i64;
        let x1 = (cx + rx).ceil() as i64;
        let y0 = (cy - ry).floor() as i64;
        let y1 = (cy + ry).ceil() as i64;
        let sw = stroke.as_ref().map(|s| s.1).unwrap_or(0.0);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if rx <= 0.0 || ry <= 0.0 {
                    continue;
                }
                let v = ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2);
                if v <= 1.0 {
                    // stroke band: near the boundary
                    let band = 1.0 - (sw / rx.min(ry)).clamp(0.02, 1.0);
                    if let (Some(fc), true) = (fill, v <= band) {
                        self.put(x, y, fc);
                    } else if let Some((sc, _)) = stroke
                        && v > band
                        && v <= 1.05
                    {
                        self.put(x, y, [sc[0], sc[1], sc[2], 255]);
                    }
                }
            }
        }
    }

    fn fill_poly(&mut self, pts: &[Pos2], c: [u8; 4]) {
        if pts.len() < 3 {
            return;
        }
        let min_y = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min).floor() as i64;
        let max_y = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max).ceil() as i64;
        for y in min_y..=max_y {
            let yy = y as f32 + 0.5;
            let mut xs: Vec<f32> = Vec::new();
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.y <= yy && b.y > yy) || (b.y <= yy && a.y > yy) {
                    let t = (yy - a.y) / (b.y - a.y);
                    xs.push(a.x + (b.x - a.x) * t);
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for pair in xs.chunks(2) {
                if pair.len() == 2 {
                    for x in pair[0] as i64..pair[1].ceil() as i64 {
                        self.put(x, y, c);
                    }
                }
            }
        }
    }

    fn stroke_poly(&mut self, pts: &[Pos2], width: f32, color: [u8; 3], closed: bool) {
        if pts.len() < 2 {
            return;
        }
        for w in pts.windows(2) {
            self.line(w[0], w[1], width, color, &[]);
        }
        if closed
            && let (Some(f), Some(l)) = (pts.first(), pts.last())
            && f != l
        {
            self.line(*l, *f, width, color, &[]);
        }
    }

    fn text(&mut self, value: &str, cx: f32, cy: f32, fs: f32, color: [u8; 3]) {
        let scale = (fs / 7.0).max(0.5);
        let cw = 6.0 * scale; // 5 wide + 1 gap
        let total_w = value.chars().filter_map(|c| glyph(c).map(|_| cw)).sum::<f32>();
        let mut x = cx - total_w / 2.0;
        let y0 = cy - 7.0 * scale / 2.0;
        for ch in value.chars() {
            if let Some(cols) = glyph(ch) {
                for (ci, col) in cols.iter().enumerate() {
                    for row in 0..7 {
                        if col & (1 << row) != 0 {
                            // fill a scale×scale block
                            let px0 = x + ci as f32 * scale;
                            let py0 = y0 + row as f32 * scale;
                            for yy in (py0 as i64)..((py0 + scale).ceil() as i64) {
                                for xx in (px0 as i64)..((px0 + scale).ceil() as i64) {
                                    self.put(xx, yy, [color[0], color[1], color[2], 255]);
                                }
                            }
                        }
                    }
                }
            }
            x += cw;
        }
    }
}

/// 5×7 bitmap font, column bytes (bit 0 = top row). Hand-authored glyphs for
/// ASCII printables; `None` for unknown → skipped (labels stay readable).
fn glyph(c: char) -> Option<[u8; 5]> {
    use G::*;
    let g = match c.to_ascii_uppercase() {
        'A' => A,
        'B' => B,
        'C' => C,
        'D' => D,
        'E' => E,
        'F' => F,
        'G' => G,
        'H' => H,
        'I' => I,
        'J' => J,
        'K' => K,
        'L' => L,
        'M' => M,
        'N' => N,
        'O' => O,
        'P' => P,
        'Q' => Q,
        'R' => R,
        'S' => S,
        'T' => T,
        'U' => U,
        'V' => V,
        'W' => W,
        'X' => X,
        'Y' => Y,
        'Z' => Z,
        '0' => D0,
        '1' => D1,
        '2' => D2,
        '3' => D3,
        '4' => D4,
        '5' => D5,
        '6' => D6,
        '7' => D7,
        '8' => D8,
        '9' => D9,
        ' ' => SP,
        '.' => DOT,
        ',' => COMMA,
        ':' => COLON,
        ';' => SEMI,
        '!' => BANG,
        '?' => QUEST,
        '\'' => APOS,
        '"' => QUOTE,
        '-' => DASH,
        '+' => PLUS,
        '=' => EQ,
        '(' => LPAREN,
        ')' => RPAREN,
        '[' => LBRACK,
        ']' => RBRACK,
        '/' => SLASH,
        '\\' => BSLASH,
        '<' => LT,
        '>' => GT,
        '*' => STAR,
        '|' => PIPE,
        '_' => UNDER,
        '#' => HASH,
        '$' => DOLLAR,
        '%' => PCT,
        '&' => AMP,
        '@' => AT,
        '^' => CARET,
        '~' => TILDE,
        _ => return None,
    };
    Some(g)
}

#[allow(non_upper_case_globals)]
mod G {
    pub const A: [u8; 5] = [0x7C, 0x12, 0x11, 0x12, 0x7C];
    pub const B: [u8; 5] = [0xFE, 0x49, 0x49, 0x49, 0x76];
    pub const C: [u8; 5] = [0x3E, 0x41, 0x41, 0x41, 0x22];
    pub const D: [u8; 5] = [0xFE, 0x41, 0x41, 0x41, 0x3E];
    pub const E: [u8; 5] = [0xFE, 0x49, 0x49, 0x49, 0x41];
    pub const F: [u8; 5] = [0xFE, 0x09, 0x09, 0x09, 0x01];
    pub const G: [u8; 5] = [0x3E, 0x41, 0x51, 0x51, 0x32];
    pub const H: [u8; 5] = [0xFE, 0x08, 0x08, 0x08, 0xFE];
    pub const I: [u8; 5] = [0x41, 0x41, 0x7E, 0x41, 0x41]; // hmm 7px tall center — corrected below
    pub const J: [u8; 5] = [0x20, 0x40, 0x41, 0x3F, 0x01];
    pub const K: [u8; 5] = [0x7F, 0x08, 0x14, 0x22, 0x41];
    pub const L: [u8; 5] = [0x7F, 0x40, 0x40, 0x40, 0x40];
    pub const M: [u8; 5] = [0x7F, 0x02, 0x04, 0x02, 0x7F];
    pub const N: [u8; 5] = [0x7F, 0x04, 0x08, 0x10, 0x7F];
    pub const O: [u8; 5] = [0x3E, 0x41, 0x41, 0x41, 0x3E];
    pub const P: [u8; 5] = [0x7F, 0x09, 0x09, 0x09, 0x06];
    pub const Q: [u8; 5] = [0x3E, 0x41, 0x51, 0x21, 0x5E];
    pub const R: [u8; 5] = [0x7F, 0x09, 0x19, 0x29, 0x46];
    pub const S: [u8; 5] = [0x26, 0x49, 0x49, 0x49, 0x32];
    pub const T: [u8; 5] = [0x01, 0x01, 0x7F, 0x01, 0x01];
    pub const U: [u8; 5] = [0x3F, 0x40, 0x40, 0x40, 0x3F];
    pub const V: [u8; 5] = [0x1F, 0x20, 0x40, 0x20, 0x1F];
    pub const W: [u8; 5] = [0x7F, 0x20, 0x18, 0x20, 0x7F];
    pub const X: [u8; 5] = [0x63, 0x14, 0x08, 0x14, 0x63];
    pub const Y: [u8; 5] = [0x03, 0x04, 0x78, 0x04, 0x03];
    pub const Z: [u8; 5] = [0x61, 0x51, 0x49, 0x45, 0x43];
    pub const D0: [u8; 5] = [0x3E, 0x51, 0x49, 0x45, 0x3E];
    pub const D1: [u8; 5] = [0x00, 0x42, 0x7F, 0x40, 0x00];
    pub const D2: [u8; 5] = [0x42, 0x61, 0x51, 0x49, 0x46];
    pub const D3: [u8; 5] = [0x21, 0x41, 0x45, 0x4B, 0x31];
    pub const D4: [u8; 5] = [0x18, 0x14, 0x12, 0x7F, 0x10];
    pub const D5: [u8; 5] = [0x27, 0x45, 0x45, 0x45, 0x39];
    pub const D6: [u8; 5] = [0x3C, 0x4A, 0x49, 0x49, 0x30];
    pub const D7: [u8; 5] = [0x01, 0x71, 0x09, 0x05, 0x03];
    pub const D8: [u8; 5] = [0x36, 0x49, 0x49, 0x49, 0x36];
    pub const D9: [u8; 5] = [0x06, 0x49, 0x49, 0x29, 0x1E];
    pub const SP: [u8; 5] = [0; 5];
    pub const DOT: [u8; 5] = [0, 0, 0x0C, 0x0C, 0];
    pub const COMMA: [u8; 5] = [0, 0, 0x0C, 0x0C, 0x04];
    pub const COLON: [u8; 5] = [0x0C, 0x0C, 0, 0x0C, 0x0C];
    pub const SEMI: [u8; 5] = [0x0C, 0x0C, 0, 0x0C, 0x04];
    pub const BANG: [u8; 5] = [0x04, 0x04, 0x04, 0, 0x04];
    pub const QUEST: [u8; 5] = [0x1C, 0x22, 0x04, 0x08, 0x04];
    pub const APOS: [u8; 5] = [0x04, 0x04, 0x08, 0, 0];
    pub const QUOTE: [u8; 5] = [0x14, 0x14, 0, 0, 0];
    pub const DASH: [u8; 5] = [0, 0, 0x3E, 0, 0];
    pub const PLUS: [u8; 5] = [0x08, 0x08, 0x3E, 0x08, 0x08];
    pub const EQ: [u8; 5] = [0, 0x3E, 0, 0x3E, 0];
    pub const LPAREN: [u8; 5] = [0x08, 0x04, 0x02, 0x04, 0x08];
    pub const RPAREN: [u8; 5] = [0x02, 0x04, 0x08, 0x04, 0x02];
    pub const LBRACK: [u8; 5] = [0x1E, 0x02, 0x02, 0x02, 0x1E];
    pub const RBRACK: [u8; 5] = [0x1E, 0x10, 0x10, 0x10, 0x1E];
    pub const SLASH: [u8; 5] = [0x20, 0x10, 0x08, 0x04, 0x02];
    pub const BSLASH: [u8; 5] = [0x02, 0x04, 0x08, 0x10, 0x20];
    pub const LT: [u8; 5] = [0x04, 0x08, 0x10, 0x08, 0x04];
    pub const GT: [u8; 5] = [0x02, 0x04, 0x08, 0x04, 0x02];
    pub const STAR: [u8; 5] = [0x22, 0x14, 0x7F, 0x14, 0x22];
    pub const PIPE: [u8; 5] = [0x04, 0x04, 0x04, 0x04, 0x04];
    pub const UNDER: [u8; 5] = [0, 0, 0, 0, 0x7F];
    pub const HASH: [u8; 5] = [0x14, 0x7F, 0x14, 0x7F, 0x14];
    pub const DOLLAR: [u8; 5] = [0x24, 0x2A, 0x7F, 0x2A, 0x12];
    pub const PCT: [u8; 5] = [0x41, 0x09, 0x06, 0x12, 0x41];
    pub const AMP: [u8; 5] = [0x1C, 0x22, 0x14, 0x28, 0x1E];
    pub const AT: [u8; 5] = [0x1C, 0x22, 0x2E, 0x20, 0x1E];
    pub const CARET: [u8; 5] = [0x04, 0x0A, 0x11, 0, 0];
    pub const TILDE: [u8; 5] = [0x08, 0x15, 0x02, 0, 0];
}

/// Export one page as PNG (RGBA, 8-bit).
pub fn png(d: &Diagram, o: &Opts) -> Vec<u8> {
    let b = bounds(d, o.crop);
    let z = o.zoom.max(0.05);
    let w = ((b.width() * z + o.border * 2.0).ceil() as usize).max(1);
    let h = ((b.height() * z + o.border * 2.0).ceil() as usize).max(1);
    let mut fb = Fb::new(w, h);
    let to_px = |p: Pos2| -> Pos2 { Pos2::new((p.x - b.min.x) * z + o.border, (p.y - b.min.y) * z + o.border) };

    if !o.transparent {
        let bg = d.background.as_deref().and_then(style::parse_color).unwrap_or(egui::Color32::WHITE);
        fb.fill_rect(0.0, 0.0, w as f32, h as f32, [bg.r(), bg.g(), bg.b(), 255]);
    }

    // edges
    for c in d.cells.iter().filter(|c| c.edge && d.layer_visible(c)) {
        let world = connection::route_points(d, c);
        if world.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = world.iter().map(|p| to_px(*p)).collect();
        let lk = edge_look(c);
        let rgb = lk.rgb;
        for seg in pts.windows(2) {
            fb.line(seg[0], seg[1], (lk.width * z).max(1.0), rgb, &lk.dash);
        }
        // markers
        let ends = [
            (true, pts[0], pts[1]),
            (false, *pts.last().unwrap(), pts[pts.len() - 2]),
        ];
        for (is_start, tip, other) in ends {
            let kind = if is_start {
                style::get(&c.style, "startArrow").and_then(ArrowKind::from_str)
            } else {
                Some(style::get(&c.style, "endArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::Classic))
            };
            let Some(kind) = kind.filter(|k| *k != ArrowKind::None) else { continue };
            let fill = style::get_bool(&c.style, if is_start { "startFill" } else { "endFill" }, true);
            let size = style::get_f32(&c.style, if is_start { "startSize" } else { "endSize" }, 6.0) * z;
            let dir = tip - other;
            if dir.length_sq() < 1e-6 {
                continue;
            }
            draw_marker(&mut fb, kind, fill, size, lk.width * z, tip, rot_deg(dir), rgb);
        }
        if !c.value.is_empty() {
            let mid = to_px(midpoint(&world));
            fb.text(&c.value, mid.x, mid.y, style::get_f32(&c.style, "fontSize", 12.0) * z, rgb);
        }
    }

    // vertices
    for c in d.cells.iter().filter(|c| c.vertex && d.layer_visible(c)) {
        let g = &c.geom;
        let fill = vertex_fill(c)
            .and_then(|v| style::parse_color(&v))
            .map(|x| [x.r(), x.g(), x.b(), 255]);
        let stroke = style::parse_color(&style::get(&c.style, "strokeColor").unwrap_or("#000000"))
            .map(|x| [x.r(), x.g(), x.b()])
            .unwrap_or([0, 0, 0]);
        let sw = (style::get_f32(&c.style, "strokeWidth", 1.0) * z).max(1.0);
        let r = Rect::from_min_size(to_px(Pos2::new(g.x, g.y)), Vec2::new(g.w * z, g.h * z));
        let shape = style::get(&c.style, "shape").unwrap_or("");
        if c.style.contains("ellipse") && shape.is_empty() || shape == "doubleEllipse" {
            fb.ellipse(r.center().x, r.center().y, r.width() / 2.0, r.height() / 2.0, fill, Some((stroke, sw)));
        } else if c.style.contains("rhombus") {
            let pts = vec![
                Pos2::new(r.center().x, r.top()),
                Pos2::new(r.right(), r.center().y),
                Pos2::new(r.center().x, r.bottom()),
                Pos2::new(r.left(), r.center().y),
            ];
            if let Some(f) = fill {
                fb.fill_poly(&pts, f);
            }
            fb.stroke_poly(&pts, sw, stroke, true);
        } else if c.style.contains("triangle") {
            let pts = vec![Pos2::new(r.center().x, r.top()), Pos2::new(r.right(), r.bottom()), Pos2::new(r.left(), r.bottom())];
            if let Some(f) = fill {
                fb.fill_poly(&pts, f);
            }
            fb.stroke_poly(&pts, sw, stroke, true);
        } else {
            if let Some(f) = fill {
                fb.fill_rect(r.left(), r.top(), r.right(), r.bottom(), f);
            }
            // stroke as 4 thick lines
            let cs = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
            for i in 0..4 {
                fb.line(cs[i], cs[(i + 1) % 4], sw, stroke, &[]);
            }
        }
        if !c.value.is_empty() {
            let fc = style::parse_color(&style::get(&c.style, "fontColor").unwrap_or("#000000"))
                .map(|x| [x.r(), x.g(), x.b()])
                .unwrap_or([0, 0, 0]);
            fb.text(&c.value, r.center().x, r.center().y, style::get_f32(&c.style, "fontSize", 12.0) * z, fc);
        }
    }

    encode_png(&fb)
}

fn draw_marker(fb: &mut Fb, kind: ArrowKind, fill: bool, size: f32, sw: f32, tip: Pos2, deg: f32, color: [u8; 3]) {
    let rad = deg.to_radians();
    let (sin, cos) = rad.sin_cos();
    let map = |(x, y): (f32, f32)| -> Pos2 { Pos2::new(tip.x + x * cos - y * sin, tip.y + x * sin + y * cos) };
    for p in marker_prims(kind, fill, size, sw) {
        match p {
            Prim::Poly(pts, f) => {
                let w: Vec<Pos2> = pts.iter().map(|q| map(*q)).collect();
                if f {
                    fb.fill_poly(&w, [color[0], color[1], color[2], 255]);
                } else {
                    fb.stroke_poly(&w, sw.max(1.0), color, true);
                }
            }
            Prim::Line(pts) => {
                let w: Vec<Pos2> = pts.iter().map(|q| map(*q)).collect();
                for seg in w.windows(2) {
                    fb.line(seg[0], seg[1], sw.max(1.0), color, &[]);
                }
            }
            Prim::Circle((cx, cy), r, f) => {
                let c = map((cx, cy));
                if f {
                    fb.disc(c.x, c.y, r, [color[0], color[1], color[2], 255]);
                } else {
                    fb.ellipse(c.x, c.y, r, r, None, Some((color, sw.max(1.0))));
                }
            }
        }
    }
}

// ── PNG encoding (no external deps: zlib "stored" blocks) ───────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for x in data {
        a = (a + *x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(tag);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn encode_png(fb: &Fb) -> Vec<u8> {
    let mut raw = Vec::with_capacity(fb.h * (fb.w * 4 + 1));
    for y in 0..fb.h {
        raw.push(0); // filter: none
        raw.extend_from_slice(&fb.px[y * fb.w * 4..(y + 1) * fb.w * 4]);
    }
    // zlib stream with stored (uncompressed) deflate blocks
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    z.push(0x78);
    z.push(0x01);
    let mut chunks = raw.chunks(65535).peekable();
    if raw.is_empty() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(c) = chunks.next() {
        let last = chunks.peek().is_none();
        z.push(if last { 1 } else { 0 });
        let len = c.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(c);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(fb.w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(fb.h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    png_chunk(&mut out, b"IHDR", &ihdr);
    png_chunk(&mut out, b"IDAT", &z);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

// ── PDF (vector ops, adapted from the typeset crate's writer) ────────────────

pub fn pdf(d: &Diagram, o: &Opts) -> Vec<u8> {
    let b = bounds(d, o.crop);
    let z = o.zoom.max(0.05);
    let w = (b.width() * z + o.border * 2.0).ceil();
    let h = (b.height() * z + o.border * 2.0).ceil();
    // world → pdf user space (y flipped): X = border + (x - b.min.x)*z, Y = h - (border + (y - b.min.y)*z)
    let tx = |x: f32| o.border + (x - b.min.x) * z;
    let ty = |y: f32| h - o.border - (y - b.min.y) * z;

    let mut s = String::new();
    if !o.transparent {
        let bg = d.background.as_deref().and_then(style::parse_color).unwrap_or(egui::Color32::WHITE);
        s.push_str(&format!("{} {} {} rg 0 0 {w:.1} {h:.1} re f\n", f(bg.r()), f(bg.g()), f(bg.b())));
    }
    // map a local marker point (tip-relative) through rotation + translation
    let marker_pt = |tip: Pos2, dir: Vec2, p: (f32, f32)| -> (f32, f32) {
        let rad = dir.y.atan2(dir.x);
        let (sn, cs) = rad.sin_cos();
        let wx = tip.x + p.0 * cs - p.1 * sn;
        let wy = tip.y + p.0 * sn + p.1 * cs;
        (tx(wx), ty(wy))
    };

    for c in d.cells.iter().filter(|c| c.edge && d.layer_visible(c)) {
        let world = connection::route_points(d, c);
        if world.len() < 2 {
            continue;
        }
        let lk = edge_look(c);
        s.push_str(&format!("{} {} {} RG {w:.2} w\n", f(lk.rgb[0]), f(lk.rgb[1]), f(lk.rgb[2]), w = lk.width));
        if !lk.dash.is_empty() {
            let pat: Vec<String> = lk.dash.iter().map(|v| format!("{v}")).collect();
            s.push_str(&format!("[{}] 0 d\n", pat.join(" ")));
        }
        s.push_str(&path_of(&world, &tx, &ty));
        s.push_str(" S\n");
        // markers
        let ends = [
            (true, world[0], world[1]),
            (false, *world.last().unwrap(), world[world.len() - 2]),
        ];
        for (is_start, tip, other) in ends {
            let kind = if is_start {
                style::get(&c.style, "startArrow").and_then(ArrowKind::from_str)
            } else {
                Some(style::get(&c.style, "endArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::Classic))
            };
            let Some(kind) = kind.filter(|k| *k != ArrowKind::None) else { continue };
            let fill = style::get_bool(&c.style, if is_start { "startFill" } else { "endFill" }, true);
            let size = style::get_f32(&c.style, if is_start { "startSize" } else { "endSize" }, 6.0) * z;
            let dir = tip - other;
            if dir.length_sq() < 1e-6 {
                continue;
            }
            let mpt = |p: (f32, f32)| marker_pt(tip, dir, p);
            for prim in marker_prims(kind, fill, size, lk.width) {
                match prim {
                    Prim::Poly(pts, fl) => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|p| mpt(*p)).collect();
                        s.push_str(&poly_pdf(&pts));
                        s.push_str(if fl { " f\n" } else { " S\n" });
                    }
                    Prim::Line(pts) => {
                        let pts: Vec<(f32, f32)> = pts.iter().map(|p| mpt(*p)).collect();
                        s.push_str(&poly_pdf(&pts));
                        s.push_str(" S\n");
                    }
                    Prim::Circle((cx, cy), r, fl) => {
                        let (ccx, ccy) = mpt((cx, cy));
                        s.push_str(&ellipse_pdf(ccx, ccy, r, r));
                        s.push_str(if fl { " f\n" } else { " S\n" });
                    }
                }
            }
        }
        s.push_str("[] 0 d\n");
        if !c.value.is_empty() {
            let mid = midpoint(&world);
            s.push_str(&text_pdf(&c.value, tx(mid.x), ty(mid.y) + 4.0, style::get_f32(&c.style, "fontSize", 12.0) * z, &lk.rgb, false));
        }
    }

    for c in d.cells.iter().filter(|c| c.vertex && d.layer_visible(c)) {
        let g = &c.geom;
        let (x, y) = (tx(g.x), ty(g.y + g.h));
        let (gw, gh) = (g.w * z, g.h * z);
        let shape = style::get(&c.style, "shape").unwrap_or("");
        let fill = vertex_fill(c).and_then(|v| style::parse_color(&v)).map(|x| [x.r(), x.g(), x.b()]);
        let stroke = style::parse_color(&style::get(&c.style, "strokeColor").unwrap_or("#000000"))
            .map(|x| [x.r(), x.g(), x.b()])
            .unwrap_or([0, 0, 0]);
        let sw = style::get_f32(&c.style, "strokeWidth", 1.0);
        s.push_str(&format!("{} {} {} RG {sw:.2} w\n", f(stroke[0]), f(stroke[1]), f(stroke[2])));
        if let Some(fc) = fill {
            s.push_str(&format!("{} {} {} rg\n", f(fc[0]), f(fc[1]), f(fc[2])));
        }
        if c.style.contains("ellipse") && shape.is_empty() || shape == "doubleEllipse" {
            s.push_str(&ellipse_pdf(x + gw / 2.0, y + gh / 2.0, gw / 2.0, gh / 2.0));
            s.push_str(if fill.is_some() { " B\n" } else { " S\n" });
        } else if c.style.contains("rhombus") {
            let pts = vec![(x + gw / 2.0, y), (x + gw, y + gh / 2.0), (x + gw / 2.0, y + gh), (x, y + gh / 2.0)];
            s.push_str(&poly_pdf(&pts));
            s.push_str(if fill.is_some() { " B\n" } else { " S\n" });
        } else if c.style.contains("triangle") {
            let pts = vec![(x + gw / 2.0, y), (x + gw, y), (x, y)];
            s.push_str(&poly_pdf(&pts));
            s.push_str(if fill.is_some() { " B\n" } else { " S\n" });
        } else {
            s.push_str(&format!("{x:.2} {y:.2} {gw:.2} {gh:.2} re {}\n", if fill.is_some() { "B" } else { "S" }));
        }
        if !c.value.is_empty() {
            let fc = style::parse_color(&style::get(&c.style, "fontColor").unwrap_or("#000000"))
                .map(|x| [x.r(), x.g(), x.b()])
                .unwrap_or([0, 0, 0]);
            s.push_str(&text_pdf(
                &c.value,
                x + gw / 2.0,
                y + gh / 2.0 - style::get_f32(&c.style, "fontSize", 12.0) * z * 0.35,
                style::get_f32(&c.style, "fontSize", 12.0) * z,
                &fc,
                style::font_style(&c.style) & 1 != 0,
            ));
        }
    }

    // assemble the PDF (objects: 1 catalog, 2 pages, 3-4 fonts, 5 content, 6 page)
    let content = s;
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [6 0 R] /Count 1 >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".into(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.0} {h:.0}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents 5 0 R >>"
        ),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, ob) in objects.iter().enumerate() {
        offsets.push(out.len() as u32);
        out.push_str(&format!("{} 0 obj\n{ob}\nendobj\n", i + 1));
    }
    let xref_at = out.len() as u32;
    out.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    out.push_str("0000000000 65535 f \n");
    for off in offsets {
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R /Info << /Title (DocCraft Draw Export) /Producer (DocCraft Draw) >> >>\nstartxref\n{xref_at}\n%%EOF",
        objects.len() + 1
    ));
    out.into_bytes()
}

fn f(v: u8) -> String {
    format!("{:.3}", v as f32 / 255.0)
}

/// PDF path ops (in user space, already flipped).
fn path_of(pts: &[Pos2], tx: &impl Fn(f32) -> f32, ty: &impl Fn(f32) -> f32) -> String {
    let mut s = String::new();
    for (i, p) in pts.iter().enumerate() {
        s.push_str(&format!("{:.2} {:.2} {}\n", tx(p.x), ty(p.y), if i == 0 { "m" } else { "l" }));
    }
    s
}

fn poly_pdf(pts: &[(f32, f32)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        s.push_str(&format!("{x:.2} {y:.2} {}\n", if i == 0 { "m" } else { "l" }));
    }
    s.push_str("h");
    s
}

/// Ellipse as 4 Bézier arcs (kappa 0.5523), starting at (cx+rx, cy).
fn ellipse_pdf(cx: f32, cy: f32, rx: f32, ry: f32) -> String {
    const K: f32 = 0.5523;
    let kx = rx * K;
    let ky = ry * K;
    format!(
        "{cx2:.2} {cy:.2} m\n{cx2:.2} {cy1:.2} {cx1:.2} {cy2:.2} {cx:.2} {cy2:.2} c\n{cx0:.2} {cy2:.2} {cx0:.2} {cy1:.2} {cx:.2} {cy:.2} c\n{cx0:.2} {cy0:.2} {cx1:.2} {cy0:.2} {cx:.2} {cy0b:.2} c\n{cx2:.2} {cy0:.2} {cx2:.2} {cy1:.2} {cx:.2} {cy:.2} c\nh",
        cx2 = cx + rx,
        cy2 = cy + ry,
        cy1 = cy + ky,
        cx1 = cx + kx,
        cx0 = cx - kx,
        cy0 = cy - ry,
        cy0b = cy - ry,
        cx = cx,
        cy = cy
    )
}

fn text_pdf(value: &str, x: f32, y: f32, size: f32, rgb: &[u8; 3], bold: bool) -> String {
    let t = value.replace(['\\', '(', ')'], "");
    format!(
        "BT {} {} {} rg /{} {size:.1} Tf 1 0 0 1 {x:.2} {y:.2} Tm ({t}) Tj ET\n",
        f(rgb[0]),
        f(rgb[1]),
        f(rgb[2]),
        if bold { "F2" } else { "F1" }
    )
}

// silence unused warnings for the JumpStyle import used in docs only
