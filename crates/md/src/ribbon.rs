//! The WordCraft-anatomy chrome for the Markdown craft: a title strip with a
//! Quick Access Toolbar, the signature tab strip + grouped command strip
//! (small uppercase captions under each group, thin dividers between), in
//! egui 0.36 style with 22–28 px controls and 4–6 px corner radii.

use crate::ui::MdApp;
use doccraft_theme::Tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};

// ── chrome geometry (WordCraft anatomy; every color comes from doccraft_theme) ──
pub const TITLE_H: f32 = 34.0;
pub const TABS_H: f32 = 28.0;
pub const RIBBON_H: f32 = 78.0;
pub const STATUS_H: f32 = 24.0;
const CONTENT_H: f32 = 54.0;
const LABEL_H: f32 = 16.0;

pub fn accent(app: &MdApp) -> Color32 {
    app.settings.accent.color()
}

// ── title strip + Quick Access Toolbar ───────────────────────────────────────

/// One 26×24 QAT button, WordCraft chrome.rs pattern (hover fill, glyph centred).
pub fn qat_btn(ui: &mut Ui, glyph: &str, tip: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 5.0, t.hover);
    }
    let c = if enabled { t.text } else { t.text_tertiary };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(14.0), c);
    resp.on_hover_text(tip).on_disabled_hover_text(tip)
}

/// One 26×24 QAT button painting a shared vector icon (no font → no tofu).
pub fn qat_btn_icon(ui: &mut Ui, icon: &str, tip: &str, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 5.0, t.hover);
    }
    let c = if enabled { t.text } else { t.text_tertiary };
    doccraft_theme::icons::paint(ui.painter(), r, icon, c, t.panel);
    resp.on_hover_text(tip).on_disabled_hover_text(tip)
}

/// The top title strip: craft + document name + dirty dot left, QAT + theme toggle right.
pub fn title_strip(app: &mut MdApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("md_title")
        .exact_size(TITLE_H)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 10, right: 6, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                ui.label(egui::RichText::new("Markdown").strong().size(13.5).color(t.text));
                ui.separator();
                let name = app.path.clone().unwrap_or_else(|| "Untitled.md".into());
                ui.label(egui::RichText::new(name).size(12.0).color(t.text_secondary));
                // dirty dot
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                if app.dirty {
                    ui.painter().circle_filled(r.center(), 3.0, accent(app));
                } else {
                    ui.painter().circle_stroke(r.center(), 3.0, Stroke::new(1.0, t.border));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    if qat_btn_icon(ui, "gear", "Markdown settings", true).clicked() {
                        app.show_settings = !app.show_settings;
                    }
                    // Sun / Moon theme toggle, next to the QAT
                    let (icon, tip) = if app.dark { ("sun", "Switch to the light theme") } else { ("moon", "Switch to the dark theme") };
                    if qat_btn_icon(ui, icon, tip, true).clicked() {
                        app.dark = !app.dark;
                    }
                    if qat_btn_icon(ui, "redo", "Redo", !app.redo.is_empty()).clicked() {
                        app.redo_edit();
                    }
                    if qat_btn_icon(ui, "undo", "Undo", !app.undo.is_empty()).clicked() {
                        app.undo_edit();
                    }
                    if qat_btn_icon(ui, "save", "Save (writes the opened file)", true).clicked() {
                        app.save();
                    }
                });
            });
        });
}

// ── tab strip ────────────────────────────────────────────────────────────────

/// The ribbon tab strip, using craft-ui's tab fitting helper for overflow.
pub fn tab_strip(app: &mut MdApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("md_tabs")
        .exact_size(TABS_H)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 8, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let names = ["Home", "Insert", "View"];
                let natural: Vec<f32> = names
                    .iter()
                    .map(|n| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*n).to_string(), FontId::proportional(12.5), t.text)).size().x + 26.0)
                    .collect();
                let fit = craft_ui::tabs::fit(&natural, app.tab as usize, ui.available_width() - 130.0, 52.0, 26.0);
                let a = accent(app);
                for (i, w) in &fit.shown {
                    let active = app.tab as usize == *i;
                    let (r, resp) = ui.allocate_exact_size(vec2(*w, TABS_H), Sense::click());
                    if active {
                        ui.painter().rect_filled(r, 0.0, t.tab_active);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r.shrink2(vec2(0.0, 4.0)), 5.0, t.hover);
                    }
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        names[*i],
                        FontId::proportional(12.5),
                        if active { t.text } else { t.text_secondary },
                    );
                    if active {
                        let u = Rect::from_center_size(pos2(r.center().x, r.max.y - 2.0), vec2(w - 16.0, 3.0));
                        ui.painter().rect_filled(u, 2.0, a);
                    }
                    if resp.clicked() {
                        app.tab = match *i {
                            1 => crate::ui::RibbonTab::Insert,
                            2 => crate::ui::RibbonTab::View,
                            _ => crate::ui::RibbonTab::Home,
                        };
                    }
                }
                if !fit.overflow.is_empty() {
                    let (r, resp) = ui.allocate_exact_size(vec2(fit.overflow_width, TABS_H), Sense::click());
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "»", FontId::proportional(12.5), t.text_secondary);
                    egui::Popup::menu(&resp).show(|ui| {
                        for i in &fit.overflow {
                            if ui.selectable_label(false, names[*i]).clicked() {
                                app.tab = match *i {
                                    1 => crate::ui::RibbonTab::Insert,
                                    2 => crate::ui::RibbonTab::View,
                                    _ => crate::ui::RibbonTab::Home,
                                };
                                ui.close();
                            }
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Markdown craft · CommonMark subset").small().color(t.text_tertiary));
                });
            });
        });
}

// ── grouped command strip ────────────────────────────────────────────────────

/// The command strip: contents switch by tab, organized in captioned groups.
pub fn command_strip(app: &mut MdApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("md_ribbon")
        .exact_size(RIBBON_H)
        .frame(
            egui::Frame::NONE
                .fill(t.panel)
                .inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 0 })
                .stroke(Stroke::new(1.0, t.border)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        match app.tab {
                            crate::ui::RibbonTab::Home => home(app, ui),
                            crate::ui::RibbonTab::Insert => insert(app, ui),
                            crate::ui::RibbonTab::View => view(app, ui),
                        }
                    });
                });
        });
}

/// A ribbon group: content, a small uppercase caption below, divider right.
fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    let start = ui.cursor().min;
    let inner = ui.vertical(|ui| {
        ui.set_min_width(10.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), CONTENT_H),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                add(ui);
            },
        );
    });
    let r = Rect::from_min_max(start, pos2(inner.response.rect.max.x, start.y + CONTENT_H + LABEL_H));
    ui.painter().text(
        pos2(r.center().x, r.max.y - LABEL_H / 2.0),
        Align2::CENTER_CENTER,
        title.to_uppercase(),
        FontId::proportional(9.5),
        t.text_secondary,
    );
    ui.add_space(5.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment([pos2(x, start.y + 3.0), pos2(x, start.y + CONTENT_H + LABEL_H - 3.0)], Stroke::new(1.0, t.border));
    ui.add_space(6.0);
}

fn stack(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 3.0);
        add(ui);
    });
}

fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
        add(ui);
    });
}

/// A small 26×22 ribbon tile: `field` fill, `hover` on hover, accent + white icon when on.
fn small_btn(ui: &mut Ui, glyph: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    if on {
        ui.painter().rect_filled(r, 5.0, t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    } else {
        ui.painter().rect_filled(r, 5.0, t.field);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(13.0), if on { Color32::WHITE } else { t.text });
    resp.on_hover_text(tip)
}

/// A small 26×22 ribbon tile painting a shared vector icon (no font → no tofu).
fn small_btn_icon(ui: &mut Ui, icon: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    ui.painter().rect_filled(r, 5.0, fill);
    doccraft_theme::icons::paint(ui.painter(), r, icon, if on { Color32::WHITE } else { t.text }, fill);
    resp.on_hover_text(tip)
}

/// A labelled small button (icon-ish glyph + caption).
fn small_text_btn(ui: &mut Ui, glyph: &str, label: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let w = 26.0 + ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(11.5), t.text)).size().x + 4.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    if on {
        ui.painter().rect_filled(r, 5.0, t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    } else {
        ui.painter().rect_filled(r, 5.0, t.field);
    }
    let ink = if on { Color32::WHITE } else { t.text };
    let (g, tip2) = (glyph.to_string(), tip.to_string());
    ui.painter().text(pos2(r.min.x + 13.0, r.center().y), Align2::CENTER_CENTER, g, FontId::proportional(12.0), ink);
    ui.painter().text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(11.5), ink);
    resp.on_hover_text(tip2)
}

/// A labelled small button: vector icon + caption (no font → no tofu).
fn small_text_btn_icon(ui: &mut Ui, icon: &str, label: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let w = 26.0 + ui.ctx().fonts_mut(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(11.5), t.text)).size().x + 4.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    ui.painter().rect_filled(r, 5.0, fill);
    let ink = if on { Color32::WHITE } else { t.text };
    doccraft_theme::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(18.0, 18.0)), icon, ink, fill);
    ui.painter().text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(11.5), ink);
    resp.on_hover_text(tip)
}

/// A large ribbon button: glyph over a (possibly two-line) label.
fn big_btn(ui: &mut Ui, glyph: &str, label: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let lines: Vec<&str> = label.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), FontId::proportional(10.5), t.text)).size().x)
        .fold(0.0_f32, f32::max)
        + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w.max(44.0), CONTENT_H), Sense::click());
    if on {
        ui.painter().rect_filled(r, 5.0, t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    } else {
        ui.painter().rect_filled(r, 5.0, t.field);
    }
    let ink = if on { Color32::WHITE } else { t.text };
    ui.painter().text(pos2(r.center().x, r.min.y + 17.0), Align2::CENTER_CENTER, glyph, FontId::proportional(16.0), ink);
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(
            pos2(r.center().x, r.min.y + 32.0 + i as f32 * 12.0),
            Align2::CENTER_CENTER,
            *l,
            FontId::proportional(10.5),
            if on { Color32::WHITE } else { t.text_secondary },
        );
    }
    resp.on_hover_text(tip)
}

/// A large ribbon button: vector icon over a (possibly two-line) label
/// (no font → no tofu).
fn big_btn_icon(ui: &mut Ui, icon: &str, label: &str, tip: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let lines: Vec<&str> = label.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), FontId::proportional(10.5), t.text)).size().x)
        .fold(0.0_f32, f32::max)
        + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w.max(44.0), CONTENT_H), Sense::click());
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    ui.painter().rect_filled(r, 5.0, fill);
    doccraft_theme::icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.center().x, r.min.y + 17.0), vec2(22.0, 22.0)),
        icon,
        if on { Color32::WHITE } else { t.text },
        fill,
    );
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(
            pos2(r.center().x, r.min.y + 32.0 + i as f32 * 12.0),
            Align2::CENTER_CENTER,
            *l,
            FontId::proportional(10.5),
            if on { Color32::WHITE } else { t.text_secondary },
        );
    }
    resp.on_hover_text(tip)
}

fn preview_pos_row(app: &mut MdApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    row(ui, |ui| {
        ui.label(egui::RichText::new("Preview").small().color(t.text_secondary));
        for (pos, label) in [
            (crate::settings::PreviewPos::Off, "Off"),
            (crate::settings::PreviewPos::Right, "Right"),
            (crate::settings::PreviewPos::Bottom, "Bottom"),
        ] {
            let sel = app.settings.preview_pos == pos;
            let r = ui.add(egui::Button::selectable(sel, egui::RichText::new(label).size(11.0)).corner_radius(CornerRadius::same(5)));
            if r.clicked() {
                app.settings.preview_pos = pos;
                app.note(format!("Preview {label}"));
            }
        }
    });
}

// ── Home ─────────────────────────────────────────────────────────────────────

fn home(app: &mut MdApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    group(ui, "Clipboard", |ui| {
        if big_btn_icon(ui, "clipboard", "Paste", "Paste from the craft clipboard", false).clicked() {
            app.paste(&ctx);
        }
        stack(ui, |ui| {
            if small_btn_icon(ui, "duplicate", "Copy selection", false).clicked() {
                app.copy(&ctx);
            }
            if small_btn_icon(ui, "cut", "Cut selection", false).clicked() {
                app.cut(&ctx);
            }
        });
    });
    group(ui, "Font", |ui| {
        stack(ui, |ui| {
            row(ui, |ui| {
                if small_btn(ui, "B", "Bold (**…**)", app.wrapped_with(&ctx, "**")).clicked() {
                    app.wrap(&ctx, "**");
                }
                if small_btn(ui, "I", "Italic (*…*)", app.wrapped_with(&ctx, "*")).clicked() {
                    app.wrap(&ctx, "*");
                }
                if small_btn(ui, "S", "Strikethrough (~~…~~)", app.wrapped_with(&ctx, "~~")).clicked() {
                    app.wrap(&ctx, "~~");
                }
                if small_btn(ui, "`", "Inline code (`…`)", app.wrapped_with(&ctx, "`")).clicked() {
                    app.wrap(&ctx, "`");
                }
            });
            row(ui, |ui| {
                if small_text_btn_icon(ui, "quote", "Quote", "Blockquote (> …)", false).clicked() {
                    app.quote(&ctx);
                }
                if small_text_btn_icon(ui, "code", "Code block", "Fenced code block", false).clicked() {
                    app.code_block(&ctx);
                }
            });
        });
    });
    group(ui, "Styles", |ui| {
        for (lvl, glyph, label) in [(1u8, "H1", "Heading 1"), (2, "H2", "Heading 2"), (3, "H3", "Heading 3")] {
            if big_btn(ui, glyph, &label.replace("Heading ", "Heading\n"), label, app.heading_is(&ctx, lvl)).clicked() {
                app.heading(&ctx, lvl);
            }
        }
    });
    group(ui, "Paragraph", |ui| {
        stack(ui, |ui| {
            row(ui, |ui| {
                if small_btn_icon(ui, "list", "Bulleted list", false).clicked() {
                    app.list(&ctx);
                }
                if small_btn_icon(ui, "list-ol", "Numbered list", false).clicked() {
                    app.numbered(&ctx);
                }
                if small_btn_icon(ui, "checkbox", "Task list", false).clicked() {
                    app.task(&ctx);
                }
            });
            row(ui, |ui| {
                if small_btn_icon(ui, "indent", "Indent", false).clicked() {
                    app.indent(&ctx, true);
                }
                if small_btn_icon(ui, "outdent", "Outdent", false).clicked() {
                    app.indent(&ctx, false);
                }
            });
        });
    });
    group(ui, "Editing", |ui| {
        if big_btn_icon(ui, "search", "Find &\nReplace", "Find and replace text", app.show_find).clicked() {
            app.show_find = !app.show_find;
        }
    });
}

// ── Insert ───────────────────────────────────────────────────────────────────

fn insert(app: &mut MdApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    group(ui, "Links", |ui| {
        if big_btn_icon(ui, "link", "Link", "Insert a link", false).clicked() {
            app.insert_snippet(&ctx, "[label](https://)");
        }
        if big_btn_icon(ui, "image", "Image", "Insert an image reference", false).clicked() {
            app.insert_snippet(&ctx, "![alt](https://)");
        }
    });
    group(ui, "Elements", |ui| {
        if big_btn_icon(ui, "table", "Table", "Insert a 3×3 table", false).clicked() {
            app.insert_snippet(&ctx, "\n| Column A | Column B | Column C |\n| --- | --- | --- |\n| value | value | value |\n| value | value | value |\n");
        }
        stack(ui, |ui| {
            row(ui, |ui| {
                if small_btn_icon(ui, "code", "Code block", false).clicked() {
                    app.code_block(&ctx);
                }
                if small_btn_icon(ui, "hr", "Horizontal rule", false).clicked() {
                    app.insert_snippet(&ctx, "\n---\n");
                }
            });
            row(ui, |ui| {
                if small_text_btn(ui, "¹", "Footnote", "Insert a footnote reference + definition", false).clicked() {
                    app.footnote(&ctx);
                }
            });
        });
    });
    group(ui, "Time", |ui| {
        stack(ui, |ui| {
            row(ui, |ui| {
                if small_text_btn_icon(ui, "calendar", "Date", "Insert today's date", false).clicked() {
                    let (d, _) = app.now_datetime();
                    app.insert_snippet(&ctx, &d);
                }
            });
            row(ui, |ui| {
                if small_text_btn_icon(ui, "clock", "Time", "Insert the current time", false).clicked() {
                    let (_, t) = app.now_datetime();
                    app.insert_snippet(&ctx, &t);
                }
            });
        });
    });
}

// ── View ─────────────────────────────────────────────────────────────────────

fn view(app: &mut MdApp, ui: &mut Ui) {
    group(ui, "Show", |ui| {
        stack(ui, |ui| {
            preview_pos_row(app, ui);
            row(ui, |ui| {
                if small_text_btn(ui, "#", "Line numbers", "Show line numbers in the editor", app.settings.line_numbers).clicked() {
                    app.settings.line_numbers = !app.settings.line_numbers;
                }
                if small_text_btn_icon(ui, "sync", "Sync scroll", "Scroll the preview with the editor", app.settings.sync_scroll).clicked() {
                    app.settings.sync_scroll = !app.settings.sync_scroll;
                }
            });
        });
    });
    group(ui, "Zoom", |ui| {
        let t = Tokens::get(ui.ctx());
        stack(ui, |ui| {
            row(ui, |ui| {
                if small_btn(ui, "−", "Smaller editor text", false).clicked() {
                    app.settings.editor_size = (app.settings.editor_size - 1.0).max(10.0);
                }
                ui.add_sized(
                    vec2(34.0, 22.0),
                    egui::Label::new(egui::RichText::new(format!("{:.0}", app.settings.editor_size)).size(12.0).color(t.text)),
                );
                if small_btn(ui, "+", "Larger editor text", false).clicked() {
                    app.settings.editor_size = (app.settings.editor_size + 1.0).min(24.0);
                }
            });
            ui.label(egui::RichText::new("editor font").size(9.5).color(t.text_tertiary));
        });
    });
    group(ui, "Outline", |ui| {
        if big_btn_icon(ui, "layers", "Outline", "Show the document outline pane", app.outline_open).clicked() {
            app.outline_open = !app.outline_open;
        }
        if big_btn_icon(ui, "gear", "Settings", "Open Markdown settings", app.show_settings).clicked() {
            app.show_settings = !app.show_settings;
        }
    });
}
