//! DocCraft Markdown — the native Markdown craft.
//!
//! A CommonMark-subset parser producing a typed block model, and a
//! WordCraft-anatomy editor: title strip + Quick Access Toolbar, ribbon tabs
//! with grouped command strips, live preview pane, outline, a settings well
//! and a status bar — in the same visual language as the real crafts.

pub mod model;
pub mod parse;
pub mod preview;
pub mod ribbon;
pub mod settings;
pub mod ui;

pub use model::{Block, Heading, Inline};
pub use settings::{Accent, PreviewPos, Settings};
pub use ui::MdApp;
