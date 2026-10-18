//! DocCraft Typeset — the native typesetting craft.
//!
//! A small typesetting pipeline of its own: markup source → styled blocks →
//! broken, justified, paginated A4 pages → live preview (+ a minimal PDF
//! export using Type1 base-14 fonts, like the classic TeX engines do).

pub mod model;
pub mod layout;
pub mod panes;
pub mod ribbon;
pub mod settings;
pub mod ui;

pub use layout::{Page, Run};
