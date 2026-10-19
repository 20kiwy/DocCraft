//! The Vector editor shell: Illustrator-style slim menu bar (real egui
//! menus), control bar (fill/stroke swatches, scrubby X/Y/W/H, stroke width,
//! opacity, align), a two-column toolbox with 17 tools + keyboard
//! shortcuts, the right dock (collapsible Properties / Layers / Swatches),
//! rulers + 8/64 pt grid canvas with smart guides and a status bar with a
//! zoom slider, tool hint and document size.

use crate::canvas::{self, Xform};
use crate::model::*;
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Key, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use std::time::Instant;

// ── dark chrome palette (same family as the drawio craft) ────────────────────
pub const ACCENT: Color32 = Color32::from_rgb(0xf0, 0x87, 0x2d);
const PANEL: Color32 = Color32::from_rgb(0x23, 0x23, 0x27);
const PANEL2: Color32 = Color32::from_rgb(0x26, 0x26, 0x2c);
const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x22);
const BORDER: Color32 = Color32::from_rgb(0x34, 0x34, 0x3c);
const TEXT: Color32 = Color32::from_rgb(0xd8, 0xd8, 0xde);
const DIM: Color32 = Color32::from_rgb(0x9a, 0x9a, 0xa4);
const AI_BLUE: Color32 = canvas::AI_BLUE;

// ── tools ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Direct,
    Pen,
    Anchor,
    Type,
    Line,
    Rect,
    Ellipse,
    Poly,
    Brush,
    Pencil,
    Smooth,
    Erase,
    Gradient,
    Dropper,
    Hand,
    Zoom,
}

pub const TOOLS: [Tool; 17] = [
    Tool::Select,
    Tool::Direct,
    Tool::Pen,
    Tool::Anchor,
    Tool::Type,
    Tool::Line,
    Tool::Rect,
    Tool::Ellipse,
    Tool::Poly,
    Tool::Brush,
    Tool::Pencil,
    Tool::Smooth,
    Tool::Erase,
    Tool::Gradient,
    Tool::Dropper,
    Tool::Hand,
    Tool::Zoom,
];

impl Tool {
    pub fn key(self) -> Option<Key> {
        Some(match self {
            Tool::Select => Key::V,
            Tool::Direct => Key::A,
            Tool::Pen => Key::P,
            Tool::Anchor => Key::Plus,
            Tool::Type => Key::T,
            Tool::Line => Key::Backslash,
            Tool::Rect => Key::M,
            Tool::Ellipse => Key::L,
            Tool::Poly => Key::Y,
            Tool::Brush => Key::B,
            Tool::Pencil => Key::N,
            Tool::Smooth => Key::S,
            Tool::Erase => Key::E,
            Tool::Gradient => Key::G,
            Tool::Dropper => Key::I,
            Tool::Hand => Key::H,
            Tool::Zoom => Key::Z,
        })
    }
    pub fn key_label(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Direct => "A",
            Tool::Pen => "P",
            Tool::Anchor => "+",
            Tool::Type => "T",
            Tool::Line => "\\",
            Tool::Rect => "M",
            Tool::Ellipse => "L",
            Tool::Poly => "Y",
            Tool::Brush => "B",
            Tool::Pencil => "N",
            Tool::Smooth => "S",
            Tool::Erase => "E",
            Tool::Gradient => "G",
            Tool::Dropper => "I",
            Tool::Hand => "H",
            Tool::Zoom => "Z",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Selection",
            Tool::Direct => "Direct Selection",
            Tool::Pen => "Pen",
            Tool::Anchor => "Add / Remove Anchor",
            Tool::Type => "Type",
            Tool::Line => "Line Segment",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Poly => "Polygon",
            Tool::Brush => "Paintbrush",
            Tool::Pencil => "Pencil",
            Tool::Smooth => "Smooth",
            Tool::Erase => "Eraser",
            Tool::Gradient => "Gradient",
            Tool::Dropper => "Eyedropper",
            Tool::Hand => "Hand",
            Tool::Zoom => "Zoom",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Drag to select · drag handles to resize · double-click text to edit",
            Tool::Direct => "Drag the square anchors of a path · pull direction handles",
            Tool::Pen => "Click to add anchors · drag to curve · Enter commits · Esc cancels · click the first anchor to close",
            Tool::Anchor => "Click a path segment to insert an anchor · click an anchor to remove it",
            Tool::Type => "Click on the artboard to place a text item",
            Tool::Line => "Drag to draw a straight line segment",
            Tool::Rect => "Drag to draw a rectangle (snaps to the 8 pt grid)",
            Tool::Ellipse => "Drag to draw an ellipse",
            Tool::Poly => "Drag to draw a regular polygon (set the sides in the control bar)",
            Tool::Brush => "Drag to paint a smoothed freehand stroke",
            Tool::Pencil => "Drag to draw a raw freehand stroke",
            Tool::Smooth => "Click a path to round its anchors · click a stroke to smooth it",
            Tool::Erase => "Click or drag across items to erase them",
            Tool::Gradient => "Click a shape to toggle a two-stop vertical gradient fill",
            Tool::Dropper => "Click an item to pick its fill / stroke / weight",
            Tool::Hand => "Drag to pan · scroll wheel pans · Ctrl + wheel zooms",
            Tool::Zoom => "Click to zoom in · Alt-click to zoom out",
        }
    }
}

// ── drag state ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Drag {
    None,
    Pan { last: Pos2 },
    Move { grab: Pos2, orig: Vec<(u64, Rect)>, bounds: Rect, moved: bool },
    Resize { handle: usize, orig: Rect },
    Marquee { start: Pos2 },
    Draw { start: Pos2 },
    DrawLine { start: Pos2 },
    Freehand,
    AnchorMove { id: u64, idx: usize },
    HandleDrag { id: u64, idx: usize, out: bool },
}

pub struct VectorApp {
    pub doc: Doc,
    pub path: Option<String>,
    pub dirty: bool,
    pub tool: Tool,
    pub sel: Vec<u64>,

    // pen
    pen: Vec<Anchor>,
    pen_drag: Option<usize>,

    // freehand pending
    pending: Vec<Pos2>,

    drag: Drag,
    pub pan: Pos2,
    pub zoom: f32,

    // defaults for new items
    pub fill: Color32,
    pub stroke: Color32,
    pub stroke_w: f32,
    pub opacity: f32,
    pub poly_sides: usize,
    pick_fill: bool,

    // view flags
    pub show_grid: bool,
    pub snap_grid: bool,
    pub show_rulers: bool,
    pub show_guides: bool,
    pub open_props: bool,
    pub open_layers: bool,
    pub open_swatches: bool,

    // guides in screen coords (computed while dragging, painted same frame)
    guides_v: Vec<f32>,
    guides_h: Vec<f32>,
    view_rect: Option<Rect>,

    undo: Vec<Doc>,
    redo: Vec<Doc>,
    undo_tag: Option<String>,
    undo_at: Option<Instant>,

    editing: Option<(u64, String)>,
    edit_fresh: bool,

    clip: Vec<Node>,
    pub status: String,
    pub pending_export: bool,
}

impl Default for VectorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorApp {
    pub fn new() -> Self {
        Self {
            doc: Doc::sample(),
            path: None,
            dirty: false,
            tool: Tool::Select,
            sel: Vec::new(),
            pen: Vec::new(),
            pen_drag: None,
            pending: Vec::new(),
            drag: Drag::None,
            pan: pos2(90.0, 56.0),
            zoom: 1.0,
            fill: Color32::WHITE,
            stroke: Color32::from_rgb(0x26, 0x26, 0x26),
            stroke_w: 1.0,
            opacity: 1.0,
            poly_sides: 6,
            pick_fill: true,
            show_grid: true,
            snap_grid: true,
            show_rulers: true,
            show_guides: true,
            open_props: true,
            open_layers: true,
            open_swatches: true,
            guides_v: Vec::new(),
            guides_h: Vec::new(),
            view_rect: None,
            undo: Vec::new(),
            redo: Vec::new(),
            undo_tag: None,
            undo_at: None,
            editing: None,
            edit_fresh: false,
            clip: Vec::new(),
            status: "Ready — Vector craft (Illustrator-style native)".into(),
            pending_export: false,
        }
    }

    // ── host API ─────────────────────────────────────────────────────────────

    /// Import an `.svg` (host file routing). Non-svg payloads are rejected.
    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        match crate::svg::parse(bytes) {
            Ok((doc, skipped)) => {
                self.doc = doc;
                self.path = Some(name.to_string());
                self.sel.clear();
                self.undo.clear();
                self.redo.clear();
                self.dirty = false;
                self.status = format!("Opened {name}{} nodes, {skipped} skipped", if skipped == 1 { " — 1 " } else { " — " });
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    /// Serialize the document as a standalone `.svg`.
    pub fn export_svg(&self) -> String {
        crate::svg::write(&self.doc)
    }

    /// `.svg` bytes for the host save dialog.
    pub fn svg_bytes(&self) -> Vec<u8> {
        self.export_svg().into_bytes()
    }

    // ── undo / redo ──────────────────────────────────────────────────────────

    fn push_undo(&mut self) {
        let snap = self.doc.clone();
        if self.undo.last() == Some(&snap) {
            return;
        }
        self.undo.push(snap);
        if self.undo.len() > 60 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Undo snapshot coalescing for continuous edits (sliders / scrubbing).
    fn push_undo_coalesced(&mut self, tag: &str) {
        let fresh = self.undo_at.map(|t| t.elapsed().as_secs_f32() < 0.8).unwrap_or(false);
        if fresh && self.undo_tag.as_deref() == Some(tag) {
            self.undo_at = Some(Instant::now());
            self.dirty = true;
            return;
        }
        self.push_undo();
        self.undo_tag = Some(tag.to_string());
        self.undo_at = Some(Instant::now());
    }

    pub fn undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(self.doc.clone());
            self.doc = d;
            self.sel.retain(|id| self.doc.node(*id).is_some());
            self.dirty = true;
            self.status = "Undo".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(self.doc.clone());
            self.doc = d;
            self.dirty = true;
            self.status = "Redo".into();
        }
    }

    fn delete_selection(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        self.push_undo();
        let sel = std::mem::take(&mut self.sel);
        self.doc.delete(&sel);
        self.status = format!("Deleted {} item(s)", sel.len());
    }

    fn duplicate_selection(&mut self) {
        if self.sel.is_empty() {
            return;
        }
        self.push_undo();
        let sel = self.sel.clone();
        self.sel = self.doc.duplicate(&sel);
        self.status = "Duplicated (offset +16,+16)".into();
    }

    // ── frame ────────────────────────────────────────────────────────────────

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.style_mut().visuals = egui::Visuals::dark();
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);

        self.menu_bar(ui);
        self.control_bar(ui);
        self.toolbox(ui);
        self.dock(ui);
        self.status_bar(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(0))
            .show_inside(ui, |ui| {
                self.canvas(ui);
            });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.editing.is_some() {
            return;
        }
        for t in TOOLS {
            if let Some(k) = t.key() {
                if ctx.input(|i| i.key_pressed(k)) {
                    self.tool = t;
                    if t != Tool::Pen {
                        self.pen.clear();
                        self.pen_drag = None;
                    }
                }
            }
        }
        // pen commit / cancel
        if !self.pen.is_empty() {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.pen.clear();
                self.pen_drag = None;
                self.status = "Pen path cancelled".into();
            } else if ctx.input(|i| i.key_pressed(Key::Enter)) {
                self.commit_pen(false);
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || (i.modifiers.ctrl && i.key_pressed(Key::Backspace))) {
            self.delete_selection();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::D)) {
            self.duplicate_selection();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Z) && !i.modifiers.shift) {
            self.undo();
        }
        if ctx.input(|i| i.modifiers.ctrl && (i.key_pressed(Key::Y) || (i.key_pressed(Key::Z) && i.modifiers.shift))) {
            self.redo();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::A)) {
            self.sel = self.doc.nodes.iter().map(|n| n.id()).collect();
        }
        if ctx.input(|i| i.key_pressed(Key::Equals)) {
            self.zoom_by(1.25);
        }
        if ctx.input(|i| i.key_pressed(Key::Minus)) {
            self.zoom_by(1.0 / 1.25);
        }
    }

    fn zoom_by(&mut self, f: f32) {
        let center = self.view_rect.map(|r| r.center()).unwrap_or(pos2(400.0, 300.0));
        self.zoom_at(center, f);
    }

    fn zoom_at(&mut self, sp: Pos2, f: f32) {
        let xf = self.xform();
        let before = xf.s2d(sp);
        self.zoom = (self.zoom * f).clamp(0.05, 16.0);
        let after = self.xform().s2d(sp);
        self.pan += (after - before) * self.zoom;
    }

    fn zoom_fit(&mut self) {
        let Some(vr) = self.view_rect else { return };
        let pad = 40.0;
        let zw = (vr.width() - pad * 2.0) / self.doc.artboard.x.max(1.0);
        let zh = (vr.height() - pad * 2.0) / self.doc.artboard.y.max(1.0);
        self.zoom = zw.min(zh).clamp(0.05, 16.0);
        let c = vr.center() - self.doc.artboard * (self.zoom / 2.0);
        self.pan = pos2(c.x, c.y);
        self.status = "Zoom to fit".into();
    }

    fn xform(&self) -> Xform {
        Xform { origin: self.pan, zoom: self.zoom }
    }

    // ── menu bar ─────────────────────────────────────────────────────────────

    fn menu_item(ui: &mut Ui, label: &str, on: bool) -> bool {
        let mark = if on { "✓ " } else { "   " };
        ui.selectable_label(false, format!("{mark}{label}")).clicked()
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("vector_menu")
            .exact_size(26.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("Vector").strong().size(12.5).color(ACCENT));
                    ui.separator();

                    ui.menu_button("File", |ui| {
                        if ui.button("New Document").clicked() {
                            self.push_undo();
                            self.doc = Doc::new(640.0, 480.0);
                            self.sel.clear();
                            self.path = None;
                            self.status = "New 640 × 480 artboard".into();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Import .svg…").clicked() {
                            self.status = "Import: route a .svg through the host — VectorApp::open(name, bytes)".into();
                            ui.close();
                        }
                        if ui.button("Export .svg…").clicked() {
                            self.pending_export = true;
                            self.status = "SVG ready — host calls export_svg() / svg_bytes()".into();
                            ui.close();
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.button("Undo  (Ctrl+Z)").clicked() {
                            self.undo();
                            ui.close();
                        }
                        if ui.button("Redo  (Ctrl+Y)").clicked() {
                            self.redo();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Duplicate  (Ctrl+D)").clicked() {
                            self.duplicate_selection();
                            ui.close();
                        }
                        if ui.button("Delete  (Del)").clicked() {
                            self.delete_selection();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Select All  (Ctrl+A)").clicked() {
                            self.sel = self.doc.nodes.iter().map(|n| n.id()).collect();
                            ui.close();
                        }
                        if ui.button("Deselect  (Esc twice)").clicked() {
                            self.sel.clear();
                            ui.close();
                        }
                    });
                    ui.menu_button("Object", |ui| {
                        if ui.button("Bring to Front").clicked() {
                            self.push_undo();
                            for id in self.sel.clone() {
                                self.doc.reorder(id, true);
                            }
                            ui.close();
                        }
                        if ui.button("Send to Back").clicked() {
                            self.push_undo();
                            for id in self.sel.clone() {
                                self.doc.reorder(id, false);
                            }
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Hide Selection").clicked() {
                            self.push_undo();
                            for id in &self.sel {
                                if let Some(n) = self.doc.node_mut(*id) {
                                    n.set_hidden(true);
                                }
                            }
                            ui.close();
                        }
                        if ui.button("Show All").clicked() {
                            self.push_undo();
                            for n in self.doc.nodes.iter_mut() {
                                n.set_hidden(false);
                            }
                            ui.close();
                        }
                        ui.separator();
                        for (label, k) in [("Align Left", 0), ("Center Horizontally", 1), ("Align Right", 2), ("Center Vertically", 3)] {
                            if ui.button(label).clicked() {
                                self.align(k);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Type", |ui| {
                        if ui.button("New Text Item  (T)").clicked() {
                            self.tool = Tool::Type;
                            self.status = Tool::Type.hint().to_string();
                            ui.close();
                        }
                    });
                    ui.menu_button("Select", |ui| {
                        if ui.button("All").clicked() {
                            self.sel = self.doc.nodes.iter().map(|n| n.id()).collect();
                            ui.close();
                        }
                        if ui.button("Deselect").clicked() {
                            self.sel.clear();
                            ui.close();
                        }
                        if ui.button("Inverse").clicked() {
                            let all: Vec<u64> = self.doc.nodes.iter().map(|n| n.id()).collect();
                            self.sel = all.into_iter().filter(|id| !self.sel.contains(id)).collect();
                            ui.close();
                        }
                    });
                    ui.menu_button("Effect", |ui| {
                        if ui.button("Toggle Gradient on Selection").clicked() {
                            self.toggle_gradient_selection();
                            ui.close();
                        }
                        if ui.button("Smooth Path").clicked() {
                            self.tool = Tool::Smooth;
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if Self::menu_item(ui, "Show Grid  (8/64 pt)", self.show_grid) {
                            self.show_grid = !self.show_grid;
                            ui.close();
                        }
                        if Self::menu_item(ui, "Snap to Grid", self.snap_grid) {
                            self.snap_grid = !self.snap_grid;
                            ui.close();
                        }
                        if Self::menu_item(ui, "Rulers", self.show_rulers) {
                            self.show_rulers = !self.show_rulers;
                            ui.close();
                        }
                        if Self::menu_item(ui, "Smart Guides", self.show_guides) {
                            self.show_guides = !self.show_guides;
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Zoom In  (+)").clicked() {
                            self.zoom_by(1.25);
                            ui.close();
                        }
                        if ui.button("Zoom Out  (−)").clicked() {
                            self.zoom_by(1.0 / 1.25);
                            ui.close();
                        }
                        if ui.button("Fit Artboard").clicked() {
                            self.zoom_fit();
                            ui.close();
                        }
                    });
                    ui.menu_button("Window", |ui| {
                        if Self::menu_item(ui, "Properties", self.open_props) {
                            self.open_props = !self.open_props;
                            ui.close();
                        }
                        if Self::menu_item(ui, "Layers", self.open_layers) {
                            self.open_layers = !self.open_layers;
                            ui.close();
                        }
                        if Self::menu_item(ui, "Swatches", self.open_swatches) {
                            self.open_swatches = !self.open_swatches;
                            ui.close();
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        if ui.button("Shortcut Keys").clicked() {
                            self.status = "Tools: V A P + T \\ M L Y B N S E G I H Z · Ctrl+Z/Y/D/A · Del · +/− zoom".into();
                            ui.close();
                        }
                        if ui.button("About Vector").clicked() {
                            self.status = "Vector craft 3.0 — native Illustrator-style editor (DocCraft suite)".into();
                            ui.close();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let name = self.path.clone().unwrap_or_else(|| "Untitled.svg".into());
                        let dot = if self.dirty { " ●" } else { "" };
                        ui.label(egui::RichText::new(format!("{name}{dot}")).weak().size(10.5));
                    });
                });
            });
    }

    fn align(&mut self, k: usize) {
        let Some(id) = self.sel.first().copied() else { return };
        self.push_undo();
        let ab = self.doc.artboard;
        if let Some(n) = self.doc.node_mut(id) {
            let mut r = n.bounds();
            match k {
                0 => r.min.x = 0.0,
                1 => r.min.x = (ab.x - r.width()) / 2.0,
                2 => r.min.x = ab.x - r.width(),
                _ => r.min.y = (ab.y - r.height()) / 2.0,
            }
            match k {
                0 | 1 | 2 => r.max.x = r.min.x + r.width(),
                _ => r.max.y = r.min.y + r.height(),
            }
            n.set_bounds(r);
        }
        self.status = "Aligned".into();
    }

    fn toggle_gradient_selection(&mut self) {
        if self.sel.is_empty() {
            self.status = "Gradient: select a shape first".into();
            return;
        }
        self.push_undo();
        for id in &self.sel {
            if let Some(Node::Shape(s)) = self.doc.node_mut(*id) {
                s.gradient = match s.gradient {
                    Some(_) => None,
                    None => Some((lerp(s.fill, Color32::WHITE, 0.55), s.fill)),
                };
            }
        }
        self.status = "Gradient toggled on selection".into();
    }

    // ── control bar ──────────────────────────────────────────────────────────

    fn control_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("vector_control")
            .exact_size(40.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL2)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    self.swatch_pair(ui);
                    ui.separator();

                    // X / Y / W / H of the single selection
                    if self.sel.len() == 1 {
                        let id = self.sel[0];
                        let b = self.doc.node(id).map(|n| n.bounds()).unwrap_or(Rect::ZERO);
                        let (mut x, mut y, mut w, mut h) = (b.min.x, b.min.y, b.width(), b.height());
                        let mut ch = false;
                        ch |= self.dv(ui, "X", &mut x);
                        ch |= self.dv(ui, "Y", &mut y);
                        ch |= self.dv(ui, "W", &mut w);
                        ch |= self.dv(ui, "H", &mut h);
                        if ch {
                            self.push_undo_coalesced("xywh");
                            if let Some(n) = self.doc.node_mut(id) {
                                let r = Rect::from_min_size(pos2(x, y), vec2(w.max(2.0), h.max(2.0)));
                                n.set_bounds(r);
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new(if self.sel.len() > 1 {
                            format!("{} items selected", self.sel.len())
                        } else {
                            "no selection — drag a shape tool on the artboard".to_string()
                        })
                        .size(10.5)
                        .color(DIM));
                    }
                    ui.separator();

                    // stroke width
                    let mut sw = self.stroke_w;
                    if ui
                        .add_sized([52.0, 18.0], egui::DragValue::new(&mut sw).speed(0.25).range(0.1..=40.0).prefix("⌷ ").suffix(" pt"))
                        .changed()
                    {
                        self.stroke_w = sw;
                        self.apply_stroke_width();
                    }
                    // opacity
                    let mut op = self.opacity * 100.0;
                    if ui
                        .add_sized([58.0, 18.0], egui::DragValue::new(&mut op).speed(1.0).range(0.0..=100.0).prefix("◍ ").suffix("%"))
                        .changed()
                    {
                        self.opacity = op / 100.0;
                        self.apply_opacity();
                    }
                    // polygon sides when relevant
                    if self.tool == Tool::Poly {
                        ui.add_sized([56.0, 18.0], egui::DragValue::new(&mut self.poly_sides).range(3..=24).prefix("⬡ ").suffix(" sides"));
                    }
                    ui.separator();
                    self.align_buttons(ui);
                });
            });
    }

    fn dv(&self, ui: &mut Ui, label: &str, v: &mut f32) -> bool {
        ui.add_sized([52.0, 18.0], egui::DragValue::new(v).speed(0.5).prefix(format!("{label} ")))
            .changed()
    }

    fn apply_stroke_width(&mut self) {
        self.push_undo_coalesced("stroke_w");
        for id in &self.sel {
            match self.doc.node_mut(*id) {
                Some(Node::Shape(s)) => s.stroke_w = self.stroke_w,
                Some(Node::Stroke(s)) => s.width = self.stroke_w.max(0.5),
                _ => {}
            }
        }
    }

    fn apply_opacity(&mut self) {
        self.push_undo_coalesced("opacity");
        for id in &self.sel {
            if let Some(n) = self.doc.node_mut(*id) {
                match n {
                    Node::Shape(s) => s.opacity = self.opacity,
                    Node::Stroke(s) => s.opacity = self.opacity,
                    Node::Text(t) => t.opacity = self.opacity,
                }
            }
        }
    }

    /// The Illustrator fill/stroke double swatch with picker popups.
    fn swatch_pair(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(42.0, 28.0), Sense::hover());
        let p = ui.painter_at(rect);
        let back = Rect::from_min_size(rect.min + vec2(18.0, 8.0), vec2(18.0, 18.0));
        let front = Rect::from_min_size(rect.min + vec2(2.0, 1.0), vec2(18.0, 18.0));
        p.rect_filled(back, 0.0, self.stroke);
        p.rect_stroke(back, CornerRadius::ZERO, Stroke::new(1.0, BORDER), StrokeKind::Middle);
        p.rect_filled(front, 0.0, self.fill);
        p.rect_stroke(front, CornerRadius::ZERO, Stroke::new(1.0, DIM), StrokeKind::Middle);

        let rf = ui.interact(front, Id::new("vector_fill_swatch"), Sense::click());
        let rs = ui.interact(back, Id::new("vector_stroke_swatch"), Sense::click());
        let rf = rf.on_hover_text("Fill color");
        let rs = rs.on_hover_text("Stroke color");
        if rf.clicked() {
            self.pick_fill = true;
            egui::Popup::menu(&rf).show(|ui| color_menu(ui, &mut self.fill));
        }
        if rs.clicked() {
            self.pick_fill = false;
            egui::Popup::menu(&rs).show(|ui| color_menu(ui, &mut self.stroke));
        }
    }

    fn align_buttons(&mut self, ui: &mut Ui) {
        for i in 0..4 {
            let (ar, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
            let ap = ui.painter_at(ar);
            let c = ar.center();
            let col = if self.sel.is_empty() { Color32::from_gray(80) } else { DIM };
            match i {
                0 => {
                    ap.line_segment([c + vec2(-6.0, -5.0), c + vec2(-6.0, 5.0)], Stroke::new(1.4, col));
                    ap.rect_filled(Rect::from_center_size(c + vec2(2.0, 0.0), vec2(10.0, 6.0)), 0.0, col);
                }
                1 => {
                    ap.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], Stroke::new(1.4, col));
                    ap.rect_filled(Rect::from_center_size(c + vec2(-3.0, 0.0), vec2(10.0, 6.0)), 0.0, col);
                }
                2 => {
                    ap.line_segment([c + vec2(6.0, -5.0), c + vec2(6.0, 5.0)], Stroke::new(1.4, col));
                    ap.rect_filled(Rect::from_center_size(c + vec2(-2.0, 0.0), vec2(10.0, 6.0)), 0.0, col);
                }
                _ => {
                    ap.line_segment([c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(1.4, col));
                    ap.rect_filled(Rect::from_center_size(c + vec2(0.0, -3.0), vec2(6.0, 10.0)), 0.0, col);
                }
            }
            let tip = ["Align left to artboard", "Center horizontally", "Align right to artboard", "Center vertically"][i];
            if resp.clicked() && !self.sel.is_empty() {
                self.align(i);
            }
            resp.on_hover_text(tip);
        }
    }

    // ── toolbox ──────────────────────────────────────────────────────────────

    fn toolbox(&mut self, ui: &mut Ui) {
        egui::Panel::left("vector_tools")
            .exact_size(56.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(3, 4))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show_inside(ui, |ui| {
                let cols = 2.0f32;
                let cell_w = (ui.available_width() - 2.0) / cols;
                let cell_h = 26.0;
                for row in TOOLS.chunks(2) {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        self.tool_cell(ui, row[0], cell_w, cell_h);
                        if row.len() > 1 {
                            self.tool_cell(ui, row[1], cell_w, cell_h);
                        }
                    });
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                // mini fill/stroke display (swap on click)
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                let p = ui.painter_at(r);
                let back = Rect::from_min_size(r.min + vec2(r.width() - 20.0, 12.0), vec2(16.0, 16.0));
                let front = Rect::from_min_size(r.min + vec2(4.0, 2.0), vec2(16.0, 16.0));
                p.rect_filled(back, 0.0, self.stroke);
                p.rect_stroke(back, CornerRadius::ZERO, Stroke::new(1.0, BORDER), StrokeKind::Middle);
                p.rect_filled(front, 0.0, self.fill);
                p.rect_stroke(front, CornerRadius::ZERO, Stroke::new(1.0, DIM), StrokeKind::Middle);
                if resp.clicked() {
                    std::mem::swap(&mut self.fill, &mut self.stroke);
                }
                resp.on_hover_text("Fill / stroke — click to swap");
            });
    }

    fn tool_cell(&mut self, ui: &mut Ui, tool: Tool, w: f32, h: f32) {
        let active = self.tool == tool;
        let (cell, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        let p = ui.painter_at(cell);
        let inner = cell.shrink2(vec2(2.0, 3.0));
        if active {
            p.rect_filled(inner, CornerRadius::same(3), Color32::from_rgb(0x3a, 0x3a, 0x42));
            p.rect_stroke(inner, CornerRadius::same(3), Stroke::new(1.0, ACCENT.linear_multiply(0.7)), StrokeKind::Middle);
        } else if resp.hovered() {
            p.rect_filled(inner, CornerRadius::same(3), Color32::from_rgb(0x2e, 0x2e, 0x35));
        }
        draw_tool_icon(&p, tool, cell.center(), active);
        let resp = resp.on_hover_text(format!("{} ({})", tool.label(), tool.key_label()));
        if resp.clicked() {
            self.tool = tool;
            if tool != Tool::Pen {
                self.pen.clear();
                self.pen_drag = None;
            }
        }
    }

    // ── right dock ───────────────────────────────────────────────────────────

    fn dock(&mut self, ui: &mut Ui) {
        egui::Panel::right("vector_dock")
            .default_size(244.0)
            .resizable(true)
            .size_range(200.0..=340.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::same(6))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("vector_dock_scroll").show(ui, |ui| {
                    if self.open_props {
                        egui::CollapsingHeader::new(egui::RichText::new("Properties").strong().size(11.5).color(TEXT))
                            .default_open(true)
                            .show(ui, |ui| self.properties_panel(ui));
                    }
                    if self.open_layers {
                        egui::CollapsingHeader::new(egui::RichText::new("Layers").strong().size(11.5).color(TEXT))
                            .default_open(true)
                            .show(ui, |ui| self.layers_panel(ui));
                    }
                    if self.open_swatches {
                        egui::CollapsingHeader::new(egui::RichText::new("Swatches").strong().size(11.5).color(TEXT))
                            .default_open(true)
                            .show(ui, |ui| self.swatches_panel(ui));
                    }
                });
            });
    }

    fn properties_panel(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let Some(id) = self.sel.first().copied() else {
            ui.label(egui::RichText::new("No selection").size(11.0).color(DIM));
            ui.label(egui::RichText::new("• Draw with M / L / \\ / Y tools\n• Pen builds bezier paths\n• Drag handles to transform\n• Ctrl+wheel zooms, wheel pans").size(10.0).color(DIM));
            return;
        };
        // snapshot everything first so mutations don't fight the borrow
        let Some(node) = self.doc.node(id) else { return };
        let (b, fill, stroke, stroke_w, opacity, is_grad_shape, is_text, text_size) = match node {
            Node::Shape(s) => (s.rect(), s.fill, s.stroke, s.stroke_w, s.opacity, matches!(s.kind, Kind::Rect | Kind::Ellipse), false, 0.0),
            Node::Stroke(s) => (s.bounds(), s.color, s.color, s.width, s.opacity, false, false, 0.0),
            Node::Text(t) => (t.bounds(), t.color, t.color, 0.0, t.opacity, false, true, t.size),
        };
        let grad = match self.doc.node(id) {
            Some(Node::Shape(s)) => s.gradient,
            _ => None,
        };

        // transform
        ui.label(egui::RichText::new("TRANSFORM").size(9.5).color(DIM));
        let (mut x, mut y, mut w, mut h) = (b.min.x, b.min.y, b.width(), b.height());
        let mut ch = false;
        ui.horizontal(|ui| {
            ch |= self.dv(ui, "X", &mut x);
            ch |= self.dv(ui, "Y", &mut y);
        });
        ui.horizontal(|ui| {
            ch |= self.dv(ui, "W", &mut w);
            ch |= self.dv(ui, "H", &mut h);
        });
        if ch {
            self.push_undo_coalesced("prop_xywh");
            if let Some(n) = self.doc.node_mut(id) {
                n.set_bounds(Rect::from_min_size(pos2(x, y), vec2(w.max(2.0), h.max(2.0))));
            }
        }

        // appearance
        ui.label(egui::RichText::new("APPEARANCE").size(9.5).color(DIM));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Fill").size(10.5).color(DIM));
            let mut fill_pick = fill;
            if ui.color_edit_button_srgba(&mut fill_pick).changed() {
                self.apply_fill(fill_pick);
            }
            ui.label(egui::RichText::new(format!("#{:02x}{:02x}{:02x}", fill.r(), fill.g(), fill.b())).size(9.5).color(DIM));
            ui.label(egui::RichText::new("Stroke").size(10.5).color(DIM));
            let mut stroke_pick = stroke;
            if ui.color_edit_button_srgba(&mut stroke_pick).changed() {
                self.apply_stroke(stroke_pick);
            }
        });
        let mut sw = stroke_w;
        if ui.add(egui::Slider::new(&mut sw, 0.25..=12.0).text("weight").step_by(0.25)).changed() {
            self.stroke_w = sw;
            self.apply_stroke_width();
        }
        let mut op = opacity * 100.0;
        if ui.add(egui::Slider::new(&mut op, 0.0..=100.0).text("opacity").suffix("%")).changed() {
            self.opacity = op / 100.0;
            self.apply_opacity();
        }
        // gradient controls for shapes
        if is_grad_shape {
            let mut g = grad;
            ui.horizontal(|ui| {
                if ui.selectable_label(g.is_some(), egui::RichText::new("Gradient").size(11.0)).clicked() {
                    self.toggle_gradient_selection();
                }
                if let Some((top, bot)) = g.as_mut() {
                    if ui.color_edit_button_srgba(top).changed() || ui.color_edit_button_srgba(bot).changed() {
                        self.push_undo_coalesced("gradient");
                        let (t, b2) = (*top, *bot);
                        if let Some(Node::Shape(s)) = self.doc.node_mut(id) {
                            s.gradient = Some((t, b2));
                        }
                    }
                }
            });
        }
        // text controls
        if is_text {
            let mut size = text_size;
            if ui.add(egui::Slider::new(&mut size, 8.0..=72.0).text("font size")).changed() {
                self.push_undo_coalesced("font_size");
                if let Some(Node::Text(t)) = self.doc.node_mut(id) {
                    t.size = size;
                }
            }
        }
        ui.separator();
        ui.checkbox(&mut self.show_grid, egui::RichText::new("Show Grid").size(11.0));
        ui.checkbox(&mut self.snap_grid, egui::RichText::new("Snap to Grid").size(11.0));
        ui.checkbox(&mut self.show_rulers, egui::RichText::new("Rulers").size(11.0));
        ui.checkbox(&mut self.show_guides, egui::RichText::new("Smart Guides").size(11.0));
    }

    fn apply_fill(&mut self, c: Color32) {
        self.fill = c;
        self.push_undo_coalesced("fill");
        for id in &self.sel {
            match self.doc.node_mut(*id) {
                Some(Node::Shape(s)) => s.fill = c,
                Some(Node::Text(t)) => t.color = c,
                Some(Node::Stroke(s)) => s.color = c,
                _ => {}
            }
        }
    }

    fn apply_stroke(&mut self, c: Color32) {
        self.stroke = c;
        self.push_undo_coalesced("stroke");
        for id in &self.sel {
            match self.doc.node_mut(*id) {
                Some(Node::Shape(s)) => s.stroke = c,
                Some(Node::Text(t)) => t.color = c,
                Some(Node::Stroke(s)) => s.color = c,
                _ => {}
            }
        }
    }

    fn layers_panel(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        let count = self.doc.nodes.len();
        let mut toggle_hidden: Option<u64> = None;
        let mut toggle_lock: Option<u64> = None;
        let mut clicked: Option<u64> = None;
        for n in self.doc.nodes.iter().rev() {
            let (id, hidden, locked) = (n.id(), n.hidden(), n.locked());
            let name = n.name();
            let kind = n.kind_label();
            let sel = self.sel.contains(&id);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
            let p = ui.painter_at(r);
            if sel {
                p.rect_filled(r, CornerRadius::same(3), Color32::from_rgb(0x0b, 0x64, 0xb8).linear_multiply(0.35));
            } else if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(3), Color32::from_rgb(0x2e, 0x2e, 0x35));
            }
            // eye
            let eye = Rect::from_center_size(pos2(r.left() + 11.0, r.center().y), vec2(16.0, 16.0));
            let col = if hidden { Color32::from_gray(90) } else { DIM };
            p.circle_stroke(eye.center(), 3.4, Stroke::new(1.2, col));
            p.circle_filled(eye.center(), 1.4, col);
            // lock
            let lk = Rect::from_center_size(pos2(r.right() - 26.0, r.center().y), vec2(14.0, 14.0));
            p.text(lk.center(), Align2::CENTER_CENTER, if locked { "🔒" } else { "🔓" }, FontId::proportional(9.0), if locked { ACCENT } else { Color32::from_gray(90) });
            let name_col = if hidden { Color32::from_gray(110) } else { TEXT };
            p.text(pos2(r.left() + 24.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(11.0), name_col);
            p.text(pos2(r.right() - 4.0, r.center().y), Align2::RIGHT_CENTER, kind, FontId::proportional(9.0), DIM);
            if resp.clicked() {
                let pt = resp.interact_pointer_pos().unwrap_or(Pos2::ZERO);
                if eye.expand(2.0).contains(pt) {
                    toggle_hidden = Some(id);
                } else if lk.expand(2.0).contains(pt) {
                    toggle_lock = Some(id);
                } else {
                    clicked = Some(id);
                }
            }
        }
        if count == 0 {
            ui.label(egui::RichText::new("No items yet").size(11.0).color(DIM));
        }
        if let Some(id) = toggle_hidden {
            self.push_undo();
            if let Some(n) = self.doc.node_mut(id) {
                let v = !n.hidden();
                n.set_hidden(v);
            }
        }
        if let Some(id) = toggle_lock {
            self.push_undo();
            if let Some(n) = self.doc.node_mut(id) {
                match n {
                    Node::Shape(s) => s.locked = !s.locked,
                    Node::Stroke(s) => s.locked = !s.locked,
                    Node::Text(t) => t.locked = !t.locked,
                }
            }
        }
        if let Some(id) = clicked {
            self.sel = vec![id];
        }
    }

    fn swatches_panel(&mut self, ui: &mut Ui) {
        const SWATCHES: [Color32; 24] = [
            Color32::WHITE,
            Color32::from_rgb(0xf2, 0xf2, 0xf2),
            Color32::from_rgb(0xd9, 0xd9, 0xd9),
            Color32::from_rgb(0xb3, 0xb3, 0xb3),
            Color32::from_rgb(0x80, 0x80, 0x80),
            Color32::from_rgb(0x4d, 0x4d, 0x4d),
            Color32::from_rgb(0x26, 0x26, 0x26),
            Color32::BLACK,
            Color32::from_rgb(0x11, 0x2b, 0x49),
            Color32::from_rgb(0x18, 0x5a, 0xbd),
            Color32::from_rgb(0x2e, 0x9e, 0xcc),
            Color32::from_rgb(0x9b, 0xde, 0xf1),
            Color32::from_rgb(0x0f, 0x6b, 0x3c),
            Color32::from_rgb(0x33, 0xa0, 0x52),
            Color32::from_rgb(0xa6, 0xd9, 0xa4),
            Color32::from_rgb(0xe8, 0xf5, 0xe4),
            Color32::from_rgb(0x8c, 0x2f, 0x0b),
            Color32::from_rgb(0xe8, 0x5d, 0x2a),
            Color32::from_rgb(0xf5, 0xc4, 0x42),
            Color32::from_rgb(0xfd, 0xf0, 0xd5),
            Color32::from_rgb(0x7a, 0x1f, 0x5c),
            Color32::from_rgb(0xc2, 0x4e, 0xa0),
            Color32::from_rgb(0xe8, 0xb4, 0xda),
            Color32::TRANSPARENT,
        ];
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        let cols = 6usize;
        let cw = (ui.available_width() - cols as f32 * 2.0) / cols as f32;
        let mut picked: Option<(Color32, bool)> = None;
        for (i, c) in SWATCHES.iter().enumerate() {
            if i % cols == 0 {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    for c in &SWATCHES[i..(i + cols).min(SWATCHES.len())] {
                        let (r, resp) = ui.allocate_exact_size(vec2(cw, 20.0), Sense::click());
                        let p = ui.painter_at(r);
                        if *c == Color32::TRANSPARENT {
                            p.rect_filled(r, CornerRadius::same(3), Color32::WHITE);
                            p.line_segment([r.left_top() + vec2(2.0, -1.0), r.right_bottom() - vec2(2.0, -1.0)], Stroke::new(1.2, Color32::from_rgb(0xd0, 0x40, 0x40)));
                        } else {
                            p.rect_filled(r, CornerRadius::same(3), *c);
                        }
                        p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, BORDER), StrokeKind::Middle);
                        if resp.clicked() {
                            picked = Some((*c, true));
                        }
                        if resp.secondary_clicked() {
                            picked = Some((*c, false));
                        }
                    }
                });
            }
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("click = fill · right-click = stroke").size(9.5).color(DIM));
        if let Some((c, is_fill)) = picked {
            if is_fill {
                self.apply_fill(c);
            } else {
                self.apply_stroke(c);
            }
        }
    }

    // ── status bar ───────────────────────────────────────────────────────────

    fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("vector_status")
            .exact_size(26.0)
            .frame(
                egui::Frame::default()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(10, 0))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                    ui.label(egui::RichText::new(self.tool.label()).strong().size(10.5).color(ACCENT));
                    ui.separator();
                    ui.label(egui::RichText::new(self.tool.hint()).size(10.5).color(DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.0}%",
                                self.zoom * 100.0
                            ))
                            .size(10.5)
                            .color(TEXT),
                        );
                        if ui
                            .add_sized(
                                [120.0, 14.0],
                                egui::Slider::new(&mut self.zoom, 0.05..=16.0).logarithmic(true).show_value(false),
                            )
                            .changed()
                        {
                            // keep the view anchored roughly
                        }
                        if ui.button(egui::RichText::new("−").size(11.0)).clicked() {
                            self.zoom_by(1.0 / 1.25);
                        }
                        if ui.button(egui::RichText::new("+").size(11.0)).clicked() {
                            self.zoom_by(1.25);
                        }
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!(
                                "{} × {} pt · {} items",
                                self.doc.artboard.x as i32,
                                self.doc.artboard.y as i32,
                                self.doc.nodes.len()
                            ))
                            .size(10.5)
                            .color(DIM),
                        );
                    });
                });
            });
    }

    // ── canvas ───────────────────────────────────────────────────────────────

    fn canvas(&mut self, ui: &mut Ui) {
        let (full_rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        self.view_rect = Some(full_rect);
        let rulers = if self.show_rulers { 18.0 } else { 0.0 };
        let canvas_rect = Rect::from_min_max(pos2(full_rect.left() + rulers, full_rect.top() + rulers), full_rect.max);
        let xf = self.xform();
        let p = ui.painter_at(full_rect);

        canvas::paint_artboard(&p, full_rect, xf, &self.doc, self.show_grid);

        // nodes (paint order = list order)
        {
            let pan = self.pan;
            let zoom = self.zoom;
            let to_screen = move |q: Pos2| pos2(q.x * zoom + pan.x, q.y * zoom + pan.y);
            for n in &self.doc.nodes {
                if !n.hidden() {
                    n.paint(&p, zoom, to_screen);
                }
            }
        }

        // live freehand preview
        if self.pending.len() >= 2 {
            let pts: Vec<Pos2> = self.pending.iter().map(|q| xf.d2s(*q)).collect();
            p.add(egui::Shape::line(pts, Stroke::new((self.stroke_w * self.zoom).max(1.2), self.stroke)));
        }

        // pen preview
        if !self.pen.is_empty() {
            let pts: Vec<Pos2> = self.pen.iter().map(|a| xf.d2s(a.p)).collect();
            p.add(egui::Shape::line(pts.clone(), Stroke::new(1.2, AI_BLUE)));
            canvas::paint_path_overlay(&p, &self.pen, false, xf, self.pen_drag, true);
            if let Some(mp) = ui.input(|i| i.pointer.latest_pos()) {
                if canvas_rect.contains(mp) {
                    p.line_segment([pts[pts.len() - 1], mp], Stroke::new(0.8, AI_BLUE.linear_multiply(0.6)));
                }
            }
        }

        // selection overlay
        let sel_bounds = self.doc.selection_bounds(&self.sel);
        if let Some(b) = sel_bounds {
            let sr = xf.d2s_rect(b);
            canvas::paint_selection(&p, sr);
        }
        // direct-selection path overlay (single path selected)
        if self.sel.len() == 1 {
            if let Some(Node::Shape(s)) = self.doc.node(self.sel[0]) {
                if matches!(s.kind, Kind::Path | Kind::Line) {
                    let show_handles = self.tool == Tool::Direct;
                    canvas::paint_path_overlay(&p, &s.anchors, s.closed, xf, None, show_handles);
                }
            }
        }

        // marquee
        if let Drag::Marquee { start } = self.drag.clone() {
            let cur = ui.input(|i| i.pointer.latest_pos()).unwrap_or(start);
            canvas::paint_marquee(&p, Rect::from_two_pos(start, cur));
        }
        // draw preview
        if let Drag::Draw { start } | Drag::DrawLine { start } = self.drag.clone() {
            let cur = ui.input(|i| i.pointer.latest_pos()).unwrap_or(start);
            canvas::paint_marquee(&p, Rect::from_two_pos(start, cur));
        }

        // ── interaction ──
        let resp = ui.interact(canvas_rect, Id::new("vector_canvas"), Sense::click_and_drag());
        let sp = resp.interact_pointer_pos();
        let dp = sp.map(|q| xf.s2d(q));

        // middle / right button pans with any tool
        if ui.input(|i| i.pointer.secondary_down()) && resp.dragged() {
            self.pan += ui.input(|i| i.pointer.delta());
        } else {
            self.tool_logic(ui, &resp, sp, dp, xf);
        }

        // smart guides (painted on top)
        canvas::paint_guides(&p, full_rect, &self.guides_v, &self.guides_h);

        // wheel: scroll pans, ctrl zooms
        if ui.rect_contains_pointer(full_rect) {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if scroll != Vec2::ZERO && !ui.input(|i| i.modifiers.ctrl) {
                self.pan += scroll;
            }
            let zd = ui.input(|i| i.zoom_delta());
            if zd != 0.0 && zd != 1.0 {
                if let Some(mp) = ui.input(|i| i.pointer.hover_pos()) {
                    self.zoom_at(mp, zd);
                }
            }
        }

        // rulers on top
        if self.show_rulers {
            let rh = Rect::from_min_max(pos2(full_rect.left() + rulers, full_rect.top()), pos2(full_rect.right(), full_rect.top() + rulers));
            let rv = Rect::from_min_max(pos2(full_rect.left(), full_rect.top()), pos2(full_rect.left() + rulers, full_rect.bottom()));
            canvas::paint_ruler_h(&ui.painter_at(rh), rh, xf);
            canvas::paint_ruler_v(&ui.painter_at(rv), rv, xf);
            canvas::paint_ruler_corner(&ui.painter_at(Rect::from_min_size(full_rect.left_top(), vec2(rulers, rulers))), Rect::from_min_size(full_rect.left_top(), vec2(rulers, rulers)));
        }

        self.inline_text_editor(ui, xf);
    }

    fn tool_logic(&mut self, ui: &mut Ui, resp: &egui::Response, sp: Option<Pos2>, dp: Option<Pos2>, xf: Xform) {
        let snap_on = self.snap_grid;
        match self.drag.clone() {
            Drag::None => {
                // start interactions
                match self.tool {
                    Tool::Hand => {
                        if resp.drag_started() {
                            self.drag = Drag::Pan { last: sp.unwrap_or(Pos2::ZERO) };
                        }
                    }
                    Tool::Zoom => {
                        if resp.clicked() {
                            let out = ui.input(|i| i.modifiers.alt);
                            if let Some(mp) = sp {
                                self.zoom_at(mp, if out { 1.0 / 1.3 } else { 1.3 });
                            }
                        }
                    }
                    Tool::Dropper => {
                        if resp.clicked() {
                            self.dropper_pick(dp);
                        }
                    }
                    Tool::Gradient => {
                        if resp.clicked() {
                            self.gradient_click(dp);
                        }
                    }
                    Tool::Smooth => {
                        if resp.clicked() {
                            self.smooth_click(dp);
                        }
                    }
                    Tool::Erase => {
                        if resp.clicked() || resp.dragged() {
                            if let Some(id) = dp.and_then(|d| self.doc.hit(d)) {
                                self.push_undo_coalesced("erase");
                                self.doc.delete(&[id]);
                                self.sel.retain(|s| *s != id);
                                self.status = "Erased an item".into();
                            }
                        }
                    }
                    Tool::Type => {
                        if resp.clicked() {
                            if let Some(dp) = dp {
                                self.push_undo();
                                let id = self.doc.alloc();
                                let t = TextItem {
                                    id,
                                    x: snap(dp.x, snap_on),
                                    y: snap(dp.y - 12.0, snap_on),
                                    w: 180.0,
                                    text: "Type here".into(),
                                    size: 14.0,
                                    color: self.stroke,
                                    opacity: self.opacity,
                                    hidden: false,
                                    locked: false,
                                };
                                self.doc.nodes.push(Node::Text(t));
                                self.sel = vec![id];
                                self.editing = Some((id, "Type here".into()));
                                self.edit_fresh = true;
                                self.tool = Tool::Select;
                            }
                        }
                    }
                    Tool::Pen => {
                        if resp.clicked() {
                            if let (Some(dp), Some(mp)) = (dp, sp) {
                                let d = snap_p(dp, snap_on);
                                if self.pen.len() >= 3 && (xf.d2s(self.pen[0].p) - mp).length() < 7.0 {
                                    self.commit_pen(true);
                                } else {
                                    self.pen.push(Anchor::corner(d));
                                    self.pen_drag = Some(self.pen.len() - 1);
                                }
                            }
                        }
                        // drag out a direction handle for the newest anchor
                        if let Some(i) = self.pen_drag {
                            if resp.dragged() {
                                if let Some(dp) = dp {
                                    if let Some(a) = self.pen.get_mut(i) {
                                        let h = dp - a.p;
                                        if h.length() > 3.0 {
                                            a.h_out = h;
                                            a.h_in = -h;
                                        }
                                    }
                                }
                            }
                            if resp.drag_stopped() {
                                self.pen_drag = None;
                            }
                        }
                        if resp.double_clicked() && self.pen.len() >= 2 {
                            // drop the duplicate anchor a double-click adds
                            if self.pen.len() >= 2 {
                                let n = self.pen.len();
                                if (self.pen[n - 1].p - self.pen[n - 2].p).length() < 2.0 {
                                    self.pen.pop();
                                }
                            }
                            self.commit_pen(false);
                        }
                    }
                    Tool::Anchor => {
                        if resp.clicked() {
                            self.anchor_tool_click(dp, sp, xf);
                        }
                    }
                    Tool::Rect | Tool::Ellipse | Tool::Poly => {
                        if resp.drag_started() {
                            if let Some(dp) = dp {
                                self.drag = Drag::Draw { start: snap_p(dp, snap_on) };
                            }
                        }
                    }
                    Tool::Line => {
                        if resp.drag_started() {
                            if let Some(dp) = dp {
                                self.drag = Drag::DrawLine { start: snap_p(dp, snap_on) };
                            }
                        }
                    }
                    Tool::Brush | Tool::Pencil => {
                        if resp.drag_started() {
                            if let Some(dp) = dp {
                                self.push_undo();
                                self.pending = vec![dp];
                                self.drag = Drag::Freehand;
                            }
                        }
                    }
                    Tool::Select | Tool::Direct => {
                        self.select_start(ui, resp, sp, dp, xf, snap_on);
                    }
                }
            }
            Drag::Pan { last } => {
                if resp.dragged() {
                    if let Some(mp) = sp {
                        self.pan += mp - last;
                        self.drag = Drag::Pan { last: mp };
                    }
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                }
            }
            Drag::Move { grab, orig, bounds, moved } => {
                if resp.dragged() {
                    if let Some(dp) = dp {
                        let raw = dp - grab;
                        let mut delta = raw;
                        self.guides_v.clear();
                        self.guides_h.clear();
                        let moving = bounds.translate(raw);
                        if self.show_guides {
                            let others: Vec<Rect> = self
                                .doc
                                .nodes
                                .iter()
                                .filter(|n| !orig.iter().any(|(id, _)| *id == n.id()) && !n.hidden() && !n.locked())
                                .map(|n| n.bounds())
                                .collect();
                            let (sdx, sdy, vg, hg) = canvas::smart_snap(moving, &others, 5.0 / self.zoom);
                            delta = raw + vec2(sdx, sdy);
                            for x in vg {
                                self.guides_v.push(xf.d2s(pos2(x, 0.0)).x);
                            }
                            for y in hg {
                                self.guides_h.push(xf.d2s(pos2(0.0, y)).y);
                            }
                        } else if snap_on {
                            let nx = snap(moving.min.x, true);
                            let ny = snap(moving.min.y, true);
                            delta = raw + vec2(nx - moving.min.x, ny - moving.min.y);
                        }
                        for (id, r0) in &orig {
                            if let Some(n) = self.doc.node_mut(*id) {
                                n.set_bounds(r0.translate(delta));
                            }
                        }
                        self.dirty = true;
                        self.drag = Drag::Move { grab, orig, bounds, moved: true };
                    }
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                    let _ = moved;
                }
            }
            Drag::Resize { handle, orig } => {
                if let (Some(dp), Some(id)) = (dp, self.sel.first().copied()) {
                    let d = dp - orig.min;
                    let (mut x, mut y, mut w, mut h) = (orig.min.x, orig.min.y, orig.width(), orig.height());
                    match handle {
                        0 => {
                            x = snap(x + d.x, snap_on);
                            y = snap(y + d.y, snap_on);
                            w -= d.x;
                            h -= d.y;
                        }
                        1 => {
                            y = snap(y + d.y, snap_on);
                            h -= d.y;
                        }
                        2 => {
                            y = snap(y + d.y, snap_on);
                            w += d.x;
                            h -= d.y;
                        }
                        3 => w += d.x,
                        4 => {
                            w += d.x;
                            h += d.y;
                        }
                        5 => h += d.y,
                        6 => {
                            x = snap(x + d.x, snap_on);
                            w -= d.x;
                            h += d.y;
                        }
                        _ => {
                            x = snap(x + d.x, snap_on);
                            w -= d.x;
                        }
                    }
                    if let Some(n) = self.doc.node_mut(id) {
                        n.set_bounds(Rect::from_min_size(pos2(x, y), vec2(w.max(2.0), h.max(2.0))));
                    }
                    self.dirty = true;
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                }
            }
            Drag::Marquee { start } => {
                if let Some(dp) = dp {
                    let r = Rect::from_two_pos(start, dp);
                    self.sel = self
                        .doc
                        .nodes
                        .iter()
                        .filter(|n| !n.hidden() && !n.locked() && n.bounds().intersects(r))
                        .map(|n| n.id())
                        .collect();
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                }
            }
            Drag::Draw { start } => {
                if resp.drag_stopped() {
                    if let Some(dp) = dp {
                        let (a, b) = (start, snap_p(dp, snap_on));
                        if (b - a).length() > 4.0 {
                            self.push_undo();
                            let id = self.doc.alloc();
                            let kind = match self.tool {
                                Tool::Ellipse => Kind::Ellipse,
                                Tool::Poly => Kind::Poly,
                                _ => Kind::Rect,
                            };
                            let mut s = Shape::new(id, kind, a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs());
                            s.sides = self.poly_sides;
                            s.fill = self.fill;
                            s.stroke = self.stroke;
                            s.stroke_w = self.stroke_w;
                            s.opacity = self.opacity;
                            let sa = s.anchors.len();
        self.doc.nodes.push(Node::Shape(s));
                            self.sel = vec![id];
                            self.status = format!("{} added", kind.label());
                        }
                    }
                    self.drag = Drag::None;
                }
            }
            Drag::DrawLine { start } => {
                if resp.drag_stopped() {
                    if let Some(dp) = dp {
                        let (a, b) = (start, snap_p(dp, snap_on));
                        if (b - a).length() > 4.0 {
                            self.push_undo();
                            let id = self.doc.alloc();
                            let mut s = Shape::new(id, Kind::Line, 0.0, 0.0, 0.0, 0.0);
                            s.anchors = vec![Anchor::corner(a), Anchor::corner(b)];
                            s.fill = Color32::TRANSPARENT;
                            s.stroke = self.stroke;
                            s.stroke_w = self.stroke_w;
                            s.opacity = self.opacity;
                            s.sync_bounds();
                            self.doc.nodes.push(Node::Shape(s));
                            self.sel = vec![id];
                            self.status = "Line added".into();
                        }
                    }
                    self.drag = Drag::None;
                }
            }
            Drag::Freehand => {
                if resp.dragged() {
                    if let Some(dp) = dp {
                        if self.pending.last().map(|l| (*l - dp).length() > 2.0 / self.zoom).unwrap_or(true) {
                            self.pending.push(dp);
                        }
                    }
                }
                if resp.drag_stopped() {
                    self.commit_freehand();
                }
            }
            Drag::AnchorMove { id, idx } => {
                if let Some(dp) = dp {
                    if let Some(Node::Shape(s)) = self.doc.node_mut(id) {
                        if let Some(a) = s.anchors.get_mut(idx) {
                            a.p = snap_p(dp, snap_on);
                        }
                        s.sync_bounds();
                    }
                    self.dirty = true;
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                }
            }
            Drag::HandleDrag { id, idx, out } => {
                if let Some(dp) = dp {
                    if let Some(Node::Shape(s)) = self.doc.node_mut(id) {
                        if let Some(a) = s.anchors.get_mut(idx) {
                            let h = dp - a.p;
                            if out {
                                a.h_out = h;
                            } else {
                                a.h_in = h;
                            }
                        }
                    }
                    self.dirty = true;
                }
                if resp.drag_stopped() {
                    self.drag = Drag::None;
                }
            }
        }
        // guides only live while a drag is active
        if self.drag == Drag::None {
            self.guides_v.clear();
            self.guides_h.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn select_start(&mut self, ui: &mut Ui, resp: &egui::Response, sp: Option<Pos2>, dp: Option<Pos2>, xf: Xform, snap_on: bool) {
        if resp.drag_started() {
            if let (Some(dp), Some(mp)) = (dp, sp) {
                // handle resize pickup (single selection first)
                if self.sel.len() == 1 {
                    if let Some(b) = self.doc.selection_bounds(&self.sel) {
                        if let Some(hi) = canvas::hit_handle(xf.d2s_rect(b), mp) {
                            self.push_undo();
                            self.drag = Drag::Resize { handle: hi, orig: b };
                            return;
                        }
                    }
                }
                if let Some(id) = self.doc.hit(dp) {
                    let direct_anchor = self.tool == Tool::Direct
                        && matches!(self.doc.node(id), Some(Node::Shape(s)) if matches!(s.kind, Kind::Path | Kind::Line));
                    if direct_anchor {
                        if let Some(Node::Shape(s)) = self.doc.node(id) {
                            if let Some(ai) = canvas::hit_anchor(&s.anchors, xf, mp, 8.0) {
                                // direction handles take priority
                                if canvas::hit_dir_handle(&s.anchors[ai], xf, mp, 8.0) {
                                    let out = (xf.d2s(s.anchors[ai].p + s.anchors[ai].h_out) - mp).length()
                                        <= (xf.d2s(s.anchors[ai].p + s.anchors[ai].h_in) - mp).length();
                                    self.push_undo();
                                    self.drag = Drag::HandleDrag { id, idx: ai, out };
                                    return;
                                }
                                self.push_undo();
                                self.drag = Drag::AnchorMove { id, idx: ai };
                                return;
                            }
                        }
                    }
                    if !self.sel.contains(&id) {
                        self.sel = vec![id];
                    }
                    self.push_undo();
                    let orig = self.sel.iter().filter_map(|i| self.doc.node(*i).map(|n| (*i, n.bounds()))).collect();
                    let bounds = self.doc.selection_bounds(&self.sel).unwrap_or(Rect::ZERO);
                    self.drag = Drag::Move { grab: dp, orig, bounds, moved: false };
                } else {
                    self.drag = Drag::Marquee { start: mp };
                    if !ui.input(|i| i.modifiers.shift) {
                        self.sel.clear();
                    }
                }
            }
        } else if resp.clicked() {
            // plain click selection / deselect / text edit
            if let Some(dp) = dp {
                match self.doc.hit(dp) {
                    Some(id) => {
                        if ui.input(|i| i.modifiers.shift) {
                            if self.sel.contains(&id) {
                                self.sel.retain(|s| *s != id);
                            } else {
                                self.sel.push(id);
                            }
                        } else if !self.sel.contains(&id) {
                            self.sel = vec![id];
                        }
                        if let Some(Node::Text(t)) = self.doc.node(id) {
                            if resp.double_clicked() {
                                self.editing = Some((id, t.text.clone()));
                                self.edit_fresh = true;
                            }
                        }
                    }
                    None => {
                        self.sel.clear();
                    }
                }
            }
        }
        let _ = snap_on;
    }

    fn commit_pen(&mut self, closed: bool) {
        if self.pen.len() < 2 {
            self.pen.clear();
            self.pen_drag = None;
            return;
        }
        self.push_undo();
        let id = self.doc.alloc();
        let mut s = Shape::new(id, Kind::Path, 0.0, 0.0, 0.0, 0.0);
        s.anchors = std::mem::take(&mut self.pen);
        s.closed = closed;
        s.fill = if closed { self.fill } else { Color32::TRANSPARENT };
        s.stroke = self.stroke;
        s.stroke_w = self.stroke_w;
        s.opacity = self.opacity;
        s.sync_bounds();
        let n_anchors = s.anchors.len();
        self.doc.nodes.push(Node::Shape(s));
        self.sel = vec![id];
        self.pen_drag = None;
        self.status = format!("Path committed ({} anchors{})", n_anchors, if closed { ", closed" } else { "" });
    }

    fn commit_freehand(&mut self) {
        if self.pending.len() >= 2 {
            // light smoothing for the brush
            if self.tool == Tool::Brush {
                for _ in 0..2 {
                    let mut np = self.pending.clone();
                    for i in 1..np.len() - 1 {
                        np[i] = ((self.pending[i - 1].to_vec2() + self.pending[i + 1].to_vec2()) * 0.5).to_pos2();
                    }
                    self.pending = np;
                }
            }
            let id = self.doc.alloc();
            self.doc.nodes.push(Node::Stroke(FreeStroke {
                id,
                pts: std::mem::take(&mut self.pending),
                color: self.stroke,
                width: if self.tool == Tool::Brush { self.stroke_w.max(1.8) } else { self.stroke_w.max(1.0) },
                opacity: self.opacity,
                hidden: false,
                locked: false,
            }));
            self.sel = vec![id];
            self.status = "Freehand stroke added".into();
        }
        self.pending.clear();
        self.drag = Drag::None;
    }

    fn dropper_pick(&mut self, dp: Option<Pos2>) {
        let Some(dp) = dp.and_then(|d| self.doc.hit(d)) else { return };
        match self.doc.node(dp) {
            Some(Node::Shape(s)) => {
                self.fill = s.fill;
                self.stroke = s.stroke;
                self.stroke_w = s.stroke_w;
                self.opacity = s.opacity;
                self.status = format!("Eyedropped # {:02x}{:02x}{:02x} / weight {:.1}", s.fill.r(), s.fill.g(), s.fill.b(), s.stroke_w);
            }
            Some(Node::Stroke(s)) => {
                self.stroke = s.color;
                self.stroke_w = s.width;
                self.status = "Eyedropped stroke color".into();
            }
            Some(Node::Text(t)) => {
                self.stroke = t.color;
                self.status = "Eyedropped text color".into();
            }
            None => {}
        }
    }

    fn gradient_click(&mut self, dp: Option<Pos2>) {
        let Some(id) = dp.and_then(|d| self.doc.hit(d)) else {
            self.status = "Gradient: click a shape".into();
            return;
        };
        self.push_undo();
        match self.doc.node_mut(id) {
            Some(Node::Shape(s)) => {
                if matches!(s.kind, Kind::Rect | Kind::Ellipse) {
                    s.gradient = match s.gradient {
                        Some(_) => None,
                        None => Some((lerp(s.fill, Color32::WHITE, 0.55), s.fill)),
                    };
                    self.status = "Gradient toggled".into();
                } else {
                    self.status = "Gradient supports rectangles and ellipses".into();
                }
            }
            _ => self.status = "Gradient supports shapes only".into(),
        }
    }

    fn smooth_click(&mut self, dp: Option<Pos2>) {
        let Some(id) = dp.and_then(|d| self.doc.hit(d)) else { return };
        self.push_undo();
        match self.doc.node_mut(id) {
            Some(Node::Shape(s)) if s.kind == Kind::Path && s.anchors.len() >= 3 => {
                let n = s.anchors.len();
                for i in 0..n {
                    let interior = s.closed || (i > 0 && i < n - 1);
                    if !interior {
                        continue;
                    }
                    let prev = s.anchors[(i + n - 1) % n].p;
                    let next = s.anchors[(i + 1) % n].p;
                    let h = (next - prev) * (1.0 / 6.0);
                    s.anchors[i].h_in = -h;
                    s.anchors[i].h_out = h;
                }
                self.status = "Path smoothed (mirrored handles)".into();
            }
            Some(Node::Stroke(st)) => {
                for _ in 0..2 {
                    let mut np = st.pts.clone();
                    for i in 1..np.len().saturating_sub(1) {
                        np[i] = ((st.pts[i - 1].to_vec2() + st.pts[i + 1].to_vec2()) * 0.5).to_pos2();
                    }
                    st.pts = np;
                }
                self.status = "Stroke smoothed".into();
            }
            _ => self.status = "Smooth works on paths and strokes".into(),
        }
    }

    fn anchor_tool_click(&mut self, dp: Option<Pos2>, sp: Option<Pos2>, xf: Xform) {
        let (Some(id), Some(mp)) = (dp.and_then(|d| self.doc.hit(d)), sp) else {
            self.status = "Anchor tool: click a path or line".into();
            return;
        };
        let Some(Node::Shape(s)) = self.doc.node(id) else {
            self.status = "Anchor tool works on paths and lines".into();
            return;
        };
        if !matches!(s.kind, Kind::Path | Kind::Line) {
            self.status = "Anchor tool works on paths and lines".into();
            return;
        }
        // remove an existing anchor?
        if let Some(ai) = canvas::hit_anchor(&s.anchors, xf, mp, 8.0) {
            if s.anchors.len() > 2 {
                self.push_undo();
                if let Some(Node::Shape(s)) = self.doc.node_mut(id) {
                    s.anchors.remove(ai);
                    s.sync_bounds();
                }
                self.status = "Anchor removed".into();
            } else {
                self.status = "A path needs at least 2 anchors".into();
            }
            return;
        }
        // insert at the nearest segment midpoint
        let pts = flatten(&s.anchors, s.closed);
        let closed = s.closed;
        let mut best: Option<(usize, f32, Pos2)> = None;
        let segs = if closed { pts.len() } else { pts.len() - 1 };
        for i in 0..segs {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            let d = dist_point_seg(mp_to_doc(mp, xf), a, b);
            if best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
                let mid = (a + b.to_vec2()) * 0.5;
                best = Some((i, d, mid));
            }
        }
        if let Some((i, _, mid)) = best {
            self.push_undo();
            if let Some(Node::Shape(s)) = self.doc.node_mut(id) {
                let at = (i + 1).min(s.anchors.len());
                s.anchors.insert(at, Anchor::corner(mid));
                s.sync_bounds();
            }
            self.status = "Anchor added".into();
        }
    }

    fn inline_text_editor(&mut self, ui: &mut Ui, xf: Xform) {
        let Some(id) = self.editing.as_ref().map(|(id, _)| *id) else { return };
        let Some((sp0, w, size)) = (match self.doc.node(id) {
            Some(Node::Text(t)) => Some((xf.d2s(pos2(t.x, t.y)), t.w, t.size)),
            _ => None,
        }) else {
            self.editing = None;
            return;
        };
        let zoom = self.zoom;
        let just_opened = self.edit_fresh;
        self.edit_fresh = false;
        let mut commit = false;
        if let Some(buf) = self.editing.as_mut().map(|(_, b)| b) {
            egui::Area::new(Id::new("vector_text_edit"))
                .fixed_pos(sp0)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::default()
                        .fill(Color32::WHITE)
                        .stroke(Stroke::new(1.4, AI_BLUE))
                        .corner_radius(2.0)
                        .show(ui, |ui| {
                            let r = ui.add_sized(
                                [(w * zoom).max(90.0), (size * zoom + 8.0).max(22.0)],
                                egui::TextEdit::singleline(buf).font(egui::FontId::proportional((size * zoom).max(9.0))),
                            );
                            if just_opened {
                                r.request_focus();
                            }
                            if r.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                                commit = true;
                            }
                        });
                });
        }
        if commit {
            if let Some((_, buf)) = self.editing.take() {
                if buf.trim().is_empty() {
                    self.doc.delete(&[id]);
                    self.sel.retain(|s| *s != id);
                } else if let Some(Node::Text(t)) = self.doc.node_mut(id) {
                    t.text = buf;
                }
                self.dirty = true;
            }
        }
    }
}

fn mp_to_doc(mp: Pos2, xf: Xform) -> Pos2 {
    xf.s2d(mp)
}

// ---------------------------------------------------------------------------
// toolbox icons (simple strokes, Illustrator glyphs)
// ---------------------------------------------------------------------------

fn draw_tool_icon(p: &Painter, tool: Tool, c: Pos2, _on: bool) {
    let ink = Color32::from_rgb(0xd8, 0xd8, 0xde);
    let dark = Color32::from_rgb(0x1e, 0x1e, 0x1e);
    let s = Stroke::new(1.5, ink);
    match tool {
        Tool::Select => {
            let pts = vec![
                c + vec2(-5.0, -7.0),
                c + vec2(-5.0, 7.0),
                c + vec2(-1.0, 3.5),
                c + vec2(2.5, 8.0),
                c + vec2(4.5, 6.5),
                c + vec2(1.5, 2.5),
                c + vec2(6.0, 1.5),
            ];
            p.add(egui::Shape::convex_polygon(pts, ink, Stroke::NONE));
        }
        Tool::Direct => {
            let pts = vec![
                c + vec2(-5.0, -7.0),
                c + vec2(-5.0, 7.0),
                c + vec2(-1.0, 3.5),
                c + vec2(2.5, 8.0),
                c + vec2(4.5, 6.5),
                c + vec2(1.5, 2.5),
                c + vec2(6.0, 1.5),
            ];
            p.add(egui::Shape::convex_polygon(pts, Color32::WHITE, Stroke::new(1.0, ink)));
            p.circle_filled(c + vec2(4.0, -4.0), 2.0, ink);
        }
        Tool::Pen => {
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(2.0, -6.0)], s);
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(2.0, -6.0), c + vec2(5.0, -7.0), c + vec2(6.0, -4.0), c + vec2(3.0, -2.0)],
                ink,
                Stroke::NONE,
            ));
            p.circle_stroke(c + vec2(-6.0, 6.0), 2.0, s);
        }
        Tool::Anchor => {
            p.line_segment([c + vec2(-2.0, 0.0), c + vec2(2.0, 0.0)], s);
            p.line_segment([c + vec2(0.0, -2.0), c + vec2(0.0, 2.0)], s);
            p.rect_filled(Rect::from_center_size(c + vec2(3.0, -3.0), vec2(6.0, 6.0)), 0.0, Color32::WHITE);
            p.rect_stroke(Rect::from_center_size(c + vec2(3.0, -3.0), vec2(6.0, 6.0)), CornerRadius::ZERO, Stroke::new(1.0, ink), StrokeKind::Middle);
            p.circle_filled(c + vec2(-3.5, 4.0), 2.0, ink);
        }
        Tool::Type => {
            p.text(c, Align2::CENTER_CENTER, "T", FontId::proportional(13.0), ink);
        }
        Tool::Line => {
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(6.0, -6.0)], s);
        }
        Tool::Rect => {
            p.rect_stroke(Rect::from_center_size(c, vec2(12.0, 9.0)), CornerRadius::ZERO, s, StrokeKind::Middle);
        }
        Tool::Ellipse => {
            p.circle_stroke(c, 5.5, s);
        }
        Tool::Poly => {
            let r = Rect::from_center_size(c, vec2(13.0, 11.0));
            let pts = poly_points(r, 6);
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s));
        }
        Tool::Brush => {
            p.line_segment([c + vec2(-5.0, 5.0), c + vec2(4.0, -4.0)], Stroke::new(2.4, ink));
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(-7.0, 7.0), c + vec2(-4.5, 4.5), c + vec2(-3.0, 7.0), c + vec2(-5.5, 8.5)],
                ink,
                Stroke::NONE,
            ));
        }
        Tool::Pencil => {
            p.line_segment([c + vec2(-5.0, 5.0), c + vec2(4.0, -4.0)], s);
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(4.0, -4.0), c + vec2(6.0, -6.0), c + vec2(6.5, -4.5), c + vec2(4.8, -2.8)],
                ink,
                Stroke::NONE,
            ));
        }
        Tool::Smooth => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    pos2(c.x - 6.0 + t * 12.0, c.y + (t * std::f32::consts::PI * 2.0).sin() * 4.0)
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
        }
        Tool::Erase => {
            let r = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(12.0, 7.0));
            let pts = vec![r.left_top() + vec2(2.0, 0.0), r.right_top() + vec2(-2.0, 0.0), r.right_bottom() - vec2(2.0, 0.0), r.left_bottom() + vec2(2.0, 0.0)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s));
            p.line_segment([c + vec2(-7.0, -4.0), c + vec2(7.0, -4.0)], Stroke::new(1.0, ink.linear_multiply(0.5)));
        }
        Tool::Gradient => {
            let r = Rect::from_center_size(c, vec2(13.0, 10.0));
            p.rect_stroke(r, CornerRadius::ZERO, s, StrokeKind::Middle);
            let steps = 6;
            for i in 0..steps {
                let t = i as f32 / (steps - 1) as f32;
                let col = lerp(dark, ink, t);
                let x = r.left() + r.width() * i as f32 / steps as f32;
                p.rect_filled(Rect::from_min_max(pos2(x, r.top() + 1.0), pos2(x + r.width() / steps as f32 + 0.4, r.bottom() - 1.0)), 0.0, col);
            }
        }
        Tool::Dropper => {
            p.line_segment([c + vec2(-4.0, 4.0), c + vec2(3.0, -3.0)], Stroke::new(2.0, ink));
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(3.0, -3.0), c + vec2(6.0, -6.5), c + vec2(7.5, -5.0), c + vec2(4.0, -1.5)],
                ink,
                Stroke::NONE,
            ));
            p.circle_filled(c + vec2(-5.5, 5.5), 1.8, ink);
        }
        Tool::Hand => {
            for (dx, dy) in [(-4.0, -2.0), (-1.5, -3.5), (1.5, -3.5), (4.0, -2.0)] {
                p.line_segment([c + vec2(0.0, 3.0), c + vec2(dx, dy)], s);
            }
            p.line_segment([c + vec2(-4.5, 3.0), c + vec2(4.5, 3.0)], s);
        }
        Tool::Zoom => {
            p.circle_stroke(c + vec2(-1.5, -1.5), 4.5, s);
            p.line_segment([c + vec2(2.0, 2.0), c + vec2(6.5, 6.5)], Stroke::new(1.6, ink));
            p.line_segment([c + vec2(-1.5, -3.5), c + vec2(-1.5, 0.5)], Stroke::new(1.1, ink));
            p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.5, -1.5)], Stroke::new(1.1, ink));
        }
    }
}

/// Preset color grid + native picker for the swatch popups.
fn color_menu(ui: &mut Ui, target: &mut Color32) {
    const QUICK: [Color32; 12] = [
        Color32::WHITE,
        Color32::from_rgb(0xd9, 0xd9, 0xd9),
        Color32::from_rgb(0x80, 0x80, 0x80),
        Color32::BLACK,
        Color32::from_rgb(0x18, 0x5a, 0xbd),
        Color32::from_rgb(0x2e, 0x9e, 0xcc),
        Color32::from_rgb(0x33, 0xa0, 0x52),
        Color32::from_rgb(0xf5, 0xc4, 0x42),
        Color32::from_rgb(0xe8, 0x5d, 0x2a),
        Color32::from_rgb(0xc2, 0x4e, 0xa0),
        Color32::from_rgb(0x8c, 0x2f, 0x0b),
        Color32::TRANSPARENT,
    ];
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
        for c in QUICK {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            let p = ui.painter_at(r);
            if c == Color32::TRANSPARENT {
                p.rect_filled(r, CornerRadius::same(3), Color32::WHITE);
                p.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.2, Color32::from_rgb(0xd0, 0x40, 0x40)));
            } else {
                p.rect_filled(r, CornerRadius::same(3), c);
            }
            p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, BORDER), StrokeKind::Middle);
            if resp.clicked() {
                *target = c;
                ui.close();
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.color_edit_button_srgba(target);
        ui.label(egui::RichText::new("custom color").size(11.0).color(DIM));
    });
}
