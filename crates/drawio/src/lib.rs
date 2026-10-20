//! DocCraft Draw — the native drawio craft.
//!
//! Model + XML I/O for the drawio (diagrams.net / mxGraph) format and a
//! diagrams.net-style editor UI. Built like the storytold craft crates: a
//! typed model, lossless file I/O, and an egui front end that edits the model
//! in place.
//!
//! Module map:
//! - [`model`]      — mxGraph document model (pages, cells, layers, groups)
//! - [`connection`] — connection engine (fixed points, perimeter math, drags)
//! - [`xmlio`]      — lossless `.drawio` XML reading/writing
//! - [`style`]      — mxGraph style-string parsing/editing, swatches, arrows
//! - [`shapes`]     — the shape library catalog + geometry painters
//! - [`colorpicker`]- the FigCraft-style colour picker popover
//! - [`canvas`]     — the drawing canvas (interaction, routing, guides, minimap)
//! - [`ui`]         — the editor shell (menus, toolbar, panels, dialogs)
//! - [`export`]     — SVG / PNG / PDF export of a page

pub mod canvas;
pub mod colorpicker;
pub mod connection;
pub mod export;
pub mod model;
pub mod shapes;
pub mod style;
pub mod ui;
pub mod xmlio;

pub use model::{Cell, Diagram, Geom, Layer, MxFile};
