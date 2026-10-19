//! DocCraft Vector — the native vector craft.
//!
//! An Adobe-Illustrator-style editor: slim menu bar, control bar with
//! scrubby X/Y/W/H + fill/stroke swatches + stroke width + opacity + align,
//! a two-column toolbox (16 tools with keyboard shortcuts), a right dock
//! (Properties / Layers / Swatches), labeled rulers, an 8 pt / 64 pt line
//! grid on the artboard with snap-to-grid + smart guides, square selection
//! handles, bezier pen editing (anchors + direction handles), freehand
//! brush/pencil strokes, polygons, an erase/eyedropper/gradient/smooth
//! toolkit and a zoom/pan status bar.
//!
//! Model: rect / ellipse / line / poly / path shapes with fill (+ optional
//! two-stop gradient) and stroke, freehand strokes and text items. Files are
//! a minimal hand-rolled SVG (`.svg`) writer plus a small roxmltree-based
//! reader — no heavy dependencies.
//!
//! Module map:
//! - [`model`]  — document model (shapes / strokes / text), geometry, hit tests
//! - [`svg`]    — minimal `.svg` export + import
//! - [`json`]   — optional `serde_json` document serialization (`json` feature)
//! - [`canvas`] — painting engine (grid, rulers, guides, selection, paths)
//! - [`ui`]     — the editor shell ([`ui::VectorApp`])

pub mod canvas;
#[cfg(feature = "json")]
pub mod json;
pub mod model;
pub mod svg;
pub mod ui;

pub use model::{Anchor, Doc, FreeStroke, Kind, Node, Shape, TextItem};
pub use ui::VectorApp;
