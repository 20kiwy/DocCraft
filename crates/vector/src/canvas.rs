//! The Vector painting engine: document↔screen transform, the 8 pt / 64 pt
//! artboard grid, labeled rulers, Illustrator-style square selection
//! handles, bezier anchor + direction-handle overlays, smart guides and the
//! marquee. Interaction lives in [`crate::ui`].

use crate::model::*;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

// ── colors ───────────────────────────────────────────────────────────────────
pub const PASTEBOARD: Color32 = Color32::from_rgb(0x4d, 0x4d, 0x4d);
pub const AI_BLUE: Color32 = Color32::from_rgb(0x0b, 0x64, 0xb8);
const RULER_BG: Color32 = Color32::from_rgb(0xfa, 0xfa, 0xfb);
const RULER_LINE: Color32 = Color32::from_rgb(0xd9, 0xdc, 0xe1);
const RULER_TICK: Color32 = Color32::from_rgb(0xb6, 0xbb, 0xc2);
const RULER_TEXT: Color32 = Color32::from_rgb(0x9a, 0xa0, 0xa8);
const GRID_MINOR_COL: Color32 = Color32::from_rgb(0xe3, 0xe5, 0xe8);
const GRID_MAJOR_COL: Color32 = Color32::from_rgb(0xc9, 0xcd, 0xd2);
pub const GUIDE_COL: Color32 = Color32::from_rgb(0xf0, 0x44, 0x2e);

// ── transform ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Xform {
    /// Screen position of the document origin.
    pub origin: Pos2,
    pub zoom: f32,
}

impl Xform {
    pub fn s2d(&self, p: Pos2) -> Pos2 {
        pos2((p.x - self.origin.x) / self.zoom, (p.y - self.origin.y) / self.zoom)
    }
    pub fn d2s(&self, p: Pos2) -> Pos2 {
        pos2(p.x * self.zoom + self.origin.x, p.y * self.zoom + self.origin.y)
    }
    pub fn d2s_rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.d2s(r.min), self.d2s(r.max))
    }
}

// ── artboard + grid ──────────────────────────────────────────────────────────

pub fn paint_artboard(p: &Painter, clip: Rect, xf: Xform, doc: &Doc, show_grid: bool) {
    p.rect_filled(clip, 0.0, PASTEBOARD);
    let ab = xf.d2s_rect(Rect::from_min_size(Pos2::ZERO, doc.artboard));
    // drop shadow
    for (off, a) in [(vec2(5.0, 6.0), 70), (vec2(2.0, 3.0), 40)] {
        p.rect_filled(ab.translate(off), 0.0, Color32::from_black_alpha(a));
    }
    p.rect_filled(ab, 0.0, Color32::WHITE);
    p.rect_stroke(ab, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(0x9a, 0x9e, 0xa3)), StrokeKind::Outside);

    if show_grid {
        let gclip = p.clip_rect().intersect(ab);
        if gclip.is_positive() {
            let minor = GRID_MINOR * xf.zoom;
            if minor >= 4.0 {
                let x0 = ((gclip.left() - xf.origin.x) / minor).floor() * minor + xf.origin.x;
                let mut x = x0;
                let mut i = ((x - xf.origin.x) / minor).round() as i64;
                while x < gclip.right() + 0.5 {
                    let major = i.rem_euclid(GRID_MAJOR_EVERY as i64) == 0;
                    p.line_segment(
                        [pos2(x, gclip.top()), pos2(x, gclip.bottom())],
                        Stroke::new(if major { 1.0 } else { 0.5 }, if major { GRID_MAJOR_COL } else { GRID_MINOR_COL }),
                    );
                    x += minor;
                    i += 1;
                }
                let y0 = ((gclip.top() - xf.origin.y) / minor).floor() * minor + xf.origin.y;
                let mut y = y0;
                let mut j = ((y - xf.origin.y) / minor).round() as i64;
                while y < gclip.bottom() + 0.5 {
                    let major = j.rem_euclid(GRID_MAJOR_EVERY as i64) == 0;
                    p.line_segment(
                        [pos2(gclip.left(), y), pos2(gclip.right(), y)],
                        Stroke::new(if major { 1.0 } else { 0.5 }, if major { GRID_MAJOR_COL } else { GRID_MINOR_COL }),
                    );
                    y += minor;
                    j += 1;
                }
            }
        }
    }
}

// ── rulers ───────────────────────────────────────────────────────────────────

pub fn paint_ruler_h(p: &Painter, bar: Rect, xf: Xform) {
    p.rect_filled(bar, 0.0, RULER_BG);
    p.line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, RULER_LINE));
    let unit = GRID_MINOR;
    let step_px = unit * xf.zoom;
    let step = if step_px < 8.0 { unit * 10.0 } else if step_px < 24.0 { unit * 5.0 } else { unit };
    let sp = step * xf.zoom;
    let start = ((bar.left() - xf.origin.x) / sp).floor() * sp;
    let mut x = start + xf.origin.x;
    let mut v = (start / step).round() * step;
    while x < bar.right() + 1.0 {
        p.line_segment([pos2(x, bar.bottom() - 5.0), pos2(x, bar.bottom())], Stroke::new(1.0, RULER_TICK));
        let g = p.layout_no_wrap(format!("{}", v as i32), FontId::proportional(8.5), RULER_TEXT);
        p.galley(pos2(x + 2.0, bar.top() + 2.0), g, RULER_TEXT);
        x += sp;
        v += step;
    }
}

pub fn paint_ruler_v(p: &Painter, bar: Rect, xf: Xform) {
    p.rect_filled(bar, 0.0, RULER_BG);
    p.line_segment([bar.right_top(), bar.right_bottom()], Stroke::new(1.0, RULER_LINE));
    let unit = GRID_MINOR;
    let step_px = unit * xf.zoom;
    let step = if step_px < 8.0 { unit * 10.0 } else if step_px < 24.0 { unit * 5.0 } else { unit };
    let sp = step * xf.zoom;
    let start = ((bar.top() - xf.origin.y) / sp).floor() * sp;
    let mut y = start + xf.origin.y;
    let mut v = (start / step).round() * step;
    while y < bar.bottom() + 1.0 {
        p.line_segment([pos2(bar.right() - 5.0, y), pos2(bar.right(), y)], Stroke::new(1.0, RULER_TICK));
        let g = p.layout_no_wrap(format!("{}", v as i32), FontId::proportional(8.5), RULER_TEXT);
        let gp = pos2(bar.right() - g.size().x - 3.0, y + 1.0);
        p.rect_filled(Rect::from_min_size(gp - vec2(1.0, 0.0), g.size() + vec2(2.0, 0.0)), 0.0, RULER_BG);
        p.galley(gp, g, RULER_TEXT);
        y += sp;
        v += step;
    }
}

pub fn paint_ruler_corner(p: &Painter, r: Rect) {
    p.rect_filled(r, 0.0, RULER_BG);
    p.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, RULER_LINE), StrokeKind::Outside);
}

// ── selection + path overlays ────────────────────────────────────────────────

pub const HANDLE: f32 = 7.0;
pub const ANCHOR: f32 = 6.0;

pub fn handle_rects(r: Rect) -> [Rect; 8] {
    let mk = |c: Pos2| Rect::from_center_size(c, vec2(HANDLE, HANDLE));
    [
        mk(r.left_top()),
        mk(pos2(r.center().x, r.top())),
        mk(r.right_top()),
        mk(pos2(r.right(), r.center().y)),
        mk(r.right_bottom()),
        mk(pos2(r.center().x, r.bottom())),
        mk(r.left_bottom()),
        mk(pos2(r.left(), r.center().y)),
    ]
}

/// Illustrator selection: thin blue box + white square handles.
pub fn paint_selection(p: &Painter, r: Rect) {
    let rr = r.expand(0.5);
    p.rect_stroke(rr, CornerRadius::ZERO, Stroke::new(1.0, AI_BLUE), StrokeKind::Middle);
    for hr in handle_rects(rr) {
        p.rect_filled(hr, 0.0, Color32::WHITE);
        p.rect_stroke(hr, CornerRadius::ZERO, Stroke::new(1.0, AI_BLUE), StrokeKind::Middle);
    }
}

/// Which of the 8 handles (or none) is under a screen point.
pub fn hit_handle(r: Rect, sp: Pos2) -> Option<usize> {
    for (i, hr) in handle_rects(r.expand(1.5)).iter().enumerate() {
        if hr.expand(3.0).contains(sp) {
            return Some(i);
        }
    }
    None
}

/// Anchor squares + direction handles for a selected path (direct selection).
pub fn paint_path_overlay(p: &Painter, anchors: &[Anchor], closed: bool, xf: Xform, active: Option<usize>, show_handles: bool) {
    let pts: Vec<Pos2> = flatten(anchors, closed).iter().map(|q| xf.d2s(*q)).collect();
    if pts.len() >= 2 {
        p.add(egui::Shape::line(pts, Stroke::new(1.0, AI_BLUE.linear_multiply(0.5))));
    }
    for (i, a) in anchors.iter().enumerate() {
        let sp = xf.d2s(a.p);
        let sq = Rect::from_center_size(sp, vec2(ANCHOR, ANCHOR));
        let on = active == Some(i);
        p.rect_filled(sq, 0.0, if on { AI_BLUE } else { Color32::WHITE });
        p.rect_stroke(sq, CornerRadius::ZERO, Stroke::new(1.0, AI_BLUE), StrokeKind::Middle);
        if show_handles {
            for h in [a.h_in, a.h_out] {
                if h != Vec2::ZERO {
                    let hp = xf.d2s(a.p + h);
                    p.line_segment([sp, hp], Stroke::new(0.9, AI_BLUE.linear_multiply(0.7)));
                    p.circle_filled(hp, 2.6, Color32::WHITE);
                    p.circle_stroke(hp, 2.6, Stroke::new(1.0, AI_BLUE));
                }
            }
        }
    }
}

/// Nearest anchor index of a path within `tol` screen px.
pub fn hit_anchor(anchors: &[Anchor], xf: Xform, sp: Pos2, tol: f32) -> Option<usize> {
    anchors
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let da = (xf.d2s(a.1.p) - sp).length();
            let db = (xf.d2s(b.1.p) - sp).length();
            da.total_cmp(&db)
        })
        .filter(|(_, a)| (xf.d2s(a.p) - sp).length() <= tol)
        .map(|(i, _)| i)
}

/// Nearest direction-handle end within `tol` screen px → (anchor, which).
pub fn hit_dir_handle(a: &Anchor, xf: Xform, sp: Pos2, tol: f32) -> bool {
    for h in [a.h_in, a.h_out] {
        if h != Vec2::ZERO && (xf.d2s(a.p + h) - sp).length() <= tol {
            return true;
        }
    }
    false
}

// ── marquee + smart guides ───────────────────────────────────────────────────

pub fn paint_marquee(p: &Painter, r: Rect) {
    p.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, AI_BLUE), StrokeKind::Middle);
    p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(AI_BLUE.r(), AI_BLUE.g(), AI_BLUE.b(), 18));
}

/// Red smart-guide lines (screen space) while dragging.
pub fn paint_guides(p: &Painter, clip: Rect, v: &[f32], h: &[f32]) {
    for x in v {
        p.line_segment([pos2(*x, clip.top()), pos2(*x, clip.bottom())], Stroke::new(1.0, GUIDE_COL));
    }
    for y in h {
        p.line_segment([pos2(clip.left(), *y), pos2(clip.right(), *y)], Stroke::new(1.0, GUIDE_COL));
    }
}

/// Compute snap + guide lines for a moving bounds rect against other nodes.
/// Returns (snapped dx, snapped dy, vertical guide xs, horizontal guide ys).
pub fn smart_snap(moving: Rect, others: &[Rect], tol: f32) -> (f32, f32, Vec<f32>, Vec<f32>) {
    let mut best: Option<(f32, f32)> = None; // (dx, dy) snap deltas
    fn better(best: Option<(f32, f32)>, dx: f32, dy: f32, tol: f32) -> Option<(f32, f32)> {
        let score = dx.abs() + dy.abs();
        let cur_ok = best.map(|(bx, by)| bx.abs() + by.abs()).unwrap_or(f32::MAX);
        if score < cur_ok && (dx.abs() <= tol || dy.abs() <= tol) {
            Some((dx, dy))
        } else {
            best
        }
    }
    let mut consider = |best: &mut Option<(f32, f32)>, dx: f32, dy: f32| {
        *best = better(*best, dx, dy, tol);
    };
    let mut vg = Vec::new();
    let mut hg = Vec::new();
    for o in others {
        // vertical alignments → x snap
        for (mx, ox) in [
            (moving.left(), o.left()),
            (moving.center().x, o.center().x),
            (moving.right(), o.right()),
            (moving.left(), o.right()),
            (moving.right(), o.left()),
        ] {
            let dx = ox - mx;
            if dx.abs() <= tol {
                consider(&mut best, dx, 0.0);
                if best == Some((dx, 0.0)) {
                    vg.push(ox);
                }
            }
        }
        // horizontal alignments → y snap
        for (my, oy) in [
            (moving.top(), o.top()),
            (moving.center().y, o.center().y),
            (moving.bottom(), o.bottom()),
            (moving.top(), o.bottom()),
            (moving.bottom(), o.top()),
        ] {
            let dy = oy - my;
            if dy.abs() <= tol {
                consider(&mut best, 0.0, dy);
                if best == Some((0.0, dy)) {
                    hg.push(oy);
                }
            }
        }
    }
    let (dx, dy) = best.unwrap_or((0.0, 0.0));
    (dx, dy, vg, hg)
}
