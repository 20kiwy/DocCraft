//! The live preview: a rich Markdown renderer over the parse tree (headings,
//! paragraphs with inline flow, lists, task checkboxes, quotes, code cards,
//! tables, footnotes, links, rules), the Outline / Document / Export right
//! pane sections, and a small `.html` exporter.

use crate::model::{Block, Inline};
use crate::parse;
use crate::ribbon::{accent, BORDER, CARD, CODE_BG, DIM, LINK, TEXT, WEAK};
use crate::ui::MdApp;
use egui::{Align, Align2, Color32, CornerRadius, FontId, Frame, RichText, Stroke, TextFormat, Ui, Vec2, WidgetText};

// ── preview renderer ─────────────────────────────────────────────────────────

/// Rendered preview of the parsed document, in the preview font size, with
/// the settings accent on headings and links and on code cards per setting.
pub fn preview(app: &MdApp, ui: &mut Ui) {
    let blocks = parse::parse(&app.source);
    let size = app.settings.preview_size;
    let a = accent(app);
    let body = Color32::from_rgb(0xd6, 0xd8, 0xde);
    if blocks.is_empty() {
        ui.label(RichText::new("Nothing to preview yet — type Markdown on the left.").weak());
        return;
    }
    for b in blocks {
        match b {
            Block::Heading { level, text } => {
                let s = match level { 1 => size + 11.0, 2 => size + 6.0, 3 => size + 3.0, 4 => size + 1.0, _ => size };
                ui.add_space(8.0);
                let mut job = egui::text::LayoutJob::default();
                push_inlines(&mut job, &text, Run { size: s, color: a, bold: true, ..Run::default() });
                job.wrap.max_width = ui.available_width();
                ui.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
                if level <= 2 {
                    ui.add_space(2.0);
                    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, a.gamma_multiply(0.4)));
                }
            }
            Block::Paragraph(inl) => {
                ui.add_space(3.0);
                let mut job = egui::text::LayoutJob::default();
                push_inlines(&mut job, &inl, Run { size, color: body, ..Run::default() });
                job.wrap.max_width = ui.available_width();
                ui.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
            }
            Block::Bullet(items) => {
                ui.add_space(2.0);
                for item in &items {
                    list_row(ui, "•", WEAK, |ui| job_label(ui, item, Run { size, color: body, ..Run::default() }));
                }
            }
            Block::Numbered(items) => {
                ui.add_space(2.0);
                for (i, item) in items.iter().enumerate() {
                    let n = format!("{}.", i + 1);
                    list_row(ui, &n, WEAK, |ui| job_label(ui, item, Run { size, color: body, ..Run::default() }));
                }
            }
            Block::Task(items) => {
                ui.add_space(2.0);
                for (done, item) in &items {
                    list_row(ui, "", WEAK, |ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(13.0, 13.0), egui::Sense::hover());
                        let rr = r.shrink2(Vec2::new(1.5, 4.0));
                        if *done {
                            ui.painter().rect_filled(rr, 3.0, a);
                            ui.painter().text(rr.center(), Align2::CENTER_CENTER, "✓", FontId::proportional(9.5), Color32::WHITE);
                        } else {
                            ui.painter().rect_stroke(rr, 3.0, Stroke::new(1.2, WEAK), egui::StrokeKind::Inside);
                        }
                        let mut run = Run { size, color: body, ..Run::default() };
                        if *done {
                            run.strike = true;
                            run.color = WEAK;
                        }
                        job_label(ui, item, run);
                    });
                }
            }
            Block::Quote(paras) => {
                ui.add_space(4.0);
                let w = ui.available_width();
                let h = paras.len() as f32 * (size * 1.5) + 8.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, h.max(20.0)), egui::Sense::hover());
                let bar = egui::Rect::from_min_size(r.left_top() + Vec2::new(0.0, 3.0), Vec2::new(3.0, h - 6.0));
                ui.painter().rect_filled(bar, 2.0, a.gamma_multiply(0.55));
                let inner = r.shrink2(Vec2::new(10.0, 0.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).id_salt("md_quote"));
                for p in &paras {
                    let mut job = egui::text::LayoutJob::default();
                    push_inlines(&mut job, p, Run { size: size - 0.5, color: WEAK, italic: true, ..Run::default() });
                    job.wrap.max_width = child.available_width();
                    child.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
                }
            }
            Block::Code { lang, code } => {
                ui.add_space(4.0);
                let rows = code.lines().count().max(1);
                let w = ui.available_width();
                let h = 10.0 + rows as f32 * (size * 1.35) + if lang.is_empty() { 0.0 } else { 16.0 };
                Frame::NONE
                    .fill(if app.settings.dark_code { CODE_BG } else { Color32::from_rgb(0xf2, 0xf2, 0xf5) })
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(6)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.set_width(w - 22.0);
                        if !lang.is_empty() {
                            ui.label(
                                RichText::new(lang.clone())
                                    .small()
                                    .color(if app.settings.dark_code { DIM } else { WEAK }),
                            );
                        }
                        let mut job = egui::text::LayoutJob::default();
                        job.append(
                            &code,
                            0.0,
                            TextFormat::simple(FontId::monospace(size - 1.5), if app.settings.dark_code { Color32::from_rgb(0xd8, 0xde, 0xe4) } else { Color32::from_rgb(0x26, 0x26, 0x2c) }),
                        );
                        job.wrap.max_width = ui.available_width();
                        ui.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
                    });
            }
            Block::Rule => {
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), egui::Sense::hover());
                ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.5, BORDER));
                ui.add_space(4.0);
            }
            Block::Table { head, rows } => table(app, ui, &head, &rows),
            Block::FootnoteDef { label, text } => {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("^{label}")).small().color(accent(app)));
                    let mut job = egui::text::LayoutJob::default();
                    push_inlines(&mut job, &text, Run { size: size - 2.5, color: DIM, ..Run::default() });
                    job.wrap.max_width = ui.available_width();
                    ui.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
                });
            }
        }
    }
}

fn list_row(ui: &mut Ui, bullet: &str, color: Color32, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
        if !bullet.is_empty() {
            ui.label(RichText::new(bullet).size(13.0).color(color));
        }
        add(ui);
    });
}

fn table(app: &MdApp, ui: &mut Ui, head: &[String], rows: &[Vec<String>]) {
    ui.add_space(5.0);
    Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(0)
        .show(ui, |ui| {
            let a = accent(app);
            egui::Grid::new(ui.id().with("tbl"))
                .striped(true)
                .min_col_width(56.0)
                .spacing(Vec2::new(14.0, 4.0))
                .show(ui, |ui| {
                    for h in head {
                        ui.label(RichText::new(h.clone()).strong().size(app.settings.preview_size - 1.0).color(a));
                    }
                    ui.end_row();
                    for row in rows {
                        for c in row {
                            ui.label(RichText::new(c.clone()).size(app.settings.preview_size - 1.0).color(TEXT));
                        }
                        ui.end_row();
                    }
                });
        });
    ui.add_space(4.0);
}

/// One inline-formatting state for the LayoutJob walker.
#[derive(Clone, Copy)]
struct Run {
    size: f32,
    color: Color32,
    bold: bool,
    italic: bool,
    strike: bool,
}

impl Default for Run {
    fn default() -> Self {
        Self { size: 14.0, color: TEXT, bold: false, italic: false, strike: false }
    }
}

fn text_fmt(r: Run, mono: bool) -> TextFormat {
    let font = if mono { FontId::monospace(r.size - 1.0) } else { FontId::proportional(r.size) };
    TextFormat {
        font_id: font,
        color: r.color,
        italics: r.italic,
        underline: Stroke::NONE,
        strikethrough: if r.strike { Stroke::new(1.0, r.color) } else { Stroke::NONE },
        ..Default::default()
    }
}

fn push_inlines(job: &mut egui::text::LayoutJob, inl: &[Inline], r: Run) {
    for i in inl {
        match i {
            Inline::Text(t) => job.append(t, 0.0, text_fmt(r, false)),
            Inline::Bold(inner) => push_inlines(job, inner, Run { bold: true, ..r }),
            Inline::Italic(inner) => push_inlines(job, inner, Run { italic: true, ..r }),
            Inline::Strike(inner) => push_inlines(job, inner, Run { strike: true, ..r }),
            Inline::Code(c) => {
                let mut f = text_fmt(r, true);
                f.background = Color32::from_rgb(0x30, 0x30, 0x38);
                job.append(c, 0.0, f);
            }
            Inline::Link { label, .. } => {
                let mut f = text_fmt(r, false);
                f.color = LINK;
                f.underline = Stroke::new(1.0, LINK);
                job.append(label, 0.0, f);
            }
            Inline::FootnoteRef(l) => {
                let mut f = text_fmt(r, false);
                f.font_id = FontId::proportional(r.size - 3.5);
                f.valign = Align::BOTTOM;
                f.color = LINK;
                job.append(&format!("[^{l}]"), 0.0, f);
            }
        }
    }
}

fn job_label(ui: &mut Ui, inl: &[Inline], r: Run) {
    let mut job = egui::text::LayoutJob::default();
    push_inlines(&mut job, inl, r);
    job.wrap.max_width = ui.available_width();
    ui.add(egui::Label::new(WidgetText::LayoutJob(std::sync::Arc::new(job))));
}

// ── right pane sections ──────────────────────────────────────────────────────

/// Section caption in the right pane (small uppercase, weak).
fn pane_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title.to_uppercase()).size(10.0).color(WEAK));
    ui.add_space(2.0);
}

/// The live heading tree: click a heading to move the caret there. The section
/// holding the caret is highlighted.
pub fn outline_pane(app: &mut MdApp, ui: &mut Ui) {
    pane_title(ui, "Outline");
    let headings = parse::outline(&app.source);
    if headings.is_empty() {
        ui.label(RichText::new("No headings yet. Add `#`, `##`, `###` lines to build an outline.").small().color(DIM));
        return;
    }
    let cur = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.char_off <= app.cursor_char)
        .map(|(i, _)| i)
        .next()
        .unwrap_or(0);
    egui::ScrollArea::vertical().id_salt("md_outline").max_height(190.0).show(ui, |ui| {
        let a = accent(app);
        for (i, h) in headings.iter().enumerate() {
            let selected = i == cur;
            ui.horizontal(|ui| {
                ui.add_space(h.level.saturating_sub(1) as f32 * 14.0);
                let mut text = RichText::new(h.text.clone())
                    .size(12.0)
                    .color(if selected { a } else if h.level == 1 { TEXT } else { WEAK });
                if h.level <= 2 {
                    text = text.strong();
                }
                let resp = ui.add(egui::Button::selectable(selected, text));
                if resp.clicked() {
                    app.goto = Some(h.char_off);
                }
                resp.on_hover_text(format!("Line {}", h.line));
            });
        }
    });
}

/// Document statistics: words, characters, reading time, headings.
pub fn document_pane(app: &MdApp, ui: &mut Ui) {
    pane_title(ui, "Document");
    let (words, chars, heads) = parse::stats(&app.source);
    egui::Grid::new("md_doc_stats").num_columns(2).spacing(Vec2::new(10.0, 2.0)).show(ui, |ui| {
        for (label, value) in [
            ("words", words.to_string()),
            ("characters", chars.to_string()),
            ("reading", format!("{} min", parse::reading_min(words))),
            ("headings", heads.to_string()),
        ] {
            ui.label(RichText::new(value).strong().size(12.5).color(TEXT));
            ui.label(RichText::new(label).small().color(DIM));
            ui.end_row();
        }
    });
}

/// Export section: Save .md, Export .html, Export .pdf (pointer to Typeset).
pub fn export_pane(app: &mut MdApp, ui: &mut Ui) {
    pane_title(ui, "Export");
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(RichText::new("💾 Save .md").size(11.5).color(TEXT))
                    .fill(CARD)
                    .corner_radius(CornerRadius::same(4)),
            )
            .on_hover_text("Write the document back to its file")
            .clicked()
        {
            app.save();
        }
        if ui
            .add(
                egui::Button::new(RichText::new("Export .html").size(11.5).color(TEXT))
                    .fill(CARD)
                    .corner_radius(CornerRadius::same(4)),
            )
            .on_hover_text("Render the document to a small standalone HTML file")
            .clicked()
        {
            app.export_html();
        }
    });
    ui.add(
        egui::Button::new(RichText::new("Export .pdf — via the Typeset craft").size(10.5).color(DIM))
            .fill(egui::Color32::TRANSPARENT)
            .sense(egui::Sense::hover()),
    )
    .on_hover_text("PDF export lives in the Typeset craft (real PDF engine). This craft keeps Markdown-native exports.");
}

// ── HTML export ──────────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn inline_html(inl: &[Inline]) -> String {
    let mut out = String::new();
    for i in inl {
        match i {
            Inline::Text(t) => out.push_str(&esc(t)),
            Inline::Bold(v) => out.push_str(&format!("<strong>{}</strong>", inline_html(v))),
            Inline::Italic(v) => out.push_str(&format!("<em>{}</em>", inline_html(v))),
            Inline::Strike(v) => out.push_str(&format!("<del>{}</del>", inline_html(v))),
            Inline::Code(c) => out.push_str(&format!("<code>{}</code>", esc(c))),
            Inline::Link { label, url } => out.push_str(&format!("<a href=\"{}\">{}</a>", esc(url), esc(label))),
            Inline::FootnoteRef(l) => out.push_str(&format!("<sup>[^{l}]</sup>")),
        }
    }
    out
}

/// A small standalone dark-themed HTML export from the parse tree.
pub fn to_html(src: &str) -> String {
    let mut b = String::new();
    let mut footnotes = String::new();
    for block in parse::parse(src) {
        match block {
            Block::Heading { level, text } => {
                let l = (level as usize).clamp(1, 6);
                b.push_str(&format!("<h{l}>{}</h{l}>\n", inline_html(&text)));
            }
            Block::Paragraph(t) => b.push_str(&format!("<p>{}</p>\n", inline_html(&t))),
            Block::Bullet(items) => {
                b.push_str("<ul>\n");
                for i in items {
                    b.push_str(&format!("<li>{}</li>\n", inline_html(&i)));
                }
                b.push_str("</ul>\n");
            }
            Block::Numbered(items) => {
                b.push_str("<ol>\n");
                for i in items {
                    b.push_str(&format!("<li>{}</li>\n", inline_html(&i)));
                }
                b.push_str("</ol>\n");
            }
            Block::Task(items) => {
                b.push_str("<ul>\n");
                for (done, i) in items {
                    b.push_str(&format!("<li><input type=\"checkbox\"{} disabled> {}</li>\n", if done { " checked" } else { "" }, inline_html(&i)));
                }
                b.push_str("</ul>\n");
            }
            Block::Quote(paras) => {
                b.push_str("<blockquote>\n");
                for p in paras {
                    b.push_str(&format!("<p>{}</p>\n", inline_html(&p)));
                }
                b.push_str("</blockquote>\n");
            }
            Block::Code { lang, code } => {
                b.push_str(&format!("<pre><code class=\"{}\">{}</code></pre>\n", esc(&lang), esc(&code)));
            }
            Block::Rule => b.push_str("<hr>\n"),
            Block::Table { head, rows } => {
                b.push_str("<table>\n<thead><tr>");
                for h in &head {
                    b.push_str(&format!("<th>{}</th>", esc(h)));
                }
                b.push_str("</tr></thead>\n<tbody>\n");
                for r in &rows {
                    b.push_str("<tr>");
                    for c in r {
                        b.push_str(&format!("<td>{}</td>", esc(c)));
                    }
                    b.push_str("</tr>\n");
                }
                b.push_str("</tbody>\n</table>\n");
            }
            Block::FootnoteDef { label, text } => {
                footnotes.push_str(&format!("<p id=\"fn-{label}\"><sup>{label}</sup> {}</p>\n", inline_html(&text)));
            }
        }
    }
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Markdown document</title>\n<style>\nbody{{margin:0;background:#1e1e22;color:#d6d8de;font:16px/1.6 Georgia,serif;padding:40px 20%;}}\nh1,h2{{color:#4d6dfe;border-bottom:1px solid #35353c;padding-bottom:4px;}} a{{color:#7f9bff;}} code{{background:#303038;border-radius:4px;padding:1px 5px;font-family:ui-monospace,monospace;font-size:90%;}}\npre{{background:#16161a;border:1px solid #35353c;border-radius:6px;padding:12px;overflow:auto;}} pre code{{background:none;padding:0;}}\nblockquote{{border-left:3px solid #4d6dfe55;margin:0;padding:2px 14px;color:#9a9aa4;}} table{{border-collapse:collapse;}} th,td{{border:1px solid #35353c;padding:5px 10px;}} hr{{border:none;border-top:1px solid #35353c;}}\n</style>\n</head>\n<body>\n{b}{foot}</body>\n</html>\n",
        b = b,
        foot = if footnotes.is_empty() { String::new() } else { format!("<hr>\n<footer>\n{footnotes}</footer>\n") }
    )
}
