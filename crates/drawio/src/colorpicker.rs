//! The FigCraft-style colour picker popover for Draw.
//!
//! Mirrors `clone/figcraft/crates/ui-egui/src/colorpicker.rs` on top of the
//! drawio style model: a borderless 240-wide window with a Custom header, the
//! saturation/brightness square, hue + alpha sliders (same mesh painting and
//! ring knobs), Hex/RGB model fields with an alpha %, the classic drawio
//! preset swatches, the colours used on the page, and recent colours. The
//! colour math itself comes from `figcraft-color` (`Hsb`), so both crafts
//! convert identically.
//!
//! drawio mapping: the picker edits `fillColor` / `strokeColor` / `fontColor`
//! (hex) and the alpha slider writes the style's `opacity` /
//! `strokeOpacity` / `textOpacity` percentage — the drawio way of carrying
//! alpha. Page Background writes `mxGraphModel.background`.

use crate::style;
use crate::ui::DrawApp;
use doccraft_theme::Tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Mesh, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use figcraft_color::{Color, Hsb};

/// What the picker edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickTarget {
    Fill,
    Stroke,
    Font,
    PageBackground,
}

impl PickTarget {
    /// The mxGraph style key (empty = page background, not a style key).
    fn key(self) -> &'static str {
        match self {
            Self::Fill => "fillColor",
            Self::Stroke => "strokeColor",
            Self::Font => "fontColor",
            Self::PageBackground => "",
        }
    }

    /// The style key carrying the alpha % (drawio has no #RRGGBBAA).
    fn opacity_key(self) -> &'static str {
        match self {
            Self::Fill => "opacity",
            Self::Stroke => "strokeOpacity",
            Self::Font => "textOpacity",
            Self::PageBackground => "",
        }
    }

    fn presets(self) -> &'static [&'static str; 12] {
        match self {
            Self::Fill | Self::PageBackground => &style::FILL_PRESETS,
            Self::Stroke | Self::Font => &style::STROKE_PRESETS,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Stroke => "Line",
            Self::Font => "Font",
            Self::PageBackground => "Background",
        }
    }

    fn supports_alpha(self) -> bool {
        self != Self::PageBackground
    }
}

/// Open-picker state (one live picker at a time, like FigCraft's).
#[derive(Debug, Clone)]
pub struct Pick {
    pub target: PickTarget,
    /// (h, s, b, rgba at the last write) — keeps square/slider drags stable
    /// across frames the same way FigCraft caches its cp_hsb temp.
    pub hsb: Option<(f32, f32, f32, [u8; 4])>,
    /// 0 = Hex, 1 = RGB model row.
    pub model: usize,
    /// Undo coalescing: one entry per gesture (reset when a drag finishes).
    pub(crate) undone: bool,
}

impl Pick {
    pub fn new(target: PickTarget) -> Self {
        Self { target, hsb: None, model: 0, undone: false }
    }
}

impl DrawApp {
    /// The colour the picker shows for `target` (None = drawio "none").
    pub(crate) fn picker_current(&self, t: PickTarget) -> Option<Color32> {
        match t {
            PickTarget::PageBackground => self.page().background.as_deref().and_then(style::parse_color),
            _ => {
                let st = self.page().cell(&self.selected.first()?).map(|c| c.style.clone()).unwrap_or_default();
                if style::is_none_color(&st, t.key()) {
                    None
                } else {
                    let fallback = match t {
                        PickTarget::Fill => Color32::WHITE,
                        _ => Color32::BLACK,
                    };
                    Some(style::color_or(&st, t.key(), fallback))
                }
            }
        }
    }

    /// Write the picked colour (`None` = drawio "none"). `alpha_pct` (0..100)
    /// rides along when the alpha slider moved. `live` = mid-drag.
    pub(crate) fn picker_write(&mut self, t: PickTarget, col: Option<Color32>, alpha_pct: Option<f32>, live: bool) {
        // one undo per gesture (FigCraft's write/live/finish split)
        if !live {
            if let Some(p) = self.picker.as_mut() {
                p.undone = false;
            }
        } else if !self.picker.as_ref().is_some_and(|p| p.undone) {
            self.push_undo();
            if let Some(p) = self.picker.as_mut() {
                p.undone = true;
            }
        }
        match t {
            PickTarget::PageBackground => {
                self.page_mut().background = col.map(style::color_hex);
                self.file.dirty = true;
            }
            _ => {
                let (key, op) = (t.key(), t.opacity_key());
                self.apply_style_now(|s| {
                    *s = match col {
                        Some(c) => style::set(s, key, &style::color_hex(c)),
                        None => style::set(s, key, "none"),
                    };
                    if let Some(a) = alpha_pct {
                        *s = style::set(s, op, &format!("{:.0}", a.round()));
                    }
                });
            }
        }
        if !live
            && let Some(c) = col
        {
            self.remember_recent(c);
        }
    }

    /// FigCraft's recent-colour list: most-recent-first, deduped, capped.
    pub(crate) fn remember_recent(&mut self, c: Color32) {
        self.recent_colors.retain(|x| *x != c);
        self.recent_colors.insert(0, c);
        self.recent_colors.truncate(14);
    }

    /// Colours used on the active page (picker's "On this page" row).
    fn page_colors(&self) -> Vec<Color32> {
        let mut out: Vec<Color32> = Vec::new();
        for c in &self.page().cells {
            if c.edge {
                continue;
            }
            for key in ["fillColor", "strokeColor"] {
                if let Some(hex) = style::get(&c.style, key)
                    && let Some(col) = style::parse_color(&hex)
                    && !out.contains(&col)
                {
                    out.push(col);
                    if out.len() >= 14 {
                        return out;
                    }
                }
            }
        }
        out
    }
}

/// Show the picker window (called from [`DrawApp::dialogs`]).
pub fn show(app: &mut DrawApp, ctx: &egui::Context) {
    let Some(target) = app.picker.as_ref().map(|p| p.target) else { return };
    // A cell-scoped picker follows FigCraft's rule: the selection it was
    // opened for is gone → close instead of writing into nothing.
    if target != PickTarget::PageBackground && app.selected.is_empty() {
        app.picker = None;
        return;
    }
    let t = Tokens::get(ctx);
    let current = app.picker_current(target);
    let screen = ctx.content_rect();
    // Beside the format panel, exactly where FigCraft parks its picker.
    let pos = pos2((screen.max.x - 512.0).max(screen.min.x + 260.0), 128.0);
    let mut open = true;
    let mut close_now = false;
    egui::Window::new("draw_color_picker")
        .title_bar(false)
        .fixed_pos(pos)
        .fixed_size(vec2(240.0, 0.0))
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).stroke(doccraft_theme::hairline(&t)).corner_radius(8.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            // Header — "Custom · Fill" + the close ×, FigCraft layout.
            let (hr, _) = ui.allocate_exact_size(vec2(240.0, 32.0), Sense::hover());
            ui.painter().text(
                pos2(hr.min.x + 14.0, hr.center().y),
                Align2::LEFT_CENTER,
                format!("Custom · {}", target.label()),
                FontId::proportional(11.5),
                t.text,
            );
            let cr = Rect::from_center_size(pos2(hr.max.x - 18.0, hr.center().y), vec2(22.0, 22.0));
            let cresp = ui.interact(cr, Id::new("draw_cp_close"), Sense::click());
            if cresp.hovered() {
                ui.painter().rect_filled(cr, CornerRadius::same(4), t.hover);
            }
            doccraft_theme::icons::paint(ui.painter(), cr.shrink(6.0), "close", t.icon, t.panel);
            if cresp.clicked() {
                close_now = true;
            }

            // ── SV square + hue + alpha (FigCraft geometry) ──
            let mut color = current.unwrap_or(Color32::WHITE);
            let mut alpha_pct = match target {
                PickTarget::PageBackground => None,
                _ => {
                    let st = app
                        .page()
                        .cell(app.selected.first().map(|s| s.as_str()).unwrap_or(""))
                        .map(|c| c.style.clone())
                        .unwrap_or_default();
                    Some(style::get_f32(&st, target.opacity_key(), 100.0))
                }
            };
            let mut done = false;
            let base = Color::from_rgba8(color.r(), color.g(), color.b(), 255);
            let mut hsb = app
                .picker
                .as_ref()
                .and_then(|p| p.hsb.as_ref())
                .filter(|(_, _, _, rgba)| *rgba == color.to_array() || *rgba == c32(base).to_array())
                .map(|(h, s, b, _)| Hsb { h: *h, s: *s, b: *b })
                .unwrap_or_else(|| Hsb::from_color(base));
            let (sq, _) = ui.allocate_exact_size(vec2(240.0, 200.0), Sense::hover());
            let (c1, d1) = sv_square(ui, sq, &mut hsb);
            done |= d1;
            let (hr2, _) = ui.allocate_exact_size(vec2(240.0, 26.0), Sense::hover());
            let hr2 = Rect::from_min_max(pos2(hr2.min.x + 12.0, hr2.min.y + 3.0), pos2(hr2.max.x - 12.0, hr2.max.y - 3.0));
            let (c2, d2) = hue_slider(ui, hr2, &mut hsb);
            done |= d2;
            let mut nc = hsb.to_color();
            let mut changed = c1 || c2;
            if target.supports_alpha() {
                let (ar, _) = ui.allocate_exact_size(vec2(240.0, 18.0), Sense::hover());
                let ar = Rect::from_min_max(pos2(ar.min.x + 12.0, ar.min.y + 1.0), pos2(ar.max.x - 12.0, ar.max.y - 1.0));
                let a0 = (alpha_pct.unwrap_or(100.0) / 100.0).clamp(0.0, 1.0);
                if let Some((a, d)) = alpha_slider(ui, ar, c32(nc), a0) {
                    alpha_pct = Some(a * 100.0);
                    changed = true;
                    done |= d;
                }
            }
            if changed {
                nc = nc.with_alpha(1.0);
                // cache hsb against the colour being written
                if let Some(p) = app.picker.as_mut() {
                    p.hsb = Some((hsb.h, hsb.s, hsb.b, c32(nc).to_array()));
                }
                app.picker_write(target, Some(c32(nc)), alpha_pct, !done);
                color = c32(nc);
            }
            // Gesture finished (FigCraft's finish()): record the final colour
            // in Recent and close the undo gesture even when the last frame
            // produced no new write (release-after-drag).
            if done {
                if let Some(c) = app.picker_current(target) {
                    app.remember_recent(c);
                }
                if let Some(p) = app.picker.as_mut() {
                    p.undone = false;
                }
            }

            // ── model fields: Hex | RGB + A% (FigCraft's model_fields) ──
            let model = app.picker.as_ref().map(|p| p.model).unwrap_or(0);
            let mut new_model = model;
            let mut field_write: Option<(Option<Color32>, Option<f32>)> = None;
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                egui::ComboBox::from_id_salt("draw_cp_model")
                    .selected_text(if model == 0 { "Hex" } else { "RGB" })
                    .width(58.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(model == 0, "Hex").clicked() {
                            new_model = 0;
                        }
                        if ui.selectable_label(model == 1, "RGB").clicked() {
                            new_model = 1;
                        }
                    });
                match model {
                    1 => {
                        let [r, g, b, _] = color.to_array();
                        let mut rgb = [f64::from(r), f64::from(g), f64::from(b)];
                        for (k, slot) in rgb.iter_mut().enumerate() {
                            let mut v = *slot;
                            if ui.add(egui::DragValue::new(&mut v).range(0.0..=255.0).speed(1.0).max_decimals(0)).changed() {
                                *slot = v.clamp(0.0, 255.0);
                            }
                            let _ = k;
                        }
                        let c2 = Color32::from_rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8);
                        if c2 != color {
                            field_write = Some((Some(c2), None));
                        }
                    }
                    _ => {
                        let mut hex = style::color_hex(color);
                        if ui.add(egui::TextEdit::singleline(&mut hex).desired_width(120.0)).lost_focus()
                            && let Some(c2) = style::parse_color(hex.trim())
                        {
                            field_write = Some((Some(c2), None));
                        }
                    }
                }
                if target.supports_alpha() {
                    let mut a = alpha_pct.unwrap_or(100.0);
                    if ui.add(egui::DragValue::new(&mut a).range(0.0..=100.0).suffix("%").max_decimals(0)).changed() {
                        alpha_pct = Some(a.clamp(0.0, 100.0));
                        field_write = Some((Some(color), alpha_pct));
                    }
                }
            });
            if new_model != model
                && let Some(p) = app.picker.as_mut()
            {
                p.model = new_model;
            }
            if let Some((c2, a2)) = field_write {
                app.picker_write(target, c2, a2, false);
            }

            // ── Presets (the classic drawio style-panel palette) ──
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(egui::RichText::new("PRESETS").size(9.5).strong().color(t.text_secondary));
            });
            let mut preset_pick: Option<Option<Color32>> = None;
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().spacing.item_spacing = Vec2::splat(3.0);
                let (nr, nresp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                ui.painter().rect_filled(nr.shrink(2.0), 2.0, t.panel);
                ui.painter().line_segment([nr.left_bottom() + Vec2::new(3.0, -3.0), nr.right_top() - Vec2::new(3.0, -3.0)], Stroke::new(1.2, t.danger));
                ui.painter().rect_stroke(nr, CornerRadius::same(3), Stroke::new(1.0, t.border), StrokeKind::Outside);
                if nresp.clicked() {
                    preset_pick = Some(None);
                }
                nresp.on_hover_text("None");
                for hex in target.presets() {
                    let c = style::parse_color(hex).unwrap_or(Color32::WHITE);
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                    ui.painter().rect_filled(r.shrink(2.0), 2.0, c);
                    ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.border), StrokeKind::Outside);
                    if resp.clicked() {
                        preset_pick = Some(Some(c));
                    }
                    resp.on_hover_text(*hex);
                }
            });
            if let Some(pick) = preset_pick {
                app.picker_write(target, pick, if pick.is_some() { alpha_pct } else { None }, false);
            }

            // ── On this page (FigCraft's document colours) ──
            let doc_colors = app.page_colors();
            if !doc_colors.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("ON THIS PAGE").size(9.5).strong().color(t.text_secondary));
                });
                let mut pick = None;
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::splat(3.0);
                    for c in &doc_colors {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                        ui.painter().rect_filled(r.shrink(2.0), 2.0, *c);
                        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.border), StrokeKind::Outside);
                        if resp.clicked() {
                            pick = Some(*c);
                        }
                        resp.on_hover_text(style::color_hex(*c));
                    }
                });
                if let Some(c) = pick {
                    app.picker_write(target, Some(c), alpha_pct, false);
                }
            }

            // ── Recent (FigCraft's recent colours) ──
            let recent = app.recent_colors.clone();
            if !recent.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("RECENT").size(9.5).strong().color(t.text_secondary));
                });
                let mut pick = None;
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::splat(3.0);
                    for c in recent.iter().take(14) {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                        ui.painter().rect_filled(r.shrink(2.0), 2.0, *c);
                        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.border), StrokeKind::Outside);
                        if resp.clicked() {
                            pick = Some(*c);
                        }
                        resp.on_hover_text(style::color_hex(*c));
                    }
                });
                if let Some(c) = pick {
                    app.picker_write(target, Some(c), alpha_pct, false);
                }
            }
            ui.add_space(4.0);
        });
    if !open || close_now || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(c) = app.picker_current(target) {
            app.remember_recent(c);
        }
        app.picker = None;
    }
}

/// Full-saturation hue for the square's horizontal axis and the hue strip.
fn hue_color(h: f32) -> Color32 {
    c32(Hsb { h, s: 1.0, b: 1.0 }.to_color())
}

fn c32(c: Color) -> Color32 {
    let [r, g, b, a] = c.to_rgba8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Checkerboard under transparent swatches / the alpha slider.
fn checker(p: &egui::Painter, r: Rect, s: f32) {
    let light = Color32::from_gray(226);
    let dark = Color32::from_gray(180);
    let mut y = r.min.y;
    let mut row = 0usize;
    while y < r.max.y {
        let mut x = r.min.x;
        let mut col = row;
        while x < r.max.x {
            let cell = Rect::from_min_max(pos2(x, y), pos2((x + s).min(r.max.x), (y + s).min(r.max.y)));
            p.rect_filled(cell, 0.0, if col % 2 == 0 { light } else { dark });
            x += s;
            col += 1;
        }
        y += s;
        row += 1;
    }
}

/// White → hue horizontally, transparent → black vertically, ring knob —
/// the FigCraft SV square.
fn sv_square(ui: &mut Ui, rect: Rect, hsb: &mut Hsb) -> (bool, bool) {
    let p = ui.painter();
    let mut m = Mesh::default();
    let hc = hue_color(hsb.h);
    for (pt, c) in [(rect.left_top(), Color32::WHITE), (rect.right_top(), hc), (rect.right_bottom(), hc), (rect.left_bottom(), Color32::WHITE)] {
        m.colored_vertex(pt, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
    let mut m2 = Mesh::default();
    for (pt, c) in [
        (rect.left_top(), Color32::TRANSPARENT),
        (rect.right_top(), Color32::TRANSPARENT),
        (rect.right_bottom(), Color32::BLACK),
        (rect.left_bottom(), Color32::BLACK),
    ] {
        m2.colored_vertex(pt, c);
    }
    m2.add_triangle(0, 1, 2);
    m2.add_triangle(0, 2, 3);
    p.add(m2);
    let resp = ui.interact(rect, Id::new("draw_cp_sv"), Sense::click_and_drag());
    let mut changed = false;
    if (resp.dragged() || resp.clicked() || resp.drag_started())
        && let Some(pt) = resp.interact_pointer_pos()
    {
        hsb.s = ((pt.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        hsb.b = 1.0 - ((pt.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let knob = pos2(rect.min.x + hsb.s * rect.width(), rect.min.y + (1.0 - hsb.b) * rect.height());
    p.circle_stroke(knob, 6.0, Stroke::new(2.0, Color32::WHITE));
    p.circle_stroke(knob, 7.0, Stroke::new(1.0, Color32::from_black_alpha(80)));
    (changed, resp.drag_stopped() || resp.clicked())
}

/// 12-stop hue strip with the FigCraft ring knob.
fn hue_slider(ui: &mut Ui, rect: Rect, hsb: &mut Hsb) -> (bool, bool) {
    let p = ui.painter();
    let n = 12;
    let mut m = Mesh::default();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = rect.min.x + t * rect.width();
        let c = hue_color(t * 360.0);
        m.colored_vertex(pos2(x, rect.min.y), c);
        m.colored_vertex(pos2(x, rect.max.y), c);
        if i > 0 {
            let k = (i * 2) as u32;
            m.add_triangle(k - 2, k - 1, k);
            m.add_triangle(k - 1, k, k + 1);
        }
    }
    p.add(m);
    slider_knob(ui, rect, hsb.h / 360.0, Id::new("draw_cp_hue"))
        .map(|(t, done)| {
            hsb.h = (t * 360.0).clamp(0.0, 359.9);
            (true, done)
        })
        .unwrap_or((false, false))
}

fn slider_knob(ui: &mut Ui, rect: Rect, t: f32, id: Id) -> Option<(f32, bool)> {
    let p = ui.painter();
    let resp = ui.interact(rect.expand(4.0), id, Sense::click_and_drag());
    let x = rect.min.x + t.clamp(0.0, 1.0) * rect.width();
    p.circle_stroke(pos2(x, rect.center().y), rect.height() / 2.0 + 1.0, Stroke::new(2.0, Color32::WHITE));
    p.circle_stroke(pos2(x, rect.center().y), rect.height() / 2.0 + 2.0, Stroke::new(1.0, Color32::from_black_alpha(80)));
    if (resp.dragged() || resp.clicked() || resp.drag_started())
        && let Some(pt) = resp.interact_pointer_pos()
    {
        return Some((((pt.x - rect.min.x) / rect.width()).clamp(0.0, 1.0), resp.drag_stopped() || resp.clicked()));
    }
    if resp.drag_stopped() {
        return Some((t, true));
    }
    None
}

/// Checkerboard → opaque colour, carrying the drawio opacity %.
fn alpha_slider(ui: &mut Ui, rect: Rect, color: Color32, a: f32) -> Option<(f32, bool)> {
    let p = ui.painter().with_clip_rect(rect);
    checker(&p, rect, rect.height() / 2.0);
    let mut m = Mesh::default();
    let opaque = color;
    for (pt, c) in [
        (rect.left_top(), Color32::TRANSPARENT),
        (rect.right_top(), opaque),
        (rect.right_bottom(), opaque),
        (rect.left_bottom(), Color32::TRANSPARENT),
    ] {
        m.colored_vertex(pt, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    ui.painter().add(m);
    slider_knob(ui, rect, a, Id::new("draw_cp_alpha"))
}
