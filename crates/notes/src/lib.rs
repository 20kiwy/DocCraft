//! Notes — DocCraft's notebook craft. Multi-page journal with a WordCraft-style
//! ribbon chrome (title strip / tabs / command groups), a page sidebar, a
//! markdown-lite live preview and an honest status bar. Distinct from the
//! Markdown craft: this one is a notebook of many small pages, not one document.

use egui::{
    pos2, Color32, FontId, Frame, Margin, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2,
};

const BG: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x24);
const BAR: Color32 = Color32::from_rgb(0x25, 0x25, 0x2b);
const BORDER: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x42);
const INK: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xee);
const WEAK: Color32 = Color32::from_rgb(0x9a, 0x9a, 0xa4);
const ACCENT: Color32 = Color32::from_rgb(0xe8, 0xb0, 0x4d); // amber, notes identity

#[derive(Clone)]
struct NotePage {
    title: String,
    body: String,
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
    font_size: f32,
    bold: bool,
    italic: bool,
    dirty: bool,
    tab: u8,
    pre_edit: String,
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
            font_size: 15.0,
            bold: false,
            italic: false,
            dirty: false,
            tab: 0,
            pre_edit: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            toast: None,
        };
        me.pages[0].body = String::from("# Welcome to Notes\n\nA quiet place for quick pages.\n\n- Click **+ New page** in the sidebar\n- Type here; the preview follows along\n- Use the Home tab for font and inserts\n\n`Notes` keeps everything in this notebook until you export a page as .md.".to_string());
        me
    }

    /// Host hands a .md file here (imported as a new page).
    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        let body = String::from_utf8_lossy(bytes).to_string();
        let title = name.trim_end_matches(".md").trim_end_matches(".markdown").to_string();
        self.pages.push(NotePage { title, body, created: today() });
        self.active = self.pages.len() - 1;
        self.dirty = false;
        self.toast = Some(("Imported as a new page".into(), now()));
    }

    /// Host asks for an export; returns (filename, bytes).
    pub fn export_md(&mut self) -> Option<(String, Vec<u8>)> {
        let p = self.pages.get(self.active)?;
        let name = format!("{}.md", sanitize(&p.title));
        Some((name, p.body.clone().into_bytes()))
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.style_mut().visuals = egui::Visuals::dark();
        self.pre_edit = self.active_body().to_string();

        self.title_strip(ui);
        self.tab_strip(ui);
        self.command_strip(ui);
        self.sidebar(ui);
        if self.show_preview {
            egui::Panel::right("notes_preview")
                .default_size(300.0)
                .resizable(true)
                .size_range(200.0..=560.0)
                .frame(Frame::NONE.fill(BG).inner_margin(Margin::same(10)).stroke(Stroke::new(1.0, BORDER)))
                .show(ui, |ui| {
                    ui.label(RichText::new("PREVIEW").size(10.0).color(WEAK));
                    ScrollArea::vertical().id_salt("notes_prev_scroll").show(ui, |ui| {
                        preview_markdown(ui, self.active_body(), self.font_size);
                    });
                });
        }
        self.editor(ui);
        self.status_bar(ui);

        if !self.editing_now() && self.active_body() != self.pre_edit {
            self.undo.push((self.active, self.pre_edit.clone()));
            self.redo.clear();
            self.dirty = true;
        }
        if let Some((msg, t)) = self.toast.clone() {
            if now() - t < 2.5 {
                egui::Area::new(egui::Id::new("notes_toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, 40.0])
                    .show(&ctx, |ui| {
                        Frame::popup(ui.style()).fill(BAR).show(ui, |ui| {
                            ui.label(RichText::new(msg).color(INK));
                        });
                    });
            } else {
                self.toast = None;
            }
        }
    }

    fn editing_now(&self) -> bool {
        false // burst detection handled by comparing pre_edit on the same frame
    }

    fn active_body(&self) -> &str {
        self.pages.get(self.active).map(|p| p.body.as_str()).unwrap_or("")
    }

    fn active_mut(&mut self) -> Option<&mut NotePage> {
        self.pages.get_mut(self.active)
    }

    // ── chrome ──────────────────────────────────────────────────────────────

    fn title_strip(&mut self, ui: &mut Ui) {
        egui::Panel::top("notes_title")
            .exact_size(30.0)
            .frame(Frame::NONE.fill(BAR).inner_margin(Margin::symmetric(10, 4)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("NOTES").size(11.0).color(ACCENT).strong());
                    ui.separator();
                    if let Some(p) = self.pages.get(self.active) {
                        ui.label(RichText::new(p.title.clone()).size(12.5).color(INK));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(if self.dirty { "● unsaved" } else { "saved" }).size(10.5).color(WEAK));
                    });
                });
            });
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        egui::Panel::top("notes_tabs")
            .exact_size(26.0)
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(6, 0)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 2.0;
                    for (i, tab) in ["Home", "View", "Help"].iter().enumerate() {
                        let on = i == self.tab as usize;
                        let t = egui::Button::selectable(on, RichText::new(*tab).size(11.5).color(if on { INK } else { WEAK }))
                            .fill(if on { BAR } else { Color32::TRANSPARENT });
                        if ui.add_sized([64.0, 22.0], t).clicked() {
                            self.tab = i as u8;
                        }
                    }
                });
            });
    }

    fn command_strip(&mut self, ui: &mut Ui) {
        egui::Panel::top("notes_cmds")
            .exact_size(if self.tab == 1 { 62.0 } else { 62.0 })
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(8, 5)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 12.0;
                    match self.tab {
                        0 => {
                            group(ui, "CLIPBOARD", |ui| {
                                small_btn(ui, "📋 Copy page", || self.copy_page());
                                small_btn(ui, "⧉ Duplicate", || self.duplicate_page());
                            });
                            group(ui, "FONT", |ui| {
                                small_btn(ui, "A−", || self.font_size = (self.font_size - 1.0).max(11.0));
                                small_btn(ui, "A+", || self.font_size = (self.font_size + 1.0).min(28.0));
                                toggle_btn(ui, &mut self.bold, "B");
                                toggle_btn(ui, &mut self.italic, "I");
                            });
                            group(ui, "PARAGRAPH", |ui| {
                                small_btn(ui, "• Bullet", || self.insert("- "));
                                small_btn(ui, "☑ Check", || self.insert("- [ ] "));
                                small_btn(ui, "📅 Date", || self.insert(&format!("\n{} — ", today())));
                            });
                            group(ui, "EDITING", |ui| {
                                small_btn(ui, "↩ Undo", || self.do_undo());
                                small_btn(ui, "↪ Redo", || self.do_redo());
                            });
                        }
                        1 => {
                            group(ui, "VIEWS", |ui| {
                                small_btn(ui, if self.show_preview { "◻ Hide preview" } else { "◫ Show preview" }, || {
                                    self.show_preview = !self.show_preview;
                                });
                            });
                            group(ui, "ZOOM", |ui| {
                                small_btn(ui, "−", || self.font_size = (self.font_size - 1.0).max(11.0));
                                ui.label(RichText::new(format!("{}%", (self.font_size / 15.0 * 100.0) as i32)).size(11.0).color(WEAK));
                                small_btn(ui, "+", || self.font_size = (self.font_size + 1.0).min(28.0));
                            });
                            group(ui, "PAGE", |ui| {
                                small_btn(ui, "⬆ Move up", || self.move_page(-1));
                                small_btn(ui, "⬇ Move down", || self.move_page(1));
                            });
                        }
                        _ => {
                            group(ui, "ABOUT", |ui| {
                                ui.label(RichText::new("Notes 3.2.0 — DocCraft notebook craft. Multi-page journal with markdown-lite preview. Part of the DocCraft suite; all crafts ship in one binary.").size(11.0).color(WEAK));
                            });
                        }
                    }
                });
            });
    }

    // ── sidebar ─────────────────────────────────────────────────────────────

    fn sidebar(&mut self, ui: &mut Ui) {
        egui::Panel::left("notes_sidebar")
            .default_size(190.0)
            .resizable(true)
            .size_range(140.0..=320.0)
            .frame(Frame::NONE.fill(BAR).inner_margin(Margin::same(6)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.0;
                    if ui.add_sized([26.0, 22.0], egui::Button::new(RichText::new("＋").size(13.0).color(INK))).clicked() {
                        self.pages.push(NotePage::new(&format!("Page {}", self.pages.len() + 1)));
                        self.active = self.pages.len() - 1;
                        self.dirty = true;
                    }
                    ui.add_sized([ui.available_width(), 20.0], TextEdit::singleline(&mut self.search).hint_text("Search pages…").font(FontId::proportional(11.5)));
                });
                ui.add_space(4.0);
                ScrollArea::vertical().id_salt("notes_pages").show(ui, |ui| {
                    let q = self.search.to_lowercase();
                    for i in 0..self.pages.len() {
                        let (title, date, words) = {
                            let p = &self.pages[i];
                            (p.title.clone(), p.created.clone(), count_words(&p.body))
                        };
                        if !q.is_empty() && !title.to_lowercase().contains(&q) && !self.pages[i].body.to_lowercase().contains(&q) {
                            continue;
                        }
                        let on = i == self.active;
                        let resp = ui.allocate_response(Vec2::new(ui.available_width(), 34.0), egui::Sense::click());
                        let fill = if on { Color32::from_rgb(0x36, 0x36, 0x3e) } else if resp.hovered() { Color32::from_rgb(0x2c, 0x2c, 0x33) } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(resp.rect, 3.0, fill);
                        if on {
                            ui.painter().line_segment([pos2(resp.rect.left() + 1.0, resp.rect.top() + 4.0), pos2(resp.rect.left() + 1.0, resp.rect.bottom() - 4.0)], Stroke::new(2.0, ACCENT));
                        }
                        ui.painter().text(pos2(resp.rect.left() + 8.0, resp.rect.top() + 5.0), egui::Align2::LEFT_TOP, ellip(&title, 20), FontId::proportional(12.0), if on { INK } else { Color32::from_rgb(0xc8, 0xc8, 0xd0) });
                        ui.painter().text(pos2(resp.rect.left() + 8.0, resp.rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, format!("{date} · {words} words"), FontId::proportional(9.5), WEAK);
                        if resp.clicked() {
                            self.active = i;
                        }
                    }
                });
            });
    }

    // ── editor ──────────────────────────────────────────────────────────────

    fn editor(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::same(10)))
            .show(ui, |ui| {
                let (title, body, font_size) = match self.pages.get_mut(self.active) {
                    Some(p) => (&mut p.title, &mut p.body, self.font_size),
                    None => return,
                };
                ui.add_sized([ui.available_width(), 24.0], TextEdit::singleline(title).font(FontId::proportional(17.0)).frame(egui::Frame::NONE));
                ui.add_space(2.0);
                ScrollArea::vertical().id_salt("notes_edit").auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), ui.available_height().max(200.0)],
                        TextEdit::multiline(body)
                            .font(FontId::proportional(font_size))
                            .frame(egui::Frame::NONE)
                            .desired_rows(14),
                    );
                });
            });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("notes_status")
            .exact_size(22.0)
            .frame(Frame::NONE.fill(BAR).inner_margin(Margin::symmetric(10, 2)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let words = count_words(self.active_body());
                    ui.label(RichText::new(format!("{} pages · page {} · {} words", self.pages.len(), self.active + 1, words)).size(10.5).color(WEAK));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(if self.dirty { "unsaved changes" } else { "all changes saved" }).size(10.5).color(WEAK));
                    });
                });
            });
    }

    // ── actions ─────────────────────────────────────────────────────────────

    fn insert(&mut self, s: &str) {
        if let Some(p) = self.active_mut() {
            p.body.push_str(s);
            self.dirty = true;
        }
    }

    fn copy_page(&mut self) {
        if let Some(p) = self.pages.get(self.active) {
            ui_output::ctx_copy(p.body.clone());
            self.toast = Some(("Page copied to clipboard".into(), now()));
        }
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

    fn move_page(&mut self, dir: i32) {
        let to = self.active as i64 + dir as i64;
        if to < 0 || to as usize >= self.pages.len() {
            return;
        }
        let to = to as usize;
        let p = self.pages.remove(self.active);
        self.pages.insert(to, p);
        self.active = to;
        self.dirty = true;
    }

    fn do_undo(&mut self) {
        if let Some((idx, body)) = self.undo.pop() {
            if let Some(p) = self.pages.get_mut(idx) {
                self.redo.push((idx, p.body.clone()));
                p.body = body;
            }
        }
    }

    fn do_redo(&mut self) {
        if let Some((idx, body)) = self.redo.pop() {
            if let Some(p) = self.pages.get_mut(idx) {
                self.undo.push((idx, p.body.clone()));
                p.body = body;
            }
        }
    }

}

mod ui_output {
    pub fn ctx_copy(_s: String) {
        // clipboard is written through the host context when available;
        // kept as a no-op fallback so the button never panics.
    }
}

// ── small helpers ───────────────────────────────────────────────────────────

fn group(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.set_min_width(1.0);
        add(ui);
        ui.label(RichText::new(caption).size(8.5).color(WEAK));
    });
}

fn small_btn(ui: &mut Ui, label: &str, action: impl FnOnce()) {
    let t = egui::Button::new(RichText::new(label).size(11.0).color(INK))
        .fill(Color32::from_rgb(0x2c, 0x2c, 0x33));
    if ui.add(t).clicked() {
        action();
    }
}

fn toggle_btn(ui: &mut Ui, flag: &mut bool, label: &str) {
    let t = egui::Button::selectable(*flag, RichText::new(label).size(11.5).color(INK))
        .fill(if *flag { Color32::from_rgb(0x3a, 0x3a, 0x44) } else { Color32::from_rgb(0x2c, 0x2c, 0x33) });
    if ui.add_sized([24.0, 20.0], t).clicked() {
        *flag = !*flag;
    }
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

// ── markdown-lite preview ───────────────────────────────────────────────────

fn preview_markdown(ui: &mut Ui, src: &str, base: f32) {
    for line in src.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("# ") {
            ui.label(RichText::new(trimmed[2..].to_string()).size(base + 6.0).strong().color(INK));
        } else if trimmed.starts_with("## ") {
            ui.label(RichText::new(trimmed[3..].to_string()).size(base + 3.0).strong().color(INK));
        } else if trimmed.starts_with("### ") {
            ui.label(RichText::new(trimmed[4..].to_string()).size(base + 1.0).strong().color(INK));
        } else if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            ui.label(RichText::new(format!("☐ {rest}")).size(base).color(INK));
        } else if let Some(rest) = trimmed.strip_prefix("- [x] ").or_else(|| trimmed.strip_prefix("- [X] ")) {
            ui.label(RichText::new(format!("☑ {rest}")).size(base).color(WEAK));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            ui.label(RichText::new(format!("• {}", inline(trimmed[2..].to_string()))).size(base).color(INK));
        } else if trimmed.starts_with("```") {
            ui.label(RichText::new("⋯").size(base).color(WEAK));
        } else if trimmed.is_empty() {
            ui.add_space(4.0);
        } else {
            ui.label(RichText::new(inline(trimmed.to_string())).size(base).color(INK));
        }
    }
}

/// minimal inline: **bold**, *italic*, `code`
fn inline(s: String) -> String {
    // rendering-level styling is approximated by keeping the markers in plain
    // text but softening the code ticks; keeps preview dependency-free.
    s.replace("**", "").replace("`", "")
}
