//! The typeset source model: a `\command {…}` markup parsed into styled blocks.
//! Ribbon-friendly shorthand (`#`, `##`, `###`, `> `, fences, `|` tables) parses too.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Title,
    Chapter,
    Section,
    Subsection,
    Body,
    Bullet,
    Numbered,
    Quote,
    Mono,
    CodeBlock,
    Center,
    Right,
    Table,
    Image,
    Break,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub style: Style,
    pub runs: Vec<(String, bool, bool)>, // text, bold, italic
    pub line: usize,                     // 1-based source line (error reporting)
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct DocMeta {
    pub title: String,
    pub author: String,
    pub date: String,
}

/// A problem the validator found, with its source line (Recompile pane shows it in red).
#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub msg: String,
}

#[derive(Debug, Clone, Default)]
pub struct Parsed {
    pub meta: DocMeta,
    pub blocks: Vec<Block>,
    pub error: Option<ParseError>,
}

pub fn parse(src: &str) -> Parsed {
    let mut p = Parsed { meta: DocMeta { title: String::new(), author: String::new(), date: String::new() }, blocks: Vec::new(), error: None };
    let mut in_code = false;
    for (i, raw) in src.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if !in_code {
            if line.is_empty() || line.starts_with('%') {
                continue;
            }
            // directives that carry metadata or breaks
            if let Some(t) = arg(line, "\\title") {
                p.meta.title = t;
                continue;
            } else if let Some(t) = arg(line, "\\author") {
                p.meta.author = t;
                continue;
            } else if let Some(t) = arg(line, "\\date") {
                p.meta.date = t;
                continue;
            } else if line == "\\pagebreak" {
                p.blocks.push(Block { style: Style::Break, runs: Vec::new(), line: n });
                continue;
            }
        }
        let (style, text): (Style, String) = if line == "```" {
            in_code = !in_code;
            continue;
        } else if in_code || line.starts_with("```") {
            (Style::CodeBlock, line.trim_start_matches("```").to_string())
        } else if let Some(t) = line.strip_prefix("### ") {
            (Style::Subsection, t.trim().to_string())
        } else if let Some(t) = line.strip_prefix("## ") {
            (Style::Section, t.trim().to_string())
        } else if let Some(t) = line.strip_prefix("# ") {
            (Style::Chapter, t.trim().to_string())
        } else if let Some(t) = arg(line, "\\section") {
            (Style::Section, t)
        } else if let Some(t) = arg(line, "\\subsection") {
            (Style::Subsection, t)
        } else if let Some(t) = arg(line, "\\image") {
            (Style::Image, t)
        } else if let Some(t) = arg(line, "\\center") {
            (Style::Center, t)
        } else if let Some(t) = arg(line, "\\right") {
            (Style::Right, t)
        } else if let Some(t) = line.strip_prefix("\\item ").or_else(|| line.strip_prefix("- ")) {
            (Style::Bullet, t.to_string())
        } else if let Some(t) = line.strip_prefix("\\enum ") {
            (Style::Numbered, t.to_string())
        } else if let Some(t) = line.strip_prefix("\\quote ").or_else(|| line.strip_prefix("> ")) {
            (Style::Quote, t.to_string())
        } else if let Some(t) = arg(line, "\\code") {
            (Style::Mono, t)
        } else if line.starts_with('|') && line.len() > 1 {
            (Style::Table, line.to_string())
        } else {
            (Style::Body, line.to_string())
        };
        // unterminated \command{ args: report the line instead of silently eating text
        if matches!(style, Style::Center | Style::Right | Style::Image | Style::Mono) && text.is_empty() {
            p.error = Some(ParseError { line: n, msg: format!("\\{cmd}{{…}} is missing its closing brace", cmd = match style { Style::Center => "center", Style::Right => "right", Style::Image => "image", _ => "code" }) });
        }
        let runs = if matches!(style, Style::CodeBlock | Style::Table | Style::Image) { vec![(text, false, false)] } else { runs_from_markup(&text) };
        p.blocks.push(Block { style, runs, line: n });
    }
    if in_code && p.error.is_none() {
        p.error = Some(ParseError { line: src.lines().count(), msg: "a ``` code fence was opened but never closed".into() });
    }
    p
}

/// `\command{argument}` → `argument`, when the line is exactly that form.
fn arg(line: &str, cmd: &str) -> Option<String> {
    line.strip_prefix(cmd)?.strip_prefix('{')?.strip_suffix('}').map(str::to_string)
}

/// An outline entry: (level 1..=3, text).
pub fn headings(blocks: &[Block]) -> Vec<(u8, String)> {
    blocks
        .iter()
        .filter_map(|b| match b.style {
            Style::Chapter => Some((1u8, flat(b))),
            Style::Section => Some((2, flat(b))),
            Style::Subsection => Some((3, flat(b))),
            _ => None,
        })
        .collect()
}

fn flat(b: &Block) -> String {
    b.runs.iter().map(|(t, _, _)| t.as_str()).collect()
}

pub fn word_count(src: &str) -> usize {
    src.split_whitespace().count()
}

/// `**bold**` and `*italic*` inside a line become separate runs.
fn runs_from_markup(s: &str) -> Vec<(String, bool, bool)> {
    let mut runs = Vec::new();
    let mut rest = s;
    let mut bold = false;
    let mut italic = false;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**") {
            bold = !bold;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('*') {
            italic = !italic;
            rest = r;
        } else {
            let next = rest.find("**").map(|i| i).or(rest.find('*')).unwrap_or(rest.len());
            let (chunk, tail) = rest.split_at(next);
            if !chunk.is_empty() {
                runs.push((chunk.to_string(), bold, italic));
            }
            rest = if tail.starts_with("**") { &tail[2..] } else { &tail[1.min(tail.len())..] };
            if tail.is_empty() {
                break;
            }
        }
    }
    if runs.is_empty() {
        runs.push((String::new(), false, false));
    }
    runs
}
