//! The DocCraft host: launcher, craft switcher, and one place that owns every
//! craft instance. The chrome copies the storytold craft-launcher design system
//! (ArtCraft Launcher): hard square corners, 1px hairlines, uppercase mono
//! labels, Archivo-style tracked headings, card grid on shared hairlines,
//! 0.3s hover motion — all in this file's `theme` module.

use egui::{
    pos2, vec2, Align2, Color32, Context, CornerRadius, FontId, Frame, Margin, Pos2, Rect, RichText,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2, ViewportCommand,
};

// ── the real crafts (upstream git submodules) ────────────────────────────────
use gridcraft_ui_egui as grid_ui;
use wordcraft_ui_egui as word_ui;
use deckcraft_ui_egui as deck_ui;
use pdfcraft_ui_egui as pdf_ui;
use figcraft_ui_egui as fig_ui;

use crate::crafts::{DrawCraft, MdCraft, NotesCraft, TypesetCraft};

// ── ArtCraft design tokens (from craft-launcher crates/ui-egui/src/theme.rs) ─
pub mod theme {
    use egui::Color32;
    pub const fn hex(c: u32) -> Color32 {
        Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
    }
    /// Dark theme tokens — getartcraft.com globals.css
    pub const BG: Color32 = hex(0x121316);
    pub const SUNKEN: Color32 = hex(0x060607);
    pub const INK: Color32 = hex(0xf2f1ee);
    pub const INK_STRONG: Color32 = hex(0xffffff);
    pub const fn unmul(r: u8, g: u8, b: u8, a: u8) -> Color32 {
        let pr = ((r as u32 * a as u32 + 127) / 255) as u8;
        let pg = ((g as u32 * a as u32 + 127) / 255) as u8;
        let pb = ((b as u32 * a as u32 + 127) / 255) as u8;
        Color32::from_rgba_premultiplied(pr, pg, pb, a)
    }
    pub const MUTED: Color32 = unmul(0xf2, 0xf1, 0xee, 153);
    pub const FAINT: Color32 = unmul(0xf2, 0xf1, 0xee, 97);
    pub const LINE: Color32 = unmul(0xf2, 0xf1, 0xee, 38);
    pub const LINE_STRONG: Color32 = unmul(0xf2, 0xf1, 0xee, 102);
    pub const ACCENT: Color32 = hex(0x2d81ff);
    pub const ACCENT_INK: Color32 = hex(0x74aaff);
    pub const INVERT_BG: Color32 = hex(0xffffff);
    pub const INVERT_FG: Color32 = hex(0x0b0b0c);
    pub const GOOD: Color32 = hex(0x3fd6b8);
    pub const WARN: Color32 = hex(0xffc14d);
    pub const BAD: Color32 = hex(0xff6259);
    pub const CLOSE_RED: Color32 = hex(0xc42b1c);
    pub const HEADER_H: f32 = 48.0;
    pub const FOOTER_H: f32 = 34.0;
    pub const CONTROL_H: f32 = 34.0;
    pub const CARD_MIN_W: f32 = 262.0;
    pub const CARD_H: f32 = 260.0;
    pub const CARD_PAD: f32 = 20.0;
    pub const WEBSITE_URL: &str = "https://github.com/storytold";
    pub const ORG_URL: &str = "https://github.com/storytold";
}

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
    Notes,
}

impl Craft {
    pub const ALL: [Craft; 9] = [
        Craft::Word,
        Craft::Grid,
        Craft::Deck,
        Craft::Pdf,
        Craft::Fig,
        Craft::Draw,
        Craft::Markdown,
        Craft::Typeset,
        Craft::Notes,
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
            Craft::Notes => "Notes",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Craft::Launcher => "Home",
            Craft::Word => "Writer",
            Craft::Grid => "Sheets",
            Craft::Deck => "Slides",
            Craft::Pdf => "Reader",
            Craft::Fig => "Design",
            Craft::Draw => "Draw",
            Craft::Markdown => "Markdown",
            Craft::Typeset => "Typeset",
            Craft::Notes => "Notes",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Craft::Word => "v0.8.0",
            Craft::Grid => "v0.8.0",
            Craft::Deck => "v0.8.0",
            Craft::Pdf => "v0.9.0",
            Craft::Fig => "v0.7.0",
            Craft::Draw => "3.2.0",
            Craft::Markdown => "3.1.0",
            Craft::Typeset => "3.1.0",
            Craft::Notes => "3.2.0",
            Craft::Launcher => "3.2.0",
        }
    }

    pub fn repo(self) -> &'static str {
        match self {
            Craft::Word => "storytold/wordcraft",
            Craft::Grid => "storytold/gridcraft",
            Craft::Deck => "storytold/deckcraft",
            Craft::Pdf => "storytold/pdfcraft",
            Craft::Fig => "storytold/figcraft",
            _ => "doccraft-suite (native)",
        }
    }

    /// The honest readiness numbers from each upstream ROADMAP (checked today).
    pub fn status(self) -> &'static str {
        match self {
            Craft::Word => "alpha · ribbon catalog 97% · ready ~60%",
            Craft::Grid => "alpha · ribbon 94.5% · functions 98.1% · ~75%",
            Craft::Deck => "alpha · breadth 84% · ready ~55%",
            Craft::Pdf => "alpha · ready ~50%",
            Craft::Fig => "alpha · menu 100% · breadth ~85% · ready ~53%",
            Craft::Draw => "native · drawio connections + full export",
            Craft::Markdown => "native · CommonMark subset + settings",
            Craft::Typeset => "native · page layout engine + PDF",
            Craft::Notes => "native · notebook + preview",
            Craft::Launcher => "",
        }
    }

    /// Per-app brand colors (from craft-launcher core catalog where they exist).
    pub fn accent(self) -> Color32 {
        match self {
            Craft::Word => theme::hex(0x3F5FE0),   // WordCraft
            Craft::Grid => theme::hex(0x1F9D55),   // GridCraft
            Craft::Deck => theme::hex(0xF07A1E),   // DeckCraft
            Craft::Pdf => theme::hex(0x12A58A),    // PdfCraft
            Craft::Fig => theme::hex(0x02DA96),    // FigCraft
            Craft::Draw => theme::hex(0xF08724),   // drawio orange
            Craft::Markdown => theme::hex(0x4D6DFE),
            Craft::Typeset => theme::hex(0x7A4FA8),
            Craft::Notes => theme::hex(0xE8A13D),
            Craft::Launcher => theme::ACCENT,
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            Craft::Word => "Word documents · .docx",
            Craft::Grid => "Spreadsheets · .xlsx .csv",
            Craft::Deck => "Presentations · .pptx",
            Craft::Pdf => "PDF · .pdf",
            Craft::Fig => "Figma-style · .fig .svg",
            Craft::Draw => "Diagrams · .drawio",
            Craft::Markdown => "Writing · .md",
            Craft::Typeset => "Typesetting · .typ .tex",
            Craft::Notes => "Notebook · notes",
            Craft::Launcher => "",
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
    pub notes: NotesCraft,
    pub about_open: bool,
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
            notes: NotesCraft::new(),
            about_open: false,
            toast: None,
            pending: None,
        };
        // Open paths passed on the command line: `doccraft report.docx plan.drawio`
        for arg in std::env::args().skip(1) {
            if arg == "--register-associations" {
                crate::register_associations::run();
                std::process::exit(0);
            }
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
            "note" | "notes" => (Craft::Notes, String::new()),
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
            Craft::Notes => self.notes.open(&name, &bytes),
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
    /// map. Guarantee every family name used anywhere in the suite stays bound.
    fn rebind_missing_families(&mut self, ctx: &Context) {
        const NAMED: &[(&str, bool)] = &[
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
            Craft::Launcher | Craft::Draw | Craft::Markdown | Craft::Typeset | Craft::Notes => {}
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ArtCraft shell: custom header (logo + window buttons), hub strip at
        // the bottom; crafts keep their own full chrome untouched between them.
        header(ui, &mut self.about_open, self.active);

        egui::Panel::bottom("doccraft_hub")
            .exact_size(34.0)
            .frame(Frame::NONE.fill(theme::BG).inner_margin(Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, theme::LINE)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing = vec2(4.0, 0.0);
                    mono_label(ui, "DOCRAFT", theme::ACCENT_INK, 11.0);
                    vrule(ui);
                    let b = |ui: &mut Ui, on: bool, label: &str| -> bool {
                        let t = egui::Button::selectable(on, RichText::new(label).font(FontId::monospace(10.5)).color(if on { theme::INK_STRONG } else { theme::MUTED }))
                            .fill(if on { theme::SUNKEN } else { Color32::TRANSPARENT })
                            .corner_radius(CornerRadius::ZERO)
                            .stroke(Stroke::NONE);
                        ui.add_sized([76.0, 24.0], t).clicked()
                    };
                    if b(ui, self.active == Craft::Launcher, "⌂ HOME") {
                        self.active = Craft::Launcher;
                    }
                    for c in Craft::ALL {
                        if b(ui, self.active == c, c.short().to_uppercase().as_str()) {
                            self.switch(c, &ctx);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        mono_label(ui, "ONE SUITE · REAL CRAFTS · UPSTREAM GIT", theme::FAINT, 10.0);
                    });
                });
            });

        if self.active == Craft::Launcher {
            egui::Panel::bottom("launcher_footer")
                .exact_size(theme::FOOTER_H + 1.0)
                .frame(Frame::NONE.fill(theme::BG).inner_margin(Margin::symmetric(24, 0)).stroke(Stroke::new(1.0, theme::LINE)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.style_mut().spacing.item_spacing = vec2(14.0, 0.0);
                        mono_label(ui, &format!("DOCRAFT SUITE v{} · NINE CRAFTS · UPSTREAM EMBEDDED", env!("CARGO_PKG_VERSION")), theme::FAINT, 10.5);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if mono_link(ui, "GITHUB ↗") {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(theme::ORG_URL));
                            }
                            if mono_link(ui, "WEBSITE ↗") {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(theme::WEBSITE_URL));
                            }
                        });
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(if self.active == Craft::Launcher { theme::BG } else { Color32::from_rgb(0x1e, 0x1e, 0x22) }).inner_margin(0))
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
                Craft::Notes => self.notes.ui(ui),
            });

        about_window(&ctx, &mut self.about_open);

        // toast
        if let Some((msg, t)) = self.toast.clone()
            && now() - t < 3.0
        {
            egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, 44.0]).show(&ctx, |ui| {
                Frame::popup(ui.style()).fill(Color32::from_rgb(0x2c, 0x2c, 0x33)).show(ui, |ui| {
                    ui.label(RichText::new(msg).color(theme::INK));
                });
            });
        } else if self.toast.is_some() {
            self.toast = None;
        }
    }
}

// ── header (custom title bar, ArtCraft style) ────────────────────────────────

fn header(ui: &mut Ui, about_open: &mut bool, _active: Craft) {
    egui::Panel::top("doccraft_header")
        .exact_size(theme::HEADER_H)
        .frame(Frame::NONE.fill(theme::BG).inner_margin(Margin::symmetric(0, 0)).stroke(Stroke::new(1.0, theme::LINE_STRONG)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = vec2(0.0, 0.0);
                // logo cell
                let (lrect, lresp) = ui.allocate_exact_size(vec2(150.0, theme::HEADER_H), Sense::hover());
                paint_logo_mark(ui, lrect.min + vec2(14.0, 12.0), 24.0);
                ui.painter().text(
                    lrect.min + vec2(46.0, lrect.height() / 2.0 + 1.0),
                    Align2::LEFT_CENTER,
                    "Doc",
                    FontId::proportional(17.0),
                    theme::INK_STRONG,
                );
                ui.painter().text(
                    lrect.min + vec2(46.0 + 40.0, lrect.height() / 2.0 + 1.0),
                    Align2::LEFT_CENTER,
                    "Craft",
                    FontId::proportional(17.0),
                    theme::ACCENT,
                );
                let _ = lresp;
                // drag region
                let (drect, dresp) = ui.allocate_exact_size(vec2(ui.available_width() - 210.0, theme::HEADER_H), Sense::click_and_drag());
                if dresp.dragged() && dresp.drag_started() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if dresp.double_clicked() {
                    let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
                }
                let _ = drect;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().spacing.item_spacing = vec2(0.0, 0.0);
                    window_buttons(ui);
                    // about gear
                    let (arect, aresp) = ui.allocate_exact_size(vec2(46.0, theme::HEADER_H), Sense::click());
                    if aresp.hovered() {
                        ui.painter().rect_filled(arect, 0.0, theme::SUNKEN);
                    }
                    ui.painter().text(arect.center(), Align2::CENTER_CENTER, "ⓘ", FontId::proportional(15.0), if aresp.hovered() { theme::INK_STRONG } else { theme::MUTED });
                    if aresp.clicked() {
                        *about_open = !*about_open;
                    }
                    let vx = arect.left();
                    ui.painter().line_segment([Pos2::new(vx, arect.top() + 8.0), Pos2::new(vx, arect.bottom() - 8.0)], Stroke::new(1.0, theme::LINE));
                });
            });
        });
}

fn window_buttons(ui: &mut Ui) {
    let h = theme::HEADER_H;
    for (i, (glyph, action)) in [("–", "min"), ("☐", "max"), ("✕", "close")].iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(vec2(46.0, h), Sense::click());
        let close = *action == "close";
        let hovered = resp.hovered();
        if hovered {
            ui.painter().rect_filled(rect, 0.0, if close { theme::CLOSE_RED } else { theme::LINE });
        }
        let col = if close && hovered { theme::INK_STRONG } else { theme::MUTED };
        let c = rect.center();
        let p = ui.painter();
        match *action {
            "min" => {
                p.line_segment([c - vec2(4.5, 0.0), c + vec2(4.5, 0.0)], Stroke::new(1.2, col));
            }
            "max" => {
                p.rect_stroke(Rect::from_center_size(c, vec2(9.0, 9.0)), CornerRadius::ZERO, Stroke::new(1.2, col), StrokeKind::Middle);
            }
            _ => {
                p.line_segment([c - vec2(3.5, 3.5), c + vec2(3.5, 3.5)], Stroke::new(1.2, col));
                p.line_segment([c - vec2(3.5, 3.5) * vec2(1.0, -1.0), c + vec2(3.5, 3.5) * vec2(-1.0, 1.0)], Stroke::new(1.2, col));
            }
        }
        let _ = glyph;
        if resp.clicked() {
            match *action {
                "min" => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                "max" => {
                    let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
                }
                _ => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
            }
        }
        if i < 2 {
            // keep cells flush; spacing handled by allocate
        }
    }
}

fn paint_logo_mark(ui: &mut Ui, at: Pos2, size: f32) {
    let p = ui.painter();
    let r = Rect::from_min_size(at, vec2(size, size));
    p.rect_filled(r, 6.0, theme::ACCENT);
    // white page lines inside the mark
    for (i, w) in [0.62f32, 0.44, 0.62, 0.30].iter().enumerate() {
        let y = r.top() + size * (0.30 + 0.15 * i as f32);
        let x0 = r.left() + size * 0.22;
        let x1 = x0 + size * w;
        p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.6, theme::INVERT_BG));
    }
    // mint spark
    p.rect_filled(Rect::from_min_size(r.right_top() - vec2(size * 0.34, -size * 0.06), vec2(size * 0.28, size * 0.28)), 4.0, theme::GOOD);
}

// ── the launcher (craft-launcher card grid) ──────────────────────────────────

impl HostApp {
    fn launcher(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().id_salt("launcher_scroll").show(ui, |ui| {
            ui.add_space(26.0);
            ui.vertical_centered(|ui| {
                // tracked display heading, ArtCraft hero style
                ui.label(RichText::new("DocCraft Suite").size(32.0).strong().color(theme::INK_STRONG).extra_letter_spacing(-0.6));
                ui.add_space(2.0);
                ui.label(RichText::new("Ten crafts in one binary — five real storytold apps embedded from git submodules, five DocCraft-native.").size(13.5).color(theme::MUTED));
                ui.add_space(2.0);
                mono_label(ui, &format!("v{} · ALL CRAFTS UPDATABLE PER REPO · scripts/update-crafts.sh", env!("CARGO_PKG_VERSION")), theme::FAINT, 10.5);
            });
            ui.add_space(18.0);

            let width = ui.available_width().min(1240.0) - 48.0;
            let cols = ((width / theme::CARD_MIN_W).floor() as usize).max(1).min(4);
            let card_w = (width + (cols as f32 - 1.0)) / cols as f32;
            let card_h = theme::CARD_H;
            let mut grid_rects: Vec<(Craft, Rect)> = Vec::new();
            // measure: place cards in a centered block using manual layout
            let total_w = card_w * cols as f32;
            let x0 = (ui.available_width() - total_w) / 2.0;
            for (i, c) in Craft::ALL.iter().enumerate() {
                let (col, row) = (i % cols, i / cols);
                let min = ui.cursor().min + vec2(x0 + col as f32 * (card_w - 1.0), row as f32 * (card_h - 1.0));
                grid_rects.push((*c, Rect::from_min_size(min, vec2(card_w, card_h))));
            }
            let rows = (Craft::ALL.len() + cols - 1) / cols;
            ui.allocate_exact_size(vec2(width, card_h * rows as f32), Sense::hover());

            for (c, rect) in &grid_rects {
                self.card(ui, *c, *rect);
            }
            ui.add_space(16.0);
        });

    }

    fn card(&mut self, ui: &mut Ui, c: Craft, rect: Rect) {
        let card_resp = ui
            .scope_builder(egui::UiBuilder::new().max_rect(rect).sense(Sense::click()), |ui| {
                let hovered = ui.rect_contains_pointer(rect);
                let lift = ui.ctx().animate_bool_with_time(egui::Id::new(("card-hover", c as u8)), hovered, 0.3);
                let p = ui.painter().clone();
                // shared-hairline grid: 1px stroke, neighbours overlap by 1px
                let base_fill = Color32::from_rgb(0x18, 0x18, 0x1c);
                let mut fill = base_fill;
                fill.lerp_to_gamma(c.accent(), 0.07 * lift);
                p.rect_filled(rect, 0.0, fill);
                p.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, theme::LINE), StrokeKind::Middle);
                // 2px app-color rule wiping in from the left along the top
                let rule_w = (rect.width() - 2.0) * lift;
                if rule_w > 1.0 {
                    p.rect_filled(Rect::from_min_size(rect.min + vec2(1.0, 0.0), vec2(rule_w, 2.0)), 0.0, c.accent());
                }
                let inside = rect.shrink(theme::CARD_PAD);
                // top strip: version left, repo link right
                p.text(pos2(inside.left(), inside.top() + 8.0), Align2::LEFT_TOP, c.version(), FontId::monospace(10.5), if lift > 0.3 { c.accent() } else { theme::MUTED });
                p.text(pos2(inside.right(), inside.top() + 8.0), Align2::RIGHT_TOP, "GITHUB ↗", FontId::monospace(10.5), theme::FAINT);
                // icon row: painted glyph rising 4px on hover
                let icon_y = inside.top() + 44.0 - 4.0 * lift;
                paint_craft_glyph(ui, c, pos2(inside.left() + 4.0, icon_y), 44.0, lift);
                // wordmark + tagline
                p.text(pos2(inside.left() + 62.0, icon_y + 12.0), Align2::LEFT_TOP, c.short(), FontId::proportional(21.0), theme::INK_STRONG);
                p.text(pos2(inside.left() + 62.0, icon_y + 38.0), Align2::LEFT_TOP, c.tagline(), FontId::proportional(12.0), theme::MUTED);
                // status badge
                let badge = Rect::from_min_size(pos2(inside.left(), inside.top() + 104.0), vec2(inside.width(), 22.0));
                p.rect_filled(badge, 0.0, theme::SUNKEN);
                p.rect_stroke(badge, CornerRadius::ZERO, Stroke::new(1.0, theme::LINE), StrokeKind::Middle);
                p.text(pos2(badge.left() + 8.0, badge.center().y), Align2::LEFT_CENTER, c.status().to_uppercase(), FontId::monospace(9.5), c.accent());
                // blurb
                p.text(pos2(inside.left(), inside.top() + 138.0), Align2::LEFT_TOP, craft_blurb(c), FontId::proportional(12.5), theme::INK);
                // bottom action row
                let brect = Rect::from_min_size(pos2(inside.left(), rect.bottom() - 40.0), vec2(110.0, 28.0));
                let inverted = lift > 0.5;
                p.rect_filled(brect, 0.0, if inverted { theme::INVERT_BG } else { theme::SUNKEN });
                p.rect_stroke(brect, CornerRadius::ZERO, Stroke::new(1.0, if inverted { theme::INVERT_BG } else { theme::LINE_STRONG }), StrokeKind::Middle);
                p.text(brect.center(), Align2::CENTER_CENTER, "OPEN", FontId::monospace(11.0), if inverted { theme::INVERT_FG } else { theme::INK_STRONG });
            })
            ;
        let resp = card_resp.response;
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.switch(c, &ctx);
        }
    }
}

fn craft_blurb(c: Craft) -> &'static str {
    match c {
        Craft::Word => "Real WordCraft ribbon and styles engine; opens and renders .docx.",
        Craft::Grid => "Real GridCraft spreadsheet: 500+ functions, workbook model.",
        Craft::Deck => "Real DeckCraft slide editor with backstage themes.",
        Craft::Pdf => "Real PdfCraft reader built on the patched krilla/hayro stack.",
        Craft::Fig => "Real FigCraft vector design tool: frames, components, assets.",
        Craft::Draw => "drawio-native: green connection dots, arrow ends, XML/SVG/PNG/PDF export.",
        Craft::Markdown => "WordCraft-anatomy editor with live preview and settings.",
        Craft::Typeset => "Justified page layout engine with real PDF export.",
        Craft::Notes => "Multi-page notebook with markdown-lite preview.",
        Craft::Launcher => "",
    }
}

fn paint_craft_glyph(ui: &mut Ui, c: Craft, at: Pos2, size: f32, lift: f32) {
    let p = ui.painter();
    let accent = c.accent();
    let mut base = theme::MUTED;
    base.lerp_to_gamma(accent, lift);
    let r = Rect::from_min_size(at, vec2(size, size));
    let s = size / 44.0;
    let l = |a: Pos2, b: Pos2, w: f32| p.line_segment([at + (a - Pos2::ZERO) * s, at + (b - Pos2::ZERO) * s], Stroke::new(w * s, base));
    match c {
        Craft::Word => {
            // document sheet with lines
            p.rect_stroke(Rect::from_min_size(at + vec2(10.0 * s, 4.0 * s), vec2(24.0 * s, 36.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            for i in 0..4 {
                l(pos2(15.0, 12.0 + 7.0 * i as f32), pos2(29.0, 12.0 + 7.0 * i as f32), 1.4);
            }
        }
        Craft::Grid => {
            // table grid
            p.rect_stroke(Rect::from_min_size(at + vec2(6.0 * s, 8.0 * s), vec2(32.0 * s, 28.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            for i in 1..4 {
                l(pos2(6.0, 8.0 + 7.0 * i as f32), pos2(38.0, 8.0 + 7.0 * i as f32), 1.2);
            }
            for i in 1..3 {
                l(pos2(6.0 + 10.6 * i as f32, 8.0), pos2(6.0 + 10.6 * i as f32, 36.0), 1.2);
            }
        }
        Craft::Deck => {
            // slide screen + play triangle
            p.rect_stroke(Rect::from_min_size(at + vec2(6.0 * s, 9.0 * s), vec2(32.0 * s, 24.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at + vec2(18.0 * s, 16.0 * s), at + vec2(18.0 * s, 26.0 * s), at + vec2(27.0 * s, 21.0 * s)], base, Stroke::NONE));
            l(pos2(14.0, 38.0), pos2(30.0, 38.0), 1.4);
        }
        Craft::Pdf => {
            // page with folded corner + A
            p.rect_stroke(Rect::from_min_size(at + vec2(11.0 * s, 4.0 * s), vec2(22.0 * s, 36.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            p.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, "A", FontId::proportional(15.0 * s), base);
        }
        Craft::Fig => {
            // pen nib: diamond + circle
            p.add(Shape::convex_polygon(vec![at + vec2(22.0 * s, 4.0 * s), at + vec2(34.0 * s, 22.0 * s), at + vec2(22.0 * s, 40.0 * s), at + vec2(10.0 * s, 22.0 * s)], Color32::TRANSPARENT, Stroke::new(1.5 * s, base)));
            p.circle_stroke(r.center(), 4.0 * s, Stroke::new(1.4 * s, base));
        }
        Craft::Draw => {
            // two nodes linked by an orthogonal edge (drawio motif)
            p.rect_stroke(Rect::from_min_size(at + vec2(4.0 * s, 4.0 * s), vec2(13.0 * s, 11.0 * s)), CornerRadius::ZERO, Stroke::new(1.4 * s, base), StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_size(at + vec2(27.0 * s, 29.0 * s), vec2(13.0 * s, 11.0 * s)), CornerRadius::ZERO, Stroke::new(1.4 * s, base), StrokeKind::Middle);
            l(pos2(10.5, 15.0), pos2(10.5, 34.5), 1.4);
            l(pos2(10.5, 34.5), pos2(33.5, 34.5), 1.4);
            p.circle_filled(at + vec2(10.5 * s, 34.5 * s), 2.2 * s, accent);
        }
        Craft::Markdown => {
            p.rect_stroke(Rect::from_min_size(at + vec2(4.0 * s, 12.0 * s), vec2(36.0 * s, 20.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            p.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, "M↓", FontId::proportional(11.0 * s), base);
        }
        Craft::Typeset => {
            p.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, "¶", FontId::proportional(24.0 * s), base);
            l(pos2(14.0, 36.0), pos2(30.0, 36.0), 1.4);
        }
        Craft::Notes => {
            // sticky note with folded corner
            p.rect_stroke(Rect::from_min_size(at + vec2(8.0 * s, 7.0 * s), vec2(28.0 * s, 30.0 * s)), CornerRadius::ZERO, Stroke::new(1.5 * s, base), StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at + vec2(36.0 * s, 30.0 * s), at + vec2(28.0 * s, 37.0 * s), at + vec2(36.0 * s, 37.0 * s)], base, Stroke::NONE));
            for i in 0..3 {
                l(pos2(13.0, 14.0 + 7.0 * i as f32), pos2(31.0, 14.0 + 7.0 * i as f32), 1.3);
            }
        }
        Craft::Launcher => {}
    }
}

// ── about window (the ONE about: about the system) ───────────────────────────

fn about_window(ctx: &Context, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new("About DocCraft")
        .open(open)
        .default_width(560.0)
        .default_height(520.0)
        .collapsible(false)
        .frame(Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, theme::LINE_STRONG)).corner_radius(CornerRadius::ZERO).inner_margin(Margin::same(0)))
        .show(ctx, |ui| {
            // hero
            let (hr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 84.0), Sense::hover());
            let mut hero = theme::ACCENT;
            hero = Color32::from_rgba_unmultiplied(hero.r(), hero.g(), hero.b(), 26);
            ui.painter().rect_filled(hr, 0.0, hero);
            ui.painter().rect_filled(Rect::from_min_size(hr.left_bottom(), vec2(hr.width(), 2.0)), 0.0, theme::ACCENT);
            paint_logo_mark(ui, hr.min + vec2(20.0, 20.0), 44.0);
            ui.painter().text(hr.min + vec2(78.0, 34.0), Align2::LEFT_CENTER, "About DocCraft", FontId::proportional(24.0), theme::INK_STRONG);
            ui.painter().text(hr.min + vec2(78.0, 60.0), Align2::LEFT_CENTER, format!("DocCraft Suite v{} · one binary · ten crafts", env!("CARGO_PKG_VERSION")), FontId::monospace(11.5), theme::MUTED);

            egui::ScrollArea::vertical().id_salt("about_scroll").max_height(340.0).show(ui, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("THE SYSTEM").font(FontId::monospace(10.0)).color(theme::FAINT));
                        ui.add_space(4.0);
                        ui.label(RichText::new("DocCraft hosts the real storytold craft systems — WordCraft, GridCraft, DeckCraft, PdfCraft and FigCraft — embedded unchanged from git submodules, plus four DocCraft-native crafts (Draw, Markdown, Typeset, Notes). Every upstream craft stays linked to its GitHub repo and can be updated independently.").size(13.0).color(theme::INK));
                        ui.add_space(12.0);
                        ui.label(RichText::new("CRAFTS & READINESS").font(FontId::monospace(10.0)).color(theme::FAINT));
                        ui.add_space(4.0);
                        for c in Craft::ALL {
                            ui.horizontal(|ui| {
                                let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                ui.painter().circle_filled(dot.center(), 4.0, c.accent());
                                ui.label(RichText::new(c.name()).size(12.5).color(theme::INK_STRONG));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(c.status()).font(FontId::monospace(10.0)).color(theme::MUTED));
                                });
                            });
                        }
                        ui.add_space(12.0);
                        ui.label(RichText::new("UPDATING ANY CRAFT").font(FontId::monospace(10.0)).color(theme::FAINT));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Each real craft is a git submodule:  git submodule update --remote upstream/<craft>  then  cargo build --profile fast.  scripts/update-crafts.sh updates all of them at once.").size(12.5).color(theme::INK));
                        ui.add_space(12.0);
                        ui.label(RichText::new("FILE TYPES THIS SYSTEM OPENS").font(FontId::monospace(10.0)).color(theme::FAINT));
                        ui.add_space(4.0);
                        ui.label(RichText::new(".docx .doc .odt .rtf · .xlsx .xls .ods .csv · .pptx · .pdf · .drawio · .fig .svg · .md · .typ .tex · .note").font(FontId::monospace(11.0)).color(theme::INK));
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(RichText::new("Register file associations (Windows)").size(12.0))).clicked() {
                            crate::register_associations::run();
                            ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
                        }
                    });
                });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                if ui.add(egui::Button::new(RichText::new("WEBSITE ↗").font(FontId::monospace(11.0)))).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(theme::WEBSITE_URL));
                }
                if ui.add(egui::Button::new(RichText::new("GITHUB ↗").font(FontId::monospace(11.0)))).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(theme::ORG_URL));
                }
            });
            ui.add_space(14.0);
        });
}

// ── small widgets ────────────────────────────────────────────────────────────

fn mono_label(ui: &mut Ui, text: &str, col: Color32, size: f32) {
    ui.label(RichText::new(text).font(FontId::monospace(size)).color(col).extra_letter_spacing(0.7));
}

fn mono_link(ui: &mut Ui, text: &str) -> bool {
    let t = egui::Button::new(RichText::new(text).font(FontId::monospace(10.5)).color(theme::MUTED))
        .fill(Color32::TRANSPARENT)
        .corner_radius(CornerRadius::ZERO)
        .stroke(Stroke::NONE);
    let r = ui.add(t);
    let hovered = r.hovered();
    if hovered {
        ui.painter().text(r.rect.center(), Align2::CENTER_CENTER, text, FontId::monospace(10.5), theme::INK_STRONG);
    }
    r.clicked()
}

fn vrule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme::LINE);
}
