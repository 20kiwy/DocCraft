//! The DocCraft host: launcher, craft switcher, and one place that owns every
//! craft instance (the four real storytold apps + the three native crafts).

use egui::{Color32, Context, RichText, Ui, Vec2};

// ── the real crafts (upstream git submodules) ────────────────────────────────
use gridcraft_ui_egui as grid_ui;
use wordcraft_ui_egui as word_ui;
use deckcraft_ui_egui as deck_ui;
use pdfcraft_ui_egui as pdf_ui;
use figcraft_ui_egui as fig_ui;

use crate::crafts::{DrawCraft, MdCraft, TypesetCraft};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Craft {
    Launcher,
    Word,
    Grid,
    Deck,
    Pdf,
    Fig,
    Draw,
    Markdown,
    Typeset,
}

impl Craft {
    pub const ALL: [Craft; 8] = [
        Craft::Word,
        Craft::Grid,
        Craft::Deck,
        Craft::Pdf,
        Craft::Fig,
        Craft::Draw,
        Craft::Markdown,
        Craft::Typeset,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Craft::Launcher => "Launcher",
            Craft::Word => "Writer — WordCraft",
            Craft::Grid => "Sheets — GridCraft",
            Craft::Deck => "Slides — DeckCraft",
            Craft::Pdf => "Reader — PdfCraft",
            Craft::Fig => "Design — FigCraft",
            Craft::Draw => "Draw — drawio",
            Craft::Markdown => "Markdown",
            Craft::Typeset => "Typeset",
        }
    }

    pub fn repo(self) -> &'static str {
        match self {
            Craft::Word => "storytold/wordcraft",
            Craft::Grid => "storytold/gridcraft",
            Craft::Deck => "storytold/deckcraft",
            Craft::Pdf => "storytold/pdfcraft",
            Craft::Fig => "storytold/figcraft",
            _ => "doccraft-suite (this repo)",
        }
    }

    /// The honest readiness numbers from each upstream ROADMAP (checked today).
    pub fn status(self) -> &'static str {
        match self {
            Craft::Word => "v0.8.0 · alpha · ribbon catalog 97% · ready ~60%",
            Craft::Grid => "v0.8.0 · alpha · ribbon 94.5% · functions 98.1% · breadth ~75%",
            Craft::Deck => "v0.8.0 · alpha · breadth 84% · ready ~55%",
            Craft::Pdf => "v0.9.0 · alpha · ready ~50%",
            Craft::Fig => "v0.7.0 · alpha · menu 100% · breadth ~85% · ready ~53%",
            Craft::Draw => "3.1.0 · native · mxGraph XML round-trip",
            Craft::Markdown => "3.1.0 · native · CommonMark subset",
            Craft::Typeset => "3.1.0 · native · page layout engine + PDF",
            Craft::Launcher => "",
        }
    }

    pub fn accent(self) -> Color32 {
        match self {
            Craft::Word => Color32::from_rgb(0x2b, 0x57, 0x9a),
            Craft::Grid => Color32::from_rgb(0x1e, 0x71, 0x45),
            Craft::Deck => Color32::from_rgb(0xc4, 0x3e, 0x1c),
            Craft::Pdf => Color32::from_rgb(0x8a, 0x8a, 0x96),
            Craft::Fig => Color32::from_rgb(0xa2, 0x59, 0xff),
            Craft::Draw => Color32::from_rgb(0xf0, 0x87, 0x2d),
            Craft::Markdown => Color32::from_rgb(0x4d, 0x6d, 0xfe),
            Craft::Typeset => Color32::from_rgb(0x7a, 0x4f, 0xa8),
            Craft::Launcher => Color32::from_rgb(0x40, 0x40, 0x48),
        }
    }
}

/// Which real/natural craft is mounted, lazily.
pub struct HostApp {
    pub active: Craft,
    pub word: Option<word_ui::WordApp>,
    pub grid: Option<grid_ui::SheetApp>,
    pub deck: Option<deck_ui::SlideApp>,
    pub pdf: Option<pdf_ui::PdfCraftApp>,
    pub fig: Option<fig_ui::FigApp>,
    pub draw: DrawCraft,
    pub md: MdCraft,
    pub typeset: TypesetCraft,
    pub toast: Option<(String, f64)>,
    /// queued file open, flushed on the next frame with the real context
    pub pending: Option<(Craft, String, Vec<u8>, String)>,
}

impl HostApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut me = Self {
            active: Craft::Launcher,
            word: None,
            grid: None,
            deck: None,
            pdf: None,
            fig: None,
            draw: DrawCraft::new(),
            md: MdCraft::new(),
            typeset: TypesetCraft::new(),
            toast: None,
            pending: None,
        };
        // Open paths passed on the command line: `doccraft report.docx plan.drawio`
        for arg in std::env::args().skip(1) {
            me.open_path(&arg);
        }
        let _ = cc;
        me
    }

    pub fn ensure(&mut self, craft: Craft, ctx: &Context) {
        match craft {
            Craft::Word => {
                if self.word.is_none() {
                    let doc = wordcraft_engine::sample::sample_document();
                    let session = wordcraft_engine::Session::new(doc);
                    self.word = Some(word_ui::WordApp::new(session, crate::services::word_services()));
                    if let Some(w) = self.word.as_mut() {
                        w.watch_desktop_theme(ctx);
                    }
                }
            }
            Craft::Grid => {
                if self.grid.is_none() {
                    grid_ui::SheetApp::setup_context_for_language(ctx, true, grid_ui::i18n::Language::default());
                    self.grid = Some(grid_ui::SheetApp::new(gridcraft_engine::Session::new(), crate::services::grid_services()));
                }
            }
            Craft::Deck => {
                if self.deck.is_none() {
                    self.deck = Some(deck_ui::SlideApp::new(deckcraft_engine::Session::new(), crate::services::deck_services()));
                }
            }
            Craft::Pdf => {
                if self.pdf.is_none() {
                    self.pdf = Some(pdf_ui::PdfCraftApp::new());
                }
            }
            Craft::Fig => {
                if self.fig.is_none() {
                    let session = figcraft_engine::Session::empty();
                    self.fig = Some(fig_ui::FigApp::new(session, Box::new(crate::services::FigServices)));
                }
            }
            _ => {}
        }
        self.rebind_missing_families(ctx);
    }

    /// Route a file to the craft that owns its extension, mount that craft and
    /// hand the file over through the craft's own command API.
    pub fn open_path(&mut self, path: &str) {
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.toast = Some((format!("Open failed: {e}"), now()));
                return;
            }
        };
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
        let (craft, _note): (Craft, String) = match ext.as_str() {
            "docx" | "doc" | "odt" | "rtf" => (Craft::Word, String::new()),
            "xlsx" | "xlsm" | "xls" | "ods" | "csv" | "tsv" => (Craft::Grid, String::new()),
            "pptx" | "potx" | "ppsx" => (Craft::Deck, String::new()),
            "pdf" => (Craft::Pdf, String::new()),
            "fig" | "figcraft" | "svg" => (Craft::Fig, String::new()),
            "drawio" | "xml" => (Craft::Draw, String::new()),
            "md" | "markdown" => (Craft::Markdown, String::new()),
            "typ" | "tex" | "ts" => (Craft::Typeset, String::new()),
            _ => {
                self.toast = Some((format!("No craft claims .{ext}"), now()));
                return;
            }
        };
        self.pending = Some((craft, name, bytes, path.to_string()));
        self.active = craft;
    }

    /// Mount the craft with the real context, then hand the file over through
    /// the craft's own command API.
    fn flush_pending(&mut self, ctx: &Context) {
        let Some((craft, name, bytes, path)) = self.pending.take() else { return };
        self.ensure(craft, ctx);
        match craft {
            Craft::Word => {
                if let Some(w) = self.word.as_mut() {
                    let _ = w.run("file.open", serde_json::json!({ "path": path }));
                }
            }
            Craft::Grid => {
                if let Some(g) = self.grid.as_mut() {
                    let _ = g.run("file.open", serde_json::json!({ "path": path }));
                }
            }
            Craft::Deck => {
                if let Some(d) = self.deck.as_mut() {
                    let _ = d.session.execute("file.open", &serde_json::json!({ "path": path }));
                }
            }
            Craft::Pdf => {
                if let Some(p) = self.pdf.as_mut() {
                    p.open_bytes(&name, Some(path.clone()), bytes);
                }
            }
            Craft::Fig => {
                if let Some(f) = self.fig.as_mut() {
                    f.run_user("file.open", serde_json::json!({ "path": path }));
                }
            }
            Craft::Draw => self.draw.open(&name, &bytes),
            Craft::Markdown => self.md.open(&name, &bytes),
            Craft::Typeset => self.typeset.open(&name, &bytes),
            Craft::Launcher => {}
        }
        self.toast = Some((format!("Opened {name}"), now()));
    }

    fn switch(&mut self, craft: Craft, ctx: &Context) {
        self.ensure(craft, ctx);
        self.active = craft;
        self.rebind_missing_families(ctx);
    }

    /// Every upstream craft binds its own named font families (`mono`, `ui`,
    /// `bold`, `medium`, …) with `set_fonts`, which replaces the whole family
    /// map. When the next craft (or the host) then asks for a family the last
    /// one unbound, epaint panics. Guarantee every family name used anywhere
    /// in the suite stays bound to a sane face — additively, never clobbering
    /// what the active craft installed.
    fn rebind_missing_families(&mut self, ctx: &Context) {
        const NAMED: &[(&str, bool)] = &[
            // (family name, is monospace)
            ("mono", true),
            ("serif", false),
            ("ui", false),
            ("ui-bold", true),
            ("cell", false),
            ("cell-bold", true),
            ("cell-italic", false),
            ("cell-bold-italic", true),
            ("bold", true),
            ("medium", false),
            ("semibold", false),
            ("system-fallback", false),
            ("system-devanagari-fallback", false),
            ("system-telugu-fallback", false),
            ("system-sinhala-fallback", false),
        ];
        let mut defs = ctx.fonts(|f| f.definitions().clone());
        let mut changed = false;
        for (name, mono) in NAMED {
            let family = egui::FontFamily::Name((*name).into());
            let missing = match defs.families.get(&family) {
                Some(v) => v.is_empty(),
                None => true,
            };
            if missing {
                let faces: Vec<String> = if *mono {
                    vec!["Hack".into(), "Ubuntu-Light".into()]
                } else {
                    vec!["Ubuntu-Light".into(), "Hack".into()]
                };
                defs.families.insert(family, faces);
                changed = true;
            }
        }
        if changed {
            ctx.set_fonts(defs);
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl eframe::App for HostApp {
    fn logic(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.flush_pending(ctx);
        match self.active {
            Craft::Word => {
                if let Some(w) = self.word.as_mut() {
                    w.logic(ctx);
                }
            }
            Craft::Grid => {
                if let Some(g) = self.grid.as_mut() {
                    g.logic(ctx);
                }
            }
            Craft::Deck => {
                if let Some(d) = self.deck.as_mut() {
                    d.logic(ctx);
                }
            }
            Craft::Pdf => {
                if let Some(p) = self.pdf.as_mut() {
                    eframe::App::logic(p, ctx, frame);
                }
            }
            Craft::Fig => {
                if let Some(f) = self.fig.as_mut() {
                    f.logic(ctx);
                }
            }
            Craft::Launcher | Craft::Draw | Craft::Markdown | Craft::Typeset => {}
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The DocCraft hub strip stays at the bottom; crafts keep their own
        // full chrome (title bar, ribbon, panels, status bar) untouched above.
        egui::Panel::bottom("doccraft_hub")
            .frame(egui::Frame::default().fill(Color32::from_rgb(0x18, 0x18, 0x1c)).inner_margin(egui::Margin::symmetric(8, 3)))
            .show(ui, |ui| {
                // Fixed-size hub buttons: craft styles must not move the strip.
                ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = Vec2::new(6.0, 0.0);
                ui.add_sized([70.0, 22.0], egui::Label::new(RichText::new("DocCraft").strong().font(egui::FontId::proportional(13.0)).color(Color32::from_rgb(0xe8, 0xe8, 0xec))));
                ui.separator();
                let btn = |ui: &mut Ui, on: bool, label: &str| -> bool {
                    let mut clicked = false;
                    let text = RichText::new(label).font(egui::FontId::proportional(12.5)).color(Color32::from_rgb(0xd0, 0xd0, 0xd6));
                    let r = ui.add_sized([64.0, 22.0], egui::Button::selectable(on, text));
                    if r.clicked() {
                        clicked = true;
                    }
                    clicked
                };
                if btn(ui, self.active == Craft::Launcher, "⌂ Home") {
                    self.active = Craft::Launcher;
                }
                for c in Craft::ALL {
                    let label = match c {
                        Craft::Word => "Writer",
                        Craft::Grid => "Sheets",
                        Craft::Deck => "Slides",
                        Craft::Pdf => "Reader",
                        Craft::Fig => "Design",
                        Craft::Draw => "Draw",
                        Craft::Markdown => "Markdown",
                        Craft::Typeset => "Typeset",
                        Craft::Launcher => unreachable!(),
                    };
                    if btn(ui, self.active == c, label) {
                        self.switch(c, &ctx);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("one suite · real crafts · storytold upstream").weak().font(egui::FontId::proportional(11.0)).color(Color32::from_rgb(0x70, 0x70, 0x78)));
                });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::from_rgb(0x1e, 0x1e, 0x22)).inner_margin(0))
            .show(ui, |ui| match self.active {
                Craft::Launcher => self.launcher(ui),
                Craft::Word => {
                    if let Some(w) = self.word.as_mut() {
                        w.ui(ui);
                    }
                }
                Craft::Grid => {
                    if let Some(g) = self.grid.as_mut() {
                        g.ui(ui);
                    }
                }
                Craft::Deck => {
                    if let Some(d) = self.deck.as_mut() {
                        d.ui(ui);
                    }
                }
                Craft::Pdf => {
                    if let Some(p) = self.pdf.as_mut() {
                        eframe::App::ui(p, ui, frame);
                    }
                }
                Craft::Fig => {
                    if let Some(f) = self.fig.as_mut() {
                        f.ui(ui);
                    }
                }
                Craft::Draw => self.draw.ui(ui),
                Craft::Markdown => self.md.ui(ui),
                Craft::Typeset => self.typeset.ui(ui),
            });

        // toast
        if let Some((msg, t)) = self.toast.clone()
            && now() - t < 3.0
        {
            egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, 36.0]).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(Color32::from_rgb(0x2c, 0x2c, 0x33)).show(ui, |ui| {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(0xf0, 0xf0, 0xf4)));
                });
            });
        } else if self.toast.is_some() {
            self.toast = None;
        }
    }
}

impl HostApp {
    /// The launcher: one card per craft, with the honest upstream status.
    fn launcher(&mut self, ui: &mut Ui) {
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("DocCraft 3.1").size(30.0).strong().color(Color32::from_rgb(0xf2, 0xf2, 0xf6)));
            ui.label(
                RichText::new("One office suite hosting the real storytold crafts — linked from upstream git submodules, updatable per repo.")
                    .color(Color32::from_rgb(0x9a, 0x9a, 0xa4)),
            );
        });
        ui.add_space(14.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                let width = ui.available_width().min(1180.0);
                let card_w = (width - 3.0 * 14.0) / 4.0;
                let card_h = 148.0;
                egui::Grid::new("crafts")
                    .num_columns(4)
                    .spacing([14.0, 14.0])
                    .show(ui, |ui| {
                        for row in Craft::ALL.chunks(4) {
                            for c in row {
                                self.card(ui, *c, card_w, card_h);
                            }
                            ui.end_row();
                        }
                    });
            });
        });
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Update the upstream crafts any time:  scripts/update-crafts.sh  (git submodule update --remote per repo)")
                    .small()
                    .color(Color32::from_rgb(0x6c, 0x6c, 0x76)),
            );
        });
    }

    fn card(&mut self, ui: &mut Ui, c: Craft, w: f32, h: f32) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
        let hovered = resp.hovered();
        let base = c.accent();
        let fill = if hovered { base.gamma_multiply(0.35) } else { Color32::from_rgb(0x26, 0x26, 0x2c) };
        ui.painter().rect_filled(rect, 8.0, fill);
        ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(if hovered { 2.0 } else { 1.0 }, base), egui::StrokeKind::Outside);
        let (title, sub) = match c {
            Craft::Word => ("Writer", "Word documents · .docx"),
            Craft::Grid => ("Sheets", "Spreadsheets · .xlsx .csv"),
            Craft::Deck => ("Slides", "Presentations · .pptx"),
            Craft::Pdf => ("Reader", "PDF · .pdf"),
            Craft::Fig => ("Design", "Figma-style editor · .fig .svg"),
            Craft::Draw => ("Draw", "Diagrams · .drawio"),
            Craft::Markdown => ("Markdown", "Writing · .md"),
            Craft::Typeset => ("Typeset", "Typesetting · .typ"),
            Craft::Launcher => unreachable!(),
        };
        ui.painter().text(
            egui::pos2(rect.left() + 14.0, rect.top() + 18.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(19.0),
            Color32::from_rgb(0xf4, 0xf4, 0xf8),
        );
        ui.painter().text(
            egui::pos2(rect.left() + 14.0, rect.top() + 46.0),
            egui::Align2::LEFT_TOP,
            sub,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(0xb8, 0xb8, 0xc2),
        );
        ui.painter().text(
            egui::pos2(rect.left() + 14.0, rect.bottom() - 44.0),
            egui::Align2::LEFT_TOP,
            c.status(),
            egui::FontId::proportional(11.0),
            base,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 14.0, rect.bottom() - 20.0),
            egui::Align2::LEFT_TOP,
            c.repo(),
            egui::FontId::proportional(10.5),
            Color32::from_rgb(0x82, 0x82, 0x8c),
        );
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.switch(c, &ctx);
        }
    }
}
