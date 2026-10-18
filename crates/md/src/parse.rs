//! A pragmatic CommonMark-subset parser (headings, emphasis, strikethrough,
//! inline code, links, footnotes, fenced code, lists, task lists, quotes,
//! rules and pipe tables) plus outline extraction and document statistics.

use crate::model::{Block, Heading, Inline};

pub fn parse(src: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = src.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(h) = trimmed.strip_prefix('#') {
            let level = h.len().min(6) as u8;
            let text = h.trim_start_matches('#').trim_start();
            if !text.is_empty() || level == 1 {
                blocks.push(Block::Heading { level, text: parse_inline(text) });
                continue;
            }
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            blocks.push(Block::Rule);
            continue;
        }
        if let Some(def) = trimmed.strip_prefix("[^")
            && let Some((label, text)) = def.split_once("]:")
        {
            blocks.push(Block::FootnoteDef { label: label.trim().to_string(), text: parse_inline(text.trim()) });
            continue;
        }
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').to_string();
            let mut code = Vec::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push(l);
            }
            blocks.push(Block::Code { lang, code: code.join("\n") });
            continue;
        }
        if trimmed.starts_with('>') {
            let mut inner = vec![trimmed.trim_start_matches('>').trim_start().to_string()];
            while let Some(l) = lines.peek()
                && l.trim_start().starts_with('>')
            {
                inner.push(l.trim_start().trim_start_matches('>').trim_start().to_string());
                lines.next();
            }
            blocks.push(Block::Quote(inner.iter().map(|p| parse_inline(p)).collect()));
            continue;
        }
        if trimmed.starts_with("| ") || (trimmed.starts_with('|') && trimmed.ends_with('|')) {
            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut first = true;
            let mut head = Vec::new();
            for l in std::iter::once(line).chain(lines.by_ref().take_while(|l| l.trim_start().starts_with('|'))) {
                let cells: Vec<String> = l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
                if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) && cells.len() > 1 {
                    continue; // separator row
                }
                if first {
                    head = cells;
                    first = false;
                } else {
                    rows.push(cells);
                }
            }
            blocks.push(Block::Table { head, rows });
            continue;
        }
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let mut items: Vec<Vec<Inline>> = Vec::new();
            let mut tasks: Vec<(bool, Vec<Inline>)> = Vec::new();
            let mut rest = trimmed[2..].to_string();
            loop {
                let (done, body) = if let Some(r) = rest.strip_prefix("[ ] ") {
                    (Some(false), r)
                } else if let Some(r) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                    (Some(true), r)
                } else {
                    (None, rest.as_str())
                };
                match done {
                    Some(d) => tasks.push((d, parse_inline(body))),
                    None if tasks.is_empty() => items.push(parse_inline(&rest)),
                    None => tasks.push((false, parse_inline(&rest))),
                }
                match lines.peek() {
                    Some(l)
                        if l.trim_start().starts_with("- ")
                            || l.trim_start().starts_with("* ") =>
                    {
                        rest = l.trim_start()[2..].to_string();
                        lines.next();
                    }
                    _ => break,
                }
            }
            if !tasks.is_empty() {
                blocks.push(Block::Task(tasks));
            } else {
                blocks.push(Block::Bullet(items));
            }
            continue;
        }
        if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains(". ") {
            let mut items = Vec::new();
            let mut rest = trimmed.split_once(". ").unwrap().1.to_string();
            loop {
                items.push(parse_inline(&rest));
                match lines.peek() {
                    Some(l)
                        if l.trim_start().starts_with(|c: char| c.is_ascii_digit())
                            && l.trim_start().contains(". ") =>
                    {
                        rest = l.trim_start().split_once(". ").unwrap().1.to_string();
                        lines.next();
                    }
                    _ => break,
                }
            }
            blocks.push(Block::Numbered(items));
            continue;
        }
        // paragraph: join until blank line or the start of another block
        let mut para = vec![trimmed.to_string()];
        while let Some(l) = lines.peek()
            && !l.trim().is_empty()
            && !l.trim_start().starts_with('#')
            && !l.trim_start().starts_with("```")
            && !l.trim_start().starts_with('>')
            && !l.trim_start().starts_with("- ")
            && !l.trim_start().starts_with('|')
        {
            para.push(l.trim().to_string());
            lines.next();
        }
        blocks.push(Block::Paragraph(parse_inline(&para.join(" "))));
    }
    blocks
}

pub fn parse_inline(s: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        // earliest of the inline markers
        let mut best: Option<(usize, char)> = None;
        for m in ['*', '`', '[', '~'] {
            if let Some(i) = rest.find(m)
                && best.map_or(true, |(bi, _)| i < bi)
            {
                best = Some((i, m));
            }
        }
        let Some((i, m)) = best else {
            out.push(Inline::Text(rest.to_string()));
            break;
        };
        if i > 0 {
            out.push(Inline::Text(rest[..i].to_string()));
        }
        match m {
            '*' => {
                let after = &rest[i + 1..];
                if let Some(b) = after.find('*') {
                    // ** bold when the closing is immediately doubled
                    if after.starts_with('*') && let Some(b2) = after[1..].find("**") {
                        out.push(Inline::Bold(parse_inline(&after[1..1 + b2])));
                        rest = &after[1 + b2 + 2..];
                        continue;
                    }
                    out.push(Inline::Italic(parse_inline(&after[..b])));
                    rest = &after[b + 1..];
                } else {
                    out.push(Inline::Text("*".into()));
                    rest = after;
                }
            }
            '~' => {
                let after = &rest[i + 1..];
                if after.starts_with('~') && let Some(b) = after[1..].find("~~") {
                    out.push(Inline::Strike(parse_inline(&after[1..1 + b])));
                    rest = &after[1 + b + 2..];
                } else {
                    out.push(Inline::Text("~".into()));
                    rest = after;
                }
            }
            '`' => {
                let after = &rest[i + 1..];
                if let Some(b) = after.find('`') {
                    out.push(Inline::Code(after[..b].to_string()));
                    rest = &after[b + 1..];
                } else {
                    out.push(Inline::Text("`".into()));
                    rest = after;
                }
            }
            _ => {
                // '[' link — only when the closing "](url)" exists
                if let Some(mid) = rest[i..].find("](")
                    && let Some(endrel) = rest[i + mid + 2..].find(')')
                {
                    let label = rest[i + 1..i + mid].to_string();
                    let url = rest[i + mid + 2..i + mid + 2 + endrel].to_string();
                    out.push(Inline::Link { label, url });
                    rest = &rest[i + mid + 2 + endrel + 1..];
                } else if rest[i..].starts_with("[^")
                    && let Some(end) = rest[i + 2..].find(']')
                    && !rest[i + 2..i + 2 + end].contains(' ')
                {
                    out.push(Inline::FootnoteRef(rest[i + 2..i + 2 + end].to_string()));
                    rest = &rest[i + 2 + end + 1..];
                } else {
                    out.push(Inline::Text("[".into()));
                    rest = &rest[i + 1..];
                }
            }
        }
    }
    out
}

/// The document outline: every heading with its level, text, source line and
/// character offset. Live-computed, cheap on editor-sized documents.
pub fn outline(src: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut off = 0usize; // character offset of the line start
    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes >= 1 && hashes <= 6 {
            let rest = &trimmed[hashes..];
            if rest.trim().is_empty() || rest.starts_with(' ') {
                let text = rest.trim().to_string();
                let indent = line.chars().count() - trimmed.chars().count();
                out.push(Heading { level: hashes as u8, text, line: i + 1, char_off: off + indent });
            }
        }
        off += line.chars().count() + 1;
    }
    out
}

/// Document statistics: (words, chars, headings).
pub fn stats(src: &str) -> (usize, usize, usize) {
    (
        src.split_whitespace().count(),
        src.chars().count(),
        outline(src).len(),
    )
}

/// Reading time in whole minutes at ~200 wpm (at least 1).
pub fn reading_min(words: usize) -> usize {
    (words / 200).max(1)
}

/// Plain text (for status bar word counts).
pub fn word_count(src: &str) -> usize {
    src.split_whitespace().count()
}
