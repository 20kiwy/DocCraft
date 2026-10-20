//! Hand-drawn vector icons (16px design grid) — no font dependency, no tofu.
//!
//! Ported 1:1 from the Typeset craft's `paint_icon` (save/undo/compile/gear/
//! sun/moon/break/image/table, which renders perfectly with zero font
//! dependency) and extended with the rest of the suite's chrome glyphs. Every
//! native craft paints its toolbar / ribbon / panel icons from here, so emoji
//! and symbol codepoints the installed fonts can't render (the □ tofu boxes:
//! 💾 🖼 ↶ ↷ ⧉ 🗑 ➔ ⬚ ⤢ ⟩⟩ 🗎 ＋ ☰ ☐ ❝ 🔍 …) never reach the screen.
//!
//! Style: simple stroke drawings, ~1.4 px, in color `c`, on the 16 px design
//! grid (the drawing is centered in `r` and capped at 16×16). `bg` is the
//! surface color behind the icon — used to "bite" shapes (the moon crescent,
//! knocked-out overlaps in duplicate/clipboard/eye-off).

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2, pos2};

/// Paint icon `name` centered in `r`. `bg` = the surface color behind the
/// icon (used to "bite" shapes, e.g. the moon crescent).
pub fn paint(p: &egui::Painter, r: Rect, name: &str, c: Color32, bg: Color32) {
    let m = Rect::from_center_size(r.center(), Vec2::splat(r.height().min(16.0)));
    let (x0, y0, x1, y1) = (m.min.x, m.min.y, m.max.x, m.max.y);
    let st = Stroke::new(1.4, c);
    let ctr = m.center();
    let (cx, cy) = (ctr.x, ctr.y);
    match name {
        // ── ported from Typeset ─────────────────────────────────────────────
        "save" => {
            p.rect_stroke(m.shrink(1.0), 2.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(Rect::from_min_max(pos2(cx - 4.0, y0 + 1.0), pos2(cx + 4.0, y0 + 6.0)), 1.5, st, egui::StrokeKind::Inside);
            p.line_segment([pos2(x0 + 4.0, y1 - 3.0), pos2(x1 - 4.0, y1 - 3.0)], st);
        }
        "undo" | "redo" | "compile" => {
            // Typeset's exact undo arc; "compile" kept as an alias of "redo".
            let redo = name != "undo";
            let c0 = m.center();
            let rad = m.height() * 0.32;
            let (a, b) = if redo { (-0.8f32, 2.6f32) } else { (2.2f32, 5.6f32) };
            let mut prev = pos2(c0.x + rad * a.cos(), c0.y - rad * a.sin());
            for i in 1..=8 {
                let t = a + (b - a) * i as f32 / 8.0;
                let pt = pos2(c0.x + rad * t.cos(), c0.y - rad * t.sin());
                p.line_segment([prev, pt], st);
                prev = pt;
            }
            let t = b + if redo { -0.35 } else { 0.35 };
            let tip = pos2(c0.x + rad * t.cos(), c0.y - rad * t.sin());
            p.line_segment([prev, tip], st);
            p.line_segment([prev, pos2(prev.x - 4.0, prev.y - 1.5)], st);
            p.line_segment([prev, pos2(prev.x - 1.0, prev.y + 4.0)], st);
        }
        "gear" => {
            let c0 = m.center();
            let rad = m.height() * 0.24;
            p.circle_stroke(c0, rad, st);
            p.circle_stroke(c0, rad * 0.35, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                p.line_segment([pos2(c0.x + rad * a.cos(), c0.y + rad * a.sin()), pos2(c0.x + rad * 1.7 * a.cos(), c0.y + rad * 1.7 * a.sin())], st);
            }
        }
        "sun" => {
            let c0 = m.center();
            let rad = m.height() * 0.2;
            p.circle_filled(c0, rad, st.color);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (d0, d1) = (rad * 1.55, rad * 2.15);
                p.line_segment([pos2(c0.x + d0 * a.cos(), c0.y + d0 * a.sin()), pos2(c0.x + d1 * a.cos(), c0.y + d1 * a.sin())], st);
            }
        }
        "moon" => {
            let c0 = m.center();
            let rad = m.height() * 0.3;
            p.circle_filled(c0, rad, st.color);
            // bite the disc with the strip background to leave a crescent
            p.circle_filled(pos2(c0.x + rad * 0.8, c0.y - rad * 0.5), rad * 0.95, bg);
        }
        "break" => {
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 4.0, y0 + 1.0), pos2(x1 - 4.0, y1 - 1.0)), 1.5, st, egui::StrokeKind::Inside);
            for i in 0..3 {
                let y = m.center().y + (i as f32 - 1.0) * 2.5;
                p.line_segment([pos2(x0 + 6.5, y), pos2(x1 - 6.5, y)], st);
            }
        }
        "image" => {
            p.rect_stroke(m.shrink(1.0), 1.5, st, egui::StrokeKind::Inside);
            p.circle_filled(pos2(x0 + 5.0, y0 + 5.0), 1.6, st.color);
            p.line_segment([pos2(x0 + 2.5, y1 - 3.0), pos2(x0 + 8.0, y0 + 8.0)], st);
            p.line_segment([pos2(x0 + 8.0, y0 + 8.0), pos2(x1 - 4.0, y1 - 3.0)], st);
        }
        "table" => {
            p.rect_stroke(m.shrink(1.0), 1.5, st, egui::StrokeKind::Inside);
            let w = m.width();
            let h = m.height();
            p.line_segment([pos2(x0 + w / 3.0, y0 + 1.0), pos2(x0 + w / 3.0, y1 - 1.0)], st);
            p.line_segment([pos2(x0 + 2.0 * w / 3.0, y0 + 1.0), pos2(x0 + 2.0 * w / 3.0, y1 - 1.0)], st);
            p.line_segment([pos2(x0 + 1.0, y0 + h / 2.0), pos2(x1 - 1.0, y0 + h / 2.0)], st);
        }

        // ── clipboard / pages ───────────────────────────────────────────────
        "duplicate" => {
            let back = Rect::from_min_max(pos2(x0 + 5.0, y0 + 1.0), pos2(x1 - 1.0, y1 - 5.0));
            let front = Rect::from_min_max(pos2(x0 + 1.0, y0 + 5.0), pos2(x1 - 5.0, y1 - 1.0));
            p.rect_stroke(back, 2.0, st, egui::StrokeKind::Inside);
            p.rect_filled(front, 2.0, bg);
            p.rect_stroke(front, 2.0, st, egui::StrokeKind::Inside);
        }
        "trash" => {
            p.line_segment([pos2(cx - 2.0, y0 + 1.0), pos2(cx + 2.0, y0 + 1.0)], st); // handle
            p.line_segment([pos2(x0 + 2.5, y0 + 3.0), pos2(x1 - 2.5, y0 + 3.0)], st); // lid
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 3.5, y0 + 3.0), pos2(x1 - 3.5, y1 - 1.0)), 1.5, st, egui::StrokeKind::Inside);
            p.line_segment([pos2(cx - 2.0, y0 + 5.5), pos2(cx - 2.0, y1 - 3.0)], st);
            p.line_segment([pos2(cx + 2.0, y0 + 5.5), pos2(cx + 2.0, y1 - 3.0)], st);
        }
        "page" => {
            let fx = x1 - 4.5; // fold x
            p.line_segment([pos2(x0 + 3.0, y0 + 1.5), pos2(fx, y0 + 1.5)], st);
            p.line_segment([pos2(fx, y0 + 1.5), pos2(x1 - 1.5, y0 + 4.5)], st);
            p.line_segment([pos2(x1 - 1.5, y0 + 4.5), pos2(x1 - 1.5, y1 - 1.5)], st);
            p.line_segment([pos2(x1 - 1.5, y1 - 1.5), pos2(x0 + 3.0, y1 - 1.5)], st);
            p.line_segment([pos2(x0 + 3.0, y1 - 1.5), pos2(x0 + 3.0, y0 + 1.5)], st);
            p.line_segment([pos2(fx, y0 + 1.5), pos2(fx, y0 + 4.5)], st);
            p.line_segment([pos2(fx, y0 + 4.5), pos2(x1 - 1.5, y0 + 4.5)], st);
        }
        "file-plus" => {
            let fx = x1 - 4.5;
            p.line_segment([pos2(x0 + 3.0, y0 + 1.5), pos2(fx, y0 + 1.5)], st);
            p.line_segment([pos2(fx, y0 + 1.5), pos2(x1 - 1.5, y0 + 4.5)], st);
            p.line_segment([pos2(x1 - 1.5, y0 + 4.5), pos2(x1 - 1.5, y1 - 1.5)], st);
            p.line_segment([pos2(x1 - 1.5, y1 - 1.5), pos2(x0 + 3.0, y1 - 1.5)], st);
            p.line_segment([pos2(x0 + 3.0, y1 - 1.5), pos2(x0 + 3.0, y0 + 1.5)], st);
            let (px, py) = (cx - 0.5, cy + 1.5);
            p.line_segment([pos2(px - 2.5, py), pos2(px + 2.5, py)], st);
            p.line_segment([pos2(px, py - 2.5), pos2(px, py + 2.5)], st);
        }
        "clipboard" => {
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 2.0, y0 + 3.0), pos2(x1 - 2.0, y1 - 1.5)), 2.0, st, egui::StrokeKind::Inside);
            let clip = Rect::from_min_max(pos2(cx - 3.0, y0 + 1.0), pos2(cx + 3.0, y0 + 4.5));
            p.rect_filled(clip, 1.5, bg);
            p.rect_stroke(clip, 1.5, st, egui::StrokeKind::Inside);
        }
        "panel-right" => {
            p.rect_stroke(m.shrink(1.0), 2.0, st, egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_max(pos2(x1 - 5.5, y0 + 2.0), pos2(x1 - 2.0, y1 - 2.0)), 1.0, st.color);
        }

        // ── primitives ──────────────────────────────────────────────────────
        "plus" => {
            p.line_segment([pos2(cx - 4.5, cy), pos2(cx + 4.5, cy)], st);
            p.line_segment([pos2(cx, cy - 4.5), pos2(cx, cy + 4.5)], st);
        }
        "minus" => {
            p.line_segment([pos2(cx - 4.5, cy), pos2(cx + 4.5, cy)], st);
        }
        "close" => {
            p.line_segment([pos2(x0 + 3.0, y0 + 3.0), pos2(x1 - 3.0, y1 - 3.0)], st);
            p.line_segment([pos2(x1 - 3.0, y0 + 3.0), pos2(x0 + 3.0, y1 - 3.0)], st);
        }
        "hr" => {
            p.line_segment([pos2(x0 + 1.0, cy), pos2(x1 - 1.0, cy)], st);
        }
        "checkbox" => {
            p.rect_stroke(m.shrink(1.5), 2.0, st, egui::StrokeKind::Inside);
            p.line_segment([pos2(x0 + 4.0, cy + 0.5), pos2(cx - 0.5, y1 - 4.0)], st);
            p.line_segment([pos2(cx - 0.5, y1 - 4.0), pos2(x1 - 4.0, y0 + 4.5)], st);
        }
        "quote" => {
            for qx in [cx - 4.5, cx + 1.5] {
                p.circle_filled(pos2(qx + 1.9, cy - 1.6), 1.9, st.color);
                p.rect_filled(Rect::from_min_max(pos2(qx + 0.2, cy - 1.6), pos2(qx + 2.6, cy + 3.4)), 0.0, st.color);
            }
        }
        "list" => {
            for (i, yy) in [y0 + 3.5, cy, y1 - 3.5].into_iter().enumerate() {
                p.circle_filled(pos2(x0 + 2.5, yy), 1.4, st.color);
                let xe = if i == 1 { x1 - 4.0 } else { x1 - 1.0 };
                p.line_segment([pos2(x0 + 5.5, yy), pos2(xe, yy)], st);
            }
        }
        "list-ol" => {
            // the digits are ASCII — they render in every font
            let digits = ["1", "2", "3"];
            for (i, yy) in [y0 + 3.5, cy, y1 - 3.5].into_iter().enumerate() {
                p.text(pos2(x0 + 3.0, yy), Align2::CENTER_CENTER, digits[i], FontId::proportional(6.5), st.color);
                let xe = if i == 1 { x1 - 4.0 } else { x1 - 1.0 };
                p.line_segment([pos2(x0 + 6.5, yy), pos2(xe, yy)], st);
            }
        }
        "indent" | "outdent" => {
            for yy in [y0 + 2.5, y1 - 2.5] {
                p.line_segment([pos2(x0 + 2.0, yy), pos2(x1 - 2.0, yy)], st);
            }
            let dir = if name == "indent" { 1.0 } else { -1.0 };
            let ax = cx + 4.0 * dir; // arrow tip x
            p.line_segment([pos2(ax - 5.5 * dir, cy), pos2(ax, cy)], st);
            p.line_segment([pos2(ax, cy), pos2(ax - 2.0 * dir, cy - 2.0)], st);
            p.line_segment([pos2(ax, cy), pos2(ax - 2.0 * dir, cy + 2.0)], st);
        }
        "text-x" => {
            p.line_segment([pos2(x0 + 1.5, y0 + 3.0), pos2(x0 + 9.5, y0 + 3.0)], st);
            p.line_segment([pos2(x0 + 5.5, y0 + 3.0), pos2(x0 + 5.5, y0 + 11.0)], st);
            let (tx, ty) = (cx + 3.5, cy + 3.5);
            p.line_segment([pos2(tx - 2.0, ty - 2.0), pos2(tx + 2.0, ty + 2.0)], st);
            p.line_segment([pos2(tx + 2.0, ty - 2.0), pos2(tx - 2.0, ty + 2.0)], st);
        }
        "code" | "xml" => {
            p.line_segment([pos2(cx - 4.0, cy - 3.5), pos2(cx - 7.0, cy)], st);
            p.line_segment([pos2(cx - 7.0, cy), pos2(cx - 4.0, cy + 3.5)], st);
            p.line_segment([pos2(cx + 4.0, cy - 3.5), pos2(cx + 7.0, cy)], st);
            p.line_segment([pos2(cx + 7.0, cy), pos2(cx + 4.0, cy + 3.5)], st);
            p.line_segment([pos2(cx + 1.2, cy - 4.0), pos2(cx - 1.2, cy + 4.0)], st);
        }

        // ── view helpers ────────────────────────────────────────────────────
        "search" | "zoom-in" | "zoom-out" => {
            let c0 = pos2(cx - 1.0, cy - 1.0);
            p.circle_stroke(c0, 4.2, st);
            p.line_segment([pos2(c0.x + 3.0, c0.y + 3.0), pos2(x1 - 1.0, y1 - 1.0)], st);
            if name != "search" {
                p.line_segment([pos2(c0.x - 1.8, c0.y), pos2(c0.x + 1.8, c0.y)], st);
                if name == "zoom-in" {
                    p.line_segment([pos2(c0.x, c0.y - 1.8), pos2(c0.x, c0.y + 1.8)], st);
                }
            }
        }
        "fit" => {
            let g = 1.0;
            let l = 4.5;
            for (ax, ay, dx, dy) in [(x0 + g, y0 + g, 1.0, 1.0), (x1 - g, y0 + g, -1.0, 1.0), (x0 + g, y1 - g, 1.0, -1.0), (x1 - g, y1 - g, -1.0, -1.0)] {
                p.line_segment([pos2(ax, ay + l * dy), pos2(ax, ay)], st);
                p.line_segment([pos2(ax, ay), pos2(ax + l * dx, ay)], st);
            }
        }
        "eye" => {
            eye(p, x0, y0, x1, y1, cx, cy, st);
            p.circle_filled(pos2(cx, cy), 1.9, st.color);
        }
        "eye-off" => {
            eye(p, x0, y0, x1, y1, cx, cy, st);
            // bite the eye, then draw the slash
            let a = pos2(x0 + 2.0, y1 - 2.0);
            let b = pos2(x1 - 2.0, y0 + 2.0);
            p.line_segment([a, b], Stroke::new(3.2, bg));
            p.line_segment([a, b], st);
        }
        "lock" => {
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 3.5, cy - 0.5), pos2(x1 - 3.5, y1 - 1.5)), 1.5, st, egui::StrokeKind::Inside);
            shackle(p, cx, cy - 0.5, 3.0, 0.0, st);
            p.circle_filled(pos2(cx, cy + 3.0), 1.2, st.color);
        }
        "unlock" => {
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 3.5, cy - 0.5), pos2(x1 - 3.5, y1 - 1.5)), 1.5, st, egui::StrokeKind::Inside);
            // open: shackle raised off the body on one side
            shackle(p, cx, cy - 0.5, 3.0, 2.0, st);
        }
        "arrow-up" => arrow(p, cx, cy, 0.0, -1.0, st),
        "arrow-down" => arrow(p, cx, cy, 0.0, 1.0, st),
        "arrow-left" => arrow(p, cx, cy, -1.0, 0.0, st),
        "arrow-right" => arrow(p, cx, cy, 1.0, 0.0, st),
        "chevron-down" => chevron(p, cx, cy, 0.0, 1.0, st),
        "chevron-up" => chevron(p, cx, cy, 0.0, -1.0, st),
        "chevron-right" => chevron(p, cx, cy, 1.0, 0.0, st),
        "chevron-left" => chevron(p, cx, cy, -1.0, 0.0, st),
        "layers" => {
            for yy in [cy + 4.5, cy, cy - 4.5] {
                let d = [pos2(cx, yy - 3.0), pos2(cx + 5.5, yy), pos2(cx, yy + 3.0), pos2(cx - 5.5, yy), pos2(cx, yy - 3.0)];
                p.add(egui::Shape::line(d.to_vec(), st));
            }
        }
        "grid" => {
            for (rx, ry) in [(x0 + 1.0, y0 + 1.0), (cx + 1.0, y0 + 1.0), (x0 + 1.0, cy + 1.0), (cx + 1.0, cy + 1.0)] {
                p.rect_stroke(Rect::from_min_max(pos2(rx, ry), pos2(rx + 6.0, ry + 6.0)), 1.0, st, egui::StrokeKind::Inside);
            }
        }
        "select" => {
            let pts = [
                pos2(x0 + 4.0, y0 + 1.0),
                pos2(x0 + 4.0, y1 - 2.0),
                pos2(x0 + 7.4, y0 + 10.0),
                pos2(x0 + 9.4, y1 - 1.0),
                pos2(x0 + 11.4, y1 - 2.0),
                pos2(x0 + 9.5, y0 + 9.3),
                pos2(x1 - 2.0, y0 + 8.8),
                pos2(x0 + 4.0, y0 + 1.0),
            ];
            p.add(egui::Shape::line(pts.to_vec(), st));
        }
        "connect" => {
            let a = Rect::from_min_max(pos2(x0 + 1.0, y0 + 1.0), pos2(x0 + 6.5, y0 + 6.5));
            let b = Rect::from_min_max(pos2(x1 - 6.5, y1 - 6.5), pos2(x1 - 1.0, y1 - 1.0));
            p.rect_stroke(a, 1.5, st, egui::StrokeKind::Inside);
            p.rect_stroke(b, 1.5, st, egui::StrokeKind::Inside);
            let bend = pos2(a.center().x, b.center().y);
            let end = pos2(b.center().x, b.center().y);
            p.line_segment([pos2(a.center().x, a.max.y), bend], st);
            p.line_segment([bend, end], st);
            p.line_segment([end, pos2(end.x - 1.8, end.y - 2.5)], st);
            p.line_segment([end, pos2(end.x - 1.8, end.y + 2.5)], st);
        }
        "link" => {
            p.circle_stroke(pos2(cx - 2.4, cy), 3.2, st);
            p.circle_stroke(pos2(cx + 2.4, cy), 3.2, st);
        }
        "cut" => {
            let bl = pos2(cx - 2.6, y1 - 3.0);
            let br = pos2(cx + 2.6, y1 - 3.0);
            p.circle_stroke(bl, 1.8, st);
            p.circle_stroke(br, 1.8, st);
            p.line_segment([pos2(bl.x + 0.7, bl.y - 1.5), pos2(x1 - 3.0, y0 + 1.5)], st);
            p.line_segment([pos2(br.x - 0.7, br.y - 1.5), pos2(x0 + 3.0, y0 + 1.5)], st);
        }
        "calendar" => {
            p.rect_stroke(Rect::from_min_max(pos2(x0 + 1.5, y0 + 3.0), pos2(x1 - 1.5, y1 - 1.5)), 1.5, st, egui::StrokeKind::Inside);
            p.line_segment([pos2(x0 + 1.5, y0 + 6.5), pos2(x1 - 1.5, y0 + 6.5)], st);
            p.line_segment([pos2(x0 + 4.5, y0 + 1.0), pos2(x0 + 4.5, y0 + 4.0)], st);
            p.line_segment([pos2(x1 - 4.5, y0 + 1.0), pos2(x1 - 4.5, y0 + 4.0)], st);
        }
        "clock" => {
            p.circle_stroke(ctr, 5.8, st);
            p.line_segment([pos2(cx, cy), pos2(cx, cy - 3.2)], st);
            p.line_segment([pos2(cx, cy), pos2(cx + 2.4, cy + 1.4)], st);
        }
        "sync" => {
            arrow(p, cx - 2.5, cy, 0.0, -1.0, st);
            arrow(p, cx + 2.5, cy, 0.0, 1.0, st);
        }

        // ── arrange (drawio format panel) ───────────────────────────────────
        "align-left" | "align-center" | "align-right" => {
            let l0 = 12.0f32;
            let l1 = 7.0f32;
            let l2 = 9.5f32;
            for (i, len) in [l0, l1, l2].into_iter().enumerate() {
                let yy = y0 + 3.0 + i as f32 * 5.0;
                let (ax, bx) = match name {
                    "align-left" => (x0 + 2.0, x0 + 2.0 + len),
                    "align-center" => (cx - len / 2.0, cx + len / 2.0),
                    _ => (x1 - 2.0 - len, x1 - 2.0),
                };
                p.line_segment([pos2(ax, yy), pos2(bx, yy)], st);
            }
        }
        "align-top" | "align-middle" | "align-bottom" => {
            let l0 = 12.0f32;
            let l1 = 7.0f32;
            let l2 = 9.5f32;
            for (i, len) in [l0, l1, l2].into_iter().enumerate() {
                let xx = x0 + 3.0 + i as f32 * 5.0;
                let (ay, by) = match name {
                    "align-top" => (y0 + 2.0, y0 + 2.0 + len),
                    "align-middle" => (cy - len / 2.0, cy + len / 2.0),
                    _ => (y1 - 2.0 - len, y1 - 2.0),
                };
                p.line_segment([pos2(xx, ay), pos2(xx, by)], st);
            }
        }
        "distribute-h" => {
            p.line_segment([pos2(x0 + 2.0, y0 + 2.0), pos2(x0 + 2.0, y1 - 2.0)], st);
            p.line_segment([pos2(x1 - 2.0, y0 + 2.0), pos2(x1 - 2.0, y1 - 2.0)], st);
            arrow(p, cx - 1.5, cy, -1.0, 0.0, st);
            arrow(p, cx + 1.5, cy, 1.0, 0.0, st);
        }
        "distribute-v" => {
            p.line_segment([pos2(x0 + 2.0, y0 + 2.0), pos2(x1 - 2.0, y0 + 2.0)], st);
            p.line_segment([pos2(x0 + 2.0, y1 - 2.0), pos2(x1 - 2.0, y1 - 2.0)], st);
            arrow(p, cx, cy - 1.5, 0.0, -1.0, st);
            arrow(p, cx, cy + 1.5, 0.0, 1.0, st);
        }
        "rotate-cw" | "rotate-ccw" => {
            let c0 = m.center();
            let rad = m.height() * 0.32;
            let (a, b) = (2.2f32, 5.6f32);
            let sx = if name == "rotate-cw" { -1.0 } else { 1.0 };
            let mut prev = pos2(c0.x + sx * rad * a.cos(), c0.y - rad * a.sin());
            for i in 1..=8 {
                let t = a + (b - a) * i as f32 / 8.0;
                let pt = pos2(c0.x + sx * rad * t.cos(), c0.y - rad * t.sin());
                p.line_segment([prev, pt], st);
                prev = pt;
            }
            let tip = pos2(c0.x + sx * rad * (b + 0.35).cos(), c0.y - rad * (b + 0.35).sin());
            p.line_segment([prev, tip], st);
            p.line_segment([prev, pos2(prev.x + sx * 4.0, prev.y - 1.5)], st);
            p.line_segment([prev, pos2(prev.x + sx * 1.0, prev.y + 4.0)], st);
        }
        "flip-h" | "flip-v" => {
            let horiz = name == "flip-h";
            // dashed mirror axis
            for i in 0..3 {
                let t = y0 + 2.0 + i as f32 * 5.0;
                let tt = x0 + 2.0 + i as f32 * 5.0;
                if horiz {
                    p.line_segment([pos2(cx, t), pos2(cx, t + 2.6)], st);
                } else {
                    p.line_segment([pos2(tt, cy), pos2(tt + 2.6, cy)], st);
                }
            }
            for s in [-1.0, 1.0] {
                let tri = if horiz {
                    [pos2(cx + s * 6.0, cy), pos2(cx + s * 1.8, cy - 4.2), pos2(cx + s * 1.8, cy + 4.2), pos2(cx + s * 6.0, cy)]
                } else {
                    [pos2(cx, cy + s * 6.0), pos2(cx - 4.2, cy + s * 1.8), pos2(cx + 4.2, cy + s * 1.8), pos2(cx, cy + s * 6.0)]
                };
                p.add(egui::Shape::line(tri.to_vec(), st));
            }
        }
        "to-front" | "to-back" => {
            let back = Rect::from_min_max(pos2(x0 + 4.0, y0 + 1.0), pos2(x1 - 1.0, y1 - 4.0));
            let front = Rect::from_min_max(pos2(x0 + 1.0, y0 + 4.0), pos2(x1 - 4.0, y1 - 1.0));
            if name == "to-front" {
                p.rect_stroke(back, 2.0, st, egui::StrokeKind::Inside);
                p.rect_filled(front, 2.0, st.color);
            } else {
                p.rect_filled(back, 2.0, st.color);
                p.rect_filled(front, 2.0, bg);
                p.rect_stroke(front, 2.0, st, egui::StrokeKind::Inside);
            }
        }

        // ── fallback: first character as text (used by letter tiles) ────────
        _ => {
            p.text(m.center(), Align2::CENTER_CENTER, name.chars().next().unwrap_or('?').to_string(), FontId::proportional(11.0), c);
        }
    }
}

/// The eye's almond outline (shared by eye / eye-off).
fn eye(p: &egui::Painter, x0: f32, y0: f32, x1: f32, _y1: f32, cx: f32, cy: f32, st: Stroke) {
    let lx = pos2(x0 + 1.0, cy);
    let rx = pos2(x1 - 1.0, cy);
    quad(p, lx, pos2(cx, y0 - 0.5), rx, st);
    quad(p, lx, pos2(cx, y0 + 16.5), rx, st);
}

/// Quadratic bezier sampled as short segments.
fn quad(p: &egui::Painter, a: Pos2, q: Pos2, b: Pos2, st: Stroke) {
    let mut prev = a;
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        let mt = 1.0 - t;
        let pt = pos2(mt * mt * a.x + 2.0 * mt * t * q.x + t * t * b.x, mt * mt * a.y + 2.0 * mt * t * q.y + t * t * b.y);
        p.line_segment([prev, pt], st);
        prev = pt;
    }
}

/// Lock shackle: an upper semicircle from the left anchor to the right end,
/// with the right end lifted `lift` px off the body (0 = closed lock).
fn shackle(p: &egui::Painter, cx: f32, base: f32, rad: f32, lift: f32, st: Stroke) {
    let mut prev = pos2(cx - rad, base);
    if lift > 0.0 {
        p.line_segment([prev, pos2(cx - rad, base - lift)], st);
    }
    for i in 1..=8 {
        let t = std::f32::consts::PI - std::f32::consts::PI * i as f32 / 8.0;
        let pt = pos2(cx + rad * t.cos(), base - rad * t.sin() - lift);
        p.line_segment([prev, pt], st);
        prev = pt;
    }
}

/// A simple arrow: shaft from the center pointing in `(dx, dy)` with a head.
fn arrow(p: &egui::Painter, cx: f32, cy: f32, dx: f32, dy: f32, st: Stroke) {
    let tail = pos2(cx - dx * 4.5, cy - dy * 4.5);
    let tip = pos2(cx + dx * 4.5, cy + dy * 4.5);
    p.line_segment([tail, tip], st);
    // head barbs, perpendicular to the shaft
    let (px, py) = (-dy, dx);
    p.line_segment([tip, pos2(tip.x - dx * 3.2 + px * 2.4, tip.y - dy * 3.2 + py * 2.4)], st);
    p.line_segment([tip, pos2(tip.x - dx * 3.2 - px * 2.4, tip.y - dy * 3.2 - py * 2.4)], st);
}

/// A simple chevron pointing in `(dx, dy)`.
fn chevron(p: &egui::Painter, cx: f32, cy: f32, dx: f32, dy: f32, st: Stroke) {
    let (px, py) = (-dy, dx);
    let a = pos2(cx - dx * 2.2 + px * 3.2, cy - dy * 2.2 + py * 3.2);
    let b = pos2(cx + dx * 2.4, cy + dy * 2.4);
    let c = pos2(cx - dx * 2.2 - px * 3.2, cy - dy * 2.2 - py * 3.2);
    p.line_segment([a, b], st);
    p.line_segment([b, c], st);
}
