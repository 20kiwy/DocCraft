//! The Typeset craft: WordCraft-style chrome (title strip + Quick Access Toolbar,
//! ribbon tabs with grouped commands, right pane, status bar) around the source
//! editor, plus the page-layout settings the engine consumes.

use crate::{layout, model};
use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use doccraft_theme::Tokens;
use egui::widgets::text_edit::TextEditState;
use egui::{
    Align2, Color32, Context, CornerRadius, FontFamily, FontId, Id, Margin, Sense, Stroke, Ui, pos2, vec2,
    text::{CCursor, CCursorRange},
};
use std::time::Instant;

// ── chrome geometry (colors come from doccraft_theme tokens) ─────────────
pub const CONTENT_H: f32 = 60.0;
pub const LABEL_H: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Insert,
    Page,
    View,
}

impl Tab {
    pub fn name(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Insert => "Insert",
            Tab::Page => "Page",
            Tab::View => "View",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneTab {
    Pages,
    Outline,
    Document,
    Export,
}

impl PaneTab {
    pub fn name(self) -> &'static str {
        match self {
            PaneTab::Pages => "Pages",
            PaneTab::Outline => "Outline",
            PaneTab::Document => "Doc",
            PaneTab::Export => "Export",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorPrefs {
    pub mono: bool,   // editor face: mono or serif
    pub size: f32,    // editor font size
    pub tab: usize,   // spaces per Tab
    pub wrap: bool,
    pub numbers: bool,
    pub find: String,
    pub find_open: bool,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self { mono: true, size: 13.0, tab: 4, wrap: true, numbers: false, find: String::new(), find_open: false }
    }
}

pub struct TypesetApp {
    pub source: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub status: String,
    pub settings: layout::Settings,
    pub editor: EditorPrefs,
    pub tab: Tab,
    pub pane: PaneTab,
    pub preview: bool,
    pub zoom: f32,
    pub fit: bool,
    pub rulers: bool,
    /// Light / dark tokens (default dark); toggled from the title strip or settings.
    pub dark: bool,
    pub settings_open: bool,
    pub jump: Option<usize>,
    pub caret: (usize, usize),
    pub parsed: model::Parsed,
    pub pages: Vec<layout::Page>,
    pub headings: Vec<(u8, String, usize)>,
    pub compile_ms: u128,
    recompile_at: Option<f64>,
    pre_edit: Option<String>,
    undo: Vec<String>,
    row_pitch: f32,
    focus_editor: bool,
}

const EDITOR_ID: &str = "ts_source_edit";

impl TypesetApp {
    pub fn new() -> Self {
        let source = SAMPLE.replace("__", "");
        let mut app = Self {
            source,
            path: None,
            dirty: false,
            status: "Ready — Typeset craft".into(),
            settings: layout::Settings::default(),
            editor: EditorPrefs::default(),
            tab: Tab::Home,
            pane: PaneTab::Pages,
            preview: true,
            zoom: 0.62,
            fit: true,
            rulers: false,
            dark: true,
            settings_open: false,
            jump: None,
            caret: (1, 1),
            parsed: model::Parsed::default(),
            pages: Vec::new(),
            headings: Vec::new(),
            compile_ms: 0,
            recompile_at: None,
            pre_edit: None,
            undo: Vec::new(),
            row_pitch: 16.0,
            focus_editor: false,
        };
        app.recompile();
        app
    }

    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        if let Ok(s) = std::str::from_utf8(bytes) {
            self.source = s.to_string();
            self.path = Some(name.to_string());
            self.dirty = false;
            self.recompile();
            self.status = format!("Opened {name}");
        }
    }

    /// Kept for the host's craft protocol; the app drives itself from `ui`.
    pub fn logic(&mut self, _ctx: &Context) {}

    // ── compile / files ─────────────────────────────────────────────────────

    pub fn recompile(&mut self) {
        let t0 = Instant::now();
        self.parsed = model::parse(&self.source);
        let lay = layout::layout(&self.parsed.meta, &self.parsed.blocks, &self.settings);
        self.pages = lay.pages;
        self.headings = lay.headings;
        self.compile_ms = t0.elapsed().as_millis();
        if let Some(pre) = self.pre_edit.take() {
            self.undo.push(pre);
            if self.undo.len() > 64 {
                self.undo.remove(0);
            }
        }
    }

    pub fn save(&mut self) {
        let name = self.path.clone().unwrap_or_else(|| "document.typ".into());
        let p = std::env::temp_dir().join(&name);
        match std::fs::write(&p, &self.source) {
            Ok(_) => {
                self.dirty = false;
                self.status = format!("Saved {}", p.display());
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub fn export_pdf(&mut self) {
        let pdf = layout::export_pdf(&self.parsed.meta, &self.parsed.blocks, &self.settings);
        let stem = self
            .path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
            .and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string()))
            .unwrap_or_else(|| "doccraft-typeset".into());
        let p = std::env::temp_dir().join(format!("{stem}.pdf"));
        match std::fs::write(&p, &pdf) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    pub fn export_txt(&mut self) {
        let txt = layout::plain_text(&self.parsed);
        let stem = self
            .path
            .as_deref()
            .and_then(|p| p.rsplit(['/', '\\']).next())
            .and_then(|p| p.rsplit_once('.').map(|(s, _)| s))
            .unwrap_or("doccraft-typeset");
        let p = std::env::temp_dir().join(format!("{stem}.txt"));
        match std::fs::write(&p, txt) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    fn touch(&mut self, ctx: &Context, delay: f64) {
        self.recompile_at = Some(ctx.input(|i| i.time) + delay);
    }

    fn debounce(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        if let Some(t) = self.recompile_at
            && now >= t
        {
            self.recompile_at = None;
            self.recompile();
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.source = prev;
            self.dirty = true;
            self.recompile();
        }
    }

    // ── settings setters (ribbon + dialog both land here) ───────────────────

    fn edit_page(&mut self, ctx: &Context, f: impl FnOnce(&mut layout::PageSetup)) {
        f(&mut self.settings.page);
        self.touch(ctx, 0.0);
    }

    fn edit_hf(&mut self, ctx: &Context, f: impl FnOnce(&mut layout::HeaderFooter)) {
        f(&mut self.settings.hf);
        self.touch(ctx, 0.0);
    }

    fn edit_style(&mut self, ctx: &Context, f: impl FnOnce(&mut layout::DocStyle)) {
        f(&mut self.settings.style);
        self.touch(ctx, 0.0);
    }

    pub fn set_base(&mut self, ctx: &Context, v: f32) {
        self.edit_style(ctx, |s| s.base_size = v.clamp(8.0, 24.0));
    }
    pub fn set_spacing(&mut self, ctx: &Context, v: f32) {
        self.edit_style(ctx, |s| s.line_spacing = v);
    }
    pub fn set_justify(&mut self, ctx: &Context, v: bool) {
        self.edit_style(ctx, |s| s.justify = v);
    }
    pub fn set_mono_font(&mut self, ctx: &Context, v: bool) {
        self.edit_style(ctx, |s| s.mono_font = v);
    }
    pub fn set_paper(&mut self, ctx: &Context, p: layout::Paper) {
        self.edit_page(ctx, |s| s.paper = p);
    }
    pub fn set_landscape(&mut self, ctx: &Context, v: bool) {
        self.edit_page(ctx, |s| s.landscape = v);
    }
    pub fn set_margins(&mut self, ctx: &Context, (t, b, l, r): (f32, f32, f32, f32)) {
        self.edit_page(ctx, |s| {
            (s.margin_top, s.margin_bottom, s.margin_left, s.margin_right) = (t, b, l, r);
        });
    }
    pub fn set_margin_top(&mut self, ctx: &Context, v: f32) {
        self.edit_page(ctx, |s| s.margin_top = v);
    }
    pub fn set_margin_bottom(&mut self, ctx: &Context, v: f32) {
        self.edit_page(ctx, |s| s.margin_bottom = v);
    }
    pub fn set_margin_left(&mut self, ctx: &Context, v: f32) {
        self.edit_page(ctx, |s| s.margin_left = v);
    }
    pub fn set_margin_right(&mut self, ctx: &Context, v: f32) {
        self.edit_page(ctx, |s| s.margin_right = v);
    }
    pub fn set_custom_size(&mut self, ctx: &Context, w: f32, h: f32) {
        self.edit_page(ctx, |s| {
            s.custom_w = w;
            s.custom_h = h;
        });
    }
    pub fn set_columns(&mut self, ctx: &Context, n: u8) {
        self.edit_page(ctx, |s| s.columns = n.clamp(1, 2));
    }
    pub fn set_gutter(&mut self, ctx: &Context, v: f32) {
        self.edit_page(ctx, |s| s.gutter = v.clamp(8.0, 96.0));
    }
    pub fn set_numbers(&mut self, ctx: &Context, v: bool) {
        self.edit_hf(ctx, |h| h.numbers = v);
    }
    pub fn set_number_pos(&mut self, ctx: &Context, v: layout::NumPos) {
        self.edit_hf(ctx, |h| h.number_pos = v);
    }
    pub fn set_footer(&mut self, ctx: &Context, v: String) {
        self.edit_hf(ctx, |h| h.footer = v);
    }
    pub fn set_first_different(&mut self, ctx: &Context, v: bool) {
        self.edit_hf(ctx, |h| h.first_different = v);
    }

    pub fn zoom_in(&mut self) {
        self.fit = false;
        self.zoom = (self.zoom * 1.2).min(1.5);
    }
    pub fn zoom_out(&mut self) {
        self.fit = false;
        self.zoom = (self.zoom / 1.2).max(0.15);
    }

    // ── editing commands (QAT/ribbon land here) ─────────────────────────────

    fn snapshot(&mut self) {
        self.undo.push(self.source.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.dirty = true;
    }

    fn cursor_char(&self, ctx: &Context) -> (usize, usize) {
        let n = self.source.chars().count();
        TextEditState::load(ctx, Id::new(EDITOR_ID))
            .and_then(|s| s.cursor.char_range())
            .map(|r| (r.primary.index.0.min(n), r.secondary.index.0.min(n)))
            .unwrap_or((n, n))
    }

    /// Insert text at the caret (or around the selection).
    pub fn insert_at_cursor(&mut self, ctx: &Context, text: &str) {
        self.snapshot();
        let (a, b) = self.cursor_char(ctx);
        let (a, b) = (a.min(b), a.max(b));
        self.source = splice(&self.source, a, b, text);
        self.focus_editor = true;
        set_cursor_after(ctx, a + text.chars().count());
        self.touch(ctx, 0.0);
    }

    /// A standalone paragraph block (page break, image, table snippet).
    pub fn insert_block(&mut self, ctx: &Context, snippet: &str) {
        self.insert_at_cursor(ctx, &format!("\n\n{snippet}\n"));
    }

    /// Toggle a line prefix (#, ##, ###, > …) on the caret's line.
    pub fn apply_line_prefix(&mut self, ctx: &Context, prefix: &str) {
        self.snapshot();
        let (pos, _) = self.cursor_char(ctx);
        let (s, e) = line_bounds(&self.source, pos);
        let line: String = self.source.chars().skip(s).take(e - s).collect();
        let swapped = if line.starts_with(prefix) { line[prefix.chars().count()..].to_string() } else { format!("{prefix}{line}") };
        self.source = splice(&self.source, s, e, &swapped);
        self.focus_editor = true;
        set_cursor_after(ctx, s + swapped.chars().count());
        self.touch(ctx, 0.0);
    }

    /// Wrap the caret's whole line (`\center{…}`, `\right{…}`).
    pub fn wrap_line(&mut self, ctx: &Context, open: &str, close: &str) {
        self.snapshot();
        let (pos, _) = self.cursor_char(ctx);
        let (s, e) = line_bounds(&self.source, pos);
        let line: String = self.source.chars().skip(s).take(e - s).collect();
        let wrapped = format!("{open}{line}{close}");
        self.source = splice(&self.source, s, e, &wrapped);
        self.focus_editor = true;
        set_cursor_after(ctx, s + open.chars().count() + line.chars().count());
        self.touch(ctx, 0.0);
    }

    /// Wrap the selection (or insert an empty pair at the caret): **…**, *…*.
    pub fn wrap_selection(&mut self, ctx: &Context, open: &str, close: &str) {
        self.snapshot();
        let (a, b) = self.cursor_char(ctx);
        let (a, b) = (a.min(b), a.max(b));
        let text = format!("{open}{close}");
        self.source = splice(&self.source, a, b, &text);
        self.focus_editor = true;
        set_cursor_after(ctx, a + open.chars().count());
        self.touch(ctx, 0.0);
    }

    /// Put a code fence around the caret's line.
    pub fn fence_line(&mut self, ctx: &Context) {
        self.snapshot();
        let (pos, _) = self.cursor_char(ctx);
        let (s, e) = line_bounds(&self.source, pos);
        let line: String = self.source.chars().skip(s).take(e - s).collect();
        let fenced = if line == "```" { line } else { format!("```\n{line}\n```") };
        self.source = splice(&self.source, s, e, &fenced);
        self.focus_editor = true;
        self.touch(ctx, 0.0);
    }

    /// Find: jump to the next/previous match of the find bar's query.
    pub fn find_jump(&mut self, ctx: &Context, forward: bool) {
        let q = self.editor.find.clone();
        if q.is_empty() {
            return;
        }
        let chars: Vec<char> = self.source.chars().collect();
        let qv: Vec<char> = q.chars().collect();
        if qv.len() > chars.len() {
            return;
        }
        let matches: Vec<usize> = (0..=chars.len() - qv.len()).filter(|&i| chars[i..i + qv.len()] == qv[..]).collect();
        if matches.is_empty() {
            return;
        }
        let cur = self.cursor_char(ctx).0;
        let next = if forward {
            matches.iter().find(|&&m| m > cur).or(matches.first()).copied().unwrap_or(0)
        } else {
            matches.iter().rev().find(|&&m| m < cur).or(matches.last()).copied().unwrap_or(0)
        };
        set_cursor_after(ctx, next + qv.len());
        self.focus_editor = true;
    }

    // ── frame ───────────────────────────────────────────────────────────────

    pub fn ui(&mut self, ui: &mut Ui) {
        // Shared suite design tokens, applied once at the start of every frame.
        let tokens = if self.dark { doccraft_theme::Tokens::dark() } else { doccraft_theme::Tokens::light() };
        doccraft_theme::apply(ui.ctx(), tokens);
        ui.style_mut().visuals = doccraft_theme::visuals(tokens);
        self.debounce(ui.ctx());
        title_strip(self, ui);
        crate::ribbon::show(self, ui);
        status_bar(self, ui);
        if self.preview {
            crate::panes::show(self, ui);
        }
        body(self, ui);
        if self.settings_open {
            crate::settings::show(self, ui.ctx());
        }
    }
}

// ── chrome: title strip + QAT ────────────────────────────────────────────────

fn title_strip(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("ts_title")
        .exact_size(34.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(Margin { left: 10, right: 10, top: 0, bottom: 0 }).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                ui.label(egui::RichText::new("Typeset").font(FontId::proportional(13.5)).strong().color(t.accent_text));
                ui.separator();
                let name = app.path.clone().unwrap_or_else(|| "Untitled.typ".into());
                ui.label(egui::RichText::new(name).font(FontId::proportional(12.0)).color(t.text));
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                if app.dirty {
                    ui.painter().circle_filled(r.center(), 3.0, t.accent);
                }
                ui.add_space(6.0);
                qat(app, ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Typeset").font(FontId::proportional(11.0)).color(t.text_tertiary));
                    let shown: String = app.status.chars().rev().take(52).collect::<String>().chars().rev().collect();
                    let shown = if shown.chars().count() < app.status.chars().count() { format!("…{shown}") } else { shown };
                    ui.label(egui::RichText::new(shown).font(FontId::proportional(11.0)).color(t.text_secondary));
                });
            });
        });
}

fn qat(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let style = IconButtonStyle {
        normal: ButtonVisuals { fill: Color32::TRANSPARENT, stroke: Stroke::NONE, icon: t.icon_secondary },
        hovered: ButtonVisuals { fill: t.hover, stroke: Stroke::NONE, icon: t.icon },
        pressed: None,
        selected: ButtonVisuals { fill: t.selected_row, stroke: Stroke::NONE, icon: t.icon },
        corner_radius: CornerRadius::same(5),
        focus_stroke: Stroke::NONE,
    };
    let mk = |ui: &mut Ui, icon: &str, tip: &str| IconButton::new(tip, vec2(26.0, 24.0), &style).show(ui, |ui, r, c| paint_icon(ui.painter(), r, icon, c, t.panel));
    if mk(ui, "save", "Save").clicked() {
        app.save();
    }
    if mk(ui, "undo", "Undo").clicked() {
        app.undo();
    }
    if mk(ui, "compile", "Recompile").clicked() {
        app.recompile();
    }
    ui.separator();
    // Sun / Moon theme toggle, next to the QAT
    let (icon, tip) = if app.dark { ("sun", "Switch to the light theme") } else { ("moon", "Switch to the dark theme") };
    if mk(ui, icon, tip).clicked() {
        app.dark = !app.dark;
    }
    ui.separator();
    if mk(ui, "gear", "Settings").clicked() {
        app.settings_open = !app.settings_open;
    }
}

/// Small vector icons, WordCraft-tile style: stroke drawings in a 16×16 box.
/// `bg` is the chrome fill behind the icon (used to bite the moon crescent).
/// The drawings live in the shared `doccraft_theme::icons` module so every
/// native craft paints the exact same set (zero font dependency — no tofu).
pub fn paint_icon(p: &egui::Painter, r: egui::Rect, name: &str, c: Color32, bg: Color32) {
    doccraft_theme::icons::paint(p, r, name, c, bg);
}

// ── body: find bar, gutter, editor ───────────────────────────────────────────

fn body(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.bg).inner_margin(0))
        .show(ui, |ui| {
            if app.editor.find_open {
                find_bar(app, ui);
                ui.separator();
            }
            let style = if app.editor.mono { egui::TextStyle::Monospace } else { egui::TextStyle::Body };
            let fam = if app.editor.mono { FontFamily::Monospace } else { FontFamily::Proportional };
            ui.style_mut().text_styles.insert(style, FontId::new(app.editor.size, fam));
            ui.style_mut().text_styles.insert(egui::TextStyle::Monospace, FontId::new(app.editor.size, FontFamily::Monospace));
            ui.spacing_mut().item_spacing.y = 0.0;
            let numbers = app.editor.numbers && !app.editor.wrap;
            egui::ScrollArea::vertical().id_salt("ts_editor_scroll").show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if numbers {
                        gutter(ui, app);
                    }
                    let before = app.source.clone();
                    let editor_font = FontId::new(app.editor.size, if app.editor.mono { FontFamily::Monospace } else { FontFamily::Proportional });
                    let ext_font = editor_font.clone();
                    let mut ext_layout = move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _w: f32| -> std::sync::Arc<egui::Galley> {
                        let mut job = egui::text::LayoutJob::simple(buf.as_str().to_owned(), ext_font.clone(), ui.visuals().text_color(), f32::INFINITY);
                        job.keep_trailing_whitespace = true;
                        ui.fonts_mut(|f| f.layout_job(job))
                    };
                    let mut te = egui::TextEdit::multiline(&mut app.source).font(editor_font.clone()).desired_width(f32::INFINITY).id(Id::new(EDITOR_ID));
                    if !app.editor.wrap {
                        te = te.layouter(&mut ext_layout);
                    }
                    let resp = ui.add(te);
                    if app.focus_editor {
                        resp.request_focus();
                        app.focus_editor = false;
                    }
                    // measure the real row pitch so the gutter stays aligned
                    let rows = before.lines().count().max(1) as f32;
                    app.row_pitch = ((resp.rect.height() / rows).max(4.0)).min(app.row_pitch.max(4.0) * 2.0);
                    if app.source != before {
                        app.dirty = true;
                        if app.pre_edit.is_none() {
                            app.pre_edit = Some(before);
                        }
                        app.touch(ui.ctx(), 0.5);
                    }
                    app.caret = caret(ui.ctx(), &app.source);
                });
            });
        });
}

fn gutter(ui: &mut Ui, app: &mut TypesetApp) {
    let t = Tokens::get(ui.ctx());
    let rows = app.source.lines().count().max(1);
    let pitch = app.row_pitch;
    let (r, _) = ui.allocate_exact_size(vec2(46.0, rows as f32 * pitch), Sense::hover());
    let p = ui.painter();
    let font = FontId::new(app.editor.size * 0.85, FontFamily::Monospace);
    // gutter background + hairline right edge
    p.rect_filled(r, 0.0, t.panel);
    p.line_segment([pos2(r.max.x, r.min.y), pos2(r.max.x, r.max.y)], Stroke::new(1.0, t.border));
    for i in 1..=rows {
        p.text(pos2(r.max.x - 8.0, r.min.y + i as f32 * pitch - pitch * 0.5), Align2::RIGHT_CENTER, format!("{i}"), font.clone(), t.text_tertiary);
    }
}

fn find_bar(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Find").small().color(t.text_secondary));
        let mut q = app.editor.find.clone();
        let resp = ui.add(egui::TextEdit::singleline(&mut q).hint_text("search the source…").desired_width(170.0));
        if resp.changed() {
            app.editor.find = q;
        }
        let n = if app.editor.find.is_empty() {
            0
        } else {
            app.source.matches(&app.editor.find).count()
        };
        ui.label(egui::RichText::new(format!("{n} matches")).small().color(t.text_secondary));
        // match navigation painted from the shared vector icon set (no tofu)
        let fbtn = |ui: &mut Ui, icon: &str, tip: &str| -> bool {
            let (r, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, t.hover);
            }
            paint_icon(ui.painter(), r, icon, t.text, t.panel);
            resp.on_hover_text(tip).clicked()
        };
        if fbtn(ui, "arrow-up", "Previous match") {
            app.find_jump(ui.ctx(), false);
        }
        if fbtn(ui, "arrow-down", "Next match") {
            app.find_jump(ui.ctx(), true);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if fbtn(ui, "close", "Close find") {
                app.editor.find_open = false;
            }
        });
    });
    ui.add_space(2.0);
}

fn caret(ctx: &Context, src: &str) -> (usize, usize) {
    let idx = TextEditState::load(ctx, Id::new(EDITOR_ID))
        .and_then(|s| s.cursor.char_range())
        .map(|r| r.primary.index.0.min(src.chars().count()))
        .unwrap_or(0);
    let before: String = src.chars().take(idx).collect();
    let line = before.matches('\n').count() + 1;
    let col = idx - before.rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
    (line, col)
}

// ── status bar ───────────────────────────────────────────────────────────────

fn status_bar(app: &mut TypesetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("ts_status")
        .exact_size(24.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 0 }).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            let cy = full.center().y;
            let p = ui.painter();
            let f = FontId::proportional(11.0);
            p.text(pos2(full.min.x, cy), Align2::LEFT_CENTER, format!("Ln {}, Col {}", app.caret.0, app.caret.1), f.clone(), t.text_secondary);
            p.text(
                pos2(full.center().x, cy),
                Align2::CENTER_CENTER,
                format!("{} words · {} pages", model::word_count(&app.source), app.pages.len()),
                f.clone(),
                t.text_secondary,
            );
            let name = app.path.clone().unwrap_or_else(|| "Untitled.typ".into());
            let dot = if app.dirty { " ●" } else { "" };
            p.text(pos2(full.max.x, cy), Align2::RIGHT_CENTER, format!("{name}{dot} · Typeset"), f, t.text_secondary);
        });
}

// ── text helpers ─────────────────────────────────────────────────────────────

fn splice(src: &str, a: usize, b: usize, ins: &str) -> String {
    let mut out = String::with_capacity(src.len() + ins.len());
    let mut done = false;
    for (i, c) in src.chars().enumerate() {
        if i == a {
            out.push_str(ins);
            done = true;
        }
        if i < a || i >= b {
            out.push(c);
        }
    }
    if !done {
        out.push_str(ins);
    }
    out
}

fn line_bounds(src: &str, pos: usize) -> (usize, usize) {
    let chars: Vec<char> = src.chars().collect();
    let pos = pos.min(chars.len());
    let mut s = 0;
    for (i, c) in chars.iter().enumerate().take(pos) {
        if *c == '\n' {
            s = i + 1;
        }
    }
    let e = chars[pos..].iter().position(|&c| c == '\n').map_or(chars.len(), |i| pos + i);
    (s, e)
}

fn set_cursor_after(ctx: &Context, idx: usize) {
    if let Some(mut st) = TextEditState::load(ctx, Id::new(EDITOR_ID)) {
        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
        st.store(ctx, Id::new(EDITOR_ID));
    }
}

// ── small helpers shared with the panes ──────────────────────────────────────

pub fn ribbon_small(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    crate::ribbon::small(ui, label, active)
}

pub fn pane_scroll(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        add(ui);
    });
}

pub fn pane_button(ui: &mut Ui, label: impl Into<String>) -> egui::Response {
    let text = egui::RichText::new(label.into());
    ui.add_sized(vec2(ui.available_width().min(170.0), 24.0), egui::Button::new(text))
}

const SAMPLE: &str = r#"\title{Typesetting, Natively}
\author{DocCraft — Typeset craft}
\date{October 2026}

# About this craft
Typeset is DocCraft's own page-layout engine. You write markup on the left; the engine breaks, justifies and paginates the text into live page cards on the right, and exports a real PDF (Type1 base-14 fonts, valid xref).

## The ribbon
The chrome follows the WordCraft anatomy: a title strip with the Quick Access Toolbar, ribbon tabs with grouped commands and small captions, a right pane and a status bar.

- **Font** toggles the editor face and sizes the body text
- **Styles** turn the current line into a chapter, section or subsection
- **Insert** places page breaks, image lines, tables and symbols
- **View** fits the preview, zooms and shows rulers

## Page layout
The __Page__ tab is the settings well: paper size, orientation, margins, columns, page numbers and footer text — they edit the layout engine's config directly, and the preview follows at once.

With two columns the text flows down the left column first, then continues in the right one, like any newspaper. Margins can be Normal, Narrow, Wide or custom millimetres in the settings dialog (the gear in the title strip).

\quote{Good typography is invisible; great typography is felt.}

| Craft | Opens |
| WordCraft | .docx |
| Typeset | .typ |

\image{figures/plate.pdf ; A sample figure placeholder}

# Export
The pane's Export tab saves the source under its own extension, renders a plain-text copy, and writes the PDF next to the samples in the temp directory.
"#;
