//! The settings dialog (the gear in the Quick Access Toolbar): grouped rows for
//! Typography, Page, Header/Footer and Editor, in the File › Options style.
//! Every setting lives on the app struct, so it survives craft switches.

use crate::layout::{self, NumPos, Paper};
use crate::ui::{TypesetApp, DIM, TEXT};
use egui::{Context, FontId, RichText, Ui, vec2};

pub fn show(app: &mut TypesetApp, ctx: &Context) {
    let mut open = app.settings_open;
    egui::Window::new("Typeset Settings")
        .open(&mut open)
        .collapsible(false)
        .default_width(470.0)
        .default_pos(egui::pos2(320.0, 140.0))
        .show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing.y = 6.0;
            typography(app, ui);
            ui.separator();
            page(app, ui);
            ui.separator();
            header_footer(app, ui);
            ui.separator();
            editor(app, ui);
        });
    app.settings_open = open;
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).font(FontId::proportional(13.5)).strong().color(TEXT));
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(DIM));
}

fn typography(app: &mut TypesetApp, ui: &mut Ui) {
    section(ui, "Typography");
    egui::Grid::new("set_typo").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
        label(ui, "Base font size");
        let mut s = app.settings.style.base_size;
        if ui.add(egui::DragValue::new(&mut s).suffix(" pt").range(8.0..=24.0).speed(0.2).max_decimals(1)).changed() {
            app.set_base(ui.ctx(), s);
        }
        ui.end_row();
        label(ui, "Line spacing");
        ui.horizontal(|ui| {
            for v in [1.0, 1.15, 1.5] {
                if ui.selectable_label((app.settings.style.line_spacing - v).abs() < 0.01, format!("{v}×")).clicked() {
                    app.set_spacing(ui.ctx(), v);
                }
            }
        });
        ui.end_row();
        label(ui, "Preview & PDF font");
        ui.horizontal(|ui| {
            if ui.selectable_label(!app.settings.style.mono_font, "Serif (Helvetica)").clicked() {
                app.set_mono_font(ui.ctx(), false);
            }
            if ui.selectable_label(app.settings.style.mono_font, "Mono (Courier)").clicked() {
                app.set_mono_font(ui.ctx(), true);
            }
        });
        ui.end_row();
    });
}

fn page(app: &mut TypesetApp, ui: &mut Ui) {
    section(ui, "Page");
    egui::Grid::new("set_page").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
        label(ui, "Paper size");
        ui.horizontal(|ui| {
            let cur = app.settings.page.paper;
            egui::ComboBox::from_id_salt("set_paper").selected_text(cur.name()).width(110.0).show_ui(ui, |ui| {
                for (p, n, _, _) in layout::PAPERS {
                    if ui.selectable_label(cur == p, n).clicked() {
                        app.set_paper(ui.ctx(), p);
                    }
                }
                if ui.selectable_label(cur == Paper::Custom, "Custom").clicked() {
                    app.set_paper(ui.ctx(), Paper::Custom);
                }
            });
        });
        ui.end_row();
        if app.settings.page.paper == Paper::Custom {
            label(ui, "Custom size");
            ui.horizontal(|ui| {
                let (mut w, mut h) = (layout::to_mm(app.settings.page.custom_w), layout::to_mm(app.settings.page.custom_h));
                if ui.add(egui::DragValue::new(&mut w).suffix(" mm").range(100.0..=600.0).max_decimals(1)).changed() {
                    app.set_custom_size(ui.ctx(), layout::mm(w), app.settings.page.custom_h);
                }
                if ui.add(egui::DragValue::new(&mut h).suffix(" mm").range(100.0..=600.0).max_decimals(1)).changed() {
                    app.set_custom_size(ui.ctx(), app.settings.page.custom_w, layout::mm(h));
                }
            });
            ui.end_row();
        }
        label(ui, "Orientation");
        ui.horizontal(|ui| {
            if ui.selectable_label(!app.settings.page.landscape, "Portrait").clicked() {
                app.set_landscape(ui.ctx(), false);
            }
            if ui.selectable_label(app.settings.page.landscape, "Landscape").clicked() {
                app.set_landscape(ui.ctx(), true);
            }
        });
        ui.end_row();
        label(ui, "Margins (mm)");
        let st = app.settings.page;
        ui.horizontal(|ui| {
            type Apply = Box<dyn Fn(&mut TypesetApp, &Context, f32)>;
            let rows: [(&str, f32, Apply); 4] = [
                ("T", layout::to_mm(st.margin_top), Box::new(move |a: &mut TypesetApp, c: &Context, v: f32| a.set_margin_top(c, v))),
                ("B", layout::to_mm(st.margin_bottom), Box::new(move |a: &mut TypesetApp, c: &Context, v: f32| a.set_margin_bottom(c, v))),
                ("L", layout::to_mm(st.margin_left), Box::new(move |a: &mut TypesetApp, c: &Context, v: f32| a.set_margin_left(c, v))),
                ("R", layout::to_mm(st.margin_right), Box::new(move |a: &mut TypesetApp, c: &Context, v: f32| a.set_margin_right(c, v))),
            ];
            for (name, mut v, apply) in rows {
                if ui
                    .add(egui::DragValue::new(&mut v).suffix(name).range(12.0..=90.0).max_decimals(1).speed(0.5))
                    .changed()
                {
                    apply(app, ui.ctx(), layout::mm(v));
                }
            }
        });
        ui.end_row();
        label(ui, "Columns");
        ui.horizontal(|ui| {
            if ui.selectable_label(app.settings.page.columns == 1, "1").clicked() {
                app.set_columns(ui.ctx(), 1);
            }
            if ui.selectable_label(app.settings.page.columns == 2, "2").clicked() {
                app.set_columns(ui.ctx(), 2);
            }
            if app.settings.page.columns == 2 {
                let mut g = app.settings.page.gutter;
                if ui.add(egui::DragValue::new(&mut g).suffix(" pt gutter").range(8.0..=96.0).speed(0.5)).changed() {
                    app.set_gutter(ui.ctx(), g);
                }
            }
        });
        ui.end_row();
    });
}

fn header_footer(app: &mut TypesetApp, ui: &mut Ui) {
    section(ui, "Header & Footer");
    let mut numbers = app.settings.hf.numbers;
    if ui.checkbox(&mut numbers, "Show page numbers").changed() {
        app.set_numbers(ui.ctx(), numbers);
    }
    ui.add_enabled_ui(app.settings.hf.numbers, |ui| {
        ui.horizontal(|ui| {
            label(ui, "Position");
            if ui.selectable_label(app.settings.hf.number_pos == NumPos::Center, "Center").clicked() {
                app.set_number_pos(ui.ctx(), NumPos::Center);
            }
            if ui.selectable_label(app.settings.hf.number_pos == NumPos::Outside, "Outside").clicked() {
                app.set_number_pos(ui.ctx(), NumPos::Outside);
            }
        });
    });
    let mut first = app.settings.hf.first_different;
    if ui.checkbox(&mut first, "Different first page (no header or folio)").changed() {
        app.set_first_different(ui.ctx(), first);
    }
    ui.horizontal(|ui| {
        label(ui, "Footer text");
        let mut f = app.settings.hf.footer.clone();
        if ui.add(egui::TextEdit::singleline(&mut f).hint_text("centered under the text block").desired_width(240.0)).changed() {
            app.set_footer(ui.ctx(), f);
        }
    });
}

fn editor(app: &mut TypesetApp, ui: &mut Ui) {
    section(ui, "Editor");
    egui::Grid::new("set_editor").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
        label(ui, "Editor font");
        ui.horizontal(|ui| {
            if ui.selectable_label(!app.editor.mono, "Serif").clicked() {
                app.editor.mono = false;
            }
            if ui.selectable_label(app.editor.mono, "Mono").clicked() {
                app.editor.mono = true;
            }
        });
        ui.end_row();
        label(ui, "Editor font size");
        let mut s = app.editor.size;
        if ui.add(egui::DragValue::new(&mut s).suffix(" pt").range(9.0..=20.0).speed(0.2)).changed() {
            app.editor.size = s;
        }
        ui.end_row();
        label(ui, "Tab size");
        let mut t = app.editor.tab as f32;
        if ui.add(egui::DragValue::new(&mut t).range(2.0..=8.0).speed(0.2)).changed() {
            app.editor.tab = t.round() as usize;
        }
        ui.end_row();
        label(ui, "Wrapping");
        ui.checkbox(&mut app.editor.wrap, "Wrap long lines");
        ui.end_row();
        label(ui, "Line numbers");
        ui.add_enabled_ui(!app.editor.wrap, |ui| {
            ui.checkbox(&mut app.editor.numbers, "Show in the gutter (needs wrapping off)");
        });
        ui.end_row();
    });
}
