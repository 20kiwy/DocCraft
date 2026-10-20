//! The right pane: the page preview (Pages), the heading Outline, document stats
//! and Export, plus the Recompile status line at the bottom. WordCraft panes.rs
//! anatomy: a resizable right panel with a semibold header and a close button.

use crate::layout::Page;
use crate::ui::{self, PaneTab, TypesetApp};
use doccraft_theme::{Tokens, card_shadow};
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

/// Preview pane + its tabbed panels.
pub fn show(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("ts_pane")
        .default_size(300.0)
        .min_size(240.0)
        .max_size(560.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(0).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            header(app, ui);
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for tab in [PaneTab::Pages, PaneTab::Outline, PaneTab::Document, PaneTab::Export] {
                    if pane_tab(ui, tab.name(), app.pane == tab).clicked() {
                        app.pane = tab;
                    }
                }
            });
            ui.separator();
            match app.pane {
                PaneTab::Pages => pages_body(app, ui),
                PaneTab::Outline => outline(app, ui),
                PaneTab::Document => document(app, ui),
                PaneTab::Export => export(app, ui),
            }
            ui.separator();
            recompile_line(app, ui);
        });
}

/// A pane tab: active = `tab_active` fill + full text, inactive = secondary.
fn pane_tab(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(12.0), t.text).size().x + 14.0).max(24.0), 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if active {
        ui.painter().rect_filled(r, 5.0, t.tab_active);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), if active { t.text } else { t.text_secondary });
    resp
}

fn header(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Preview & Panels").font(FontId::proportional(14.0)).strong().color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, t.hover);
            }
            let c = r.center();
            let d = 5.0;
            ui.painter().line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], Stroke::new(1.4, t.text_secondary));
            ui.painter().line_segment([pos2(c.x - d, c.y + d), pos2(c.x + d, c.y - d)], Stroke::new(1.4, t.text_secondary));
            if resp.on_hover_text("Close the preview pane (View › Preview › Off)").clicked() {
                app.preview = false;
            }
        });
    });
    ui.add_space(2.0);
}

/// The paginated preview: page cards (white paper / dark paper) on the canvas,
/// each with the theme's soft card shadow. An outline click lands here the
/// following frame and scrolls the pane to that page's card.
fn pages_body(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (pw, ph) = app.settings.page.page_size();
    let mono = app.settings.style.mono_font;
    let paper = if t.dark { Color32::from_rgb(0x25, 0x25, 0x26) } else { Color32::WHITE };
    let mut tops: Vec<f32> = Vec::new();
    let out = egui::ScrollArea::vertical()
        .id_salt("ts_pane_scroll")
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 12.0);
            let avail = ui.available_width() - 24.0;
            let k = if app.fit { (avail / pw).clamp(0.15, 1.5) } else { app.zoom };
            if app.rulers {
                ruler(ui, pw, k);
            }
            let pages: Vec<Page> = app.pages.clone();
            for (i, page) in pages.iter().enumerate() {
                let w = pw * k;
                let h = ph * k;
                ui.add_space(6.0);
                // page card: theme shadow behind the sheet, hairline frame
                let rect = egui::Frame::NONE
                    .shadow(card_shadow(&t))
                    .corner_radius(2)
                    .show(ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, paper);
                        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
                        paint_page(ui, rect, page, k, mono, t.dark);
                        rect
                    })
                    .inner;
                tops.push(rect.min.y);
                ui.painter().text(pos2(rect.center().x, rect.max.y + 8.0), Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(10.0), t.text_tertiary);
            }
            ui.add_space(10.0);
        });
    if let Some(page) = app.jump.take() {
        if let Some(top) = tops.get(page) {
            let mut st = out.state;
            st.offset.y = *top - 12.0;
            st.store(ui.ctx(), out.id);
        }
    }
}

/// One page's runs and frames, painted at scale `k` (bottom-origin → top-origin).
/// On the dark paper the layout's print ink is inverted so it stays readable.
fn paint_page(ui: &mut Ui, rect: Rect, page: &Page, k: f32, mono: bool, dark: bool) {
    let ph = rect.height() / k;
    let ink = |c: [u8; 3]| {
        if dark {
            Color32::from_rgb(255 - c[0], 255 - c[1], 255 - c[2])
        } else {
            Color32::from_rgb(c[0], c[1], c[2])
        }
    };
    let p = ui.painter_at(rect);
    for fr in &page.frames {
        let r = Rect::from_min_max(
            pos2(rect.min.x + fr.x * k, rect.min.y + (ph - fr.y) * k),
            pos2(rect.min.x + (fr.x + fr.w) * k, rect.min.y + (ph - fr.y + fr.h) * k),
        );
        p.rect_stroke(r, 1.0, Stroke::new(0.8, ink(fr.color)), egui::StrokeKind::Inside);
    }
    for run in &page.runs {
        let font = if mono && run.size < 20.0 && !run.bold {
            FontId::monospace(run.size * k)
        } else if run.bold {
            FontId::proportional(run.size * k * 1.06)
        } else {
            FontId::proportional(run.size * k)
        };
        let pos = pos2(rect.min.x + run.x * k, rect.min.y + (ph - run.y - run.size) * k);
        p.text(pos, Align2::LEFT_TOP, &run.text, font, ink(run.color));
    }
}

fn ruler(ui: &mut Ui, pw: f32, k: f32) {
    let t = Tokens::get(ui.ctx());
    let h = 14.0;
    let (r, _) = ui.allocate_exact_size(Vec2::new(pw * k, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 2.0, t.panel);
    p.rect_stroke(r, 2.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let inches = (pw / 72.0) as usize;
    for i in 0..=inches * 2 {
        let x = r.min.x + i as f32 * 36.0 * k;
        if x > r.max.x {
            break;
        }
        let long = i % 2 == 0;
        p.line_segment([pos2(x, r.max.y - if long { 8.0 } else { 4.0 }), pos2(x, r.max.y)], Stroke::new(1.0, t.text_tertiary));
        if long && i > 0 {
            p.text(pos2(x + 2.0, r.min.y + 3.0), Align2::LEFT_TOP, format!("{}", i / 2), FontId::proportional(8.0), t.text_tertiary);
        }
    }
}

fn outline(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui::pane_scroll(ui, "ts_outline", |ui| {
        let heads = app.headings.clone();
        if heads.is_empty() {
            ui.label(egui::RichText::new("No chapters or sections yet — add # / ## / ### lines.").small().color(t.text_tertiary));
        }
        for (level, text, page) in heads {
            let indent = (level as f32 - 1.0) * 14.0;
            ui.horizontal(|ui| {
                ui.add_space(indent + 2.0);
                let w = (ui.available_width() - indent - 30.0).max(60.0);
                let (r, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 5.0, t.hover);
                }
                // outline marks painted (no font dependency → no tofu):
                // level 1 = filled bar, 2 = chevron, 3+ = middle dot
                let mark_c = Rect::from_center_size(pos2(r.min.x + 4.0, r.center().y), vec2(9.0, 9.0));
                match level {
                    1 => {
                        ui.painter().rect_filled(Rect::from_center_size(pos2(mark_c.center().x, r.center().y), vec2(3.0, 12.0)), 1.0, t.accent_text);
                    }
                    2 => {
                        ui::paint_icon(ui.painter(), mark_c, "chevron-right", t.accent_text, t.panel);
                    }
                    _ => {
                        ui.painter().text(pos2(mark_c.center().x, r.center().y), Align2::CENTER_CENTER, "·", FontId::proportional(10.0), t.accent_text);
                    }
                }
                ui.painter().text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, clip(&text, r.width() - 16.0, 11.5), FontId::proportional(11.5), t.text);
                ui.painter().text(pos2(r.max.x, r.center().y), Align2::RIGHT_CENTER, format!("p.{}", page + 1), FontId::proportional(9.5), t.text_tertiary);
                if resp.on_hover_text(format!("Go to page {}", page + 1)).clicked() {
                    app.pane = PaneTab::Pages;
                    app.jump = Some(page);
                }
            });
        }
    });
}

fn document(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui::pane_scroll(ui, "ts_doc", |ui| {
        let title = app
            .headings
            .iter()
            .find(|(l, _, _)| *l == 1)
            .map(|(_, t, _)| t.clone())
            .unwrap_or_else(|| if app.parsed.meta.title.is_empty() { "Untitled".into() } else { app.parsed.meta.title.clone() });
        ui.label(egui::RichText::new(&title).font(FontId::proportional(14.0)).strong().color(t.text));
        ui.add_space(6.0);
        let chapters = app.headings.iter().filter(|(l, ..)| *l == 1).count();
        let sections = app.headings.iter().filter(|(l, ..)| *l > 1).count();
        egui::Grid::new("ts_stats").num_columns(2).spacing(vec2(14.0, 3.0)).show(ui, |ui| {
            for (k, v) in [
                ("Words", format!("{}", crate::model::word_count(&app.source))),
                ("Characters", format!("{}", app.source.chars().count())),
                ("Lines", format!("{}", app.source.lines().count())),
                ("Pages", format!("{}", app.pages.len())),
                ("Chapters", format!("{chapters}")),
                ("Sections", format!("{sections}")),
            ] {
                ui.label(egui::RichText::new(k).small().color(t.text_secondary));
                ui.label(egui::RichText::new(v).small().color(t.text));
                ui.end_row();
            }
        });
    });
}

fn export(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui::pane_scroll(ui, "ts_export", |ui| {
        let ext = app.path.as_deref().and_then(|p| p.rsplit('.').next()).unwrap_or("typ").to_string();
        if ui::pane_button(ui, format!("Save source (.{ext})")).clicked() {
            app.save();
        }
        if ui::pane_button(ui, "Export .pdf").on_hover_text("Type1 base-14 PDF of the current layout").clicked() {
            app.export_pdf();
        }
        if ui::pane_button(ui, "Export .txt").on_hover_text("Plain-text render").clicked() {
            app.export_txt();
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Files are written to the system temp directory; the status line shows the path.").small().color(t.text_tertiary));
    });
}

fn recompile_line(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        let (ms, pages) = (app.compile_ms, app.pages.len());
        match &app.parsed.error {
            Some(e) => {
                ui.label(egui::RichText::new(format!("Line {}: {}", e.line, e.msg)).small().color(t.danger));
            }
            None => {
                ui.label(egui::RichText::new(format!("OK · {pages} pages · {ms} ms")).small().color(t.success));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::pane_button(ui, "Recompile").clicked() {
                app.recompile();
            }
            ui.label(egui::RichText::new("auto ·").small().color(t.text_tertiary));
        });
    });
}

fn clip(text: &str, w: f32, size: f32) -> String {
    let max = (w / (size * 0.52)).max(4.0) as usize;
    let t: String = text.chars().take(max).collect();
    if t.chars().count() < text.chars().count() {
        format!("{t}…")
    } else {
        t
    }
}
