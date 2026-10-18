//! The layout engine: styled blocks → justified, paginated pages.
//!
//! The greedy word-wrap and justified breaking are unchanged; everything page-shaped
//! (paper, orientation, margins, columns, folios, footer, leading) is now driven by
//! [`Settings`], which the ribbon's Page tab and the settings dialog edit directly.
//! Runs and frames use the PDF convention: y grows upwards from the page's bottom edge.

use crate::model::{Block, DocMeta, Parsed, Style};

/// A positioned text run on a page (`y` = baseline above the page bottom).
#[derive(Debug, Clone)]
pub struct Run {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: [u8; 3],
}

/// A stroked rectangle on a page (`y` = top edge above the page bottom): table
/// cells and image placeholders.
#[derive(Debug, Clone)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Default)]
pub struct Page {
    pub runs: Vec<Run>,
    pub frames: Vec<Frame>,
}

// ── page setup ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    Letter,
    Legal,
    A5,
    Custom,
}

/// Name, width, height in points (portrait).
pub const PAPERS: [(Paper, &str, f32, f32); 4] =
    [(Paper::A4, "A4", 595.0, 842.0), (Paper::Letter, "Letter", 612.0, 792.0), (Paper::Legal, "Legal", 612.0, 1008.0), (Paper::A5, "A5", 420.0, 595.0)];

impl Paper {
    pub fn name(self) -> &'static str {
        PAPERS.iter().find(|(p, ..)| *p == self).map_or("Custom", |(_, n, ..)| *n)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumPos {
    Center,
    Outside,
}

/// mm → pt.
pub fn mm(v: f32) -> f32 {
    v * 72.0 / 25.4
}

/// pt → mm.
pub fn to_mm(pt: f32) -> f32 {
    pt * 25.4 / 72.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    pub paper: Paper,
    pub custom_w: f32, // pt
    pub custom_h: f32, // pt
    pub landscape: bool,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub columns: u8, // 1 or 2
    pub gutter: f32, // pt between the two columns
}

impl PageSetup {
    /// (top, bottom, left, right) margin presets.
    pub const NORMAL: (f32, f32, f32, f32) = (72.0, 72.0, 72.0, 72.0);
    pub const NARROW: (f32, f32, f32, f32) = (36.0, 36.0, 36.0, 36.0);
    pub const WIDE: (f32, f32, f32, f32) = (72.0, 72.0, 108.0, 108.0);

    pub fn margins_name(&self) -> &'static str {
        if (self.margin_top, self.margin_bottom, self.margin_left, self.margin_right) == Self::NARROW {
            "Narrow"
        } else if (self.margin_top, self.margin_bottom, self.margin_left, self.margin_right) == Self::WIDE {
            "Wide"
        } else if (self.margin_top, self.margin_bottom, self.margin_left, self.margin_right) == Self::NORMAL {
            "Normal"
        } else {
            "Custom"
        }
    }

    /// Size in points (landscape swaps the pair).
    pub fn page_size(&self) -> (f32, f32) {
        let (mut w, mut h) = match self.paper {
            Paper::Custom => (self.custom_w, self.custom_h),
            _ => PAPERS.iter().find(|(p, ..)| *p == self.paper).map_or((595.0, 842.0), |(_, _, w, h)| (*w, *h)),
        };
        if self.landscape {
            std::mem::swap(&mut w, &mut h);
        }
        (w.max(200.0), h.max(200.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeaderFooter {
    pub numbers: bool,
    pub number_pos: NumPos,
    pub footer: String,
    pub first_different: bool, // no folio / footer on page 1
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocStyle {
    pub base_size: f32,    // body pt, 8..24
    pub line_spacing: f32, // 1.0 / 1.15 / 1.5
    pub justify: bool,
    pub mono_font: bool, // serif (Helvetica) or mono (Courier) body face
}

/// Everything the engine needs; lives on the app, so it survives craft switches.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub page: PageSetup,
    pub hf: HeaderFooter,
    pub style: DocStyle,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            page: PageSetup {
                paper: Paper::A4,
                custom_w: 595.0,
                custom_h: 842.0,
                landscape: false,
                margin_top: 72.0,
                margin_bottom: 72.0,
                margin_left: 72.0,
                margin_right: 72.0,
                columns: 1,
                gutter: 24.0,
            },
            hf: HeaderFooter { numbers: true, number_pos: NumPos::Center, footer: String::new(), first_different: false },
            style: DocStyle { base_size: 11.0, line_spacing: 1.15, justify: true, mono_font: false },
        }
    }
}

/// The result of one compile: pages plus where each heading landed (the Outline pane).
#[derive(Debug, Clone, Default)]
pub struct Layouted {
    pub pages: Vec<Page>,
    /// (level 1..=3, text, page index).
    pub headings: Vec<(u8, String, usize)>,
}

struct Flow {
    pages: Vec<Page>,
    top: f32,
    bottom: f32,
    x0: f32,
    col_w: f32,
    n_cols: usize,
    gutter: f32,
    col: usize,
    y: f32, // top edge of the next line, measured from the page bottom
}

impl Flow {
    fn new(st: &Settings) -> Self {
        let (pw, ph) = st.page.page_size();
        let n_cols = (st.page.columns.max(1) as usize).min(2);
        let content_w = pw - st.page.margin_left - st.page.margin_right;
        let gutter = st.page.gutter.min(content_w * 0.4);
        let col_w = if n_cols == 2 { (content_w - gutter) / 2.0 } else { content_w };
        Self { pages: vec![Page::default()], top: ph - st.page.margin_top, bottom: st.page.margin_bottom, x0: st.page.margin_left, col_w, n_cols, gutter, col: 0, y: ph - st.page.margin_top }
    }

    fn page(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.pages.push(Page::default());
        }
        let n = self.pages.len();
        &mut self.pages[n - 1]
    }

    /// Column's left x for the current position.
    fn col_x(&self) -> f32 {
        self.x0 + self.col as f32 * (self.col_w + self.gutter)
    }

    /// Advance to the next column (or a fresh page) when the current one is spent.
    fn advance(&mut self) {
        self.col += 1;
        self.y = self.top;
        if self.col >= self.n_cols {
            self.pages.push(Page::default());
            self.col = 0;
        }
    }

    /// Reserve one line of `size`: returns its left x and baseline y, flowing to the
    /// next column or page when the current one runs out.
    fn line(&mut self, size: f32, spacing: f32) -> (f32, f32) {
        if self.y - size < self.bottom {
            self.advance();
        }
        let (x, y) = (self.col_x(), self.y - size);
        self.y -= size * 1.45 * spacing;
        (x, y)
    }

    /// Reserve a whole box (image/table row) that must not split across columns.
    fn atom(&mut self, h: f32) -> (f32, f32) {
        if self.y - h < self.bottom {
            self.advance();
        }
        let (x, y) = (self.col_x(), self.y - h);
        self.y -= h;
        (x, y)
    }
}

fn ink(style: Style) -> [u8; 3] {
    match style {
        Style::Quote => [70, 70, 80],
        Style::Mono | Style::CodeBlock | Style::Table => [30, 30, 34],
        _ => [15, 15, 18],
    }
}

/// Greedy word-wrap over an estimated character budget (`0.5 em` average advance).
fn wrap(text: &str, width_chars: usize, justify: bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if line.chars().count() + word.chars().count() + 1 > width_chars && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    // Justify every line of a paragraph but its last (capped: the estimate paints a
    // proportional face in the preview).
    if justify && lines.len() > 1 {
        let last = lines.len() - 1;
        for (i, l) in lines.iter_mut().enumerate() {
            if i != last {
                *l = justify_pad(l, width_chars);
            }
        }
    }
    lines
}

/// Spread the deficit between words so the right edge lines up.
fn justify_pad(line: &str, width_chars: usize) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 2 {
        return line.to_string();
    }
    let gaps = words.len() - 1;
    let deficit = (width_chars + 1).saturating_sub(line.chars().count());
    let per = (deficit / gaps).min(5);
    let mut s = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            for _ in 0..=per {
                s.push(' ');
            }
        }
        s.push_str(w);
    }
    s
}

pub fn layout(meta: &DocMeta, blocks: &[Block], st: &Settings) -> Layouted {
    let sp = st.style.line_spacing.max(0.8);
    let base = st.style.base_size.clamp(6.0, 32.0);
    let mut f = Flow::new(st);
    let mut out = Layouted::default();
    let mut enum_n = 0usize;
    let sz = |k: f32| k * base;

    // Title page header
    if !meta.title.is_empty() {
        let s = sz(2.3);
        let (x, y) = f.line(s, sp);
        f.page().runs.push(Run { x, y, text: meta.title.clone(), size: s, bold: true, italic: false, color: [10, 10, 10] });
        f.y -= sz(0.7);
        if !meta.author.is_empty() {
            let s = sz(1.25);
            let (x, y) = f.line(s, sp);
            f.page().runs.push(Run { x, y, text: meta.author.clone(), size: s, bold: false, italic: true, color: [60, 60, 60] });
            f.y -= sz(0.5);
        }
        if !meta.date.is_empty() {
            let s = sz(1.05);
            let (x, y) = f.line(s, sp);
            f.page().runs.push(Run { x, y, text: meta.date.clone(), size: s, bold: false, italic: false, color: [90, 90, 90] });
            f.y -= sz(0.6);
        }
        f.y -= sz(0.4);
    }

    for b in blocks {
        let text: String = b.runs.iter().map(|(t, _, _)| t.as_str()).collect();
        match b.style {
            Style::Break => {
                if f.pages.last().is_some_and(|p| !p.runs.is_empty() || !p.frames.is_empty()) {
                    f.pages.push(Page::default());
                    f.col = 0;
                    f.y = f.top;
                }
                enum_n = 0;
                continue;
            }
            Style::Image => {
                let s = sz(0.9);
                let (path, caption) = match text.split_once(';') {
                    Some((a, c)) => (a.trim(), c.trim()),
                    None => (text.trim(), ""),
                };
                let h = (f.col_w * 0.34).clamp(48.0, 200.0);
                let block_h = h + if caption.is_empty() { 0.0 } else { s * 2.4 };
                let (x, top_y) = f.atom(block_h);
                let col_w = f.col_w;
                let p = f.page();
                p.frames.push(Frame { x, y: top_y, w: col_w, h, color: [120, 120, 130] });
                let label = if path.is_empty() { "[image]".to_string() } else { format!("[{path}]") };
                let lw = label.chars().count() as f32 * s * 0.55;
                p.runs.push(Run { x: x + (col_w - lw).max(0.0) / 2.0, y: top_y - h / 2.0 - s * 0.5, text: label.to_string(), size: s, bold: false, italic: false, color: [120, 120, 130] });
                if !caption.is_empty() {
                    let cx = x + (col_w - caption.chars().count() as f32 * s * 0.24).max(0.0) / 2.0;
                    p.runs.push(Run { x: cx, y: top_y - h - s * 1.7, text: caption.to_string(), size: s, bold: false, italic: true, color: [80, 80, 88] });
                }
                continue;
            }
            Style::Table => {
                let cells: Vec<&str> = text.trim_matches('|').split('|').map(str::trim).collect();
                let s = sz(0.9);
                let n = cells.len().max(1);
                let cw = f.col_w / n as f32;
                let row_h = s * 2.1 * sp;
                let (x, top_y) = f.atom(row_h);
                let p = f.page();
                for (j, c) in cells.iter().enumerate() {
                    p.frames.push(Frame { x: x + j as f32 * cw, y: top_y, w: cw, h: row_h, color: [150, 150, 158] });
                    p.runs.push(Run { x: x + j as f32 * cw + 5.0, y: top_y - row_h * 0.5 - s * 0.5, text: (*c).to_string(), size: s, bold: false, italic: false, color: [30, 30, 34] });
                }
                continue;
            }
            _ => {}
        }
        let (k, indent, gap) = match b.style {
            Style::Title => (2.3, 0.0, 10.0),
            Style::Chapter => (1.9, 0.0, 14.0),
            Style::Section => (1.55, 0.0, 11.0),
            Style::Subsection => (1.28, 0.0, 8.0),
            Style::Body => (1.0, 0.0, 5.0),
            Style::Bullet | Style::Numbered => (1.0, 24.0, 3.0),
            Style::Quote => (1.0, 24.0, 5.0),
            Style::Mono => (0.95, 18.0, 4.0),
            Style::CodeBlock => (0.92, 18.0, 3.0),
            Style::Center | Style::Right => (1.0, 0.0, 6.0),
            _ => (1.0, 0.0, 5.0),
        };
        if b.style == Style::Numbered {
            enum_n += 1;
        } else if matches!(b.style, Style::Body | Style::Chapter | Style::Section) {
            enum_n = 0;
        }
        let prefix = match b.style {
            Style::Bullet => "•  ".to_string(),
            Style::Numbered => format!("{enum_n}.  "),
            _ => String::new(),
        };
        f.y -= gap * (base / 11.0);
        let size = sz(k);
        let width_chars = ((f.col_w - indent) / (size * 0.5)).max(16.0) as usize;
        let lines = wrap(&format!("{prefix}{text}"), width_chars, st.style.justify && matches!(b.style, Style::Body | Style::Quote | Style::Center | Style::Bullet | Style::Numbered));
        for (i, l) in lines.into_iter().enumerate() {
            let (x, y) = f.line(size, sp);
            if i == 0 && matches!(b.style, Style::Chapter | Style::Section | Style::Subsection) {
                out.headings.push((match b.style { Style::Chapter => 1, Style::Section => 2, _ => 3 }, text.clone(), f.pages.len() - 1));
            }
            // \right{…} pushes its first line to the right edge of the column.
            let right = if b.style == Style::Right && i == 0 {
                (f.col_w - indent - l.chars().count() as f32 * size * 0.5).max(0.0)
            } else {
                0.0
            };
            f.page().runs.push(Run {
                x: x + indent + right,
                y,
                text: l,
                size,
                bold: matches!(b.style, Style::Title | Style::Chapter | Style::Section | Style::Subsection),
                italic: b.style == Style::Quote,
                color: ink(b.style),
            });
        }
    }

    out.pages = std::mem::take(&mut f.pages);

    // Folios, footers and running heads, now that the pages exist.
    let (pw, ph) = st.page.page_size();
    let has_title = !meta.title.is_empty();
    let footer_y = st.page.margin_bottom * 0.5;
    let head = if has_title { meta.title.clone() } else { "DocCraft Typeset".to_string() };
    for (i, p) in out.pages.iter_mut().enumerate() {
        let n = i + 1;
        let plain = st.hf.first_different && i == 0;
        if !(plain || (i == 0 && has_title)) {
            p.runs.push(Run { x: st.page.margin_left, y: ph - st.page.margin_top * 0.5, text: head.clone(), size: 8.5, bold: false, italic: true, color: [150, 150, 160] });
        }
        if !plain {
            if !st.hf.footer.is_empty() {
                let x = (pw - st.hf.footer.chars().count() as f32 * 8.5 * 0.25) / 2.0;
                p.runs.push(Run { x, y: footer_y, text: st.hf.footer.clone(), size: 8.5, bold: false, italic: false, color: [110, 110, 120] });
            }
            if st.hf.numbers {
                // A centred folio would collide with footer text, so it steps outside then.
                let outside = !st.hf.footer.is_empty() || st.hf.number_pos == NumPos::Outside;
                let x = if outside && n % 2 == 0 {
                    st.page.margin_left
                } else if outside {
                    pw - st.page.margin_right - 10.0
                } else {
                    pw / 2.0 - 5.0
                };
                p.runs.push(Run { x, y: footer_y, text: format!("{n}"), size: 9.0, bold: false, italic: false, color: [110, 110, 120] });
            }
        }
    }
    out.pages.retain(|p| !p.runs.is_empty() || !p.frames.is_empty());
    out
}

/// Plain-text render for the Export › .txt command.
pub fn plain_text(parsed: &Parsed) -> String {
    let mut s = String::new();
    if !parsed.meta.title.is_empty() {
        s.push_str(&parsed.meta.title);
        s.push_str("\n\n");
    }
    for b in &parsed.blocks {
        let text: String = b.runs.iter().map(|(t, _, _)| t.as_str()).collect();
        match b.style {
            Style::Chapter => {
                s.push_str(&text.to_uppercase());
                s.push_str("\n\n");
            }
            Style::Break => s.push_str("― page break ―\n\n"),
            Style::Image => s.push_str(&format!("[image: {text}]\n\n")),
            Style::Table => s.push_str(&format!("{text}\n")),
            _ => s.push_str(&format!("{text}\n\n")),
        }
    }
    s
}

/// Minimal PDF export: valid xref, Type1 base-14 (Helvetica or Courier family),
/// one content stream per page, MediaBox from the page setup.
pub fn export_pdf(meta: &DocMeta, blocks: &[Block], st: &Settings) -> Vec<u8> {
    let Layouted { pages, .. } = layout(meta, blocks, st);
    let (pw, ph) = st.page.page_size();
    let fam = if st.style.mono_font { ("Courier", "Courier-Bold", "Courier-Oblique") } else { ("Helvetica", "Helvetica-Bold", "Helvetica-Oblique") };
    let mut objects: Vec<String> = Vec::new();
    // 1: catalog, 2: pages, 3..6: fonts, then per page: content + page object.
    let n_pages = pages.len().max(1);
    let mut page_ids = Vec::new();
    let first_page_obj = 6usize;
    for i in 0..n_pages {
        page_ids.push(first_page_obj + i * 2 + 1);
    }
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into()); // 1
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n_pages)); // 2
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} >>", fam.0)); // 3
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} >>", fam.1)); // 4
    objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} >>", fam.2)); // 5
    for p in &pages {
        let content_id = objects.len() as u32 + 1;
        let mut s = String::from("0.6 w\n");
        for fr in &p.frames {
            let col = format!("{} {} {}", fr.color[0] as f32 / 255.0, fr.color[1] as f32 / 255.0, fr.color[2] as f32 / 255.0);
            s.push_str(&format!("{col} RG {} {} {} {} re S\n", fr.x, fr.y - fr.h, fr.w, fr.h));
        }
        for r in &p.runs {
            let font = if r.bold { "F2" } else if r.italic { "F3" } else { "F1" };
            let col = format!("{} {} {}", r.color[0] as f32 / 255.0, r.color[1] as f32 / 255.0, r.color[2] as f32 / 255.0);
            let text = r.text.replace(['\\', '(', ')'], "");
            s.push_str(&format!("BT {col} rg /{font} {} Tf 1 0 0 1 {} {} Tm ({}) Tj ET\n", r.size, r.x, r.y, text));
        }
        objects.push(format!("<< /Length {} >>\nstream\n{}\nendstream", s.len(), s));
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.0} {ph:.0}] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> /Contents {content_id} 0 R >>"
        ));
    }
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(pdf.len() as u32);
        pdf.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
    }
    let xref_at = pdf.len() as u32;
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R /Info << /Title ({}) /Producer (DocCraft Typeset) >> >>\nstartxref\n{xref_at}\n%%EOF",
        objects.len() + 1,
        meta.title.replace(['(', ')'], "")
    ));
    pdf.into_bytes()
}
