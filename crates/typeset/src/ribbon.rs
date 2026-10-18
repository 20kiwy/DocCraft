//! The Typeset ribbon: tab strip and every tab's groups, copied from the WordCraft
//! anatomy — a 28 pt tab strip, then a grouped command strip with small uppercase
//! captions under each group. The Page tab edits the layout settings directly.

use crate::ui::{self, PaneTab, Tab, TypesetApp, ACCENT, ACTIVE, BORDER, CONTENT_H, DIM, FAINT, HOVER, LABEL_H, PANEL, TEXT};
use crate::layout;
use egui::{Align2, Color32, FontId, Margin, Rect, Sense, Stroke, Ui, WidgetText, pos2, vec2};

/// Tab strip + grouped command strip.
pub fn show(app: &mut TypesetApp, ui: &mut Ui) {
    tabs(app, ui);
    commands(app, ui);
}

fn tabs(app: &mut TypesetApp, ui: &mut Ui) {
    egui::Panel::top("ts_tabs")
        .exact_size(28.0)
        .frame(egui::Frame::NONE.fill(PANEL).inner_margin(Margin { left: 8, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for tab in [Tab::Home, Tab::Insert, Tab::Page, Tab::View] {
                    let name = tab.name();
                    let w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(name.to_string(), FontId::proportional(12.5), TEXT).size().x) + 22.0;
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    let active = app.tab == tab;
                    if resp.hovered() && !active {
                        ui.painter().rect_filled(r.shrink2(vec2(0.0, 4.0)), 4.0, HOVER);
                    }
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, name, FontId::proportional(12.5), if active { TEXT } else { DIM });
                    if active {
                        let u = Rect::from_center_size(pos2(r.center().x, r.max.y - 2.0), vec2(w - 18.0, 3.0));
                        ui.painter().rect_filled(u, 2.0, ACCENT);
                    }
                    if resp.clicked() {
                        app.tab = tab;
                    }
                }
            });
        });
}

fn commands(app: &mut TypesetApp, ui: &mut Ui) {
    egui::Panel::top("ts_ribbon")
        .exact_size(CONTENT_H + LABEL_H + 8.0)
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(Margin { left: 8, right: 8, top: 4, bottom: 4 })
                .stroke(Stroke::new(1.0, BORDER)),
        )
        .show(ui, |ui| {
            ui.style_mut().spacing.interact_size.y = 22.0;
            egui::ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        match app.tab {
                            Tab::Home => home(app, ui),
                            Tab::Insert => insert(app, ui),
                            Tab::Page => page(app, ui),
                            Tab::View => view(app, ui),
                        }
                    });
                });
        });
}

// ── Home ─────────────────────────────────────────────────────────────────────

fn home(app: &mut TypesetApp, ui: &mut Ui) {
    group(ui, "Font", |ui| {
        if small(ui, "Serif", !app.editor.mono).clicked() {
            app.editor.mono = false;
        }
        if small(ui, "Mono", app.editor.mono).clicked() {
            app.editor.mono = true;
        }
        if small(ui, "A−", false).on_hover_text("Smaller body text").clicked() {
            app.set_base(ui.ctx(), app.settings.style.base_size - 1.0);
        }
        if small(ui, "A+", false).on_hover_text("Larger body text").clicked() {
            app.set_base(ui.ctx(), app.settings.style.base_size + 1.0);
        }
        if glyph(ui, "B", 15.0, false).on_hover_text("Wrap **bold** around the selection").clicked() {
            app.wrap_selection(ui.ctx(), "**", "**");
        }
        if glyph(ui, "I", 15.0, false).on_hover_text("Wrap *italic* around the selection").clicked() {
            app.wrap_selection(ui.ctx(), "*", "*");
        }
    });
    group(ui, "Styles", |ui| {
        if big(ui, "H1", "Chapter", false).clicked() {
            app.apply_line_prefix(ui.ctx(), "# ");
        }
        if big(ui, "H2", "Section", false).clicked() {
            app.apply_line_prefix(ui.ctx(), "## ");
        }
        if big(ui, "H3", "Sub", false).clicked() {
            app.apply_line_prefix(ui.ctx(), "### ");
        }
        ui.vertical(|ui| {
            if small(ui, "> Quote", false).on_hover_text("Prefix the line with >").clicked() {
                app.apply_line_prefix(ui.ctx(), "> ");
            }
            if small(ui, "Code", false).on_hover_text("Wrap the line in a code fence").clicked() {
                app.fence_line(ui.ctx());
            }
        });
    });
    group(ui, "Paragraph", |ui| {
        if small(ui, "Center", false).on_hover_text("Wrap the line in \\center{…}").clicked() {
            app.wrap_line(ui.ctx(), "\\center{", "}");
        }
        if small(ui, "Right", false).on_hover_text("Wrap the line in \\right{…}").clicked() {
            app.wrap_line(ui.ctx(), "\\right{", "}");
        }
        if small(ui, "Justify", app.settings.style.justify).on_hover_text("Stretch lines to the column edges").clicked() {
            app.set_justify(ui.ctx(), !app.settings.style.justify);
        }
    });
    group(ui, "Editing", |ui| {
        if big(ui, "F", "Find", app.editor.find_open).clicked() {
            app.editor.find_open = !app.editor.find_open;
        }
    });
}

// ── Insert ───────────────────────────────────────────────────────────────────

fn insert(app: &mut TypesetApp, ui: &mut Ui) {
    group(ui, "Page", |ui| {
        if big_icon(ui, "break", "Page\nBreak", false).clicked() {
            app.insert_block(ui.ctx(), "\\pagebreak");
        }
    });
    group(ui, "Figures", |ui| {
        if big_icon(ui, "image", "Image", false).clicked() {
            app.insert_block(ui.ctx(), "\\image{figures/plate.pdf ; Figure caption}");
        }
    });
    group(ui, "Tables", |ui| {
        if big_icon(ui, "table", "Table", false).clicked() {
            app.insert_block(ui.ctx(), "| Item | Qty | Note |\n| Frame | 12 | oak |\n| Back | 12 | birch |");
        }
    });
    group(ui, "Symbols", |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for s in ["—", "–", "…"] {
                    if small(ui, s, false).on_hover_text(format!("Insert {s}")).clicked() {
                        app.insert_at_cursor(ui.ctx(), s);
                    }
                }
            });
            ui.horizontal(|ui| {
                for s in ["\u{201c}", "\u{201d}"] {
                    if small(ui, s, false).on_hover_text(format!("Insert {s}")).clicked() {
                        app.insert_at_cursor(ui.ctx(), s);
                    }
                }
            });
        });
    });
}

// ── Page (the settings well: edits the layout engine's config) ───────────────

fn page(app: &mut TypesetApp, ui: &mut Ui) {
    group(ui, "Setup", |ui| {
        let paper = app.settings.page.paper;
        ui.menu_button(WidgetText::from(format!("Paper · {}", paper.name())), |ui| {
            for (p, n, _, _) in layout::PAPERS {
                if ui.selectable_label(paper == p, n).clicked() {
                    app.set_paper(ui.ctx(), p);
                    ui.close();
                }
            }
            if ui.selectable_label(paper == layout::Paper::Custom, "Custom…").clicked() {
                app.set_paper(ui.ctx(), layout::Paper::Custom);
                app.settings_open = true;
                ui.close();
            }
        });
        ui.menu_button(WidgetText::from(if app.settings.page.landscape { "Landscape" } else { "Portrait" }), |ui| {
            if ui.selectable_label(!app.settings.page.landscape, "Portrait").clicked() {
                app.set_landscape(ui.ctx(), false);
                ui.close();
            }
            if ui.selectable_label(app.settings.page.landscape, "Landscape").clicked() {
                app.set_landscape(ui.ctx(), true);
                ui.close();
            }
        });
        let m = app.settings.page.margins_name();
        ui.menu_button(WidgetText::from(format!("Margins · {m}")), |ui| {
            for (name, margins) in
                [("Normal", layout::PageSetup::NORMAL), ("Narrow", layout::PageSetup::NARROW), ("Wide", layout::PageSetup::WIDE)]
            {
                if ui.selectable_label(m == name, name).clicked() {
                    app.set_margins(ui.ctx(), margins);
                    ui.close();
                }
            }
            if ui.selectable_label(m == "Custom", "Custom…").clicked() {
                app.settings_open = true;
                ui.close();
            }
        });
    });
    group(ui, "Columns", |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if small(ui, "1", app.settings.page.columns == 1).on_hover_text("One column").clicked() {
                    app.set_columns(ui.ctx(), 1);
                }
                if small(ui, "2", app.settings.page.columns == 2).on_hover_text("Two columns").clicked() {
                    app.set_columns(ui.ctx(), 2);
                }
            });
            if app.settings.page.columns == 2 {
                let mut g = app.settings.page.gutter;
                if ui.add(egui::DragValue::new(&mut g).suffix(" pt").range(8.0..=96.0).speed(0.5)).changed() {
                    app.set_gutter(ui.ctx(), g);
                }
            }
        });
    });
    group(ui, "Header & Footer", |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if small(ui, "Numbers", app.settings.hf.numbers).clicked() {
                    app.set_numbers(ui.ctx(), !app.settings.hf.numbers);
                }
                let pos = match app.settings.hf.number_pos {
                    layout::NumPos::Center => "Center",
                    layout::NumPos::Outside => "Outside",
                };
                ui.menu_button(WidgetText::from(pos.to_string()), |ui| {
                    for (n, p) in [("Center", layout::NumPos::Center), ("Outside", layout::NumPos::Outside)] {
                        if ui.selectable_label(app.settings.hf.number_pos == p, n).clicked() {
                            app.set_number_pos(ui.ctx(), p);
                            ui.close();
                        }
                    }
                });
            });
            let mut footer = app.settings.hf.footer.clone();
            let resp = ui.add(egui::TextEdit::singleline(&mut footer).hint_text("Footer text…").desired_width(120.0));
            if resp.changed() {
                app.set_footer(ui.ctx(), footer);
            }
        });
    });
}

// ── View ─────────────────────────────────────────────────────────────────────

fn view(app: &mut TypesetApp, ui: &mut Ui) {
    group(ui, "Preview", |ui| {
        if small(ui, "Right", app.preview).clicked() {
            app.preview = true;
            app.pane = PaneTab::Pages;
        }
        if small(ui, "Off", !app.preview).clicked() {
            app.preview = false;
        }
    });
    group(ui, "Zoom", |ui| {
        if glyph(ui, "−", 14.0, false).on_hover_text("Zoom out").clicked() {
            app.zoom_out();
        }
        if glyph(ui, "+", 14.0, false).on_hover_text("Zoom in").clicked() {
            app.zoom_in();
        }
        if small(ui, "Fit", app.fit).clicked() {
            app.fit = true;
        }
    });
    group(ui, "Rulers", |ui| {
        if small(ui, "Rulers", app.rulers).clicked() {
            app.rulers = !app.rulers;
        }
    });
}

// ── widgets (WordCraft group/button anatomy, Typeset colors) ─────────────────

/// A ribbon group: content, a centred uppercase caption below and a divider on the right.
fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let start = ui.cursor().min;
    let label = title.to_uppercase();
    let label_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.clone(), FontId::proportional(10.0), FAINT).size().x);
    let inner = ui.vertical(|ui| {
        ui.set_min_width(label_w.max(52.0) + 12.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), CONTENT_H),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 1.0);
                add(ui);
            },
        );
    });
    let w = inner.response.rect.max.x - start.x;
    let full_h = CONTENT_H + LABEL_H;
    ui.painter().text(
        pos2(start.x + w / 2.0, start.y + full_h - LABEL_H / 2.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.0),
        FAINT,
    );
    ui.add_space(5.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment([pos2(x, start.y + 4.0), pos2(x, start.y + full_h - 4.0)], Stroke::new(1.0, BORDER));
    ui.add_space(6.0);
}

/// A small labelled button (WordCraft's `small`, drawn by hand for the dark chrome).
pub fn small(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(12.0), TEXT).size().x) + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w.max(24.0), 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if active {
        ui.painter().rect_filled(r, 4.0, ACTIVE);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    let ink = if active { ui_color_on_active() } else { TEXT };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), ink);
    resp
}

/// A small square button with a text glyph (B, I, −, +…).
fn glyph(ui: &mut Ui, g: &str, size: f32, active: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    if active {
        ui.painter().rect_filled(r, 4.0, ACTIVE);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, g, FontId::proportional(size), TEXT);
    resp
}

/// A large button: a letter tile or vector icon on top, a two-line label below.
fn big(ui: &mut Ui, tile: &str, label: &str, active: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(50.0, CONTENT_H - 6.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.replace('\n', " ").as_str()));
    if active {
        ui.painter().rect_filled(r, 4.0, ACTIVE);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    let ink = if active { ui_color_on_active() } else { TEXT };
    tile_rect(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 16.0), vec2(28.0, 22.0)), tile, ink);
    let mut lines = label.splitn(2, '\n');
    let l1 = lines.next().unwrap_or("");
    let l2 = lines.next().unwrap_or("");
    ui.painter().text(pos2(r.center().x, r.min.y + 37.0), Align2::CENTER_CENTER, l1, FontId::proportional(10.5), ink);
    if !l2.is_empty() {
        ui.painter().text(pos2(r.center().x, r.min.y + 49.0), Align2::CENTER_CENTER, l2, FontId::proportional(10.5), ink);
    }
    resp
}

/// Like [`big`], but the tile paints a vector icon by name.
fn big_icon(ui: &mut Ui, icon: &str, label: &str, active: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(50.0, CONTENT_H - 6.0), Sense::click());
    if active {
        ui.painter().rect_filled(r, 4.0, ACTIVE);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    let ink = if active { ui_color_on_active() } else { TEXT };
    ui::paint_icon(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 16.0), vec2(24.0, 24.0)), icon, ink);
    let mut lines = label.splitn(2, '\n');
    let l1 = lines.next().unwrap_or("");
    let l2 = lines.next().unwrap_or("");
    ui.painter().text(pos2(r.center().x, r.min.y + 37.0), Align2::CENTER_CENTER, l1, FontId::proportional(10.5), ink);
    if !l2.is_empty() {
        ui.painter().text(pos2(r.center().x, r.min.y + 49.0), Align2::CENTER_CENTER, l2, FontId::proportional(10.5), ink);
    }
    resp
}

/// A lettered tile, the way WordCraft draws icons its set lacks.
fn tile_rect(p: &egui::Painter, r: Rect, label: &str, ink: Color32) {
    p.rect_stroke(r, 4.0, Stroke::new(1.2, ink), egui::StrokeKind::Inside);
    p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.5), ink);
}

fn ui_color_on_active() -> Color32 {
    Color32::from_rgb(0xf0, 0xea, 0xf8)
}
