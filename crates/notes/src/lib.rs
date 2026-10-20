//! Notes — DocCraft's notebook craft. Multi-page journal with the full
//! WordCraft-anatomy chrome (title strip + QAT, tab strip, grouped command
//! ribbon), a page sidebar, a typeset-style page-card live preview, a settings
//! window and an honest status bar. Every color is painted from the shared
//! `doccraft-theme` tokens — no hardcoded hex in the chrome — and the craft
//! switches between the suite's light and dark themes with the title-strip
//! Sun/Moon toggle. Distinct from the Markdown craft: this one is a notebook
//! of many small pages, not one document, and persists the whole notebook as
//! one `.note` (JSON) file.

use std::sync::Arc;

use egui::UiBuilder;
use egui::{
    pos2, Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Margin, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextFormat, Ui, Vec2, WidgetText,
};
use egui::text::{CCursor, CCursorRange};
use serde::{Deserialize, Serialize};

use doccraft_theme::{self as theme, Tokens};

// ── chrome metrics (mirrors the Markdown craft's ribbon anatomy) ─────────────

const TITLE_H: f32 = 34.0;
const TABS_H: f32 = 28.0;
const RIBBON_H: f32 = 78.0;
const STATUS_H: f32 = 22.0;
const CONTENT_H: f32 = 54.0;
const LABEL_H: f32 = 16.0;
const PREVIEW_MAX_W: f32 = 680.0;

/// The craft accent: "DocCraft Blue" (the shared theme accent) plus five more
/// tasteful options; overrides the tokens accent for sidebar bars, the tab
/// underline, heading rules and code chips only.
const ACCENTS: [(&str, [u8; 3]); 6] = [
    ("DocCraft Blue", [0x0D, 0x99, 0xFF]),
    ("Teal", [0x2E, 0x9E, 0x8F]),
    ("Green", [0x3F, 0x9D, 0x4E]),
    ("Amber", [0xD0, 0x8B, 0x2C]),
    ("Rose", [0xC9, 0x50, 0x8A]),
    ("Purple", [0x8B, 0x5C, 0xF6]),
];

fn body_id() -> Id {
    Id::new("notes_body_edit")
}

fn scroll_id() -> Id {
    Id::new("notes_edit")
}

#[derive(Clone, Serialize, Deserialize)]
struct NotePage {
    title: String,
    body: String,
    #[serde(default)]
    created: String,
}

impl NotePage {
    fn new(title: &str) -> Self {
        Self { title: title.into(), body: String::new(), created: today() }
    }
}

pub struct NotesApp {
    pages: Vec<NotePage>,
    active: usize,
    search: String,
    show_preview: bool,
    sidebar_open: bool,
    font_size: f32,
    body_mono: bool,
    pub dark: bool,
    show_settings: bool,
    show_find: bool,
    find: String,
    find_idx: usize,
    accent: usize,
    tab: u8,
    sel: (usize, usize),
    goto: Option<usize>,
    path: Option<String>,
    dirty: bool,
    pre_edit: String,
    pre_page: usize,
    undo: Vec<(usize, String)>,
    redo: Vec<(usize, String)>,
    toast: Option<(String, f64)>,
}

impl Default for NotesApp {
    fn default() -> Self {
        Self::new()
    }
}

impl NotesApp {
    pub fn new() -> Self {
        let pages = vec![NotePage::new("Welcome")];
        let mut me = Self {
            pages,
            active: 0,
            search: String::new(),
            show_preview: true,
            sidebar_open: true,
            font_size: 15.0,
            body_mono: false,
            dark: true,
            show_settings: false,
            show_find: false,
            find: String::new(),
            find_idx: 0,
            accent: 0,
            tab: 0,
            sel: (0, 0),
            goto: None,
            path: None,
            dirty: false,
            pre_edit: String::new(),
            pre_page: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            toast: None,
        };
        me.pages[0].body = String::from(
            "# Welcome to Notes\n\nA quiet place for quick pages.\n\n\
            - Click **+ New page** in the sidebar\n\
            - Type here; the styled preview follows along\n\
            - Use the Home ribbon for font, lists and quotes\n\n\
            `Notes` keeps everything in this notebook until you export a page as .md.\n\n\
            ## Try the ribbon\n\n\
            - [ ] Select a word and press **B** to wrap it in bold\n\
            - [x] Read this page in the page-card preview\n\n\
            > Notes shares the same DocCraft chrome as Markdown — tabs, ribbon, settings, light and dark.\n\n\
            ```\nfn main() {\n    println!(\"hello from Notes\");\n}\n```\n\n\
            Made with [DocCraft](https://doccraft.example) · Ctrl+S saves the notebook.\n",
        );
        me
    }

    /// Host hands a file here: a `.note` notebook (JSON list of pages) loads as
    /// the whole notebook; anything else imports as one markdown page.
    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        if let Ok(pages) = serde_json::from_slice::<Vec<NotePage>>(bytes)
            && !pages.is_empty()
        {
            self.pages = pages;
            self.active = 0;
            self.search.clear();
            self.path = Some(name.to_string());
            self.dirty = false;
            self.toast = Some((format!("Opened notebook {name}"), now()));
            return;
        }
        let body = String::from_utf8_lossy(bytes).to_string();
        let title = name.trim_end_matches(".md").trim_end_matches(".markdown").to_string();
        self.pages.push(NotePage { title, body, created: today() });
        self.active = self.pages.len() - 1;
        self.dirty = false;
        self.toast = Some(("Imported as a new page".into(), now()));
    }

    /// The whole notebook serialized as the `.note` JSON format.
    pub fn save_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.pages).unwrap_or_default()
    }

    /// Write the notebook to `self.path` (default `./notes.note`).
    pub fn save_to_disk(&mut self) -> bool {
        let path = self.path.clone().unwrap_or_else(|| "./notes.note".into());
        match std::fs::write(&path, self.save_bytes()) {
            Ok(()) => {
                self.path = Some(path.clone());
                self.dirty = false;
                self.toast = Some((format!("Saved to {path}"), now()));
                true
            }
            Err(e) => {
                self.toast = Some((format!("Save failed: {e}"), now()));
                false
            }
        }
    }

    /// Host asks for an export; returns (filename, bytes).
    pub fn export_md(&mut self) -> Option<(String, Vec<u8>)> {
        let p = self.pages.get(self.active)?;
        let name = format!("{}.md", sanitize(&p.title));
        Some((name, p.body.clone().into_bytes()))
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let t = if self.dark { Tokens::dark() } else { Tokens::light() };
        theme::apply(&ctx, t);
        self.pre_edit = self.active_body().to_string();
        self.pre_page = self.active;

        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save_to_disk();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::F)) {
            self.show_find = true;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::P)) {
            self.show_preview = !self.show_preview;
        }

        self.title_strip(ui, &t);
        self.tab_strip(ui, &t);
        self.command_strip(ui, &t);
        self.status_bar(ui, &t);
        if self.sidebar_open {
            self.sidebar(ui, &t);
        }
        // egui 0.36 nested panels: a right Panel here doesn't shrink the area
        // the editor CentralPanel later claims, so the preview + editor are a
        // deterministic manual split of the rect left over after the sidebar.
        let rest = ui.available_rect_before_wrap();
        let preview_w = if self.show_preview && rest.width() > 640.0 { 340.0 } else { 0.0 };
        let edit_rect = Rect::from_min_max(rest.min, pos2(rest.max.x - preview_w, rest.max.y));
        let prev_rect = Rect::from_min_max(pos2(rest.max.x - preview_w, rest.min.y), rest.max);
        if preview_w > 0.0 {
            ui.scope_builder(
                UiBuilder::new().max_rect(prev_rect).id_salt("notes_preview_area"),
                |ui| {
                    Frame::NONE.fill(t.canvas).inner_margin(Margin::same(0)).show(ui, |ui| {
                        self.preview_content(ui, &t);
                    });
                },
            );
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(edit_rect).id_salt("notes_editor_area"),
            |ui| {
                Frame::NONE.fill(t.bg).inner_margin(Margin::same(0)).show(ui, |ui| {
                    self.editor(ui, &t);
                });
            },
        );

        if self.active == self.pre_page && self.active_body() != self.pre_edit {
            self.undo.push((self.active, self.pre_edit.clone()));
            self.redo.clear();
            self.dirty = true;
        }

        if self.show_settings {
            self.settings_window(&ctx, &t);
        }
        if self.show_find {
            self.find_bar(&ctx, &t);
        }
        self.toast_ui(&ctx, &t);
    }

    /// The settings accent, overriding the tokens accent for sidebar bars, the
    /// tab underline, heading rules and code chips only.
    fn accent(&self, t: &Tokens) -> Color32 {
        ACCENTS.get(self.accent)
            .map(|(_, c)| Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(t.accent)
    }

    fn active_body(&self) -> &str {
        self.pages.get(self.active).map(|p| p.body.as_str()).unwrap_or("")
    }

    fn active_mut(&mut self) -> Option<&mut NotePage> {
        self.pages.get_mut(self.active)
    }

    // ── title strip + Quick Access Toolbar ──────────────────────────────────

    fn title_strip(&mut self, ui: &mut Ui, t: &Tokens) {
        let a = self.accent(t);
        egui::Panel::top("notes_title")
            .exact_size(TITLE_H)
            .frame(Frame::NONE.fill(t.panel).inner_margin(Margin { left: 10, right: 6, top: 0, bottom: 0 }))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    ui.label(RichText::new("NOTES").font(theme::semibold(13.0)).color(a));
                    ui.separator();
                    self.title_edit(ui, t);
                    // save-state dot
                    let (r, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                    if self.dirty {
                        ui.painter().circle_filled(r.center(), 3.0, t.warning);
                    } else {
                        ui.painter().circle_filled(r.center(), 3.0, t.success);
                    }
                    ui.label(RichText::new(if self.dirty { "unsaved" } else { "saved" }).size(10.5).color(t.text_secondary));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);
                        if qat_btn_icon(ui, if self.dark { "sun" } else { "moon" }, "Toggle light / dark theme", t) {
                            self.dark = !self.dark;
                        }
                        if qat_btn_icon(ui, "panel-right", "Toggle the live preview", t) {
                            self.show_preview = !self.show_preview;
                        }
                        if qat_btn_icon(ui, "file-plus", "New page", t) {
                            self.new_page();
                        }
                        if qat_btn_icon(ui, "save", "Save notebook (Ctrl+S)", t) {
                            self.save_to_disk();
                        }
                    });
                });
            });
    }

    /// The current page title, inline-editable; the field only appears on
    /// hover/focus so the strip reads as plain chrome otherwise.
    fn title_edit(&mut self, ui: &mut Ui, t: &Tokens) {
        const EDIT_W: f32 = 220.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(EDIT_W, 24.0), Sense::hover());
        let hov = ui.rect_contains_pointer(rect);
        if hov {
            ui.painter().rect_filled(rect, CornerRadius::same(5), t.field);
            ui.painter().rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, t.field_hover_border), StrokeKind::Inside);
        }
        let mut resp = None;
        if let Some(p) = self.pages.get_mut(self.active) {
            let mut child = ui.new_child(egui::UiBuilder::new().id_salt("notes_title_edit").max_rect(rect));
            let r = child.add_sized(
                rect.size(),
                TextEdit::singleline(&mut p.title)
                    .hint_text("Untitled page")
                    .font(theme::regular(12.5))
                    .frame(Frame::NONE),
            );
            resp = Some(r);
        }
        if let Some(resp) = resp {
            if resp.has_focus() {
                ui.painter().rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, t.focus), StrokeKind::Inside);
            }
        }
    }

    // ── tab strip ───────────────────────────────────────────────────────────

    fn tab_strip(&mut self, ui: &mut Ui, t: &Tokens) {
        let a = self.accent(t);
        egui::Panel::top("notes_tabs")
            .exact_size(TABS_H)
            .frame(Frame::NONE.fill(t.tab_bar).inner_margin(Margin { left: 8, right: 10, top: 0, bottom: 0 }))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                    let names = ["Home", "Insert", "View"];
                    let natural: Vec<f32> = names
                        .iter()
                        .map(|n| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*n).to_string(), theme::regular(12.5), t.text)).size().x + 26.0)
                        .collect();
                    let fit = craft_ui::tabs::fit(&natural, self.tab as usize, ui.available_width() - 140.0, 52.0, 26.0);
                    for (i, w) in &fit.shown {
                        let active = self.tab as usize == *i;
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(*w, TABS_H), Sense::click());
                        if resp.hovered() && !active {
                            ui.painter().rect_filled(r.shrink2(Vec2::new(0.0, 4.0)), CornerRadius::same(4), t.hover);
                        }
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            names[*i],
                            theme::regular(12.5),
                            if active { t.text } else { t.text_secondary },
                        );
                        if active {
                            let u = Rect::from_center_size(pos2(r.center().x, r.max.y - 2.0), Vec2::new(w - 16.0, 3.0));
                            ui.painter().rect_filled(u, 2.0, a);
                        }
                        if resp.clicked() {
                            self.tab = *i as u8;
                        }
                    }
                    if !fit.overflow.is_empty() {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(fit.overflow_width, TABS_H), Sense::click());
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "»", theme::regular(12.5), t.text_secondary);
                        egui::Popup::menu(&resp).show(|ui| {
                            for i in &fit.overflow {
                                if ui.selectable_label(false, names[*i]).clicked() {
                                    self.tab = *i as u8;
                                    ui.close();
                                }
                            }
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Notes craft · multi-page notebook").small().color(t.text_tertiary));
                    });
                });
            });
    }

    // ── grouped command strip ───────────────────────────────────────────────

    fn command_strip(&mut self, ui: &mut Ui, t: &Tokens) {
        egui::Panel::top("notes_cmds")
            .exact_size(RIBBON_H)
            .frame(
                Frame::NONE
                    .fill(t.panel)
                    .inner_margin(Margin { left: 8, right: 8, top: 4, bottom: 0 })
                    .stroke(Stroke::new(1.0, t.border)),
            )
            .show_inside(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
                            match self.tab {
                                0 => self.home_ribbon(ui, t),
                                1 => self.insert_ribbon(ui, t),
                                _ => self.view_ribbon(ui, t),
                            }
                        });
                    });
            });
    }

    fn home_ribbon(&mut self, ui: &mut Ui, t: &Tokens) {
        let ctx = ui.ctx().clone();
        let a = self.accent(t);
        group(ui, t, "Clipboard", |ui| {
            if tile_icon(ui, t, "file-plus", "New\npage", "Start a new page", false, a) {
                self.new_page();
            }
            if tile_icon(ui, t, "duplicate", "Duplicate", "Duplicate this page", false, a) {
                self.duplicate_page();
            }
            if tile_icon(ui, t, "trash", "Delete", "Delete this page", false, a) {
                self.delete_page();
            }
        });
        group(ui, t, "Font", |ui| {
            if tile(ui, t, "A−", "Smaller", "Smaller editor text", false, a) {
                self.font_size = (self.font_size - 1.0).max(11.0);
            }
            if tile(ui, t, "A+", "Larger", "Larger editor text", false, a) {
                self.font_size = (self.font_size + 1.0).min(28.0);
            }
            let label = if self.body_mono { "Serif" } else { "Mono" };
            if tile(ui, t, "Aa", label, "Toggle monospace / proportional body", self.body_mono, a) {
                self.body_mono = !self.body_mono;
            }
        });
        group(ui, t, "Paragraph", |ui| {
            if tile(ui, t, "B", "Bold", "Bold — wraps the selection in **…**", self.wrapped_with("**"), a) {
                self.wrap_marker(&ctx, "**");
            }
            if tile(ui, t, "I", "Italic", "Italic — wraps the selection in *…*", self.wrapped_with("*"), a) {
                self.wrap_marker(&ctx, "*");
            }
            if tile(ui, t, "`", "Code", "Inline code — wraps the selection in `…`", self.wrapped_with("`"), a) {
                self.wrap_marker(&ctx, "`");
            }
            if tile_icon(ui, t, "list", "List", "Bulleted list — toggles “- ” on the selected lines", false, a) {
                self.toggle_line_prefix(&ctx, "- ");
            }
            if tile_icon(ui, t, "checkbox", "Task", "Task checkbox — toggles “- [ ] ” on the selected lines", false, a) {
                self.toggle_line_prefix(&ctx, "- [ ] ");
            }
            if tile_icon(ui, t, "quote", "Quote", "Blockquote — toggles “> ” on the selected lines", false, a) {
                self.toggle_line_prefix(&ctx, "> ");
            }
        });
        group(ui, t, "Editing", |ui| {
            stack(ui, |ui| {
                row(ui, |ui| {
                    if tile_icon(ui, t, "search", "Find", "Find in page (Ctrl+F)", self.show_find, a) {
                        self.show_find = !self.show_find;
                    }
                    if tile(ui, t, "Tx", "Clear", "Clear markdown formatting from the selection", false, a) {
                        self.clear_formatting(&ctx);
                    }
                });
                row(ui, |ui| {
                    if small_btn_icon(ui, t, "undo", "Undo", !self.undo.is_empty()) {
                        self.do_undo();
                    }
                    if small_btn_icon(ui, t, "redo", "Redo", !self.redo.is_empty()) {
                        self.do_redo();
                    }
                });
            });
        });
    }

    fn insert_ribbon(&mut self, ui: &mut Ui, t: &Tokens) {
        let ctx = ui.ctx().clone();
        let a = self.accent(t);
        group(ui, t, "Links", |ui| {
            if tile_icon(ui, t, "link", "Link", "Insert a link at the caret", false, a) {
                self.insert_at_cursor(&ctx, "[label](https://)");
            }
            if tile_icon(ui, t, "code", "Code\nblock", "Insert a fenced code block", false, a) {
                self.insert_at_cursor(&ctx, "\n```\ncode\n```\n");
            }
        });
        group(ui, t, "Headings", |ui| {
            if tile(ui, t, "H1", "Heading\none", "Heading 1", false, a) {
                self.heading(&ctx, 1);
            }
            if tile(ui, t, "H2", "Heading\ntwo", "Heading 2", false, a) {
                self.heading(&ctx, 2);
            }
            if tile(ui, t, "H3", "Heading\nthree", "Heading 3", false, a) {
                self.heading(&ctx, 3);
            }
        });
        group(ui, t, "Time", |ui| {
            if tile_icon(ui, t, "calendar", "Date", "Insert today's date at the caret", false, a) {
                self.insert_at_cursor(&ctx, &today());
            }
        });
    }

    fn view_ribbon(&mut self, ui: &mut Ui, t: &Tokens) {
        let a = self.accent(t);
        group(ui, t, "Views", |ui| {
            if tile_icon(ui, t, "panel-right", "Preview", "Toggle the live preview pane", self.show_preview, a) {
                self.show_preview = !self.show_preview;
            }
            if tile_icon(ui, t, "layers", "Sidebar", "Toggle the pages sidebar", self.sidebar_open, a) {
                self.sidebar_open = !self.sidebar_open;
            }
            if tile_icon(ui, t, "gear", "Settings", "Open Notes settings", self.show_settings, a) {
                self.show_settings = !self.show_settings;
            }
        });
        group(ui, t, "Page", |ui| {
            let words = count_words(self.active_body());
            page_stat(ui, t, words);
        });
    }

    // ── sidebar ─────────────────────────────────────────────────────────────

    fn sidebar(&mut self, ui: &mut Ui, t: &Tokens) {
        let a = self.accent(t);
        egui::Panel::left("notes_sidebar")
            .default_size(210.0)
            .resizable(true)
            .size_range(150.0..=320.0)
            .frame(Frame::NONE.fill(t.panel).inner_margin(Margin::same(8)).stroke(Stroke::new(1.0, t.border)))
            .show_inside(ui, |ui| {
                let btn = egui::Button::new(RichText::new("New page").size(12.0).color(Color32::WHITE))
                    .fill(a)
                    .corner_radius(CornerRadius::same(5));
                if ui.add_sized([ui.available_width(), 26.0], btn).clicked() {
                    self.new_page();
                }
                ui.add_space(4.0);
                ui.add_sized(
                    [ui.available_width(), 24.0],
                    TextEdit::singleline(&mut self.search).hint_text("Search pages…").font(theme::regular(11.5)),
                );
                ui.add_space(4.0);
                ScrollArea::vertical().id_salt("notes_pages").auto_shrink([false, false]).show(ui, |ui| {
                    let q = self.search.to_lowercase();
                    for i in 0..self.pages.len() {
                        let (title, date, words) = {
                            let p = &self.pages[i];
                            (p.title.clone(), p.created.clone(), count_words(&p.body))
                        };
                        if !q.is_empty()
                            && !title.to_lowercase().contains(&q)
                            && !self.pages[i].body.to_lowercase().contains(&q)
                        {
                            continue;
                        }
                        let on = i == self.active;
                        let resp = ui.allocate_response(Vec2::new(ui.available_width(), 36.0), Sense::click());
                        let fill = if on {
                            t.selected_row
                        } else if resp.hovered() {
                            t.hover
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(resp.rect, CornerRadius::same(5), fill);
                        if on {
                            let bar = Rect::from_min_size(
                                pos2(resp.rect.left() + 1.0, resp.rect.top() + 4.0),
                                Vec2::new(2.0, resp.rect.height() - 8.0),
                            );
                            ui.painter().rect_filled(bar, 1.0, a);
                        }
                        ui.painter().text(
                            pos2(resp.rect.left() + 10.0, resp.rect.top() + 6.0),
                            Align2::LEFT_TOP,
                            ellip(&title, 22),
                            theme::semibold(12.0),
                            if on { t.text } else { t.text_secondary },
                        );
                        ui.painter().text(
                            pos2(resp.rect.left() + 10.0, resp.rect.bottom() - 6.0),
                            Align2::LEFT_BOTTOM,
                            format!("{}, {} words", relative_date(&date), words),
                            FontId::proportional(9.0),
                            t.text_tertiary,
                        );
                        if resp.clicked() {
                            self.active = i;
                        }
                    }
                });
            });
    }

    // ── preview pane (typeset-style page card) ──────────────────────────────

    fn preview_content(&mut self, ui: &mut Ui, t: &Tokens) {
        {
                ui.label(RichText::new("PREVIEW").size(10.0).color(t.text_tertiary));
                ScrollArea::vertical().id_salt("notes_prev_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    let avail = ui.available_width();
                    let card_w = avail.min(PREVIEW_MAX_W);
                    let side = ((avail - card_w) / 2.0).max(0.0);
                    // the document page: white in the light theme, 0x252526 in dark
                    let paper = if t.dark { Color32::from_rgb(0x25, 0x25, 0x26) } else { Color32::WHITE };
                    let accent = self.accent(t);
                    let body = self.active_body().to_string();
                    let (size, mono) = (self.font_size, self.body_mono);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if side > 0.0 {
                            ui.add_space(side);
                        }
                        Frame::NONE
                            .fill(paper)
                            .corner_radius(2)
                            .shadow(theme::card_shadow(t))
                            .inner_margin(Margin::same(40))
                            .show(ui, |ui| {
                                ui.set_width(card_w - 80.0);
                                preview_markdown(ui, &body, size, mono, t, accent);
                            });
                    });
                    ui.add_space(20.0);
                });
        }
    }

    // ── editor ──────────────────────────────────────────────────────────────

    fn editor(&mut self, ui: &mut Ui, t: &Tokens) {
        let ctx = ui.ctx().clone();
        // pending find jump: place the line near the top of the view
        if let Some(off) = self.goto.take() {
            let src = self.active_body().to_string();
            let line = src[..char_to_byte(&src, off)].matches('\n').count() as f32 + 1.0;
            if let Some(mut st) = egui::scroll_area::State::load(&ctx, scroll_id()) {
                st.offset.y = ((line - 1.0) * line_pitch(&ctx, self.font_size, self.body_mono) - 60.0).max(0.0);
                st.store(&ctx, scroll_id());
            }
        }

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(t.bg).inner_margin(Margin::same(12)))
            .show(ui, |ui| {
                let (title, body, font_size, mono) = match self.pages.get_mut(self.active) {
                    Some(p) => (&mut p.title, &mut p.body, self.font_size, self.body_mono),
                    None => return,
                };
                ui.add_sized(
                    [ui.available_width(), 26.0],
                    TextEdit::singleline(title)
                        .hint_text("Untitled page")
                        .font(theme::semibold(18.0))
                        .frame(Frame::NONE),
                );
                ui.add_space(2.0);
                ScrollArea::vertical().id_salt(scroll_id()).auto_shrink([false, false]).show(ui, |ui| {
                    let font = if mono { FontId::monospace(font_size) } else { FontId::proportional(font_size) };
                    ui.add(
                        TextEdit::multiline(body)
                            .id(body_id())
                            .hint_text("Write something…")
                            .font(font)
                            .frame(Frame::NONE)
                            .desired_rows(16)
                            .desired_width(ui.available_width().max(200.0)),
                    );
                });

                // track the selection for the ribbon's marker toggles
                if let Some(st) = TextEdit::load_state(&ctx, body_id())
                    && let Some(range) = st.cursor.char_range()
                {
                    let (a, b) = (range.primary.index.0, range.secondary.index.0);
                    self.sel = (a.min(b), a.max(b));
                }
            });
    }

    // ── status bar ──────────────────────────────────────────────────────────

    fn status_bar(&mut self, ui: &mut Ui, t: &Tokens) {
        egui::Panel::bottom("notes_status")
            .exact_size(STATUS_H)
            .frame(Frame::NONE.fill(t.panel).inner_margin(Margin::symmetric(10, 2)).stroke(theme::hairline(t)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(self.hint()).size(10.5).color(t.text_tertiary));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let words = self.pages.iter().map(|p| count_words(&p.body)).sum::<usize>();
                        ui.label(
                            RichText::new(format!("{} pages · {} words", self.pages.len(), words))
                                .size(10.5)
                                .color(t.text_secondary),
                        );
                    });
                });
            });
    }

    fn hint(&self) -> &'static str {
        if self.show_find {
            "Enter jumps to the next match · Esc closes find"
        } else {
            match self.tab {
                0 => "Select text and style it from the Home ribbon · Ctrl+S saves the notebook",
                1 => "Click a tile to insert at the caret",
                _ => "Toggle panes and craft settings from the View ribbon",
            }
        }
    }

    // ── find bar ────────────────────────────────────────────────────────────

    fn find_bar(&mut self, ctx: &egui::Context, t: &Tokens) {
        egui::Area::new(Id::new("notes_find"))
            .anchor(Align2::CENTER_TOP, [0.0, TITLE_H + TABS_H + RIBBON_H + 8.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::NONE
                    .fill(t.panel)
                    .stroke(theme::hairline(t))
                    .corner_radius(13)
                    .shadow(theme::card_shadow(t))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("FIND").size(9.0).color(t.text_secondary));
                            let resp = ui.add_sized(
                                [200.0, 24.0],
                                TextEdit::singleline(&mut self.find)
                                    .hint_text("Search this page…")
                                    .font(theme::regular(12.0)),
                            );
                            let n = self.matches().len();
                            ui.label(RichText::new(n.to_string()).size(11.0).color(t.text_tertiary));
                            if icon_btn(ui, t, "arrow-up", "Previous match") {
                                self.jump(ui.ctx(), -1);
                            }
                            if icon_btn(ui, t, "arrow-down", "Next match") {
                                self.jump(ui.ctx(), 1);
                            }
                            if icon_btn(ui, t, "close", "Close find") {
                                self.show_find = false;
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.jump(ui.ctx(), 1);
                            }
                            if ui.input(|i| i.key_pressed(Key::Escape)) {
                                self.show_find = false;
                            }
                        });
                    });
            });
    }

    // ── settings window ─────────────────────────────────────────────────────

    fn settings_window(&mut self, ctx: &egui::Context, t: &Tokens) {
        let mut open = self.show_settings;
        egui::Window::new(RichText::new("Notes Settings").strong())
            .open(&mut open)
            .default_width(430.0)
            .default_pos([90.0, 100.0])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                section(ui, t, "Editor");
                egui::Grid::new("notes_set_editor").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                    ui.label(RichText::new("Font size").color(t.text_secondary));
                    ui.add(egui::Slider::new(&mut self.font_size, 11.0..=28.0).suffix(" px"));
                    ui.end_row();
                    ui.label(RichText::new("Body font").color(t.text_secondary));
                    ui.horizontal(|ui| {
                        for (v, name) in [(false, "Proportional"), (true, "Monospace")] {
                            if ui
                                .add(
                                    egui::Button::selectable(self.body_mono == v, RichText::new(name).size(11.5))
                                        .min_size(Vec2::new(84.0, 22.0))
                                        .corner_radius(CornerRadius::same(5)),
                                )
                                .clicked()
                            {
                                self.body_mono = v;
                            }
                        }
                    });
                    ui.end_row();
                });

                section(ui, t, "Accent");
                ui.label(
                    RichText::new("Used for the sidebar highlight, tab underline, heading rules and code chips.")
                        .small()
                        .color(t.text_tertiary),
                );
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    for (i, (name, rgb)) in ACCENTS.iter().enumerate() {
                        let sel = self.accent == i;
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(52.0, 24.0), Sense::click());
                        ui.painter().rect_filled(r.shrink(4.0), 4.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                        if sel {
                            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(2.0, t.text), StrokeKind::Outside);
                        } else if resp.hovered() {
                            ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, t.text_secondary), StrokeKind::Outside);
                        }
                        if resp.on_hover_text(*name).clicked() {
                            self.accent = i;
                        }
                    }
                });

                section(ui, t, "Behavior");
                egui::Grid::new("notes_set_beh").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                    ui.label(RichText::new("Show preview").color(t.text_secondary));
                    ui.checkbox(&mut self.show_preview, "");
                    ui.end_row();
                    ui.label(RichText::new("Show sidebar").color(t.text_secondary));
                    ui.checkbox(&mut self.sidebar_open, "");
                    ui.end_row();
                });

                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings stay for the session (kept on the craft).").small().color(t.text_secondary));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Reset defaults").size(11.5)).corner_radius(CornerRadius::same(5))).clicked() {
                            self.font_size = 15.0;
                            self.body_mono = false;
                            self.accent = 0;
                            self.show_preview = true;
                            self.sidebar_open = true;
                        }
                    });
                });
            });
        self.show_settings = open;
    }

    // ── toast ───────────────────────────────────────────────────────────────

    fn toast_ui(&mut self, ctx: &egui::Context, t: &Tokens) {
        if let Some((msg, at)) = self.toast.clone() {
            if now() - at < 2.5 {
                egui::Area::new(Id::new("notes_toast"))
                    .anchor(Align2::CENTER_BOTTOM, [0.0, 40.0])
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        Frame::NONE
                            .fill(t.menu)
                            .stroke(theme::hairline(t))
                            .corner_radius(13)
                            .shadow(theme::card_shadow(t))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.label(RichText::new(msg).size(12.0).color(t.menu_text));
                            });
                    });
            } else {
                self.toast = None;
            }
        }
    }

    // ── actions ─────────────────────────────────────────────────────────────

    fn new_page(&mut self) {
        self.pages.push(NotePage::new(&format!("Page {}", self.pages.len() + 1)));
        self.active = self.pages.len() - 1;
        self.dirty = true;
    }

    fn duplicate_page(&mut self) {
        let mut p = match self.pages.get(self.active) {
            Some(p) => p.clone(),
            None => return,
        };
        p.title = format!("{} (copy)", p.title);
        self.pages.insert(self.active + 1, p);
        self.active += 1;
        self.dirty = true;
    }

    fn delete_page(&mut self) {
        if self.pages.len() <= 1 {
            let n = self.active + 1;
            self.pages[0] = NotePage::new(&format!("Page {n}"));
            self.active = 0;
        } else {
            self.pages.remove(self.active);
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
        }
        self.dirty = true;
        self.toast = Some(("Page deleted".into(), now()));
    }

    fn do_undo(&mut self) {
        if let Some((idx, body)) = self.undo.pop()
            && let Some(p) = self.pages.get_mut(idx)
        {
            self.redo.push((idx, p.body.clone()));
            p.body = body;
            self.dirty = true;
        }
    }

    fn do_redo(&mut self) {
        if let Some((idx, body)) = self.redo.pop()
            && let Some(p) = self.pages.get_mut(idx)
        {
            self.undo.push((idx, p.body.clone()));
            p.body = body;
            self.dirty = true;
        }
    }

    /// True when the current selection sits inside a `marker … marker` pair.
    fn wrapped_with(&self, marker: &str) -> bool {
        let mlen = marker.chars().count();
        let (a, b) = self.sel;
        if a < mlen || a == b {
            return false;
        }
        let body = self.active_body();
        let ba = char_to_byte(body, a);
        let bb = char_to_byte(body, b);
        body[char_to_byte(body, a - mlen)..ba] == *marker && body[bb..].starts_with(marker)
    }

    /// Wrap (or unwrap) the selection in an inline marker: **…**, *…*, `…`.
    fn wrap_marker(&mut self, ctx: &egui::Context, marker: &str) {
        let mlen = marker.chars().count();
        let (a, b) = self.sel;
        let (a, b) = (a.min(b), a.max(b));
        let Some(p) = self.active_mut() else { return };
        let body = p.body.clone();
        let (ba, bb) = (char_to_byte(&body, a), char_to_byte(&body, b));
        let has = a >= mlen
            && body[char_to_byte(&body, a - mlen)..ba] == *marker
            && body[bb..].starts_with(marker);
        let mut nb = body;
        let newsel = if has {
            nb.replace_range(bb..bb + marker.len(), "");
            nb.replace_range(ba - marker.len()..ba, "");
            (a - mlen, b - mlen)
        } else if a == b {
            nb.insert_str(ba, &format!("{marker}{marker}"));
            (a + mlen, a + mlen)
        } else {
            nb.insert_str(bb, marker);
            nb.insert_str(ba, marker);
            (a + mlen, b + mlen)
        };
        p.body = nb;
        set_body_cursor(ctx, newsel);
        self.dirty = true;
    }

    /// Toggle a line prefix (`- `, `- [ ] `, `> `) across the selected lines.
    fn toggle_line_prefix(&mut self, ctx: &egui::Context, prefix: &str) {
        let (a, b) = self.sel;
        let (a, b) = (a.min(b), a.max(b));
        let body = self.active_body();
        let ba = char_to_byte(body, a);
        let bb = char_to_byte(body, b);
        let ls = body[..ba].rfind('\n').map_or(0, |i| i + 1);
        let le = body[bb..].find('\n').map_or(body.len(), |i| bb + i + 1);
        let all = body[ls..le]
            .split_inclusive('\n')
            .filter(|l| !l.trim().is_empty())
            .all(|l| l.starts_with(prefix));
        if all {
            self.edit_selected_lines(ctx, |l| l.strip_prefix(prefix).unwrap_or(l).to_string());
        } else {
            self.edit_selected_lines(ctx, |l| if l.trim().is_empty() { l.to_string() } else { format!("{prefix}{l}") });
        }
    }

    /// Set (or reset) an ATX heading level on the selected lines.
    fn heading(&mut self, ctx: &egui::Context, level: u8) {
        self.edit_selected_lines(ctx, move |l| {
            let s = l.trim_start_matches('#');
            let s = s.strip_prefix(' ').unwrap_or(s);
            format!("{} {}", "#".repeat(level as usize), s)
        });
    }

    /// Rewrite every line touched by the selection through `f`, keeping the
    /// selection stretched over the rewritten block.
    fn edit_selected_lines<F: Fn(&str) -> String>(&mut self, ctx: &egui::Context, f: F) {
        let (a, b) = self.sel;
        let (a, b) = (a.min(b), a.max(b));
        let Some(p) = self.active_mut() else { return };
        let body = std::mem::take(&mut p.body);
        let ba = char_to_byte(&body, a);
        let bb = char_to_byte(&body, b);
        let ls = body[..ba].rfind('\n').map_or(0, |i| i + 1);
        let le = body[bb..].find('\n').map_or(body.len(), |i| bb + i + 1);
        let mut nb = String::new();
        for l in body[ls..le].split_inclusive('\n') {
            nb.push_str(&f(l));
        }
        let ns = body[..ls].chars().count();
        let ne = ns + nb.chars().count();
        p.body = format!("{}{nb}{}", &body[..ls], &body[le..]);
        set_body_cursor(ctx, (ns, ne));
        self.dirty = true;
    }

    /// Insert `s` at the caret (replacing the selection, if any).
    fn insert_at_cursor(&mut self, ctx: &egui::Context, s: &str) {
        let (a, b) = self.sel;
        let (a, b) = (a.min(b), a.max(b));
        let Some(p) = self.active_mut() else { return };
        let body = std::mem::take(&mut p.body);
        let (ba, bb) = (char_to_byte(&body, a), char_to_byte(&body, b));
        let mut nb = body;
        nb.replace_range(ba..bb, s);
        p.body = nb;
        let na = a + s.chars().count();
        set_body_cursor(ctx, (na, na));
        self.dirty = true;
    }

    /// Strip markdown formatting from the selection (or the whole page).
    fn clear_formatting(&mut self, ctx: &egui::Context) {
        let (a, b) = self.sel;
        let (a, b) = (a.min(b), a.max(b));
        let Some(p) = self.active_mut() else { return };
        let body = p.body.clone();
        let whole = a == b;
        let (ba, bb) = if whole { (0, body.len()) } else { (char_to_byte(&body, a), char_to_byte(&body, b)) };
        let cleaned = strip_md(&body[ba..bb]);
        p.body.replace_range(ba..bb, &cleaned);
        set_body_cursor(ctx, (a, a + cleaned.chars().count()));
        self.dirty = true;
    }

    fn matches(&self) -> Vec<usize> {
        let q = self.find.to_lowercase();
        let mut out = Vec::new();
        if q.is_empty() {
            return out;
        }
        let hay = self.active_body().to_lowercase();
        let qch = q.chars().count();
        let hc: Vec<char> = hay.chars().collect();
        let mut i = 0;
        while i + qch <= hc.len() {
            if hc[i..i + qch].iter().collect::<String>() == q {
                out.push(i);
                i += qch;
            } else {
                i += 1;
            }
        }
        out
    }

    fn jump(&mut self, ctx: &egui::Context, dir: i32) {
        let ms = self.matches();
        if ms.is_empty() {
            self.toast = Some(("No matches".into(), now()));
            return;
        }
        let n = ms.len() as i64;
        self.find_idx = (self.find_idx as i64 + dir as i64).rem_euclid(n) as usize;
        let start = ms[self.find_idx.min(ms.len() - 1)];
        let len = self.find.chars().count();
        set_body_cursor(ctx, (start, start + len));
        self.goto = Some(start);
    }
}

// ── chrome helpers (painted from tokens) ─────────────────────────────────────

/// One 24×24 QAT button painting a shared vector icon (no font → no tofu).
fn qat_btn_icon(ui: &mut Ui, icon: &str, tip: &str, t: &Tokens) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
    }
    theme::icons::paint(ui.painter(), r, icon, t.icon, t.panel);
    resp.on_hover_text(tip).clicked()
}

/// A ribbon group: content, a small uppercase caption below, divider right.
fn group(ui: &mut Ui, t: &Tokens, title: &str, add: impl FnOnce(&mut Ui)) {
    let start = ui.cursor().min;
    let inner = ui.vertical(|ui| {
        ui.set_min_width(10.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), CONTENT_H),
            egui::Layout::left_to_right(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 1.0);
                add(ui);
            },
        );
    });
    let r = Rect::from_min_max(start, pos2(inner.response.rect.max.x, start.y + CONTENT_H + LABEL_H));
    ui.painter().text(
        pos2(r.center().x, r.max.y - LABEL_H / 2.0),
        Align2::CENTER_CENTER,
        title.to_uppercase(),
        FontId::proportional(9.0),
        t.text_secondary,
    );
    ui.add_space(5.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment(
        [pos2(x, start.y + 3.0), pos2(x, start.y + CONTENT_H + LABEL_H - 3.0)],
        Stroke::new(1.0, t.border),
    );
    ui.add_space(6.0);
}

fn stack(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(1.0, 3.0);
        add(ui);
    });
}

fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);
        add(ui);
    });
}

/// A large ribbon tile: 44×52 field, glyph over a (possibly two-line) caption.
fn tile(ui: &mut Ui, t: &Tokens, glyph: &str, label: &str, tip: &str, on: bool, accent: Color32) -> bool {
    let lines: Vec<&str> = label.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), FontId::proportional(9.5), t.text_secondary)).size().x)
        .fold(44.0_f32, f32::max)
        + 12.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, CONTENT_H), Sense::click());
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(5), accent.gamma_multiply(0.22));
        ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, accent), StrokeKind::Inside);
    } else {
        ui.painter().rect_filled(r, CornerRadius::same(5), if resp.hovered() { t.hover } else { t.field });
    }
    ui.painter().text(pos2(r.center().x, r.min.y + 17.0), Align2::CENTER_CENTER, glyph, FontId::proportional(16.0), if on { accent } else { t.icon });
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(
            pos2(r.center().x, r.min.y + 33.0 + i as f32 * 11.0),
            Align2::CENTER_CENTER,
            *l,
            FontId::proportional(9.0),
            t.text_secondary,
        );
    }
    resp.on_hover_text(tip).clicked()
}

/// A large ribbon tile painting a shared vector icon over its caption
/// (no font → no tofu). Same geometry as [`tile`].
#[allow(clippy::too_many_arguments)]
fn tile_icon(ui: &mut Ui, t: &Tokens, icon: &str, label: &str, tip: &str, on: bool, accent: Color32) -> bool {
    let lines: Vec<&str> = label.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap((*l).to_string(), FontId::proportional(9.5), t.text_secondary)).size().x)
        .fold(44.0_f32, f32::max)
        + 12.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, CONTENT_H), Sense::click());
    let fill = if on { accent.gamma_multiply(0.22) } else if resp.hovered() { t.hover } else { t.field };
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(5), fill);
        ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, accent), StrokeKind::Inside);
    } else {
        ui.painter().rect_filled(r, CornerRadius::same(5), fill);
    }
    let ink = if on { accent } else { t.icon };
    theme::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 17.0), Vec2::splat(20.0)), icon, ink, fill);
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(
            pos2(r.center().x, r.min.y + 33.0 + i as f32 * 11.0),
            Align2::CENTER_CENTER,
            *l,
            FontId::proportional(9.0),
            t.text_secondary,
        );
    }
    resp.on_hover_text(tip).clicked()
}

/// A small 26×22 button painting a shared vector icon (no font → no tofu).
fn small_btn_icon(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str, enabled: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(26.0, 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
    }
    theme::icons::paint(ui.painter(), r, icon, if enabled { t.icon } else { t.icon_secondary }, t.panel);
    resp.on_hover_text(tip).clicked()
}

/// A tiny frameless icon button (find bar arrows / close).
fn icon_btn(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0, 20.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
    }
    theme::icons::paint(ui.painter(), r, icon, t.icon, t.panel);
    resp.on_hover_text(tip).clicked()
}

/// A read-only page-stat tile for the View ribbon's PAGE group.
fn page_stat(ui: &mut Ui, t: &Tokens, words: usize) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, CONTENT_H), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(5), t.field);
    ui.painter().text(pos2(r.center().x, r.min.y + 20.0), Align2::CENTER_CENTER, words.to_string(), theme::semibold(15.0), t.text);
    ui.painter().text(pos2(r.center().x, r.min.y + 37.0), Align2::CENTER_CENTER, "words", FontId::proportional(9.0), t.text_secondary);
}

fn section(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title).strong().size(14.0).color(t.text));
    ui.add_space(2.0);
}

// ── small helpers ────────────────────────────────────────────────────────────

/// Place the body TextEdit's cursor/selection (char indices) for next frame.
fn set_body_cursor(ctx: &egui::Context, sel: (usize, usize)) {
    if let Some(mut st) = TextEdit::load_state(ctx, body_id()) {
        let (a, b) = sel;
        st.cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(a.min(b)), CCursor::new(a.max(b)))));
        st.store(ctx, body_id());
    }
}

fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

fn line_pitch(ctx: &egui::Context, size: f32, mono: bool) -> f32 {
    let fid = if mono { FontId::monospace(size) } else { FontId::proportional(size) };
    ctx.fonts_mut(|f| f.layout_no_wrap("Ag\nAg".to_string(), fid, Color32::WHITE).size().y / 2.0)
        .max(size * 1.2)
}

fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

fn ellip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}

fn relative_date(created: &str) -> String {
    if created.is_empty() {
        return "undated".into();
    }
    let parts: Vec<u32> = created.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 3 {
        let today_days = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_secs() / 86400) as i64)
            .unwrap_or(0);
        let diff = today_days - days_from_civil(parts[0] as i64, parts[1], parts[2]);
        match diff {
            0 => "today".into(),
            1 => "yesterday".into(),
            2..=13 => format!("{diff} days ago"),
            _ => created.to_string(),
        }
    } else {
        created.to_string()
    }
}

/// Howard Hinnant's days_from_civil (proleptic Gregorian, days since epoch).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Remove markdown formatting markers (headings, quotes, inline emphasis/code).
fn strip_md(s: &str) -> String {
    let mut out = String::new();
    for line in s.split_inclusive('\n') {
        let t = line.trim_start();
        let stripped = if let Some(r) = t.strip_prefix("#### ") {
            r
        } else if let Some(r) = t.strip_prefix("### ") {
            r
        } else if let Some(r) = t.strip_prefix("## ") {
            r
        } else if let Some(r) = t.strip_prefix("# ") {
            r
        } else if let Some(r) = t.strip_prefix("> ") {
            r
        } else {
            line
        };
        out.push_str(&stripped.replace("**", "").replace("~~", "").replace('`', ""));
    }
    out
}

// ── markdown-lite preview ────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Block {
    H(u8, String),
    P(String),
    Bullet(Vec<String>),
    Numbered(Vec<String>),
    Task(Vec<(bool, String)>),
    Quote(Vec<String>),
    Code(String, String), // lang, code
    Rule,
}

fn parse_blocks(src: &str) -> Vec<Block> {
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("```") {
            let lang = t[3..].trim().to_string();
            i += 1;
            let mut code = String::new();
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(lines[i]);
                i += 1;
            }
            i += 1; // closing fence
            out.push(Block::Code(lang, code));
        } else if t == "---" || t == "***" || t == "___" {
            out.push(Block::Rule);
            i += 1;
        } else if let Some(rest) = t.strip_prefix("#### ") {
            out.push(Block::H(4, rest.trim_end().into()));
            i += 1;
        } else if let Some(rest) = t.strip_prefix("### ") {
            out.push(Block::H(3, rest.trim_end().into()));
            i += 1;
        } else if let Some(rest) = t.strip_prefix("## ") {
            out.push(Block::H(2, rest.trim_end().into()));
            i += 1;
        } else if let Some(rest) = t.strip_prefix("# ") {
            out.push(Block::H(1, rest.trim_end().into()));
            i += 1;
        } else if t.starts_with("- [ ] ") || t.starts_with("- [x] ") || t.starts_with("- [X] ") {
            let mut items = Vec::new();
            while i < lines.len() {
                let lt = lines[i].trim_start();
                if let Some(r) = lt.strip_prefix("- [x] ").or_else(|| lt.strip_prefix("- [X] ")) {
                    items.push((true, r.into()));
                } else if let Some(r) = lt.strip_prefix("- [ ] ") {
                    items.push((false, r.into()));
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::Task(items));
        } else if t.starts_with("- ") || t.starts_with("* ") {
            let mut items = Vec::new();
            while i < lines.len() {
                let lt = lines[i].trim_start();
                if let Some(r) = lt.strip_prefix("- ").or_else(|| lt.strip_prefix("* ")) {
                    if r.starts_with("[ ] ") || r.starts_with("[x] ") || r.starts_with("[X] ") {
                        break; // tasks are handled above
                    }
                    items.push(r.trim_end().into());
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::Bullet(items));
        } else if numbered_start(t) {
            let mut items = Vec::new();
            while i < lines.len() {
                if let Some(r) = strip_number(lines[i].trim_start()) {
                    items.push(r.trim_end().into());
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::Numbered(items));
        } else if t.starts_with('>') {
            let mut paras = Vec::new();
            while i < lines.len() {
                let lt = lines[i].trim_start();
                if let Some(inner) = lt.strip_prefix('>') {
                    let inner = inner.strip_prefix(' ').unwrap_or(inner);
                    paras.push(inner.to_string());
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::Quote(paras));
        } else if t.is_empty() {
            i += 1;
        } else {
            out.push(Block::P(t.trim_end().into()));
            i += 1;
        }
    }
    out
}

fn numbered_start(l: &str) -> bool {
    strip_number(l).is_some()
}

fn strip_number(l: &str) -> Option<&str> {
    let d = l.find('.')?;
    let n = &l[..d];
    if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
        l[d + 1..].strip_prefix(' ')
    } else {
        None
    }
}

/// One inline-formatting state for the span walker.
#[derive(Clone, Copy, Default, PartialEq)]
struct St {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
}

/// Flat inline spans: **bold**, *italic*, ~~strike~~, `code`, [label](url).
fn spans(s: &str, base: St) -> Vec<(String, St)> {
    let mut out: Vec<(String, St)> = Vec::new();
    let mut cur = String::new();
    let mut st = base;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    macro_rules! flush {
        () => {
            if !cur.is_empty() {
                out.push((std::mem::take(&mut cur), st));
            }
        };
    }
    while i < chars.len() {
        let two: String = chars[i..].iter().take(2).collect();
        if two == "**" {
            flush!();
            st.bold = !st.bold;
            i += 2;
        } else if two == "~~" {
            flush!();
            st.strike = !st.strike;
            i += 2;
        } else if chars[i] == '`' {
            let rest: String = chars[i + 1..].iter().collect();
            if let Some(end) = rest.find('`') {
                flush!();
                out.push((rest[..end].to_string(), St { code: true, ..st }));
                i += 1 + rest[..end].chars().count() + 1;
            } else {
                cur.push('`');
                i += 1;
            }
        } else if chars[i] == '*' {
            flush!();
            st.italic = !st.italic;
            i += 1;
        } else if chars[i] == '[' {
            let rest: String = chars[i..].iter().collect();
            if let Some(close) = rest.find(']')
                && rest[close + 1..].starts_with('(')
                && let Some(pend) = rest[close + 2..].find(')')
            {
                flush!();
                out.push((rest[1..close].to_string(), St { link: true, ..st }));
                i += close + 2 + pend + 1;
            } else {
                cur.push('[');
                i += 1;
            }
        } else {
            cur.push(chars[i]);
            i += 1;
        }
    }
    flush!();
    out
}

fn push_spans(job: &mut egui::text::LayoutJob, sp: &[(String, St)], size: f32, color: Color32, mono_body: bool, t: &Tokens, accent: Color32) {
    for (txt, st) in sp {
        let font = if st.code {
            FontId::monospace(size - 1.0)
        } else if mono_body {
            FontId::monospace(size)
        } else {
            FontId::proportional(if st.bold { size + 0.5 } else { size })
        };
        let f = TextFormat {
            font_id: font,
            color: if st.link {
                t.accent_text
            } else if st.bold {
                t.text
            } else {
                color
            },
            italics: st.italic,
            underline: if st.link { Stroke::new(1.0, t.accent_text) } else { Stroke::NONE },
            strikethrough: if st.strike { Stroke::new(1.0, color) } else { Stroke::NONE },
            background: if st.code { accent.gamma_multiply(0.15) } else { Color32::TRANSPARENT },
            ..Default::default()
        };
        job.append(txt, 0.0, f);
    }
}

fn job_for(ui: &Ui, text: &str, base: St, size: f32, color: Color32, mono_body: bool, t: &Tokens, accent: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let sp = spans(text, base);
    push_spans(&mut job, &sp, size, color, mono_body, t, accent);
    job.wrap.max_width = ui.available_width();
    job
}

fn job_label(ui: &mut Ui, job: egui::text::LayoutJob) {
    ui.add(egui::Label::new(WidgetText::LayoutJob(Arc::new(job))));
}

/// The styled live preview, drawn inside the settings-accented page card.
fn preview_markdown(ui: &mut Ui, src: &str, size: f32, mono_body: bool, t: &Tokens, accent: Color32) {
    let body_col = t.text.gamma_multiply(0.9);
    if parse_blocks(src).is_empty() && src.trim().is_empty() {
        ui.label(RichText::new("Nothing on this page yet.").size(size).color(t.text_tertiary));
        return;
    }
    for b in parse_blocks(src) {
        match b {
            Block::H(level, text) => {
                let s = match level {
                    1 => size + 7.0,
                    2 => size + 4.0,
                    _ => size + 2.0,
                };
                ui.add_space(6.0);
                let job = job_for(ui, &text, St::default(), s, t.text, mono_body, t, accent);
                job_label(ui, job);
                ui.add_space(2.0);
                let (alpha, h) = match level {
                    1 => (1.0_f32, 2.0_f32),
                    2 => (0.7, 1.5),
                    _ => (0.45, 1.0),
                };
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
                ui.painter().rect_filled(r, h / 2.0, accent.gamma_multiply(alpha));
            }
            Block::P(text) => {
                ui.add_space(2.0);
                let job = job_for(ui, &text, St::default(), size, body_col, mono_body, t, accent);
                job_label(ui, job);
            }
            Block::Bullet(items) => {
                ui.add_space(2.0);
                for item in items {
                    list_row(ui, "•", t.text_tertiary, &item, St::default(), size, body_col, mono_body, t, accent);
                }
            }
            Block::Numbered(items) => {
                ui.add_space(2.0);
                for (n, item) in items.iter().enumerate() {
                    list_row(ui, &format!("{}.", n + 1), t.text_tertiary, item, St::default(), size, body_col, mono_body, t, accent);
                }
            }
            Block::Task(items) => {
                ui.add_space(2.0);
                for (done, item) in items {
                    task_row(ui, done, &item, size, mono_body, t, accent);
                }
            }
            Block::Quote(paras) => {
                ui.add_space(3.0);
                let start = ui.cursor().min;
                let max = ui.max_rect();
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("notes_quote")
                        .max_rect(Rect::from_min_max(pos2(start.x + 12.0, start.y), pos2(max.right(), max.bottom()))),
                );
                for p in &paras {
                    let job = job_for(&child, p, St { italic: true, ..St::default() }, size - 0.5, t.text_secondary, mono_body, t, accent);
                    job_label(&mut child, job);
                }
                let r = child.min_rect();
                let bar = Rect::from_min_size(pos2(start.x, r.top() + 1.0), Vec2::new(3.0, (r.height() - 2.0).max(4.0)));
                ui.painter().rect_filled(bar, 2.0, accent.gamma_multiply(0.55));
                ui.advance_cursor_after_rect(r);
            }
            Block::Code(lang, code) => {
                ui.add_space(4.0);
                Frame::NONE
                    .fill(t.field)
                    .stroke(theme::hairline(t))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        if !lang.is_empty() {
                            ui.label(RichText::new(lang).size(9.0).color(t.text_tertiary));
                            ui.add_space(2.0);
                        }
                        let mut job = egui::text::LayoutJob::default();
                        job.append(&code, 0.0, TextFormat::simple(FontId::monospace(size - 1.5), t.text));
                        job.wrap.max_width = ui.available_width();
                        job_label(ui, job);
                    });
            }
            Block::Rule => {
                ui.add_space(5.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.border_strong));
                ui.add_space(3.0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn list_row(
    ui: &mut Ui,
    bullet: &str,
    bcol: Color32,
    text: &str,
    base: St,
    size: f32,
    color: Color32,
    mono_body: bool,
    t: &Tokens,
    accent: Color32,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
        ui.label(RichText::new(bullet).size(size).color(bcol));
        let job = job_for(ui, text, base, size, color, mono_body, t, accent);
        job_label(ui, job);
    });
}

fn task_row(ui: &mut Ui, done: bool, text: &str, size: f32, mono_body: bool, t: &Tokens, accent: Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(7.0, 0.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(13.0, 13.0), Sense::hover());
        let rr = r.shrink2(Vec2::new(1.5, 0.0));
        if done {
            ui.painter().rect_filled(rr, 3.0, accent);
            ui.painter().text(rr.center(), Align2::CENTER_CENTER, "✓", FontId::proportional(9.5), Color32::WHITE);
        } else {
            ui.painter().rect_stroke(rr, 3.0, Stroke::new(1.2, t.border_strong), StrokeKind::Inside);
        }
        let col = if done { t.text_tertiary } else { t.text.gamma_multiply(0.9) };
        let sp: Vec<(String, St)> = spans(text, St::default())
            .into_iter()
            .map(|(s, mut st)| {
                if done {
                    st.strike = true;
                }
                (s, st)
            })
            .collect();
        let mut job = egui::text::LayoutJob::default();
        push_spans(&mut job, &sp, size, col, mono_body, t, accent);
        job.wrap.max_width = ui.available_width();
        job_label(ui, job);
    });
}
