//! The Markdown document model.

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, text: Vec<Inline> },
    Paragraph(Vec<Inline>),
    Bullet(Vec<Vec<Inline>>),
    Numbered(Vec<Vec<Inline>>),
    /// `- [ ]` / `- [x]` task items: (done, text)
    Task(Vec<(bool, Vec<Inline>)>),
    Quote(Vec<Vec<Inline>>),
    Code { lang: String, code: String },
    Rule,
    Table { head: Vec<String>, rows: Vec<Vec<String>> },
    /// `[^label]: text` definitions, rendered small at the end.
    FootnoteDef { label: String, text: Vec<Inline> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link { label: String, url: String },
    /// `[^1]` reference; rendered as a small accent mark.
    FootnoteRef(String),
}

/// One entry of the document outline (View › Outline pane).
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// 1-based source line the heading starts on.
    pub line: usize,
    /// Character offset of the heading in the source (for caret jumps).
    pub char_off: usize,
}
