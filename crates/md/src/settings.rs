//! Markdown craft settings — the "settings well": a WordCraft options.rs-style
//! window with grouped rows (label + control) covering Editor, Preview,
//! Editor accent, Theme and Session. Settings live on `MdApp` (in memory), so
//! they survive craft switches: the host keeps the `MdApp` alive.

use crate::ui::MdApp;
use doccraft_theme::Tokens;
use egui::StrokeKind;
use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, Vec2};

/// Where the live preview renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewPos {
    Off,
    #[default]
    Right,
    Bottom,
}

/// The craft accent: headings and links in the preview, and the text selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Accent {
    #[default]
    Blue,
    Teal,
    Green,
    Amber,
    Rose,
    Purple,
}

pub const ACCENTS: [(Accent, &str, [u8; 3]); 6] = [
    (Accent::Blue, "DocCraft Blue", [0x0D, 0x99, 0xFF]),
    (Accent::Teal, "Teal", [0x2e, 0x9e, 0x8f]),
    (Accent::Green, "Green", [0x3f, 0x9d, 0x4e]),
    (Accent::Amber, "Amber", [0xd0, 0x8b, 0x2c]),
    (Accent::Rose, "Rose", [0xc9, 0x50, 0x8a]),
    (Accent::Purple, "Purple", [0x8b, 0x5c, 0xf6]),
];

impl Accent {
    pub fn color(self) -> Color32 {
        let [r, g, b] = ACCENTS.iter().find(|(a, _, _)| *a == self).map(|(_, _, c)| *c).unwrap_or([0x0D, 0x99, 0xFF]);
        Color32::from_rgb(r, g, b)
    }
}

/// All Markdown craft settings, kept on the app struct.
#[derive(Debug, Clone)]
pub struct Settings {
    pub editor_size: f32,
    pub tab_size: usize,
    pub line_numbers: bool,
    pub wrap_lines: bool,
    pub auto_close: bool,
    pub preview_pos: PreviewPos,
    pub sync_scroll: bool,
    pub preview_size: f32,
    pub dark_code: bool,
    pub accent: Accent,
    pub autosave: bool,
    pub autosave_secs: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor_size: 14.0,
            tab_size: 4,
            line_numbers: false,
            wrap_lines: true,
            auto_close: true,
            preview_pos: PreviewPos::Right,
            sync_scroll: false,
            preview_size: 14.0,
            dark_code: true,
            accent: Accent::Blue,
            autosave: false,
            autosave_secs: 15.0,
        }
    }
}

fn section(ui: &mut Ui, title: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(2.0);
    ui.label(RichText::new(title).strong().size(14.0).color(t.text));
    ui.add_space(2.0);
}

fn note(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text).small().color(t.text_tertiary));
}

/// A settings group card: `panel` fill, radius 5, hairline border.
fn card(ui: &mut Ui, t: Tokens, add: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(t.panel)
        .corner_radius(5)
        .stroke(Stroke::new(1.0, t.border))
        .inner_margin(egui::Margin::same(10))
        .show(ui, add);
}

fn grid_label(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text).color(t.text_secondary));
}

/// The settings window, in the WordCraft options style: grouped rows with a
/// label on the left and a control on the right, each group on a card.
pub fn settings_window(app: &mut MdApp, ctx: &egui::Context) {
    let t = Tokens::get(ctx);
    let mut open = app.show_settings;
    egui::Window::new(RichText::new("Markdown Settings").strong())
        .open(&mut open)
        .default_width(440.0)
        .default_pos([80.0, 90.0])
        .frame(
            egui::Frame::NONE
                .fill(t.bg)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(13)
                .inner_margin(14),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            card(ui, t, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, "Editor");
                egui::Grid::new("md_set_editor").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                    grid_label(ui, "Font size");
                    ui.add(egui::DragValue::new(&mut app.settings.editor_size).range(10.0..=24.0).speed(0.2).suffix(" px"));
                    ui.end_row();
                    grid_label(ui, "Tab size");
                    ui.horizontal(|ui| {
                        for t2 in [2usize, 4, 8] {
                            if ui
                                .add(
                                    egui::Button::selectable(
                                        app.settings.tab_size == t2,
                                        RichText::new(t2.to_string()).size(11.5),
                                    )
                                    .min_size(Vec2::new(28.0, 20.0))
                                    .corner_radius(CornerRadius::same(5)),
                                )
                                .clicked()
                            {
                                app.settings.tab_size = t2;
                            }
                        }
                    });
                    ui.end_row();
                    grid_label(ui, "Line numbers");
                    ui.checkbox(&mut app.settings.line_numbers, "");
                    ui.end_row();
                    grid_label(ui, "Wrap lines");
                    ui.checkbox(&mut app.settings.wrap_lines, "");
                    ui.end_row();
                    grid_label(ui, "Auto-close markers");
                    ui.checkbox(&mut app.settings.auto_close, "");
                    ui.end_row();
                });
                note(ui, "Auto-close inserts marker pairs with the caret inside when nothing is selected.");
            });

            card(ui, t, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, "Preview");
                egui::Grid::new("md_set_preview").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                    grid_label(ui, "Position");
                    ui.horizontal(|ui| {
                        for (pos, label) in [
                            (PreviewPos::Off, "Off"),
                            (PreviewPos::Right, "Right"),
                            (PreviewPos::Bottom, "Bottom"),
                        ] {
                            if ui
                                .add(
                                    egui::Button::selectable(
                                        app.settings.preview_pos == pos,
                                        RichText::new(label).size(11.5),
                                    )
                                    .min_size(Vec2::new(48.0, 20.0))
                                    .corner_radius(CornerRadius::same(5)),
                                )
                                .clicked()
                            {
                                app.settings.preview_pos = pos;
                            }
                        }
                    });
                    ui.end_row();
                    grid_label(ui, "Sync scroll");
                    ui.checkbox(&mut app.settings.sync_scroll, "");
                    ui.end_row();
                    grid_label(ui, "Preview font size");
                    ui.add(egui::DragValue::new(&mut app.settings.preview_size).range(11.0..=20.0).speed(0.2).suffix(" px"));
                    ui.end_row();
                    grid_label(ui, "Dark code blocks");
                    ui.checkbox(&mut app.settings.dark_code, "");
                    ui.end_row();
                });
            });

            card(ui, t, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, "Editor accent");
                ui.label(
                    RichText::new("Applies to headings and links in the preview, and to the text selection.")
                        .small()
                        .color(t.text_tertiary),
                );
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    for (acc, name, rgb) in ACCENTS {
                        let sel = app.settings.accent == acc;
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(52.0, 24.0), Sense::click());
                        let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        ui.painter().rect_filled(r.shrink(4.0), 4.0, c);
                        if sel {
                            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(2.0, t.text), StrokeKind::Outside);
                        } else if resp.hovered() {
                            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, t.text_secondary), StrokeKind::Outside);
                        }
                        if resp.on_hover_text(name).clicked() {
                            app.settings.accent = acc;
                        }
                    }
                });
            });

            card(ui, t, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, "Theme");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut app.dark, false, "Light");
                    ui.radio_value(&mut app.dark, true, "Dark");
                });
                note(ui, "Same toggle as the sun / moon button in the title strip.");
            });

            card(ui, t, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, "Session");
                egui::Grid::new("md_set_session").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                    grid_label(ui, "Autosave");
                    ui.checkbox(&mut app.settings.autosave, "");
                    ui.end_row();
                    grid_label(ui, "Autosave interval");
                    ui.add(
                        egui::DragValue::new(&mut app.settings.autosave_secs)
                            .range(5.0..=120.0)
                            .speed(0.5)
                            .suffix(" s"),
                    );
                    ui.end_row();
                });
                note(ui, "Autosave writes the opened file every interval while you edit. Untitled documents are not written.");
            });

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Settings stay for the session (kept on the craft).").small().color(t.text_secondary));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Reset defaults").size(11.5))
                                .fill(t.field)
                                .corner_radius(CornerRadius::same(5)),
                        )
                        .clicked()
                    {
                        app.settings = Settings::default();
                    }
                });
            });
        });
    app.show_settings = open;
}
