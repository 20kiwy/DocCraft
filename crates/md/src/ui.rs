//! The Markdown craft app: WordCraft-anatomy chrome (title strip + QAT, tab
//! strip, grouped command strip, right preview/outline pane, status bar)
//! around a cursor-aware Markdown editor with live preview, settings and
//! save/autosave.

use crate::parse;
use crate::preview;
use crate::ribbon;
use crate::settings::{self, PreviewPos, Settings};
use egui::{Align2, Color32, FontId, Id, RichText, Stroke, Ui, Vec2};

use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RibbonTab {
    #[default]
    Home,
    Insert,
    View,
}

fn scroll_id() -> Id {
    Id::new("md_scroll")
}

fn edit_id() -> Id {
    Id::new("md_edit")
}
const GUTTER_W: f32 = 40.0;

pub struct MdApp {
    pub source: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub tab: RibbonTab,
    pub settings: Settings,
    pub show_settings: bool,
    pub outline_open: bool,
    pub show_find: bool,
    pub find: String,
    pub replace: String,
    pub clipboard: String,
    /// caret as (line, column), 1-based, for the status bar
    pub cursor: (usize, usize),
    pub cursor_char: usize,
    pub sel_len: usize,
    /// pending caret jump (character offset): outline clicks, find hits
    pub goto: Option<usize>,
    pub status: String,
    status_at: Option<Instant>,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    /// set while a ribbon command edited the source (its undo is pushed there)
    editing: bool,
    pre_edit: String,
    last_edit: Option<Instant>,
    last_autosave: Instant,
    /// measured editor/preview scroll geometry for sync scroll
    ed_frac: f32,
    prev_content_h: f32,
    prev_view_h: f32,
}

impl MdApp {
    pub fn new() -> Self {
        let source = "# Welcome to DocCraft Markdown\n\nA **native** Markdown craft, in the same suite as *WordCraft*, *GridCraft*, *DeckCraft* and *PdfCraft*.\n\n- Type on the left, the preview updates live\n- `code`, **bold**, *italic*, ~~strike~~ and [links](https://github.com/storytold) all render\n- Tasks too:\n\n- [x] WordCraft-anatomy ribbon\n- [ ] Write something great\n\n| Craft | Opens |\n| --- | --- |\n| WordCraft | .docx |\n| GridCraft | .xlsx |\n| DeckCraft | .pptx |\n| PdfCraft | .pdf |\n| Draw | .drawio |\n| Markdown | .md |\n| Typeset | .typ |\n\n> Everything edits the same document model — no images, no fakes.\n\n---\n\n## Getting around\n\nThe **Home** tab wraps the selection, **Insert** drops snippets at the caret, **View** arranges the panes. The gear in the title strip opens the settings well.\n";
        Self {
            source: source.to_string(),
            path: None,
            dirty: false,
            tab: RibbonTab::Home,
            settings: Settings::default(),
            show_settings: false,
            outline_open: true,
            show_find: false,
            find: String::new(),
            replace: String::new(),
            clipboard: String::new(),
            cursor: (1, 1),
            cursor_char: 0,
            sel_len: 0,
            goto: None,
            status: "Ready — Markdown craft".into(),
            status_at: None,
            undo: Vec::new(),
            redo: Vec::new(),
            editing: false,
            pre_edit: String::new(),
            last_edit: None,
            last_autosave: Instant::now(),
            ed_frac: 0.0,
            prev_content_h: 1.0,
            prev_view_h: 1.0,
        }
    }

    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        match std::str::from_utf8(bytes) {
            Ok(s) => {
                self.source = s.to_string();
                self.path = Some(name.to_string());
                self.dirty = false;
                self.undo.clear();
                self.redo.clear();
                self.note(format!("Opened {name}"));
            }
            Err(e) => self.note(format!("Open failed: {e}")),
        }
    }

    /// The host drives native crafts through `ui()` only; kept for API parity.
    pub fn logic(&mut self, _ctx: &egui::Context) {}

    // ── frame ────────────────────────────────────────────────────────────────

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.pre_edit = self.source.clone();
        ui.style_mut().visuals = egui::Visuals::dark();

        ribbon::title_strip(self, ui);
        ribbon::tab_strip(self, ui);
        ribbon::command_strip(self, ui);
        self.status_bar(ui);

        match self.settings.preview_pos {
            PreviewPos::Right => {
                egui::Panel::right("md_preview_pane")
                    .default_size(300.0)
                    .resizable(true)
                    .size_range(220.0..=560.0)
                    .frame(egui::Frame::NONE.fill(ribbon::BG).inner_margin(egui::Margin::same(10)).stroke(Stroke::new(1.0, ribbon::BORDER)))
                    .show(ui, |ui| self.right_pane(ui));
            }
            PreviewPos::Bottom => {
                egui::Panel::bottom("md_preview_bottom")
                    .default_size(280.0)
                    .resizable(true)
                    .size_range(120.0..=1000.0)
                    .frame(egui::Frame::NONE.fill(ribbon::BG).inner_margin(egui::Margin::same(10)).stroke(Stroke::new(1.0, ribbon::BORDER)))
                    .show(ui, |ui| {
                        ui.label(RichText::new("PREVIEW").size(10.0).color(ribbon::WEAK));
                        self.preview_scroll(ui);
                    });
            }
            PreviewPos::Off => {}
        }

        self.editor(ui);

        if self.settings.autosave {
            self.autosave_tick();
        }
        settings::settings_window(self, &ctx);
        if self.show_find {
            self.find_bar(&ctx);
        }

        // user typing: snapshot one undo step per burst of edits
        if !self.editing && self.source != self.pre_edit {
            self.push_undo_with(self.pre_edit.clone());
            self.dirty = true;
            self.last_edit = Some(Instant::now());
        }
        self.editing = false;
    }

    // ── editor ───────────────────────────────────────────────────────────────

    fn editor(&mut self, ui: &mut Ui) {
        let size = self.settings.editor_size;
        let wrap = self.settings.wrap_lines;
        let accent = self.settings.accent.color();
        let line_h = line_pitch(ui, size);

        // pending caret jump: place the line at the top third of the view
        if let Some(off) = self.goto.take() {
            let line = line_of_char(&self.source, off) as f32;
            if let Some(mut st) = egui::scroll_area::State::load(ui.ctx(), scroll_id()) {
                st.offset.y = ((line - 1.0) * line_h - 60.0).max(0.0);
                st.store(ui.ctx(), scroll_id());
            }
        }

        let gutter = if self.settings.line_numbers { GUTTER_W } else { 0.0 };
        let out = egui::ScrollArea::vertical()
            .id_salt("md_scroll")
            .hscroll(!wrap)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if gutter > 0.0 {
                    ui.add_space(gutter);
                }
                ui.visuals_mut().selection.bg_fill = accent.gamma_multiply(0.35);
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, accent);
                let rows = self.source.lines().count().max(20);
                let mut text = egui::TextEdit::multiline(&mut self.source)
                    .id(edit_id())
                    .font(FontId::monospace(size))
                    .frame(egui::Frame::NONE)
                    .desired_rows(rows);
                // wrap is controlled through the desired width: INFINITY disables it
                text = if wrap {
                    text.desired_width(ui.available_width().max(200.0))
                } else {
                    text.desired_width(f32::INFINITY)
                };
                ui.add(text);
            });

        // caret position for the status bar (from the TextEdit state)
        if let Some(st) = egui::TextEdit::load_state(ui.ctx(), edit_id())
            && let Some(range) = st.cursor.char_range()
        {
            let caret = range.primary.index.0;
            let other = range.secondary.index.0;
            self.cursor_char = caret.min(self.source.chars().count());
            self.sel_len = caret.abs_diff(other);
            self.cursor = line_col(&self.source, self.cursor_char);
        }

        // measured geometry for sync scroll
        let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
        self.ed_frac = if max > 1.0 {
            egui::scroll_area::State::load(ui.ctx(), scroll_id()).map_or(0.0, |st| (st.offset.y / max).clamp(0.0, 1.0))
        } else {
            0.0
        };

        // line-number gutter, painted over the left edge, synced to the scroll
        if self.settings.line_numbers {
            let view = out.inner_rect;
            let oy = egui::scroll_area::State::load(ui.ctx(), scroll_id()).map_or(0.0, |st| st.offset.y);
            let char_w = mono_char_width(ui, size);
            let content_w = (view.width() - gutter - 24.0).max(40.0);
            let cpr = ((content_w / char_w).floor() as usize).max(1);
            let mut row = 0.0f32;
            let mut y = view.top() - oy;
            for (i, line) in self.source.lines().enumerate() {
                let rows = if wrap { (((line.chars().count() + 1) as f32 / cpr as f32).ceil() as usize).max(1) } else { 1 };
                if y >= view.top() - line_h && y <= view.bottom() {
                    ui.painter().text(
                        egui::pos2(view.left() + gutter - 10.0, y),
                        Align2::RIGHT_TOP,
                        (i + 1).to_string(),
                        FontId::monospace(size - 2.5),
                        ribbon::DIM,
                    );
                }
                y += rows as f32 * line_h;
                row += rows as f32;
            }
            let _ = row;
        }
    }

    // ── right pane ───────────────────────────────────────────────────────────

    fn right_pane(&mut self, ui: &mut Ui) {
        if self.outline_open {
            preview::outline_pane(self, ui);
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
        }
        self.preview_scroll(ui);
        // Document + Export pinned to the bottom of the pane
        egui::Panel::bottom("md_pane_tools")
            .frame(egui::Frame::NONE.fill(ribbon::BG).inner_margin(egui::Margin { left: 0, right: 0, top: 6, bottom: 0 }))
            .show(ui, |ui| {
                ui.separator();
                ui.add_space(4.0);
                preview::document_pane(self, ui);
                ui.add_space(4.0);
                preview::export_pane(self, ui);
            });
    }

    fn preview_scroll(&mut self, ui: &mut Ui) {
        let offset = if self.settings.sync_scroll { self.ed_frac * (self.prev_content_h - self.prev_view_h).max(0.0) } else { 0.0 };
        let mut area = egui::ScrollArea::vertical().id_salt("md_preview_scroll");
        if self.settings.sync_scroll {
            area = area.vertical_scroll_offset(offset);
        }
        let out = area.show(ui, |ui| preview::preview(self, ui));
        self.prev_content_h = out.content_size.y;
        self.prev_view_h = out.inner_rect.height();
    }

    // ── status bar ───────────────────────────────────────────────────────────

    fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("md_status")
            .exact_size(ribbon::STATUS_H)
            .frame(
                egui::Frame::NONE
                    .fill(ribbon::TITLE)
                    .inner_margin(egui::Margin { left: 12, right: 12, top: 0, bottom: 0 })
                    .stroke(Stroke::new(1.0, ribbon::BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
                    let fresh = self.status_at.is_some_and(|t| t.elapsed().as_secs_f64() < 3.0);
                    if fresh {
                        ui.label(RichText::new(self.status.clone()).small().color(self.settings.accent.color()));
                    } else {
                        let sel = if self.sel_len > 0 { format!(" ({} selected)", self.sel_len) } else { String::new() };
                        ui.label(
                            RichText::new(format!("Ln {}, Col {}{}", self.cursor.0, self.cursor.1, sel))
                                .small()
                                .color(ribbon::WEAK),
                        );
                    }
                    ui.label(RichText::new(format!("{} words · {} chars", parse::word_count(&self.source), self.source.chars().count())).small().color(ribbon::DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Markdown").small().color(ribbon::DIM));
                        if self.dirty {
                            ui.label(RichText::new("●").small().color(self.settings.accent.color()));
                        }
                        let name = self.path.clone().unwrap_or_else(|| "Untitled.md".into());
                        ui.label(RichText::new(name).small().color(ribbon::WEAK));
                    });
                });
            });
    }

    // ── find & replace ───────────────────────────────────────────────────────

    fn find_bar(&mut self, ctx: &egui::Context) {
        egui::Area::new(Id::new("md_find_bar"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_TOP, [-14.0, ribbon::TITLE_H + ribbon::TABS_H + ribbon::RIBBON_H + 12.0])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(ribbon::CARD)
                    .stroke(Stroke::new(1.0, ribbon::BORDER))
                    .corner_radius(6)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Find").small().color(ribbon::WEAK));
                            ui.add(egui::TextEdit::singleline(&mut self.find).desired_width(150.0).font(FontId::proportional(12.0)));
                            let matches = if self.find.is_empty() { 0 } else { self.source.matches(&self.find).count() };
                            ui.label(RichText::new(format!("{matches}")).small().color(ribbon::DIM));
                            if ui.add(egui::Button::new("Next").small()).clicked() {
                                self.find_next(ctx);
                            }
                            if ui.add(egui::Button::new("✕").small()).clicked() {
                                self.show_find = false;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Replace").small().color(ribbon::WEAK));
                            ui.add(egui::TextEdit::singleline(&mut self.replace).desired_width(150.0).font(FontId::proportional(12.0)));
                            if ui.add(egui::Button::new("Replace").small()).clicked() {
                                self.replace_one(ctx);
                            }
                            if ui.add(egui::Button::new("All").small()).clicked() {
                                self.replace_all(ctx);
                            }
                        });
                    });
            });
    }

    fn find_next(&mut self, ctx: &egui::Context) {
        if self.find.is_empty() {
            return;
        }
        let chars: Vec<char> = self.source.chars().collect();
        let n = self.find.chars().count();
        if n == 0 || chars.len() < n {
            self.note("No matches");
            return;
        }
        let start = (self.cursor_char + self.sel_len).min(chars.len().saturating_sub(n));
        let hit = (start..=chars.len() - n)
            .chain(0..start)
            .find(|&i| chars[i..i + n].iter().collect::<String>() == self.find);
        match hit {
            Some(i) => {
                self.goto = Some(i);
                self.set_cursor(ctx, (i, i + n));
                self.note(format!("Found at Ln {}", line_of_char(&self.source, i)));
            }
            None => self.note("No matches"),
        }
    }

    fn replace_one(&mut self, ctx: &egui::Context) {
        if self.find.is_empty() {
            return;
        }
        let sel = self.sel_range(ctx);
        let sel_matches = sel.is_some_and(|(s, e)| {
            let b = |c: usize| ch2by(&self.source, c);
            e > s && &self.source[b(s)..b(e)] == self.find
        });
        if sel_matches {
            let (s, e) = sel.unwrap();
            let rep_len = self.replace.chars().count();
            self.edit_at(ctx, Some((s, e)), &self.replace.clone(), Some((s + rep_len, s + rep_len)));
            self.note("Replaced");
        } else {
            self.find_next(ctx);
        }
    }

    fn replace_all(&mut self, ctx: &egui::Context) {
        if self.find.is_empty() {
            return;
        }
        let count = self.source.matches(&self.find).count();
        if count == 0 {
            self.note("No matches");
            return;
        }
        self.push_undo();
        self.editing = true;
        self.source = self.source.replace(&self.find, &self.replace);
        self.dirty = true;
        self.last_edit = Some(Instant::now());
        self.set_cursor(ctx, (0, 0));
        self.note(format!("Replaced {count}"));
    }

    // ── editing commands (called from the ribbon / panes) ────────────────────

    /// Character range of the editor selection, ordered.
    fn sel_range(&self, ctx: &egui::Context) -> Option<(usize, usize)> {
        let st = egui::TextEdit::load_state(ctx, edit_id())?;
        let r = st.cursor.char_range()?;
        let (a, b) = (r.primary.index.0, r.secondary.index.0);
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    fn set_cursor(&self, ctx: &egui::Context, (a, b): (usize, usize)) {
        if let Some(mut st) = egui::TextEdit::load_state(ctx, edit_id()) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(a.min(b)), egui::text::CCursor::new(a.max(b)))));
            egui::TextEdit::store_state(ctx, edit_id(), st);
        }
    }

    fn caret(&self, ctx: &egui::Context) -> usize {
        self.sel_range(ctx).map_or(self.cursor_char, |(s, e)| if self.sel_len > 0 { e } else { s })
    }

    /// Replace the `at` range (or the caret) with `insert`, then place the
    /// cursor / selection at `new_sel`. Pushes one undo step.
    fn edit_at(&mut self, ctx: &egui::Context, at: Option<(usize, usize)>, insert: &str, new_sel: Option<(usize, usize)>) {
        self.push_undo();
        self.editing = true;
        let (s, e) = at.unwrap_or_else(|| (self.caret(ctx), self.caret(ctx)));
        let (bs, be) = (ch2by(&self.source, s), ch2by(&self.source, e));
        self.source.replace_range(bs..be.min(self.source.len()), insert);
        let sel = new_sel.unwrap_or_else(|| {
            let c = s + insert.chars().count();
            (c, c)
        });
        self.set_cursor(ctx, sel);
        self.dirty = true;
        self.last_edit = Some(Instant::now());
    }

    /// Whether the current selection is already wrapped in `mark` (toggle hint).
    pub fn wrapped_with(&self, ctx: &egui::Context, mark: &str) -> bool {
        self.sel_range(ctx).is_some_and(|(s, e)| {
            let (bs, be) = (ch2by(&self.source, s), ch2by(&self.source, e));
            self.source[bs..be].starts_with(mark) && self.source[bs..be].ends_with(mark) && e - s >= 2 * mark.chars().count()
        })
    }

    /// Wrap (or unwrap) the selection with a Markdown marker; with no
    /// selection, insert a marker pair (cursor inside when auto-close is on).
    pub fn wrap(&mut self, ctx: &egui::Context, mark: &str) {
        match self.sel_range(ctx).filter(|(s, e)| e > s) {
            Some((s, e)) => {
                let (bs, be) = (ch2by(&self.source, s), ch2by(&self.source, e));
                let inner = self.source[bs..be].to_string();
                if inner.starts_with(mark) && inner.ends_with(mark) && e - s >= 2 * mark.chars().count() {
                    let stripped = inner[mark.len()..inner.len() - mark.len()].to_string();
                    let n = stripped.chars().count();
                    self.edit_at(ctx, Some((s, e)), &stripped, Some((s, s + n)));
                    self.note("Removed markers");
                } else {
                    let ins = format!("{mark}{inner}{mark}");
                    self.edit_at(ctx, Some((s, e)), &ins, Some((s, s + ins.chars().count())));
                }
            }
            _ => {
                let c = self.caret(ctx);
                let ins = format!("{mark}{mark}");
                let inside = c + mark.chars().count();
                let sel = if self.settings.auto_close { (inside, inside) } else { (c + ins.chars().count(), c + ins.chars().count()) };
                self.edit_at(ctx, Some((c, c)), &ins, Some(sel));
            }
        }
    }

    fn covered_lines(&self, ctx: &egui::Context) -> (usize, usize) {
        let start = self.sel_range(ctx).map_or(self.cursor_char, |(s, _)| s);
        let end = self.sel_range(ctx).map_or(self.cursor_char, |(_, e)| e);
        (line_of_char(&self.source, start), line_of_char(&self.source, end))
    }

    /// Apply/replace/toggle a `#{level} ` heading prefix on the covered lines.
    pub fn heading(&mut self, ctx: &egui::Context, level: u8) {
        let (l0, l1) = self.covered_lines(ctx);
        let lines: Vec<String> = self.source.lines().map(str::to_string).collect();
        let mut edits: Vec<(usize, String, String)> = Vec::new(); // (line idx, old, new)
        for ln in l0..=l1.min(lines.len()) {
            let i = ln - 1;
            let Some(line) = lines.get(i) else { continue };
            let hashes = line.chars().take_while(|c| *c == '#').count();
            let rest = if hashes > 0 { line[hashes..].trim_start() } else { line.as_str() };
            let new = if hashes == level as usize {
                rest.to_string()
            } else {
                format!("{} {rest}", "#".repeat(level as usize))
            };
            if &new != line {
                edits.push((i, line.clone(), new));
            }
        }
        if edits.is_empty() {
            return;
        }
        self.push_undo();
        self.editing = true;
        for (i, old, new) in edits {
            let bs = self.line_byte_start(i);
            self.source.replace_range(bs..bs + old.len(), &new);
        }
        self.dirty = true;
        self.last_edit = Some(Instant::now());
    }

    pub fn heading_is(&self, ctx: &egui::Context, level: u8) -> bool {
        let (l0, _) = self.covered_lines(ctx);
        self.source
            .lines()
            .nth(l0 - 1)
            .is_some_and(|l| l.chars().take_while(|c| *c == '#').count() == level as usize)
    }

    /// Prefix every covered line (list bullets, quotes, tasks, indent).
    fn prefix_lines(&mut self, ctx: &egui::Context, prefix: &str, toggle: bool) {
        let (l0, l1) = self.covered_lines(ctx);
        let lines: Vec<String> = self.source.lines().map(str::to_string).collect();
        let mut edits = Vec::new();
        for ln in l0..=l1.min(lines.len()) {
            let i = ln - 1;
            let Some(line) = lines.get(i) else { continue };
            let new = if toggle && line.starts_with(prefix) { line[prefix.len()..].to_string() } else { format!("{prefix}{line}") };
            if &new != line {
                edits.push((self.line_byte_start(i), line.len(), new));
            }
        }
        if edits.is_empty() {
            return;
        }
        self.push_undo();
        self.editing = true;
        for (bs, old_len, new) in edits.into_iter().rev() {
            self.source.replace_range(bs..bs + old_len, &new);
        }
        self.dirty = true;
        self.last_edit = Some(Instant::now());
    }

    pub fn quote(&mut self, ctx: &egui::Context) {
        self.prefix_lines(ctx, "> ", true);
    }
    pub fn list(&mut self, ctx: &egui::Context) {
        self.prefix_lines(ctx, "- ", true);
    }
    pub fn numbered(&mut self, ctx: &egui::Context) {
        let (l0, l1) = self.covered_lines(ctx);
        let lines: Vec<String> = self.source.lines().map(str::to_string).collect();
        let mut edits = Vec::new();
        for (k, ln) in (l0..=l1.min(lines.len())).enumerate() {
            let i = ln - 1;
            if let Some(line) = lines.get(i) {
                let new = if line.starts_with(|c: char| c.is_ascii_digit()) { line.clone() } else { format!("{}. {line}", k + 1) };
                if &new != line {
                    edits.push((i, line.clone(), new));
                }
            }
        }
        if edits.is_empty() {
            return;
        }
        self.push_undo();
        self.editing = true;
        for (i, old, new) in edits {
            let bs = self.line_byte_start(i);
            self.source.replace_range(bs..bs + old.len(), &new);
        }
        self.dirty = true;
        self.last_edit = Some(Instant::now());
    }
    pub fn task(&mut self, ctx: &egui::Context) {
        self.prefix_lines(ctx, "- [ ] ", true);
    }
    pub fn indent(&mut self, ctx: &egui::Context, more: bool) {
        let pad = " ".repeat(self.settings.tab_size.max(2));
        if more {
            self.prefix_lines(ctx, &pad, false);
        } else {
            let (l0, l1) = self.covered_lines(ctx);
            let lines: Vec<String> = self.source.lines().map(str::to_string).collect();
            self.push_undo();
            self.editing = true;
            for ln in (l0..=l1.min(lines.len())).rev() {
                let i = ln - 1;
                if let Some(line) = lines.get(i) {
                    let strip = line.len().min(line.chars().take_while(|c| *c == ' ').count().min(self.settings.tab_size.max(2)));
                    if strip > 0 {
                        let bs = self.line_byte_start(i);
                        self.source.replace_range(bs..bs + strip, "");
                    }
                }
            }
            self.dirty = true;
            self.last_edit = Some(Instant::now());
        }
    }

    pub fn code_block(&mut self, ctx: &egui::Context) {
        self.edit_at(ctx, None, "\n```\n\n```\n", None);
    }
    pub fn insert_snippet(&mut self, ctx: &egui::Context, text: &str) {
        self.edit_at(ctx, None, text, None);
    }
    pub fn footnote(&mut self, ctx: &egui::Context) {
        let n = 1 + self.source.matches("\n[^").count();
        self.edit_at(ctx, None, &format!("[^{n}]"), None);
        let end = self.source.chars().count();
        self.edit_at(ctx, Some((end, end)), &format!("\n[^{n}]: Footnote text."), None);
    }

    pub fn copy(&mut self, ctx: &egui::Context) {
        let text = self.sel_range(ctx).filter(|(s, e)| e > s).map_or_else(
            || {
                let ln = line_of_char(&self.source, self.cursor_char);
                self.source.lines().nth(ln - 1).unwrap_or("").to_string()
            },
            |(s, e)| self.source[ch2by(&self.source, s)..ch2by(&self.source, e)].to_string(),
        );
        self.clipboard = text.clone();
        ctx.copy_text(text);
        self.note("Copied");
    }

    pub fn cut(&mut self, ctx: &egui::Context) {
        let sel = self.sel_range(ctx).filter(|(s, e)| e > s);
        let text = match sel {
            Some((s, e)) => self.source[ch2by(&self.source, s)..ch2by(&self.source, e)].to_string(),
            None => {
                let ln = line_of_char(&self.source, self.cursor_char);
                self.source.lines().nth(ln - 1).unwrap_or("").to_string()
            }
        };
        self.clipboard = text.clone();
        ctx.copy_text(text);
        match sel {
            Some((s, e)) => self.edit_at(ctx, Some((s, e)), "", None),
            None => {
                let ln = line_of_char(&self.source, self.cursor_char);
                let start = self.line_byte_start(ln - 1);
                let end = self.source[start..].find('\n').map_or(self.source.len(), |k| start + k + 1);
                let cs = self.source[..start].chars().count();
                let ce = cs + self.source[start..end].chars().count();
                self.edit_at(ctx, Some((cs, ce)), "", None);
            }
        }
        self.note("Cut");
    }

    pub fn paste(&mut self, ctx: &egui::Context) {
        // With the editor focused, paste the real system clipboard into it;
        // otherwise insert the craft clipboard (filled by Copy / Cut).
        if ctx.memory(|m| m.focused()) == Some(edit_id()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            self.note("Paste");
        } else if !self.clipboard.is_empty() {
            let text = self.clipboard.clone();
            self.edit_at(ctx, None, &text, None);
            self.note("Pasted from craft clipboard");
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            self.note("Paste");
        }
    }

    pub fn undo_edit(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.source.clone());
            self.source = prev;
            self.editing = true;
            self.dirty = true;
            self.note("Undo");
        }
    }

    pub fn redo_edit(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.source.clone());
            self.source = next;
            self.editing = true;
            self.dirty = true;
            self.note("Redo");
        }
    }

    // ── files ────────────────────────────────────────────────────────────────

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(p) => match std::fs::write(&p, &self.source) {
                Ok(_) => {
                    self.dirty = false;
                    self.note(format!("Saved {p}"));
                }
                Err(e) => self.note(format!("Save failed: {e}")),
            },
            None => self.note("No path — the document is not saved to disk yet"),
        }
    }

    pub fn export_html(&mut self) {
        let p = self
            .path
            .as_ref()
            .and_then(|p| p.strip_suffix(".md").or_else(|| p.strip_suffix(".markdown")).map(|s| format!("{s}.html")))
            .unwrap_or_else(|| "document.html".to_string());
        match std::fs::write(&p, preview::to_html(&self.source)) {
            Ok(_) => self.note(format!("Exported {p}")),
            Err(e) => self.note(format!("Export failed: {e}")),
        }
    }

    fn autosave_tick(&mut self) {
        let edited = self.last_edit.is_some_and(|t| t.elapsed().as_secs_f32() >= self.settings.autosave_secs);
        let due = self.last_autosave.elapsed().as_secs_f32() >= self.settings.autosave_secs;
        if self.dirty && self.path.is_some() && edited && due {
            if let Some(p) = self.path.clone()
                && std::fs::write(&p, &self.source).is_ok()
            {
                self.dirty = false;
                self.last_autosave = Instant::now();
                self.note(format!("Autosaved {p}"));
            }
        }
    }

    /// Set the transient status message (shown in the status bar for 3 s).
    pub fn note(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_at = Some(Instant::now());
    }

    pub fn now_datetime(&self) -> (String, String) {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = (secs / 86_400) as i64;
        let tod = secs % 86_400;
        // Howard Hinnant's civil_from_days
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (
            format!("{y:04}-{m:02}-{d:02}"),
            format!("{:02}:{:02}:{:02}", tod / 3600, (tod % 3600) / 60, tod % 60),
        )
    }

    // ── small helpers ────────────────────────────────────────────────────────

    fn push_undo(&mut self) {
        self.push_undo_with(self.source.clone());
    }

    fn push_undo_with(&mut self, snapshot: String) {
        if self.undo.last().map(|last| last == &snapshot).unwrap_or(false) {
            return;
        }
        self.undo.push(snapshot);
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn line_byte_start(&self, line_idx: usize) -> usize {
        self.source
            .char_indices()
            .scan(0usize, |ln, (b, c)| {
                let at = *ln;
                if c == '\n' {
                    *ln += 1;
                }
                Some((at, b))
            })
            .find(|(ln, _)| *ln == line_idx)
            .map(|(_, b)| b)
            .unwrap_or(self.source.len())
    }
}

// ── free helpers ─────────────────────────────────────────────────────────────

fn ch2by(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map_or(s.len(), |(b, _)| b)
}

fn line_of_char(s: &str, ch: usize) -> usize {
    let mut line = 1;
    let mut c = 0usize;
    for chr in s.chars() {
        if c >= ch {
            break;
        }
        if chr == '\n' {
            line += 1;
        }
        c += 1;
    }
    line
}

fn line_col(s: &str, ch: usize) -> (usize, usize) {
    let before = &s[..ch2by(s, ch)];
    let ln = before.matches('\n').count() + 1;
    let col = before.chars().rev().take_while(|c| *c != '\n').count() + 1;
    (ln, col)
}

fn line_pitch(ui: &Ui, size: f32) -> f32 {
    ui.ctx()
        .fonts_mut(|f| f.layout_no_wrap("Ag\nAg".to_string(), FontId::monospace(size), Color32::WHITE).size().y / 2.0)
        .max(size * 1.2)
}

fn mono_char_width(ui: &Ui, size: f32) -> f32 {
    (ui.ctx().fonts_mut(|f| f.layout_no_wrap("0000000000".to_string(), FontId::monospace(size), Color32::WHITE).size().x) / 10.0).max(1.0)
}
