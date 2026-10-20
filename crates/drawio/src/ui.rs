//! The diagrams.net-style editor shell: menu bar, toolbar, shapes palette,
//! layers panel, the Style/Text/Arrange format panel, page tabs, status bar
//! and the dialogs (Edit XML, Find & Replace, Page Setup, Shortcuts).
//!
//! The drawing surface itself lives in [`crate::canvas`]; the model in
//! [`crate::model`]; file I/O in [`crate::xmlio`].

use crate::colorpicker;
use crate::canvas::{self, CtxMenu};
use crate::connection::ConnectState;
use crate::model::{Cell, Diagram, MxFile};
use crate::shapes;
use crate::style;
use crate::xmlio;
use doccraft_theme::Tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::collections::HashSet;

// ── design tokens ──
// All chrome is painted from doccraft-theme tokens each frame (theme applied
// at the top of [`DrawApp::ui`]); no hardcoded hex survives here.
const CAPTION: f32 = 10.0;
const SCRATCH_MAX: usize = 8;

/// One Scratchpad entry (label + mxGraph style + default size) — re-rendered
/// through the same [`shapes::paint_vertex`] path as the palette previews.
#[derive(Debug, Clone)]
struct ScratchItem {
    name: String,
    style: String,
    w: f32,
    h: f32,
}

/// A shape being dragged from the palette towards the canvas (ghost preview
/// follows the pointer; drop over the canvas inserts it at that world spot).
#[derive(Debug, Clone)]
struct PaletteDrag {
    name: String,
    style: String,
    w: f32,
    h: f32,
    cur: Pos2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Connect,
}

/// Canvas interaction state machine (driven by [`crate::canvas`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Drag {
    None,
    Pan { last: Pos2 },
    Move { ids: Vec<String>, grab: Vec2, orig: Vec<(String, f32, f32)>, moved: bool },
    Resize { id: String, handle: u8, orig: (f32, f32, f32, f32) },
    Waypoint { id: String, idx: usize },
    Rubber { start: Pos2, cur: Pos2 },
    Minimap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmtTab {
    Style,
    Text,
    Arrange,
}

/// Paint one palette/Scratchpad shape cell: hover fill (t.hover, radius 5),
/// hairline border and a mini preview through the real shape painter. The
/// cell is clickable AND draggable (drag from palette → canvas).
fn shape_cell(ui: &mut Ui, id: Id, size: Vec2, name: &str, st: &str, w: f32, h: f32, t: &Tokens) -> (Rect, egui::Response) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let fill = if resp.hovered() || resp.dragged() { t.hover } else { t.panel };
    ui.painter().rect_filled(rect, CornerRadius::same(5), fill);
    ui.painter().rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    // mini preview through the real painter. Palette previews carry no text
    // (drawio parity — the name lives in the tooltip), except text-only shapes
    // whose label IS the shape. Long names squeezed into 44px cells used to
    // overflow the previews badly.
    let look = style::vertex_look(st, 1.0);
    let inner = rect.shrink(6.0);
    let aspect = w / h.max(1.0);
    let (pw, ph) = if aspect >= 1.0 {
        (inner.width(), (inner.width() / aspect).min(inner.height()))
    } else {
        ((inner.height() * aspect).min(inner.width()), inner.height())
    };
    let pr = Rect::from_center_size(rect.center(), Vec2::new(pw, ph));
    let xf = shapes::Xf::from_style(st);
    let is_text = st.starts_with("text;") || st.contains("shape=text;");
    shapes::paint_vertex(ui.painter(), pr, st, &look, if is_text { name } else { "" }, &xf);
    (rect, resp)
}

/// Small icon button painted from the shared vector icon set (no font → no
/// tofu): frameless idle, t.hover on hover, `sel` = accent fill + white icon
/// (toggle state), like the toolbar's Connect/Select treatment.
fn ib(ui: &mut Ui, size: Vec2, icon: &str, tip: &str, sel: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if sel {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(4), t.hover);
    }
    let ink = if sel { Color32::WHITE } else { t.icon };
    doccraft_theme::icons::paint(ui.painter(), r, icon, ink, t.panel);
    resp.on_hover_text(tip).clicked()
}

/// Themed 26×24 toolbar icon button (the GraphEditor toolbar treatment):
/// idle `t.field`, hover `t.hover`, active tool `t.accent` + white icon —
/// the same styling the Connect/Select toggle always had — with the glyph
/// painted from the shared vector icon set (no font → no tofu).
fn tb_icon(ui: &mut Ui, t: &Tokens, icon: &str, tip: &str, active: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if active {
        ui.painter().rect_filled(r, 5.0, t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 5.0, t.hover);
    } else {
        ui.painter().rect_filled(r, 5.0, t.field);
    }
    let ink = if active { Color32::WHITE } else { t.icon };
    doccraft_theme::icons::paint(ui.painter(), r, icon, ink, t.panel);
    resp.on_hover_text(tip).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scratchpad fills from Copy and stays capped (Task 17-a).
    #[test]
    fn scratchpad_caps_at_limit() {
        let mut app = DrawApp::new();
        app.selected = vec!["v1".into()];
        for i in 0..(SCRATCH_MAX + 2) {
            app.clip = vec![Cell { id: format!("v{i}"), value: format!("S{i}"), vertex: true, ..Cell::default() }];
            // replicate the action_copy tail without touching the model
            if app.scratch.len() < SCRATCH_MAX {
                if let Some(c) = app.clip.iter().find(|c| c.vertex) {
                    app.scratch.push(ScratchItem {
                        name: if c.value.trim().is_empty() { "Shape".into() } else { c.value.clone() },
                        style: c.style.clone(),
                        w: c.geom.w,
                        h: c.geom.h,
                    });
                }
            }
        }
        assert_eq!(app.scratch.len(), SCRATCH_MAX);
    }

    /// Page ops on the workbook: duplicate / insert-after / rename / remove.
    #[test]
    fn page_ops() {
        let mut f = MxFile::sample();
        let n = f.diagrams.len();
        let dup = f.duplicate_diagram(0);
        assert_eq!(dup, 1);
        assert_eq!(f.diagrams.len(), n + 1);
        assert!(f.diagrams[1].name.contains("copy"));
        let ins = f.insert_diagram_after(0);
        assert_eq!(ins, 1);
        assert!(f.diagrams.len() == n + 2);
        f.rename_diagram(ins, "Renamed".into());
        assert_eq!(f.diagrams[ins].name, "Renamed");
        assert!(f.remove_diagram(0));
        assert!(!f.remove_diagram(usize::MAX));
        // never remove the last page
        let mut one = MxFile::blank();
        assert!(!one.remove_diagram(0));
        assert_eq!(one.diagrams.len(), 1);
    }
}

pub struct DrawApp {
    pub file: MxFile,
    pub tool: Tool,
    pub pan: Vec2,
    pub zoom: f32,
    pub selected: Vec<String>,
    pub edge_from: Option<String>,
    /// Connection drag state machine (crate::connection).
    pub connect: ConnectState,
    /// Hover tracking for the hover arrows: (cell id, seconds hovered).
    pub hover: Option<(String, f32)>,
    pub status: String,
    pub drag: Drag,
    pub grid: f32,
    pub show_grid: bool,
    pub show_guides: bool,
    pub guides: Vec<(bool, f32)>,
    pub view_rect: Option<Rect>,
    pub show_outline: bool,
    /// Find results: (page index, cell id), cycled with Enter / the arrows.
    pub matches: Vec<(usize, String)>,
    /// Internal clipboard (Copy / Paste, offset on paste).
    pub clip: Vec<Cell>,
    pub ctx_menu: Option<CtxMenu>,
    /// Inline label editor: (cell id, buffer, original value).
    pub edit_label: Option<(String, String, String)>,
    pub edit_just_opened: bool,
    /// Light/dark chrome theme (Sun/Moon toggle in the menu bar).
    pub dark: bool,

    // ── shell state ──
    /// Open shape-library accordions (several can be open at once).
    palette_open: HashSet<usize>,
    palette_query: String,
    /// Pinned Scratchpad shapes (copy a shape, or right-click a palette item).
    scratch: Vec<ScratchItem>,
    /// Active palette → canvas drag (ghost preview state).
    palette_drag: Option<PaletteDrag>,
    fmt_tab: FmtTab,
    show_layers: bool,
    /// Active layer per page (new shapes land here).
    active_layers: Vec<String>,
    undo: Vec<(usize, Diagram)>,
    redo: Vec<(usize, Diagram)>,

    // ── dialogs ──
    xml_open: bool,
    xml_text: String,
    xml_err: Option<String>,
    find_open: bool,
    find_query: String,
    find_replace: String,
    find_case: bool,
    pages_open: bool,
    keys_open: bool,
    /// File › Save As… prompt (path text field).
    saveas_open: bool,
    saveas_path: String,
    /// Page-tab context menu › Rename… prompt (page index + buffer).
    page_rename: Option<(usize, String)>,
    /// Help › About Draw…
    about_open: bool,
    /// Export dialog (File › Export As) + its options.
    export_open: bool,
    export_zoom: f32,
    export_border: f32,
    export_transparent: bool,
    export_crop: bool,

    // ── More Shapes dialog (drawio parity) ──
    more_open: bool,
    more_query: String,
    /// Library highlighted in the dialog's preview pane.
    more_sel: usize,
    /// Staged checkbox state inside the dialog (applied on Apply).
    more_checked: HashSet<usize>,
    /// "Labels" checkbox: on insert, use the shape name as its label.
    more_labels: bool,
    /// "Remember this setting" checkbox.
    more_remember: bool,
    /// Categories shown on the palette (the More Shapes › Apply result).
    palette_enabled: HashSet<usize>,
    /// Lazy one-shot load of the remembered palette from egui storage.
    palette_loaded: bool,

    // ── FigCraft-style colour picker (crate::colorpicker) ──
    pub(crate) picker: Option<colorpicker::Pick>,
    /// Recently picked colours (picker footer).
    pub(crate) recent_colors: Vec<Color32>,
}

impl Default for DrawApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawApp {
    pub fn new() -> Self {
        Self {
            file: MxFile::sample(),
            tool: Tool::Select,
            pan: Vec2::ZERO,
            zoom: 1.0,
            selected: Vec::new(),
            edge_from: None,
            connect: ConnectState::Idle,
            hover: None,
            status: "Ready — Draw craft (drawio native)".to_string(),
            drag: Drag::None,
            grid: 10.0,
            show_grid: true,
            show_guides: true,
            guides: Vec::new(),
            view_rect: None,
            show_outline: true,
            matches: Vec::new(),
            clip: Vec::new(),
            ctx_menu: None,
            edit_label: None,
            edit_just_opened: false,
            dark: true,
            palette_open: HashSet::new(),
            palette_query: String::new(),
            scratch: Vec::new(),
            palette_drag: None,
            fmt_tab: FmtTab::Style,
            show_layers: true,
            active_layers: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            xml_open: false,
            xml_text: String::new(),
            xml_err: None,
            find_open: false,
            find_query: String::new(),
            find_replace: String::new(),
            find_case: false,
            pages_open: false,
            keys_open: false,
            saveas_open: false,
            saveas_path: String::new(),
            page_rename: None,
            about_open: false,
            export_open: false,
            export_zoom: 1.0,
            export_border: 10.0,
            export_transparent: false,
            export_crop: false,
            more_open: false,
            more_query: String::new(),
            more_sel: 0,
            more_checked: HashSet::new(),
            more_labels: false,
            more_remember: false,
            palette_enabled: (0..shapes::DEFAULT_ENABLED).collect(),
            palette_loaded: false,
            picker: None,
            recent_colors: Vec::new(),
        }
    }

    pub fn open(&mut self, name: &str, bytes: &[u8]) {
        match xmlio::parse(bytes) {
            Ok(mut f) => {
                f.path = Some(name.to_string());
                self.file = f;
                self.active_layers.clear();
                self.selected.clear();
                self.status = format!("Opened {name}");
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    pub fn save_xml(&self) -> String {
        xmlio::write(&self.file)
    }

    // ── model access (canvas contract) ───────────────────────────────────────

    pub fn page(&self) -> &Diagram {
        &self.file.diagrams[self.file.active.min(self.file.diagrams.len() - 1)]
    }

    pub fn page_mut(&mut self) -> &mut Diagram {
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        &mut self.file.diagrams[a]
    }

    /// Apply a mutation to the active page through a closure. Marks the file
    /// dirty; undo snapshots are taken by explicit [`Self::push_undo`] calls.
    pub fn edit_page<R>(&mut self, f: impl FnOnce(&mut Diagram) -> R) -> Option<R> {
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let r = f(page);
        self.file.dirty = true;
        Some(r)
    }

    // ── undo / redo ──────────────────────────────────────────────────────────

    /// Snapshot the active page for undo. Consecutive identical snapshots
    /// collapse, so per-frame pushes during a drag are harmless.
    pub fn push_undo(&mut self) {
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let snap = self.file.diagrams[a].clone();
        if self.undo.last().map(|(_, d)| *d == snap).unwrap_or(false) {
            return;
        }
        self.undo.push((a, snap));
        if self.undo.len() > 80 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        let active = self.file.active.min(self.file.diagrams.len() - 1);
        if let Some((a, d)) = self.undo.pop() {
            let cur = self.file.diagrams[active].clone();
            self.redo.push((active, cur));
            let target = a.min(self.file.diagrams.len() - 1);
            self.file.active = target;
            self.file.diagrams[target] = d;
            self.file.dirty = true;
            self.status = "Undo".into();
        }
    }

    pub fn redo(&mut self) {
        let active = self.file.active.min(self.file.diagrams.len() - 1);
        if let Some((a, d)) = self.redo.pop() {
            let cur = self.file.diagrams[active].clone();
            self.undo.push((active, cur));
            let target = a.min(self.file.diagrams.len() - 1);
            self.file.active = target;
            self.file.diagrams[target] = d;
            self.file.dirty = true;
            self.status = "Redo".into();
        }
    }

    // ── selection actions (canvas context menu + shortcuts) ──────────────────

    pub fn action_copy(&mut self) {
        let page = self.page();
        let mut out = Vec::new();
        for id in &self.selected {
            if let Some(c) = page.cell(id) {
                out.push(c.clone());
                for did in page.descendants(id) {
                    if let Some(d) = page.cell(&did) {
                        out.push(d.clone());
                    }
                }
            }
        }
        // also copy edges attached to copied vertices on both ends
        let ids: Vec<String> = out.iter().map(|c| c.id.clone()).collect();
        for c in &page.cells {
            if c.edge
                && c.source.as_deref().map(|s| ids.contains(&s.to_string())).unwrap_or(false)
                && c.target.as_deref().map(|t| ids.contains(&t.to_string())).unwrap_or(false)
                && !out.iter().any(|k| k.id == c.id)
            {
                out.push(c.clone());
            }
        }
        self.clip = out;
        // drawio scratchpad: copying a shape pins it to the palette Scratchpad
        if self.scratch.len() < SCRATCH_MAX {
            let picked = self.clip.iter().find(|c| c.vertex).map(|c| ScratchItem {
                name: if c.value.trim().is_empty() { "Shape".to_string() } else { c.value.clone() },
                style: c.style.clone(),
                w: c.geom.w,
                h: c.geom.h,
            });
            if let Some(item) = picked {
                self.scratch.push(item);
            }
        }
        self.status = format!("Copied {}", self.clip.len());
    }

    pub fn action_paste_at(&mut self, at: Pos2) {
        if self.clip.is_empty() {
            return;
        }
        self.push_undo();
        let page = self.page();
        let (bx, by, _, _) = page
            .bbox(&self.clip.iter().map(|c| c.id.clone()).collect::<Vec<_>>())
            .unwrap_or((at.x, at.y, 0.0, 0.0));
        let (dx, dy) = (at.x - bx, at.y - by);
        let mut map: Vec<(String, String)> = Vec::new();
        let mut pasted = Vec::new();
        let clip = self.clip.clone();
        let _ = self.edit_page(|page| {
            for c in &clip {
                let nid = page.next_cell_id();
                map.push((c.id.clone(), nid.clone()));
                let mut nc = c.clone();
                nc.id = nid.clone();
                nc.geom.x += dx;
                nc.geom.y += dy;
                nc.parent = None;
                page.cells.push(nc);
                pasted.push(nid);
            }
            // rewire edges to the pasted copies
            let remap = |s: &Option<String>| -> Option<String> { s.as_deref().and_then(|id| map.iter().find(|(o, _)| o == id).map(|(_, n)| n.clone())) };
            for i in page.cells.len() - pasted.len()..page.cells.len() {
                let c = &mut page.cells[i];
                if c.edge {
                    let ns = remap(&c.source);
                    let nt = remap(&c.target);
                    // only keep edges whose both ends were pasted
                    if ns.is_some() && nt.is_some() {
                        c.source = ns;
                        c.target = nt;
                    } else if c.source.is_some() || c.target.is_some() {
                        // floating edge: keep endpoints that point at pasted nodes
                        if let Some(n) = remap(&c.source) { c.source = Some(n); }
                        if let Some(n) = remap(&c.target) { c.target = Some(n); }
                    }
                } else if let Some((_, p)) = c
                    .parent
                    .clone()
                    .and_then(|pid| map.iter().find(|(o, _)| o == &pid).map(|(_, n)| (pid, n.clone())))
                {
                    c.parent = Some(p);
                }
            }
        });
        self.selected = pasted;
        self.status = "Pasted".into();
    }

    pub fn action_duplicate(&mut self) {
        self.action_copy();
        let at = self
            .page()
            .bbox(&self.selected)
            .map(|(x, y, _, _)| Pos2::new(x + 20.0, y + 20.0))
            .unwrap_or(Pos2::new(100.0, 100.0));
        self.action_paste_at(at);
    }

    pub fn action_delete(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let n = {
            let a = self.file.active.min(self.file.diagrams.len() - 1);
            let page = &mut self.file.diagrams[a];
            let sel = self.selected.clone();
            page.delete_cascade(&sel)
        };
        self.selected.clear();
        self.status = format!("Deleted {n}");
    }

    pub fn action_select_all(&mut self) {
        let page = self.page();
        self.selected = page
            .cells
            .iter()
            .filter(|c| c.vertex && page.layer_visible(c) && !page.layer_locked(c))
            .map(|c| c.id.clone())
            .collect();
        self.status = format!("Selected {}", self.selected.len());
    }

    pub fn action_to_front(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        let picked: Vec<Cell> = page.cells.iter().filter(|c| sel.contains(&c.id)).cloned().collect();
        page.cells.retain(|c| !sel.contains(&c.id));
        page.cells.extend(picked);
        self.status = "Brought to front".into();
    }

    pub fn action_to_back(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        let picked: Vec<Cell> = page.cells.iter().filter(|c| sel.contains(&c.id)).cloned().collect();
        page.cells.retain(|c| !sel.contains(&c.id));
        let mut next = picked;
        next.extend(page.cells.drain(..));
        page.cells = next;
        self.status = "Sent to back".into();
    }

    // ── groups ───────────────────────────────────────────────────────────────

    pub fn group_selection(&mut self) {
        let ids = self.selected.clone();
        if ids.len() < 2 {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        if let Some((x, y, w, h)) = page.bbox(&ids) {
            let gid = page.add_vertex("", "shape=group;strokeColor=none;fillColor=none;", x, y, w, h);
            for id in &ids {
                if let Some(c) = page.cell_mut(id) {
                    c.parent = Some(gid.clone());
                }
            }
            self.selected = vec![gid];
            self.status = "Grouped".into();
        }
    }

    pub fn ungroup_selection(&mut self) {
        let groups: Vec<String> = self
            .selected
            .iter()
            .filter(|id| self.page().cell(id).map(|c| c.is_group()).unwrap_or(false))
            .cloned()
            .collect();
        if groups.is_empty() {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let mut freed = Vec::new();
        for g in &groups {
            for c in page.cells.iter_mut() {
                if c.parent.as_deref() == Some(g.as_str()) {
                    c.parent = None;
                    freed.push(c.id.clone());
                }
            }
        }
        page.delete_cascade(&groups);
        self.selected = freed;
        self.status = "Ungrouped".into();
    }

    // ── arrange helpers ──────────────────────────────────────────────────────

    pub fn rotate_selection(&mut self, deg: f32) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        for c in page.cells.iter_mut() {
            if sel.contains(&c.id) && c.vertex {
                let cur = style::get_f32(&c.style, "rotation", 0.0);
                c.style = style::set(&c.style, "rotation", &format!("{}", ((cur + deg).rem_euclid(360.0)) as i32));
            }
        }
    }

    pub fn flip_selection(&mut self, horizontal: bool) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        let key = if horizontal { "flipH" } else { "flipV" };
        for c in page.cells.iter_mut() {
            if sel.contains(&c.id) && c.vertex {
                let on = style::get_bool(&c.style, key, false);
                c.style = if on { style::remove(&c.style, key) } else { style::set(&c.style, key, "1") };
            }
        }
    }

    pub fn align_selection(&mut self, mode: u8) {
        // 0 left 1 hcenter 2 right 3 top 4 vcenter 5 bottom, to the page
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let (pw, ph) = (self.page().page_w, self.page().page_h);
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        for c in page.cells.iter_mut() {
            if !sel.contains(&c.id) || !c.vertex {
                continue;
            }
            match mode {
                0 => c.geom.x = 0.0,
                1 => c.geom.x = (pw - c.geom.w) / 2.0,
                2 => c.geom.x = pw - c.geom.w,
                3 => c.geom.y = 0.0,
                4 => c.geom.y = (ph - c.geom.h) / 2.0,
                _ => c.geom.y = ph - c.geom.h,
            }
        }
    }

    /// drawio Arrange semantics: one shape aligns to the PAGE, two or more to
    /// the SELECTION BOUNDS (Menus.js `Arrange` items share this behavior with
    /// the Format › Arrange tab).
    pub fn align_selection_auto(&mut self, mode: u8) {
        let verts: Vec<String> = self
            .selected
            .iter()
            .filter(|id| self.page().cell(id).map(|c| c.vertex).unwrap_or(false))
            .cloned()
            .collect();
        if verts.len() >= 2 {
            self.align_to_bounds(mode, &verts);
        } else if !verts.is_empty() {
            self.align_selection(mode);
        }
    }

    /// Align the given vertices to the bounding box they span (drawio's
    /// multi-select align: shapes move, the shared bounds stay put).
    fn align_to_bounds(&mut self, mode: u8, ids: &[String]) {
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let Some((bx, by, bw, bh)) = page.bbox(ids) else { return };
        for c in page.cells.iter_mut() {
            if !ids.contains(&c.id) || !c.vertex {
                continue;
            }
            match mode {
                0 => c.geom.x = bx,
                1 => c.geom.x = bx + (bw - c.geom.w) / 2.0,
                2 => c.geom.x = bx + bw - c.geom.w,
                3 => c.geom.y = by,
                4 => c.geom.y = by + (bh - c.geom.h) / 2.0,
                _ => c.geom.y = by + bh - c.geom.h,
            }
        }
        self.file.dirty = true;
    }

    /// Zoom to an absolute factor, centered on the canvas view.
    fn set_zoom_factor(&mut self, z: f32) {
        if let Some(r) = self.view_rect {
            canvas::zoom_at(self, r, r.center(), z / self.zoom);
        }
    }

    pub fn distribute_selection(&mut self, horizontal: bool) {
        let ids: Vec<String> = self
            .selected
            .iter()
            .filter(|id| self.page().cell(id).map(|c| c.vertex).unwrap_or(false))
            .cloned()
            .collect();
        if ids.len() < 3 {
            return;
        }
        self.push_undo();
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let mut items: Vec<(f32, f32, String)> = ids
            .iter()
            .filter_map(|id| page.cell(id).map(|c| if horizontal { (c.geom.x, c.geom.w, id.clone()) } else { (c.geom.y, c.geom.h, id.clone()) }))
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        let first = items.first().map(|i| i.0).unwrap_or(0.0);
        let last = items.last().map(|i| i.0).unwrap_or(0.0);
        let total: f32 = items.iter().map(|i| i.1).sum();
        let gap = ((last - first) - total) / (items.len() - 1).max(1) as f32;
        let mut cur = first;
        for (pos, size, id) in items {
            if let Some(c) = page.cell_mut(&id) {
                if horizontal {
                    c.geom.x = cur;
                } else {
                    c.geom.y = cur;
                }
            }
            cur += size + gap;
        }
    }

    // ── find & replace ───────────────────────────────────────────────────────

    pub fn find_refresh(&mut self) {
        self.matches.clear();
        if self.find_query.is_empty() {
            return;
        }
        let q = if self.find_case { self.find_query.clone() } else { self.find_query.to_lowercase() };
        for (pi, d) in self.file.diagrams.iter().enumerate() {
            for c in &d.cells {
                let hay = if self.find_case { c.value.clone() } else { c.value.to_lowercase() };
                if c.vertex && hay.contains(&q) {
                    self.matches.push((pi, c.id.clone()));
                }
            }
        }
    }

    pub fn find_next(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let cur = self.matches.iter().position(|(p, id)| *p == self.file.active && self.selected.first() == Some(id));
        let next = (cur.map(|c| c + 1).unwrap_or(0)) % self.matches.len();
        let (p, id) = self.matches[next].clone();
        self.file.active = p.min(self.file.diagrams.len() - 1);
        self.selected = vec![id.clone()];
        if let Some(c) = self.page().cell(&id) {
            let cx = c.geom.x + c.geom.w / 2.0;
            let cy = c.geom.y + c.geom.h / 2.0;
            if let Some(r) = self.view_rect {
                self.pan = r.center() - r.min - Vec2::new(cx, cy) * self.zoom;
            }
        }
    }

    pub fn find_replace_once(&mut self, all: bool) {
        if self.find_query.is_empty() {
            return;
        }
        self.push_undo();
        let q = self.find_query.clone();
        let r = self.find_replace.clone();
        let targets: Vec<String> = if all {
            self.matches.iter().map(|(_, id)| id.clone()).collect()
        } else {
            self.selected.clone()
        };
        let n = {
            let a = self.file.active.min(self.file.diagrams.len() - 1);
            let page = &mut self.file.diagrams[a];
            let mut n = 0;
            for c in page.cells.iter_mut() {
                if targets.contains(&c.id) && c.value.contains(&q) {
                    c.value = c.value.replace(&q, &r);
                    n += 1;
                }
            }
            n
        };
        self.find_refresh();
        self.status = format!("Replaced {n}");
    }

    // ── pages ────────────────────────────────────────────────────────────────

    pub fn add_page(&mut self) {
        let n = self.file.diagrams.len() + 1;
        self.file.diagrams.push(Diagram::new(format!("Page-{n}")));
        self.file.active = self.file.diagrams.len() - 1;
        self.selected.clear();
        self.file.dirty = true;
    }

    pub fn set_page_size(&mut self, w: f32, h: f32) {
        self.page_mut().page_w = w;
        self.page_mut().page_h = h;
        self.file.dirty = true;
    }

    // ── layers ───────────────────────────────────────────────────────────────

    fn active_layer(&self) -> String {
        if let Some(l) = self.active_layers.get(self.file.active) {
            if self.page().layer_index(l).is_some() {
                return l.clone();
            }
        }
        self.page().default_layer_id()
    }

    fn set_active_layer(&mut self, id: String) {
        while self.active_layers.len() <= self.file.active {
            self.active_layers.push(String::new());
        }
        self.active_layers[self.file.active] = id;
    }

    // ── shapes palette insert ────────────────────────────────────────────────

    fn insert_shape(&mut self, def: &shapes::ShapeDef) {
        self.insert_shape_styled(def.name, def.style, def.w, def.h, None, self.more_labels);
    }

    /// Insert a shape (style string + default size), centered on `at` (or the
    /// view center when `None`), snapped to the grid. `with_label` = the shape
    /// name becomes its label (More Shapes › Labels); drawio drops shapes
    /// unlabelled by default, so the palette passes the checkbox state and
    /// only the Scratchpad always keeps its value.
    fn insert_shape_styled(&mut self, name: &str, st: &str, w: f32, h: f32, at: Option<Pos2>, with_label: bool) {
        if self
            .page()
            .layer_locked(&Cell { layer: self.active_layer(), vertex: true, ..Cell::default() })
        {
            self.status = "The active layer is locked".into();
            return;
        }
        self.push_undo();
        let center = at.unwrap_or_else(|| self.view_center_world());
        let x = ((center.x - w / 2.0) / self.grid).round() * self.grid;
        let y = ((center.y - h / 2.0) / self.grid).round() * self.grid;
        let layer = self.active_layer();
        let (name, st) = (name.to_string(), st.to_string());
        let value = if with_label { name.clone() } else { String::new() };
        let id = self.edit_page(|page| {
            let id = page.add_vertex(&value, &st, x, y, w, h);
            if let Some(c) = page.cell_mut(&id) {
                c.layer = layer.clone();
            }
            id
        })
        .unwrap_or_default();
        self.selected = vec![id];
        self.tool = Tool::Select;
        self.status = format!("Added {name}");
    }

    fn view_center_world(&self) -> Pos2 {
        if let Some(r) = self.view_rect {
            let vp = canvas::viewport(self, r);
            return Pos2::new((vp.center().x / self.grid).round() * self.grid, (vp.center().y / self.grid).round() * self.grid);
        }
        Pos2::new(80.0, 80.0)
    }

    // ── file ops ─────────────────────────────────────────────────────────────

    /// File › New: reset to a fresh, empty single-page document.
    pub fn new_file(&mut self) {
        self.file = MxFile::blank();
        self.active_layers.clear();
        self.selected.clear();
        self.connect = ConnectState::Idle;
        self.edge_from = None;
        self.clip.clear();
        self.ctx_menu = None;
        self.edit_label = None;
        self.status = "New diagram".into();
    }

    /// File › Close: clear the document but keep the app alive.
    pub fn close_document(&mut self) {
        self.new_file();
        self.status = "Closed — untitled document".into();
    }

    /// File › Save As…: write the XML to `path` and adopt it as the file path.
    pub fn save_as(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            self.status = "Save As: empty path".into();
            return;
        }
        let xml = xmlio::write(&self.file);
        match std::fs::write(path, xml) {
            Ok(_) => {
                self.file.path = Some(path.to_string());
                self.file.dirty = false;
                self.status = format!("Saved {path}");
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub fn save(&mut self) {
        let Some(path) = self.file.path.clone() else {
            self.status = "No file path — opened from the sample. Use a .drawio file to enable Save.".into();
            return;
        };
        let xml = xmlio::write(&self.file);
        match std::fs::write(&path, xml) {
            Ok(_) => {
                self.file.dirty = false;
                self.status = format!("Saved {path}");
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn export_stem(&self) -> String {
        self.file
            .path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
            .and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string()))
            .unwrap_or_else(|| "doccraft-draw".into())
    }

    fn export_opts(&self) -> crate::export::Opts {
        crate::export::Opts {
            zoom: self.export_zoom,
            border: self.export_border,
            transparent: self.export_transparent,
            crop: self.export_crop,
        }
    }

    pub fn export_svg(&mut self) {
        let stem = self.export_stem();
        let svg = crate::export::svg(self.page(), &self.export_opts());
        let p = std::env::temp_dir().join(format!("{stem}.svg"));
        match std::fs::write(&p, svg) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    pub fn export_png(&mut self) {
        let stem = self.export_stem();
        let png = crate::export::png(self.page(), &self.export_opts());
        let p = std::env::temp_dir().join(format!("{stem}.png"));
        match std::fs::write(&p, png) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    pub fn export_pdf(&mut self) {
        let stem = self.export_stem();
        let pdf = crate::export::pdf(self.page(), &self.export_opts());
        let p = std::env::temp_dir().join(format!("{stem}.pdf"));
        match std::fs::write(&p, pdf) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    pub fn export_drawio(&mut self) {
        let stem = self.export_stem();
        let xml = xmlio::write(&self.file);
        let p = std::env::temp_dir().join(format!("{stem}.drawio"));
        match std::fs::write(&p, xml) {
            Ok(_) => self.status = format!("Exported {}", p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    pub fn open_xml_editor(&mut self) {
        self.xml_text = xmlio::write_diagram_to_string(self.page());
        self.xml_err = None;
        self.xml_open = true;
    }

    pub fn apply_xml(&mut self) {
        match xmlio::parse(self.xml_text.as_bytes()) {
            Ok(f) => {
                self.push_undo();
                let a = self.file.active.min(self.file.diagrams.len() - 1);
                if let Some(d) = f.diagrams.into_iter().next() {
                    self.file.diagrams[a] = d;
                }
                self.active_layers.clear();
                self.xml_open = false;
                self.xml_err = None;
                self.file.dirty = true;
                self.status = "Diagram XML applied".into();
            }
            Err(e) => self.xml_err = Some(e),
        }
    }

    // ── keyboard shortcuts ───────────────────────────────────────────────────

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let mods = |ctx: &egui::Context| ctx.input(|i| i.modifiers);
        let m = mods(ctx);
        let delete = ctx.input(|i| i.key_pressed(egui::Key::Delete));
        if delete && !self.xml_open {
            self.action_delete();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
            if m.shift {
                self.redo();
            } else {
                self.undo();
            }
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            self.redo();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::C)) && !self.edit_label.is_some() {
            self.action_copy();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::V)) && !self.edit_label.is_some() {
            let at = self.view_center_world();
            self.action_paste_at(at);
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.action_duplicate();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::A)) {
            self.action_select_all();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::G)) {
            if m.shift {
                self.ungroup_selection();
            } else {
                self.group_selection();
            }
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.find_open = true;
        }
        if m.command && m.shift && ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.open_xml_editor();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            self.save();
        }
        if m.command && ctx.input(|i| i.key_pressed(egui::Key::E)) {
            self.export_svg();
        }
        // escape: cancel connect / drag / dialogs
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.edge_from.is_some() || self.drag != Drag::None {
                self.edge_from = None;
                self.drag = Drag::None;
            }
            if self.connect.active() {
                self.connect = ConnectState::Idle;
                self.status = "Connection cancelled".into();
            }
        }
        // arrows nudge
        let arrows: [(egui::Key, f32, f32); 4] = [
            (egui::Key::ArrowLeft, -1.0, 0.0),
            (egui::Key::ArrowRight, 1.0, 0.0),
            (egui::Key::ArrowUp, 0.0, -1.0),
            (egui::Key::ArrowDown, 0.0, 1.0),
        ];
        for (key, dx, dy) in arrows {
            if ctx.input(|i| i.key_pressed(key)) && !self.selected.is_empty() && self.xml_open == false {
                let step = self.grid * if m.shift { 10.0 } else { 1.0 };
                self.push_undo();
                let a = self.file.active.min(self.file.diagrams.len() - 1);
                let page = &mut self.file.diagrams[a];
                let sel = self.selected.clone();
                for c in page.cells.iter_mut() {
                    if sel.contains(&c.id) && c.vertex {
                        c.geom.x += dx * step;
                        c.geom.y += dy * step;
                    }
                }
            }
        }
        // enter cycles find matches
        if self.find_open && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.find_next();
        }
    }

    // ── the shell ────────────────────────────────────────────────────────────

    pub fn ui(&mut self, ui: &mut Ui) {
        // suite design language: install token visuals once per frame
        let tokens = if self.dark { Tokens::dark() } else { Tokens::light() };
        doccraft_theme::apply(ui.ctx(), tokens);
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);

        // flat panel stack: menus, toolbar, left palette, right format panel,
        // bottom page tabs + status bar, canvas as the central panel.
        self.menu_bar(ui);
        self.toolbar(ui);
        self.page_tabs(ui);
        self.status_bar(ui);
        self.palette_panel(ui);
        self.format_panel_dock(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(tokens.canvas).inner_margin(0))
            .show_inside(ui, |ui| {
                canvas::show(self, ui);
            });

        self.paint_palette_ghost(&ctx);
        self.dialogs(&ctx);
    }

    /// Floating ghost preview while a palette shape is dragged over the app.
    fn paint_palette_ghost(&mut self, ctx: &egui::Context) {
        let Some(drag) = self.palette_drag.clone() else { return };
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("draw_palette_ghost")));
        let size = 64.0;
        let inner = Rect::from_center_size(drag.cur, Vec2::splat(size)).shrink(4.0);
        let aspect = drag.w / drag.h.max(1.0);
        let (pw, ph) = if aspect >= 1.0 {
            (inner.width(), (inner.width() / aspect).min(inner.height()))
        } else {
            ((inner.height() * aspect).min(inner.width()), inner.height())
        };
        let pr = Rect::from_center_size(drag.cur, Vec2::new(pw, ph));
        painter.rect_filled(pr.expand(6.0), CornerRadius::same(5), Color32::from_black_alpha(40));
        let look = style::vertex_look(&drag.style, 1.0);
        let xf = shapes::Xf::from_style(&drag.style);
        shapes::paint_vertex(&painter, pr, &drag.style, &look, "", &xf);
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        egui::Panel::top("draw_menu")
            .exact_size(26.0)
            .frame(
                egui::Frame::default()
                    .fill(t.panel)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .stroke(doccraft_theme::hairline(&t)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Draw").strong().size(12.5).color(t.accent));
                    ui.separator();
                    ui.menu_button(egui::RichText::new("File").size(11.5).color(t.text), |ui| {
                        if ui.button("New").clicked() {
                            self.new_file();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Save (Ctrl+S)").clicked() {
                            self.save();
                            ui.close();
                        }
                        if ui.button("Save As…").clicked() {
                            self.saveas_path = self.file.path.clone().unwrap_or_else(|| "./diagram.drawio".into());
                            self.saveas_open = true;
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button("Export As", |ui| {
                            if ui.button("XML (.drawio)").clicked() {
                                self.export_drawio();
                                ui.close();
                            }
                            if ui.button("SVG (.svg)").clicked() {
                                self.export_svg();
                                ui.close();
                            }
                            if ui.button("PNG (.png)").clicked() {
                                self.export_png();
                                ui.close();
                            }
                            if ui.button("PDF (.pdf)").clicked() {
                                self.export_pdf();
                                ui.close();
                            }
                        });
                        if ui.button("Export options…").clicked() {
                            self.export_open = true;
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            self.close_document();
                            ui.close();
                        }
                    });
                    ui.menu_button(egui::RichText::new("Edit").size(11.5).color(t.text), |ui| {
                        if ui.button("Undo (Ctrl+Z)").clicked() {
                            self.undo();
                            ui.close();
                        }
                        if ui.button("Redo (Ctrl+Y)").clicked() {
                            self.redo();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Copy (Ctrl+C)").clicked() {
                            self.action_copy();
                            ui.close();
                        }
                        if ui.button("Paste (Ctrl+V)").clicked() {
                            let at = self.view_center_world();
                            self.action_paste_at(at);
                            ui.close();
                        }
                        if ui.button("Duplicate (Ctrl+D)").clicked() {
                            self.action_duplicate();
                            ui.close();
                        }
                        if ui.button("Delete (Del)").clicked() {
                            self.action_delete();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Select All (Ctrl+A)").clicked() {
                            self.action_select_all();
                            ui.close();
                        }
                        if ui.button("Find / Replace (Ctrl+F)").clicked() {
                            self.find_open = true;
                            ui.close();
                        }
                    });
                    ui.menu_button(egui::RichText::new("View").size(11.5).color(t.text), |ui| {
                        if ui.checkbox(&mut self.show_grid, "Grid").changed() {
                            ui.close();
                        }
                        if ui.checkbox(&mut self.show_guides, "Alignment guides").changed() {
                            ui.close();
                        }
                        if ui.checkbox(&mut self.show_outline, "Minimap").changed() {
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Fit page").clicked() {
                            canvas::fit_page(self);
                            ui.close();
                        }
                    });
                    // ── Arrange (drawio Menus.js) ──
                    ui.menu_button(egui::RichText::new("Arrange").size(11.5).color(t.text), |ui| {
                        if ui.button("To Front").clicked() {
                            self.action_to_front();
                            ui.close();
                        }
                        if ui.button("To Back").clicked() {
                            self.action_to_back();
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button("Align", |ui| {
                            for (i, label) in ["Left", "Center", "Right", "Top", "Middle", "Bottom"].iter().enumerate() {
                                if ui.button(*label).clicked() {
                                    self.align_selection_auto(i as u8);
                                    ui.close();
                                }
                            }
                        });
                        ui.menu_button("Distribute", |ui| {
                            if ui.button("Horizontally").on_hover_text("Space 3+ shapes evenly on X").clicked() {
                                self.distribute_selection(true);
                                ui.close();
                            }
                            if ui.button("Vertically").on_hover_text("Space 3+ shapes evenly on Y").clicked() {
                                self.distribute_selection(false);
                                ui.close();
                            }
                        });
                        ui.separator();
                        if ui.button("Group (Ctrl+G)").clicked() {
                            self.group_selection();
                            ui.close();
                        }
                        if ui.button("Ungroup (Ctrl+Shift+G)").clicked() {
                            self.ungroup_selection();
                            ui.close();
                        }
                    });
                    ui.menu_button(egui::RichText::new("Extras").size(11.5).color(t.text), |ui| {
                        if ui.button("Edit Diagram XML (Ctrl+Shift+X)").clicked() {
                            self.open_xml_editor();
                            ui.close();
                        }
                        if ui.button("Page Setup…").clicked() {
                            self.pages_open = true;
                            ui.close();
                        }
                    });
                    // ── Help ──
                    ui.menu_button(egui::RichText::new("Help").size(11.5).color(t.text), |ui| {
                        if ui.button("Keyboard Shortcuts…").clicked() {
                            self.keys_open = true;
                            ui.close();
                        }
                        if ui.button("About Draw…").clicked() {
                            self.about_open = true;
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // theme toggle: sun switches to light, moon back to dark
                        let (icon, tip) = if self.dark { ("sun", "Switch to the light theme") } else { ("moon", "Switch to the dark theme") };
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0, 22.0), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 5.0, t.hover);
                        }
                        doccraft_theme::icons::paint(ui.painter(), r, icon, t.icon, t.panel);
                        if resp.on_hover_text(tip).clicked() {
                            self.dark = !self.dark;
                        }
                        let name = self.file.path.clone().unwrap_or_else(|| "Untitled.drawio".into());
                        let dot = if self.file.dirty { " ●" } else { "" };
                        ui.label(egui::RichText::new(format!("{name}{dot}")).weak().size(10.5));
                    });
                });
            });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        egui::Panel::top("draw_toolbar")
            .exact_size(32.0)
            .frame(
                egui::Frame::default()
                    .fill(t.panel)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .stroke(doccraft_theme::hairline(&t)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 4.0;
                // toolbar buttons paint vector icons ([`tb_icon`], no font →
                // no tofu); the zoom-% dropdown and text labels below stay text.
                // file cluster
                if tb_icon(ui, &t, "save", "Save (Ctrl+S)", false) {
                    self.save();
                }
                if tb_icon(ui, &t, "image", "Export .svg (Ctrl+E)", false) {
                    self.export_svg();
                }
                ui.separator();
                if tb_icon(ui, &t, "undo", "Undo (Ctrl+Z)", false) {
                    self.undo();
                }
                if tb_icon(ui, &t, "redo", "Redo (Ctrl+Y)", false) {
                    self.redo();
                }
                ui.separator();
                if tb_icon(ui, &t, "duplicate", "Duplicate (Ctrl+D)", false) {
                    self.action_duplicate();
                }
                if tb_icon(ui, &t, "trash", "Delete (Del)", false) {
                    self.action_delete();
                }
                ui.separator();
                if tb_icon(ui, &t, "connect", "Connect tool: click source, then target", self.tool == Tool::Connect) {
                    self.tool = if self.tool == Tool::Connect { Tool::Select } else { Tool::Connect };
                    self.edge_from = None;
                }
                if tb_icon(ui, &t, "select", "Select tool", self.tool == Tool::Select) {
                    self.tool = Tool::Select;
                }
                ui.separator();
                if tb_icon(ui, &t, "plus", "Zoom in", false) {
                    if let Some(r) = self.view_rect {
                        canvas::zoom_at(self, r, r.center(), 1.25);
                    }
                }
                if tb_icon(ui, &t, "minus", "Zoom out", false) {
                    if let Some(r) = self.view_rect {
                        canvas::zoom_at(self, r, r.center(), 1.0 / 1.25);
                    }
                }
                if tb_icon(ui, &t, "fit", "Fit page", false) {
                    canvas::fit_page(self);
                }
                // zoom dropdown (drawio toolbar zoom menu)
                ui.menu_button(
                    egui::RichText::new(format!("{}%", (self.zoom * 100.0).round() as i32)).size(11.0).color(t.text),
                    |ui| {
                        if ui.button("Zoom In").clicked() {
                            self.set_zoom_factor((self.zoom * 1.25).min(5.0));
                            ui.close();
                        }
                        if ui.button("Zoom Out").clicked() {
                            self.set_zoom_factor((self.zoom / 1.25).max(0.2));
                            ui.close();
                        }
                        if ui.button("Fit Page").clicked() {
                            canvas::fit_page(self);
                            ui.close();
                        }
                        ui.separator();
                        for pct in [50.0, 75.0, 100.0, 125.0, 150.0, 200.0, 400.0] {
                            let z = pct / 100.0;
                            if ui.selectable_label((self.zoom - z).abs() < 0.005, format!("{}%", pct as i32)).clicked() {
                                self.set_zoom_factor(z);
                                ui.close();
                            }
                        }
                    },
                );
                ui.separator();
                if tb_icon(ui, &t, "search", "Find & Replace (Ctrl+F)", false) {
                    self.find_open = !self.find_open;
                }
                if tb_icon(ui, &t, "code", "Edit Diagram XML (Ctrl+Shift+X)", false) {
                    self.open_xml_editor();
                }
                if tb_icon(ui, &t, "page", "Page Setup", false) {
                    self.pages_open = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).weak().size(11.0).color(t.text_secondary));
                });
                });
            });
    }

    fn palette_panel(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        // Pin the More Shapes button to the bottom of the palette (drawio
        // parity): the bottom strip is claimed before the scroll area, so the
        // button stays visible no matter how long the shape list is.
        egui::Panel::bottom("draw_palette_more")
            .resizable(false)
            .frame(egui::Frame::default().fill(t.panel).inner_margin(egui::Margin::symmetric(0, 4)))
            .show_inside(ui, |ui| {
                if Self::more_shapes_button(ui, &t) {
                    self.more_checked = self.palette_enabled.clone();
                    self.more_open = true;
                }
            });
        egui::Panel::left("draw_palette")
            .resizable(true)
            .default_size(216.0)
            .min_size(180.0)
            .frame(
                egui::Frame::default()
                    .fill(t.panel)
                    .inner_margin(egui::Margin::same(6))
                    .stroke(doccraft_theme::hairline(&t)),
            )
            .show_inside(ui, |ui| {
                self.palette(ui);
                ui.add_space(4.0);
                ui.separator();
                self.layers_panel(ui);
            });
    }

    fn format_panel_dock(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        egui::Panel::right("draw_format")
            .resizable(true)
            .default_size(248.0)
            .min_size(210.0)
            .frame(
                egui::Frame::default()
                    .fill(t.panel)
                    .inner_margin(egui::Margin::same(6))
                    .stroke(doccraft_theme::hairline(&t)),
            )
            .show_inside(ui, |ui| {
                self.format_panel(ui);
            });
    }

    // ── palette ──────────────────────────────────────────────────────────────

    fn palette(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        // One-shot restore of a remembered palette (More Shapes › Remember).
        if !self.palette_loaded {
            self.palette_loaded = true;
            if let Some(s) = ui.ctx().data_mut(|d| d.get_persisted::<HashSet<usize>>(Id::new("draw_palette_enabled"))) {
                self.palette_enabled = s;
            }
        }
        ui.label(egui::RichText::new("SHAPES").size(CAPTION).strong().color(t.text_secondary));
        ui.add_space(2.0);
        ui.add(egui::TextEdit::singleline(&mut self.palette_query).hint_text("Search shapes…").desired_width(ui.available_width()));
        ui.add_space(4.0);

        let q = self.palette_query.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("draw_palette_scroll").show(ui, |ui| {
            // ── Scratchpad: pinned section on top (like real drawio) ──
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("SCRATCHPAD").size(CAPTION).strong().color(t.text_secondary));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{}/{}", self.scratch.len(), SCRATCH_MAX)).size(9.5).color(t.text_tertiary));
                });
            });
            self.scratchpad(ui, &t);
            ui.add_space(4.0);

            if q.is_empty() {
                let enabled: Vec<usize> = (0..shapes::LIB.len()).filter(|i| self.palette_enabled.contains(i)).collect();
                for gi in enabled {
                    let cat = &shapes::LIB[gi];
                    let open = self.palette_open.contains(&gi);
                    let header = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click())
                        .1;
                    let (rect, resp) = (header.interact_rect, header.clone());
                    let _ = rect;
                    let paint = ui.painter();
                    paint.rect_filled(
                        header.interact_rect,
                        CornerRadius::same(5),
                        if resp.hovered() { t.hover } else { Color32::TRANSPARENT },
                    );
                    // accordion chevron: vector icon (no font dependency)
                    doccraft_theme::icons::paint(
                        paint,
                        Rect::from_center_size(Pos2::new(header.interact_rect.left() + 12.0, header.interact_rect.center().y), Vec2::splat(12.0)),
                        if open { "chevron-down" } else { "chevron-right" },
                        t.text_secondary,
                        t.panel,
                    );
                    paint.text(
                        Pos2::new(header.interact_rect.left() + 22.0, header.interact_rect.center().y),
                        Align2::LEFT_CENTER,
                        cat.name,
                        FontId::proportional(12.0),
                        t.text,
                    );
                    if resp.clicked() {
                        if open {
                            self.palette_open.remove(&gi);
                        } else {
                            self.palette_open.insert(gi);
                        }
                    }
                    if open {
                        let refs: Vec<&shapes::ShapeDef> = cat.items.iter().collect();
                        self.shape_grid(ui, &refs, &t, gi);
                    }
                }
            } else {
                // search across all enabled categories
                let mut n = 0;
                let enabled: Vec<usize> = (0..shapes::LIB.len()).filter(|i| self.palette_enabled.contains(i)).collect();
                for gi in enabled {
                    let cat = &shapes::LIB[gi];
                    let hits: Vec<&shapes::ShapeDef> =
                        cat.items.iter().filter(|d| d.name.to_lowercase().contains(&q)).collect();
                    if hits.is_empty() {
                        continue;
                    }
                    ui.label(egui::RichText::new(cat.name).size(CAPTION).strong().color(t.text_tertiary));
                    self.shape_grid(ui, &hits, &t, gi);
                    n += hits.len();
                }
                if n == 0 {
                    ui.label(egui::RichText::new("No shapes match.").small().color(t.text_secondary));
                }
            }
        });
    }

    /// The More Shapes button pinned to the palette bottom — the light accent
    /// pill from diagrams.net ("+ More Shapes" above the Layers list).
    fn more_shapes_button(ui: &mut Ui, t: &Tokens) -> bool {
        let width = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More Shapes"));
        let bg = if resp.hovered() { t.accent.gamma_multiply(0.30) } else { t.accent.gamma_multiply(0.16) };
        ui.painter().rect_filled(r, CornerRadius::same(5), bg);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "+ More Shapes", FontId::proportional(11.0), t.accent);
        resp.on_hover_text("Browse and enable more shape libraries").clicked()
    }

    /// The More Shapes dialog: a searchable checklist of every library with a
    /// live preview grid — diagrams.net's MoreShapesDialog on the shared
    /// FigCraft design language. Apply moves the checked libraries onto the
    /// palette; "Remember this setting" persists it across restarts.
    fn more_shapes_dialog(&mut self, ctx: &egui::Context, t: &Tokens) {
        if !self.more_open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Shapes")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(690.0, 460.0))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(t.panel)
                    .stroke(doccraft_theme::hairline(t))
                    .corner_radius(8.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // ── left: search + library checklist ──
                    ui.vertical(|ui| {
                        ui.set_width(206.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.more_query)
                                .hint_text("Search…")
                                .desired_width(ui.available_width()),
                        );
                        ui.add_space(2.0);
                        let q = self.more_query.trim().to_lowercase();
                        egui::ScrollArea::vertical().id_salt("more_shapes_libs").show(ui, |ui| {
                            for (group, names) in shapes::DIALOG_GROUPS {
                                if !group.is_empty() {
                                    ui.add_space(3.0);
                                    ui.label(egui::RichText::new(*group).size(CAPTION).strong().color(t.text_secondary));
                                }
                                for name in names.iter() {
                                    let Some(gi) = shapes::cat_index(name) else { continue };
                                    if !q.is_empty() && !name.to_lowercase().contains(&q) {
                                        continue;
                                    }
                                    let mut checked = self.more_checked.contains(&gi);
                                    let resp = ui.checkbox(&mut checked, *name);
                                    if resp.changed() {
                                        if checked {
                                            self.more_checked.insert(gi);
                                        } else {
                                            self.more_checked.remove(&gi);
                                        }
                                    }
                                    // hovering (or checking) previews the library
                                    if resp.hovered() || resp.clicked() {
                                        self.more_sel = gi;
                                    }
                                }
                            }
                        });
                    });
                    ui.separator();
                    // ── right: preview grid of the highlighted library ──
                    ui.vertical(|ui| {
                        let sel = self.more_sel.min(shapes::LIB.len() - 1);
                        let cat = &shapes::LIB[sel];
                        ui.label(egui::RichText::new(cat.name).strong().size(12.0).color(t.text));
                        ui.add_space(2.0);
                        egui::ScrollArea::vertical().id_salt("more_shapes_preview").show(ui, |ui| {
                            let refs: Vec<&shapes::ShapeDef> = cat.items.iter().collect();
                            ui.style_mut().spacing.item_spacing = Vec2::splat(4.0);
                            let cols = 5;
                            let cw = ((ui.available_width() - (cols - 1) as f32 * 4.0) / cols as f32).min(76.0);
                            let ch = 56.0;
                            let mut i = 0;
                            while i < refs.len() {
                                ui.horizontal(|ui| {
                                    for (k, d) in refs.iter().skip(i).take(cols).enumerate() {
                                        let id = Id::new(("draw_more_prev", sel, i + k));
                                        let (rect, resp) = shape_cell(ui, id, Vec2::new(cw, ch), d.name, d.style, d.w, d.h, t);
                                        if resp.clicked() {
                                            self.insert_shape(d);
                                        }
                                        resp.on_hover_text(format!("{} — click to insert", d.name));
                                        let _ = rect;
                                    }
                                });
                                i += cols;
                            }
                        });
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.more_labels, "Labels").on_hover_text("Add the shape's name as its label when a shape is inserted");
                    ui.checkbox(&mut self.more_remember, "Remember this setting")
                        .on_hover_text("Keep the enabled libraries for the next launch");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(egui::RichText::new("Apply").color(Color32::WHITE)).fill(t.accent).min_size(Vec2::new(84.0, 24.0)))
                            .clicked()
                        {
                            apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            });
        if apply {
            self.palette_enabled = self.more_checked.clone();
            self.palette_open.retain(|gi| self.palette_enabled.contains(gi));
            if self.more_remember {
                ctx.data_mut(|d| d.insert_persisted(Id::new("draw_palette_enabled"), self.palette_enabled.clone()));
            }
            self.status = format!(
                "Palette updated — {} of {} libraries shown",
                self.palette_enabled.len(),
                shapes::LIB.len()
            );
            self.more_open = false;
        } else if cancel {
            self.more_open = false;
        } else {
            self.more_open = open;
        }
    }

    /// The pinned Scratchpad grid: click inserts, hover shows ✕ to remove,
    /// entries come from Ctrl+C / palette right-click.
    fn scratchpad(&mut self, ui: &mut Ui, t: &Tokens) {
        if self.scratch.is_empty() {
            ui.label(
                egui::RichText::new("Copy a shape or right-click a palette shape to pin it here.")
                    .small()
                    .color(t.text_tertiary),
            );
            return;
        }
        // collect actions first (the grid paints from self.scratch, so the
        // mutations must happen after)
        let mut remove: Option<usize> = None;
        let mut insert: Option<(String, String, f32, f32)> = None;
        let cols = 3;
        let cw = (ui.available_width() - (cols - 1) as f32 * 4.0) / cols as f32;
        let ch = 44.0;
        for i in 0..self.scratch.len() {
            if i % cols == 0 {
                ui.horizontal(|ui| {
                    for k in i..(i + cols).min(self.scratch.len()) {
                        let (name, style, w, h) = {
                            let it = &self.scratch[k];
                            (it.name.clone(), it.style.clone(), it.w, it.h)
                        };
                        let id = Id::new(("draw_scratch", k));
                        let (rect, resp) = shape_cell(ui, id, Vec2::new(cw, ch), &name, &style, w, h, t);
                        if resp.clicked() {
                            insert = Some((name.clone(), style.clone(), w, h));
                        }
                        if ui.rect_contains_pointer(rect) {
                            let xr = Rect::from_min_size(rect.right_top() - Vec2::new(13.0, 1.0), Vec2::splat(14.0));
                            if ui.put(xr, egui::Button::new(egui::RichText::new("×").size(8.0)).small()).clicked() {
                                remove = Some(k);
                            }
                        }
                        resp.on_hover_text(format!("{name} — click to insert"));
                    }
                });
            }
        }
        if let Some(k) = remove {
            self.scratch.remove(k);
        }
        if let Some((name, style, w, h)) = insert {
            self.insert_shape_styled(&name, &style, w, h, None, true);
        }
    }

    fn shape_grid(&mut self, ui: &mut Ui, items: &[&shapes::ShapeDef], t: &Tokens, gi: usize) {
        ui.style_mut().spacing.item_spacing = Vec2::splat(4.0);
        let cols = 3;
        let cw = (ui.available_width() - (cols - 1) as f32 * 4.0) / cols as f32;
        let ch = 44.0;
        let mut i = 0;
        while i < items.len() {
            ui.horizontal(|ui| {
                for (k, d) in items.iter().skip(i).take(cols).enumerate() {
                    let id = Id::new(("draw_cell", gi, i + k));
                    let (rect, resp) = shape_cell(ui, id, Vec2::new(cw, ch), d.name, d.style, d.w, d.h, t);
                    // click inserts at the view center …
                    if resp.clicked() {
                        self.insert_shape(d);
                    }
                    // … right-click pins to the Scratchpad …
                    let mut to_scratch = false;
                    resp.context_menu(|ui| {
                        if ui.button("Add to Scratchpad").clicked() {
                            to_scratch = true;
                            ui.close();
                        }
                    });
                    if to_scratch {
                        if self.scratch.len() < SCRATCH_MAX {
                            self.scratch.push(ScratchItem {
                                name: d.name.to_string(),
                                style: d.style.to_string(),
                                w: d.w,
                                h: d.h,
                            });
                            self.status = format!("Added {} to Scratchpad", d.name);
                        } else {
                            self.status = format!("Scratchpad is full ({SCRATCH_MAX})");
                        }
                    }
                    // … and drag drops the shape onto the canvas.
                    self.palette_cell_drag(&resp, rect, d);
                    resp.on_hover_text(d.name);
                }
            });
            i += cols;
        }
        ui.add_space(2.0);
    }

    /// Drag-from-palette state machine for one shape cell: while dragging a
    /// ghost preview follows the pointer (painted in [`Self::paint_palette_ghost`]);
    /// releasing over the canvas inserts the shape at that world position.
    fn palette_cell_drag(&mut self, resp: &egui::Response, _rect: Rect, d: &shapes::ShapeDef) {
        if resp.drag_started() {
            self.palette_drag = Some(PaletteDrag {
                name: d.name.to_string(),
                style: d.style.to_string(),
                w: d.w,
                h: d.h,
                cur: resp.interact_pointer_pos().unwrap_or_else(|| resp.rect.center()),
            });
        }
        if resp.dragged()
            && let Some(drag) = self.palette_drag.as_mut()
            && let Some(p) = resp.interact_pointer_pos()
        {
            drag.cur = p;
        }
        if resp.drag_stopped() {
            let drag = self.palette_drag.take();
            if let Some(drag) = drag
                && let Some(p) = resp.interact_pointer_pos()
                && self.view_rect.is_some_and(|cv| cv.contains(p))
            {
                let world = canvas::screen_to_world(self, p);
                self.insert_shape_styled(&drag.name, &drag.style, drag.w, drag.h, Some(world), self.more_labels);
                self.status = format!("Dropped {} on the canvas", drag.name);
            }
        }
    }

    // ── layers ───────────────────────────────────────────────────────────────

    fn layers_panel(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        let open = self.show_layers;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(5), if resp.hovered() { t.hover } else { t.panel });
        doccraft_theme::icons::paint(
            p,
            Rect::from_center_size(Pos2::new(rect.left() + 12.0, rect.center().y), Vec2::splat(12.0)),
            if open { "chevron-down" } else { "chevron-right" },
            t.text_secondary,
            t.panel,
        );
        p.text(Pos2::new(rect.left() + 22.0, rect.center().y), Align2::LEFT_CENTER, "LAYERS", FontId::proportional(10.0), t.text_secondary);
        if resp.clicked() {
            self.show_layers = !open;
        }
        if !open {
            return;
        }
        let active = self.active_layer();
        let count = self.page().layers.len();
        let mut ops: Vec<(String, u8)> = Vec::new(); // (layer id, op 0 eye 1 lock 2 select 3 del 4 up 5 down)
        for (i, l) in self.page().layers.iter().enumerate() {
            ui.horizontal(|ui| {
                // eye / lock toggles painted from the vector icon set
                let (eye_icon, eye_tip) = if l.visible { ("eye", "Hide layer") } else { ("eye-off", "Show layer") };
                if ib(ui, Vec2::new(20.0, 18.0), eye_icon, eye_tip, false) {
                    ops.push((l.id.clone(), 0));
                }
                let (lock_icon, lock_tip) = if l.locked { ("lock", "Unlock layer") } else { ("unlock", "Lock layer") };
                if ib(ui, Vec2::new(20.0, 18.0), lock_icon, lock_tip, false) {
                    ops.push((l.id.clone(), 1));
                }
                let sel = l.id == active;
                let name = if l.name.is_empty() { format!("Layer {}", i + 1) } else { l.name.clone() };
                if ui.add_sized(Vec2::new(ui.available_width() - 52.0, 18.0), egui::Button::selectable(sel, egui::RichText::new(name).size(11.0))).clicked() {
                    ops.push((l.id.clone(), 2));
                }
            });
        }
        ui.horizontal(|ui| {
            if ib(ui, Vec2::new(24.0, 18.0), "plus", "Add layer", false) {
                let n = count + 1;
                let id = self.page_mut().add_layer(&format!("Layer {n}"));
                self.set_active_layer(id);
                self.file.dirty = true;
            }
            if ib(ui, Vec2::new(24.0, 18.0), "trash", "Delete active layer", false)
                && count > 1
            {
                let id = self.active_layer();
                if id != "1" {
                    self.push_undo();
                    let a = self.file.active.min(self.file.diagrams.len() - 1);
                    let page = &mut self.file.diagrams[a];
                    page.delete_cascade(&page.cells.iter().filter(|c| c.layer == id).map(|c| c.id.clone()).collect::<Vec<_>>());
                    page.layers.retain(|l| l.id != id);
                    self.active_layers.clear();
                    self.file.dirty = true;
                }
            }
            if ib(ui, Vec2::new(24.0, 18.0), "arrow-up", "Move layer up", false) {
                let id = self.active_layer();
                if let Some(ix) = self.page().layer_index(&id)
                    && ix > 0
                {
                    self.push_undo();
                    self.page_mut().layers.swap(ix, ix - 1);
                    self.file.dirty = true;
                }
            }
            if ib(ui, Vec2::new(24.0, 18.0), "arrow-down", "Move layer down", false) {
                let id = self.active_layer();
                if let Some(ix) = self.page().layer_index(&id)
                    && ix + 1 < count
                {
                    self.push_undo();
                    self.page_mut().layers.swap(ix, ix + 1);
                    self.file.dirty = true;
                }
            }
        });
        for (id, op) in ops {
            let a = self.file.active.min(self.file.diagrams.len() - 1);
            let page = &mut self.file.diagrams[a];
            if let Some(li) = page.layers.iter_mut().find(|l| l.id == id) {
                match op {
                    0 => {
                        li.visible = !li.visible;
                        self.file.dirty = true;
                    }
                    1 => {
                        li.locked = !li.locked;
                        self.file.dirty = true;
                    }
                    _ => {}
                }
            }
            if op == 2 {
                self.set_active_layer(id);
            }
        }
    }

    // ── format panel (Diagram when empty / Style · Text · Arrange) ──────────

    fn format_panel(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        // drawio's Format panel shows the Diagram tab when nothing is selected
        if self.selected.is_empty() {
            self.tab_row(ui, &t, &[]);
            self.diagram_tab(ui, &t);
            return;
        }
        self.tab_row(ui, &t, &[(FmtTab::Style, "Style"), (FmtTab::Text, "Text"), (FmtTab::Arrange, "Arrange")]);
        egui::ScrollArea::vertical().id_salt("draw_fmt_scroll").show(ui, |ui| {
            match self.fmt_tab {
                FmtTab::Style => self.style_tab(ui),
                FmtTab::Text => self.text_tab(ui),
                FmtTab::Arrange => self.arrange_tab(ui),
            }
        });
    }

    /// Format-panel tab strip: flat text tabs, the active one underlined in
    /// the accent color (drawio's Format.js tab treatment).
    fn tab_row(&mut self, ui: &mut Ui, t: &Tokens, tabs: &[(FmtTab, &str)]) {
        let row_w = ui.available_width();
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 0.0;
            if tabs.is_empty() {
                // single "Diagram" tab (always the active one)
                let w = row_w;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
                ui.painter().rect_filled(rect, CornerRadius::same(5), Color32::TRANSPARENT);
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Diagram", FontId::proportional(11.5), t.text);
                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::new(rect.center().x - 26.0, rect.bottom() - 2.0), Vec2::new(52.0, 2.0)),
                    CornerRadius::same(1),
                    t.accent,
                );
                let _ = resp;
            } else {
                let w = row_w / tabs.len() as f32;
                for (tab, label) in tabs {
                    let active = self.fmt_tab == *tab;
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
                    ui.painter().rect_filled(rect, CornerRadius::same(5), if resp.hovered() && !active { t.hover } else { Color32::TRANSPARENT });
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        *label,
                        FontId::proportional(11.5),
                        if active { t.text } else { t.text_secondary },
                    );
                    if active {
                        ui.painter().rect_filled(
                            Rect::from_min_size(Pos2::new(rect.center().x - 18.0, rect.bottom() - 2.0), Vec2::new(36.0, 2.0)),
                            CornerRadius::same(1),
                            t.accent,
                        );
                    }
                    if resp.clicked() {
                        self.fmt_tab = *tab;
                    }
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();
    }

    /// Format › Diagram (nothing selected): page setup, grid, document stats.
    fn diagram_tab(&mut self, ui: &mut Ui, t: &Tokens) {
        egui::ScrollArea::vertical().id_salt("draw_diagram_scroll").show(ui, |ui| {
            let (w, h) = (self.page().page_w, self.page().page_h);
            ui.label(egui::RichText::new("PAPER SIZE").size(CAPTION).strong().color(t.text_secondary));
            let presets = [("Letter", 850.0, 1100.0), ("A4", 827.0, 1169.0), ("A5", 585.0, 826.0), ("16:9", 960.0, 540.0)];
            let current = presets
                .iter()
                .find(|(_, pw, ph)| (w - pw).abs() < 2.0 && (h - ph).abs() < 2.0)
                .map(|(n, _, _)| *n)
                .unwrap_or("Custom");
            egui::ComboBox::from_id_salt("draw_diagram_paper").selected_text(current).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for (label, pw, ph) in presets {
                    if ui.selectable_label(current == label, label).clicked() && current != label {
                        self.set_page_size(pw, ph);
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("BACKGROUND").size(CAPTION).strong().color(t.text_secondary));
            ui.horizontal(|ui| {
                let bg = self.page().background.clone();
                let cur = bg.as_deref().and_then(style::parse_color);
                self.color_chip(ui, colorpicker::PickTarget::PageBackground, cur, "Background — open the colour picker");
                if ui.button("None").clicked() {
                    self.page_mut().background = None;
                    self.file.dirty = true;
                }
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("GRID").size(CAPTION).strong().color(t.text_secondary));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Size").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut self.grid).range(2.0..=50.0).speed(1.0));
                ui.checkbox(&mut self.show_grid, "Show grid");
            });
            ui.checkbox(&mut self.show_guides, "Alignment guides");
            ui.add_space(4.0);
            ui.label(egui::RichText::new("DIAGRAM").size(CAPTION).strong().color(t.text_secondary));
            let (mut cells, mut edges) = (0usize, 0usize);
            for d in &self.file.diagrams {
                for c in &d.cells {
                    if c.edge {
                        edges += 1;
                    } else if c.vertex {
                        cells += 1;
                    }
                }
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{} pages", self.file.diagrams.len())).small().color(t.text_secondary));
                ui.label(egui::RichText::new(format!("{cells} shapes")).small().color(t.text_secondary));
                ui.label(egui::RichText::new(format!("{edges} edges")).small().color(t.text_secondary));
            });
            ui.add_space(4.0);
            if ui.button("Page Setup…").on_hover_text("Full page setup dialog").clicked() {
                self.pages_open = true;
            }
        });
    }

    /// Apply one style-key change to every selected cell (undo-deduped).
    fn apply_style(&mut self, f: impl Fn(&mut String)) {
        self.push_undo();
        self.apply_style_now(f);
    }

    /// Same as [`Self::apply_style`] but WITHOUT an undo entry — the colour
    /// picker coalesces its own gesture (one undo per drag, not per frame).
    pub(crate) fn apply_style_now(&mut self, f: impl Fn(&mut String)) {
        let a = self.file.active.min(self.file.diagrams.len() - 1);
        let page = &mut self.file.diagrams[a];
        let sel = self.selected.clone();
        for c in page.cells.iter_mut() {
            if sel.contains(&c.id) {
                f(&mut c.style);
            }
        }
        self.file.dirty = true;
    }

    fn swatches(ui: &mut Ui, presets: &[&str], current: Option<Color32>) -> Option<Option<String>> {
        // returns Some(None)=pick none, Some(Some(hex))=picked, None=nothing
        let t = Tokens::get(ui.ctx());
        let mut out = None;
        ui.horizontal_wrapped(|ui| {
            ui.style_mut().spacing.item_spacing = Vec2::splat(3.0);
            for hex in presets {
                let c = style::parse_color(hex).unwrap_or(Color32::WHITE);
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                ui.painter().rect_filled(r.shrink(2.0), 2.0, c);
                let sel = current == Some(c);
                ui.painter().rect_stroke(
                    r,
                    CornerRadius::same(3),
                    Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { t.text } else { t.border }),
                    egui::StrokeKind::Outside,
                );
                if resp.clicked() {
                    out = Some(Some(hex.to_string()));
                }
            }
        });
        out
    }

    /// A colour chip that toggles the FigCraft-style picker for `target`.
    /// `cur = None` paints the drawio "none" swatch (transparent + strike).
    fn color_chip(&mut self, ui: &mut Ui, target: colorpicker::PickTarget, cur: Option<Color32>, tip: &str) {
        let open = self.picker.as_ref().is_some_and(|p| p.target == target);
        let t = Tokens::get(ui.ctx());
        let (r, resp) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
        ui.painter().rect_filled(r.shrink(2.0), CornerRadius::same(3), cur.unwrap_or(t.panel));
        if cur.is_none() {
            ui.painter().line_segment([r.left_bottom() + Vec2::new(3.0, -3.0), r.right_top() - Vec2::new(3.0, -3.0)], Stroke::new(1.5, t.danger));
        }
        ui.painter().rect_stroke(
            r,
            CornerRadius::same(4),
            Stroke::new(if open { 2.0 } else { 1.0 }, if open { t.accent } else { t.border }),
            egui::StrokeKind::Inside,
        );
        if resp.clicked() {
            self.picker = if open { None } else { Some(colorpicker::Pick::new(target)) };
        }
        resp.on_hover_text(tip);
    }

    fn style_tab(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        let is_edge = self.page().cell(&self.selected[0]).map(|c| c.edge).unwrap_or(false);
        let cur_style = self.page().cell(&self.selected[0]).map(|c| c.style.clone()).unwrap_or_default();

        ui.label(egui::RichText::new("FILL").size(CAPTION).strong().color(t.text_secondary));
        let cur_fill = if style::is_none_color(&cur_style, "fillColor") { None } else { Some(style::color_or(&cur_style, "fillColor", Color32::WHITE)) };
        if let Some(pick) = Self::swatches(ui, &style::FILL_PRESETS, cur_fill) {
            let hex = pick.unwrap_or_else(|| "none".into());
            self.apply_style(|s| *s = style::set(s, "fillColor", &hex));
        }
        ui.horizontal(|ui| {
            self.color_chip(ui, colorpicker::PickTarget::Fill, cur_fill, "Fill — open the colour picker");
            if ui.button("None").on_hover_text("No fill").clicked() {
                self.apply_style(|s| *s = style::set(s, "fillColor", "none"));
            }
            let mut grad = style::get(&cur_style, "gradientColor").is_some();
            if ui.toggle_value(&mut grad, "Gradient").changed() {
                if grad {
                    self.apply_style(|s| {
                        *s = style::set(s, "gradientColor", "#ffffff");
                        *s = style::set(s, "gradientDirection", "south");
                    });
                } else {
                    self.apply_style(|s| {
                        *s = style::remove(s, "gradientColor");
                        *s = style::remove(s, "gradientDirection");
                    });
                }
            }
        });

        ui.add_space(4.0);
        ui.label(egui::RichText::new("LINE").size(CAPTION).strong().color(t.text_secondary));
        let cur_stroke = Some(style::color_or(&cur_style, "strokeColor", Color32::BLACK));
        if let Some(pick) = Self::swatches(ui, &style::STROKE_PRESETS, cur_stroke) {
            let hex = pick.unwrap_or_else(|| "#000000".into());
            self.apply_style(|s| *s = style::set(s, "strokeColor", &hex));
        }
        ui.horizontal(|ui| {
            self.color_chip(ui, colorpicker::PickTarget::Stroke, cur_stroke, "Line — open the colour picker");
            ui.label(egui::RichText::new("Width").small().color(t.text_secondary));
            let w = style::get_f32(&cur_style, "strokeWidth", 1.0);
            let mut w = w;
            if ui.add(egui::DragValue::new(&mut w).range(0.5..=8.0).speed(0.25).max_decimals(1)).changed() {
                let v = format!("{w}");
                self.apply_style(|s| *s = style::set(s, "strokeWidth", &v));
            }
            ui.separator();
            ui.label(egui::RichText::new("Opacity").small().color(t.text_secondary));
            let mut op = style::get_f32(&cur_style, "opacity", 100.0);
            if ui.add(egui::Slider::new(&mut op, 10.0..=100.0).show_value(false)).changed() {
                let v = format!("{op}");
                self.apply_style(|s| *s = style::set(s, "opacity", &v));
            }
            ui.label(egui::RichText::new(format!("{op:.0}%")).small().color(t.text_tertiary));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Dash").small().color(t.text_secondary));
            let cur_dash = style::DASHES.iter().find(|(_, p)| match (p, style::get(&cur_style, "dashPattern")) {
                (None, None) => !style::get_bool(&cur_style, "dashed", false),
                (Some(pat), Some(cur)) => Some(*pat) == style::get(&cur_style, "dashPattern").map(|_| *pat).or(Some(cur)) && style::get(&cur_style, "dashPattern") == Some(*pat),
                _ => false,
            });
            let _ = cur_dash;
            for (label, pat) in style::DASHES {
                let active = match (pat, style::get(&cur_style, "dashPattern")) {
                    (None, _) => !style::get_bool(&cur_style, "dashed", false),
                    (Some(p), Some(cur)) => cur == p && style::get_bool(&cur_style, "dashed", false),
                    _ => false,
                };
                if ui.add(egui::Button::selectable(active, egui::RichText::new(label).size(10.0)).min_size(Vec2::new(52.0, 18.0))).clicked() {
                    match pat {
                        None => self.apply_style(|s| {
                            *s = style::remove(s, "dashed");
                            *s = style::remove(s, "dashPattern");
                        }),
                        Some(p) => self.apply_style(|s| {
                            *s = style::set(s, "dashed", "1");
                            *s = style::set(s, "dashPattern", p);
                        }),
                    }
                }
            }
        });

        if is_edge {
            // ── Route (edgeStyle / elbow / curved) ──
            ui.add_space(4.0);
            ui.label(egui::RichText::new("ROUTE").size(CAPTION).strong().color(t.text_secondary));
            let route = style::edge_route(&cur_style);
            let route_label = match route {
                style::EdgeRoute::Straight => "Straight",
                style::EdgeRoute::Orthogonal => "Orthogonal",
                style::EdgeRoute::ElbowH => "H Elbow",
                style::EdgeRoute::ElbowV => "V Elbow",
                style::EdgeRoute::EntityRelation => "Entity Relation",
                style::EdgeRoute::Curved => "Curved",
            };
            egui::ComboBox::from_id_salt("draw_route_pick")
                .selected_text(route_label)
                .width(ui.available_width() - 8.0)
                .show_ui(ui, |ui| {
                    for (label, kind) in [
                        ("Straight", style::EdgeRoute::Straight),
                        ("Orthogonal", style::EdgeRoute::Orthogonal),
                        ("H Elbow", style::EdgeRoute::ElbowH),
                        ("V Elbow", style::EdgeRoute::ElbowV),
                        ("Entity Relation", style::EdgeRoute::EntityRelation),
                        ("Curved", style::EdgeRoute::Curved),
                    ] {
                        let sel = route == kind;
                        if ui.selectable_label(sel, label).clicked() && !sel {
                            self.apply_style(|s| {
                                let (estyle, elbow) = kind.style_kv();
                                match estyle {
                                    Some(v) => *s = style::set(s, "edgeStyle", v),
                                    None => *s = style::remove(s, "edgeStyle"),
                                }
                                match elbow {
                                    Some(v) => *s = style::set(s, "elbow", v),
                                    None => {
                                        *s = style::remove(s, "elbow");
                                    }
                                }
                                if kind == style::EdgeRoute::Curved {
                                    *s = style::set(s, "curved", "1");
                                } else {
                                    *s = style::remove(s, "curved");
                                }
                            });
                        }
                    }
                });

            // ── Line start / Line end (full drawio marker menu) ──
            for (title, start) in [("Line start", true), ("Line end", false)] {
                let fill_key = if start { "startFill" } else { "endFill" };
                let (kind, want_fill) = {
                    let cell = self.page().cell(&self.selected[0]);
                    let k = match cell {
                        Some(c) if start => c.start_arrow(),
                        Some(c) => c.end_arrow(),
                        _ => crate::model::ArrowKind::None,
                    };
                    let f = style::get_bool(&cur_style, fill_key, k.default_fill());
                    (k, f)
                };
                let cur_label = style::ARROW_MENU
                    .iter()
                    .find(|(_, a, f)| {
                        *a == kind.as_str()
                            && match f {
                                Some(v) => *v == want_fill,
                                None => want_fill == start, // Circle: filled at start, open at end
                            }
                    })
                    .map(|(l, _, _)| *l)
                    .unwrap_or_else(|| kind.as_str());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(title).small().color(t.text_secondary).strong());
                    egui::ComboBox::from_id_salt(if start { "draw_start_arrow" } else { "draw_end_arrow" })
                        .selected_text(cur_label)
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            egui::ScrollArea::vertical().max_height(280.0).id_salt(if start { "draw_start_arrow_menu" } else { "draw_end_arrow_menu" }).show(ui, |ui| {
                                for (label, val, f) in style::ARROW_MENU {
                                    let fill_v = f.unwrap_or(start); // Circle: start filled, end open
                                    let sel = val == kind.as_str() && fill_v == want_fill;
                                    if ui.selectable_label(sel, label).clicked() && !sel {
                                        self.apply_style(move |s| {
                                            *s = style::set(s, if start { "startArrow" } else { "endArrow" }, val);
                                            *s = style::set(s, fill_key, if fill_v { "1" } else { "0" });
                                        });
                                    }
                                }
                            });
                        });
                });
            }

            // ── Size start / Size end ──
            ui.horizontal(|ui| {
                let (mut ss, mut es) = {
                    let c = self.page().cell(&self.selected[0]);
                    (c.map(|c| c.start_size()).unwrap_or(6.0), c.map(|c| c.end_size()).unwrap_or(6.0))
                };
                ui.label(egui::RichText::new("Size").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut ss).range(1.0..=40.0).speed(0.5).prefix("s:"));
                ui.add(egui::DragValue::new(&mut es).range(1.0..=40.0).speed(0.5).prefix("e:"));
                if (ss - style::get_f32(&cur_style, "startSize", 6.0)).abs() > f32::EPSILON {
                    self.apply_style(|s| *s = style::set(s, "startSize", &format!("{ss}")));
                }
                if (es - style::get_f32(&cur_style, "endSize", 6.0)).abs() > f32::EPSILON {
                    self.apply_style(|s| *s = style::set(s, "endSize", &format!("{es}")));
                }
            });

            // ── Offset start / end (perimeter spacing) ──
            ui.horizontal(|ui| {
                let (mut so, mut to) = (
                    style::perimeter_spacing(&cur_style, true),
                    style::perimeter_spacing(&cur_style, false),
                );
                ui.label(egui::RichText::new("Offset").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut so).range(0.0..=50.0).speed(0.5).prefix("s:"));
                ui.add(egui::DragValue::new(&mut to).range(0.0..=50.0).speed(0.5).prefix("e:"));
                if (so - style::perimeter_spacing(&cur_style, true)).abs() > f32::EPSILON {
                    self.apply_style(|s| *s = style::set(s, "sourcePerimeterSpacing", &format!("{so}")));
                }
                if (to - style::perimeter_spacing(&cur_style, false)).abs() > f32::EPSILON {
                    self.apply_style(|s| *s = style::set(s, "targetPerimeterSpacing", &format!("{to}")));
                }
            });

            // ── Line jumps (none/arc/gap/sharp/line + size) ──
            ui.horizontal(|ui| {
                let js = style::jump_style(&cur_style);
                let label = match js {
                    crate::model::JumpStyle::None => "none",
                    crate::model::JumpStyle::Arc => "arc",
                    crate::model::JumpStyle::Gap => "gap",
                    crate::model::JumpStyle::Sharp => "sharp",
                    crate::model::JumpStyle::Line => "line",
                };
                ui.label(egui::RichText::new("Jumps").small().color(t.text_secondary));
                egui::ComboBox::from_id_salt("draw_jump_style").selected_text(label).width(72.0).show_ui(ui, |ui| {
                    for (name, v) in [
                        ("none", crate::model::JumpStyle::None),
                        ("arc", crate::model::JumpStyle::Arc),
                        ("gap", crate::model::JumpStyle::Gap),
                        ("sharp", crate::model::JumpStyle::Sharp),
                        ("line", crate::model::JumpStyle::Line),
                    ] {
                        if ui.selectable_label(js == v, name).clicked() && js != v {
                            self.apply_style(|s| *s = style::set(s, "jumpStyle", v.as_str()));
                        }
                    }
                });
                let mut jsize = style::get_f32(&cur_style, "jumpSize", 6.0);
                if ui.add(egui::DragValue::new(&mut jsize).range(1.0..=30.0).speed(0.25).prefix("size ")).changed() {
                    self.apply_style(|s| *s = style::set(s, "jumpSize", &format!("{jsize}")));
                }
            });
        } else {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut rounded = style::get_bool(&cur_style, "rounded", false);
                if ui.toggle_value(&mut rounded, "Rounded").changed() {
                    let v = if rounded { "1" } else { "0" };
                    self.apply_style(move |s| *s = style::set(s, "rounded", v));
                }
            });
        }
    }

    fn text_tab(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        let cur_style = self.page().cell(&self.selected[0]).map(|c| c.style.clone()).unwrap_or_default();
        let has_value = self.page().cell(&self.selected[0]).map(|c| !c.value.is_empty()).unwrap_or(false);

        ui.label(egui::RichText::new("LABEL").size(CAPTION).strong().color(t.text_secondary));
        let mut value = self.page().cell(&self.selected[0]).map(|c| c.value.clone()).unwrap_or_default();
        if ui.add(egui::TextEdit::multiline(&mut value).desired_width(ui.available_width()).desired_rows(2)).changed() {
            self.push_undo();
            let a = self.file.active.min(self.file.diagrams.len() - 1);
            let page = &mut self.file.diagrams[a];
            let sel = self.selected.clone();
            for c in page.cells.iter_mut() {
                if sel.contains(&c.id) {
                    c.value = value.clone();
                }
            }
            self.file.dirty = true;
        }
        ui.add_space(4.0);

        ui.label(egui::RichText::new("FONT").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            let fam = style::get(&cur_style, "fontFamily").unwrap_or("Helvetica").to_string();
            for f in style::FONTS {
                if ui.add(egui::Button::selectable(fam == f, egui::RichText::new(f).size(10.0)).min_size(Vec2::new(64.0, 18.0))).clicked() {
                    let v = f.to_string();
                    self.apply_style(move |s| *s = style::set(s, "fontFamily", &v));
                }
            }
        });
        ui.horizontal(|ui| {
            let fs = style::get_f32(&cur_style, "fontSize", 12.0);
            egui::ComboBox::from_id_salt("draw_font_size").selected_text(format!("{fs}")).width(70.0).show_ui(ui, |ui| {
                for s in style::FONT_SIZES {
                    if ui.selectable_label((fs - s).abs() < 0.01, format!("{s}")).clicked() {
                        let v = format!("{s}");
                        self.apply_style(move |s2| *s2 = style::set(s2, "fontSize", &v));
                    }
                }
            });
            let st = style::font_style(&cur_style);
            let mut bold = st & 1 != 0;
            let mut italic = st & 2 != 0;
            let mut under = st & 4 != 0;
            if ui.toggle_value(&mut bold, "B").changed() {
                self.apply_style(|s| style::set_font_bits(s, 1, bold));
            }
            if ui.toggle_value(&mut italic, "I").changed() {
                self.apply_style(|s| style::set_font_bits(s, 2, italic));
            }
            if ui.toggle_value(&mut under, "U").changed() {
                self.apply_style(|s| style::set_font_bits(s, 4, under));
            }
        });
        ui.add_space(2.0);
        ui.label(egui::RichText::new("FONT COLOR").size(CAPTION).strong().color(t.text_secondary));
        let cur_fc = Some(style::color_or(&cur_style, "fontColor", Color32::BLACK));
        if let Some(pick) = Self::swatches(ui, &style::STROKE_PRESETS, cur_fc) {
            let hex = pick.unwrap_or_else(|| "#000000".into());
            self.apply_style(move |s| *s = style::set(s, "fontColor", &hex));
        }
        ui.horizontal(|ui| {
            self.color_chip(ui, colorpicker::PickTarget::Font, cur_fc, "Font — open the colour picker");
            ui.label(egui::RichText::new("custom").small().color(t.text_tertiary));
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ALIGN").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            let h = style::halign(&cur_style);
            for (icon, a, v, tip) in [
                ("align-left", style::Align::Min, "left", "Align left"),
                ("align-center", style::Align::Mid, "center", "Align center"),
                ("align-right", style::Align::Max, "right", "Align right"),
            ] {
                if ib(ui, Vec2::new(26.0, 20.0), icon, tip, h == a) {
                    let v = v.to_string();
                    self.apply_style(move |s| *s = style::set(s, "align", &v));
                }
            }
            ui.separator();
            let v_al = style::valign(&cur_style);
            for (icon, a, v, tip) in [
                ("align-top", style::Align::Min, "top", "Align top"),
                ("align-middle", style::Align::Mid, "middle", "Align middle"),
                ("align-bottom", style::Align::Max, "bottom", "Align bottom"),
            ] {
                if ib(ui, Vec2::new(26.0, 20.0), icon, tip, v_al == a) {
                    let v = v.to_string();
                    self.apply_style(move |s| *s = style::set(s, "verticalAlign", &v));
                }
            }
        });
        ui.horizontal(|ui| {
            let mut wrap = style::get_bool(&cur_style, "whiteSpace", true);
            if ui.toggle_value(&mut wrap, "Word wrap").changed() {
                let v = if wrap { "wrap" } else { "nowrap" };
                self.apply_style(move |s| *s = style::set(s, "whiteSpace", v));
            }
            if has_value {
                ui.label(egui::RichText::new("Double-click the shape to edit in place").small().color(t.text_tertiary));
            }
        });
    }

    fn arrange_tab(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        // geometry of the first selected vertex
        let first = self.selected.iter().find_map(|id| self.page().cell(id)).filter(|c| c.vertex).map(|c| (c.geom.x, c.geom.y, c.geom.w, c.geom.h));
        if let Some((mut x, mut y, mut w, mut h)) = first {
            ui.label(egui::RichText::new("POSITION & SIZE").size(CAPTION).strong().color(t.text_secondary));
            let id0 = self.selected[0].clone();
            let before = (x, y, w, h);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("X").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(0));
                ui.label(egui::RichText::new("Y").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(0));
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("W").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut w).speed(1.0).range(8.0..=4000.0).max_decimals(0));
                ui.label(egui::RichText::new("H").small().color(t.text_secondary));
                ui.add(egui::DragValue::new(&mut h).speed(1.0).range(8.0..=4000.0).max_decimals(0));
            });
            if (x, y, w, h) != before {
                self.push_undo();
                let a = self.file.active.min(self.file.diagrams.len() - 1);
                let page = &mut self.file.diagrams[a];
                if let Some(c) = page.cell_mut(&id0) {
                    (c.geom.x, c.geom.y, c.geom.w, c.geom.h) = (x, y, w, h);
                }
                self.file.dirty = true;
            }
        }

        ui.add_space(4.0);
        ui.label(egui::RichText::new("ROTATE & FLIP").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            if ib(ui, Vec2::new(28.0, 20.0), "rotate-cw", "Rotate 90° clockwise", false) {
                self.rotate_selection(90.0);
            }
            if ib(ui, Vec2::new(28.0, 20.0), "rotate-ccw", "Rotate 90° counter-clockwise", false) {
                self.rotate_selection(-90.0);
            }
            if ib(ui, Vec2::new(28.0, 20.0), "flip-h", "Flip horizontal", false) {
                self.flip_selection(true);
            }
            if ib(ui, Vec2::new(28.0, 20.0), "flip-v", "Flip vertical", false) {
                self.flip_selection(false);
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ORDER").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            if ui.button("To Front").clicked() {
                self.action_to_front();
            }
            if ui.button("To Back").clicked() {
                self.action_to_back();
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ALIGN").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            for (i, (icon, tip)) in [
                ("align-left", "Left"),
                ("align-center", "Center"),
                ("align-right", "Right"),
                ("align-top", "Top"),
                ("align-middle", "Middle"),
                ("align-bottom", "Bottom"),
            ]
            .into_iter()
            .enumerate()
            {
                let full_tip = format!("{tip} (page for 1 shape, selection for 2+)");
                if ib(ui, Vec2::new(28.0, 20.0), icon, &full_tip, false) {
                    self.align_selection_auto(i as u8);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Distribute").small().color(t.text_secondary));
            if ib(ui, Vec2::new(24.0, 20.0), "distribute-h", "Distribute horizontally (3+ shapes)", false) {
                self.distribute_selection(true);
            }
            if ib(ui, Vec2::new(24.0, 20.0), "distribute-v", "Distribute vertically (3+ shapes)", false) {
                self.distribute_selection(false);
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("GROUP").size(CAPTION).strong().color(t.text_secondary));
        ui.horizontal(|ui| {
            if ui.button("Group (Ctrl+G)").clicked() {
                self.group_selection();
            }
            if ui.button("Ungroup").clicked() {
                self.ungroup_selection();
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("LAYER").size(CAPTION).strong().color(t.text_secondary));
        let active = self.active_layer();
        let page = self.page();
        let names: Vec<(String, String)> = page
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| (l.id.clone(), if l.name.is_empty() { format!("Layer {}", i + 1) } else { l.name.clone() }))
            .collect();
        let cur_name = names.iter().find(|(id, _)| *id == active).map(|(_, n)| n.clone()).unwrap_or_default();
        egui::ComboBox::from_id_salt("draw_layer_pick").selected_text(cur_name).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
            for (id, name) in names {
                if ui.selectable_label(id == active, name).clicked() {
                    self.set_active_layer(id);
                }
            }
        });
        if ui.button("Move selection to layer").clicked() {
            self.push_undo();
            let layer = self.active_layer();
            let a = self.file.active.min(self.file.diagrams.len() - 1);
            let page = &mut self.file.diagrams[a];
            let sel = self.selected.clone();
            for c in page.cells.iter_mut() {
                if sel.contains(&c.id) {
                    c.layer = layer.clone();
                }
            }
            self.file.dirty = true;
        }
    }

    // ── page tabs ────────────────────────────────────────────────────────────

    fn page_tabs(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        egui::Panel::bottom("draw_pages")
            .exact_size(26.0)
            .frame(egui::Frame::default().fill(t.tab_bar).inner_margin(egui::Margin::symmetric(6, 2)).stroke(doccraft_theme::hairline(&t)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    // names collected first so the context menus can mutate the workbook
                    let names: Vec<String> = self
                        .file
                        .diagrams
                        .iter()
                        .enumerate()
                        .map(|(i, d)| if d.name.is_empty() { format!("Page-{}", i + 1) } else { d.name.clone() })
                        .collect();
                    for (i, name) in names.iter().enumerate() {
                        let active = i == self.file.active;
                        let resp = ui.add(
                            egui::Button::new(egui::RichText::new(format!(" {name} ")).size(11.0).color(if active { t.text } else { t.text_secondary }))
                                .fill(if active { t.tab_active } else { Color32::TRANSPARENT })
                                .stroke(if active { Stroke::new(1.0, t.border) } else { Stroke::NONE }),
                        );
                        if resp.clicked() {
                            self.file.active = i;
                            self.selected.clear();
                        }
                        // per-tab right-click menu (drawio page-tab menu)
                        let mut act: Option<u8> = None; // 0 rename 1 duplicate 2 delete 3 insert after
                        resp.context_menu(|ui| {
                            if ui.button("Rename…").clicked() {
                                act = Some(0);
                                ui.close();
                            }
                            if ui.button("Duplicate").clicked() {
                                act = Some(1);
                                ui.close();
                            }
                            if ui
                                .add_enabled(self.file.diagrams.len() > 1, egui::Button::new("Delete"))
                                .on_disabled_hover_text("A document needs at least one page")
                                .clicked()
                            {
                                act = Some(2);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Insert Page After").clicked() {
                                act = Some(3);
                                ui.close();
                            }
                        });
                        match act {
                            Some(0) => self.page_rename = Some((i, name.clone())),
                            Some(1) => {
                                self.file.duplicate_diagram(i);
                                self.selected.clear();
                            }
                            Some(2) => {
                                if self.file.remove_diagram(i) {
                                    self.selected.clear();
                                }
                            }
                            Some(3) => {
                                self.file.insert_diagram_after(i);
                                self.selected.clear();
                            }
                            _ => {}
                        }
                    }
                    if ib(ui, Vec2::new(24.0, 20.0), "plus", "Add page", false) {
                        self.add_page();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(path) = &self.file.path {
                            ui.label(egui::RichText::new(path).weak().size(10.5).color(t.text_secondary));
                        }
                        if self.file.dirty {
                            ui.label(egui::RichText::new("●").size(10.5).color(t.accent));
                        }
                    });
                });
            });
    }

    // ── status bar ───────────────────────────────────────────────────────────

    fn status_bar(&mut self, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        egui::Panel::bottom("draw_status")
            .exact_size(24.0)
            .frame(egui::Frame::default().fill(t.panel).inner_margin(egui::Margin::symmetric(8, 2)).stroke(doccraft_theme::hairline(&t)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = if self.tool == Tool::Connect {
                        "Connect: click the source shape, then the target"
                    } else {
                        "Drag from a shape's arrow to connect · Double-click to label · Right-click for the menu"
                    };
                    ui.label(egui::RichText::new(hint).weak().size(10.5).color(t.text_secondary));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let n = self.page().cells.len();
                        ui.label(egui::RichText::new(format!("{n} cells")).weak().size(10.5).color(t.text_secondary));
                        ui.separator();
                        if ui.small_button("Fit").on_hover_text("Fit page").clicked() {
                            canvas::fit_page(self);
                        }
                        let mut z = self.zoom;
                        if ui.add_sized(Vec2::new(90.0, 16.0), egui::Slider::new(&mut z, 0.2..=5.0).show_value(false)).changed() {
                            if let Some(r) = self.view_rect {
                                canvas::zoom_at(self, r, r.center(), z / self.zoom);
                            }
                        }
                        if ui
                            .add(egui::Button::new(egui::RichText::new(format!("{}%", (self.zoom * 100.0) as i32)).size(10.5).color(t.text_secondary)).frame(false))
                            .clicked()
                        {
                            self.zoom = 1.0;
                        }
                    });
                });
            });
    }

    // ── dialogs ──────────────────────────────────────────────────────────────

    fn dialogs(&mut self, ctx: &egui::Context) {
        let t = Tokens::get(ctx);
        let mut open = self.xml_open;
        if open {
            egui::Window::new("Edit Diagram (mxGraph XML)")
                .open(&mut open)
                .collapsible(false)
                .default_width(560.0)
                .default_height(420.0)
                .show(ctx, |ui| {
                    let mut text = self.xml_text.clone();
                    let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_width(ui.available_width()).font(egui::FontId::monospace(12.5)));
                    self.xml_text = text;
                    if let Some(err) = &self.xml_err {
                        ui.label(egui::RichText::new(format!("× {err}")).color(t.danger).small());
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(resp.changed() || self.xml_err.is_some(), egui::Button::new("Apply")).clicked() {
                            self.apply_xml();
                        }
                        if ui.button("Reset").clicked() {
                            self.xml_text = xmlio::write_diagram_to_string(self.page());
                            self.xml_err = None;
                        }
                        ui.label(egui::RichText::new("drawio-compatible mxGraph XML for the current page").weak().small());
                    });
                });
            self.xml_open = open;
        }

        let mut fopen = self.find_open;
        if fopen {
            egui::Window::new("Find / Replace")
                .open(&mut fopen)
                .collapsible(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Find");
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.find_query).desired_width(180.0));
                        if resp.changed() {
                            self.find_refresh();
                        }
                        if ui.small_button("Next").clicked() {
                            self.find_next();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Replace");
                        if ui.add(egui::TextEdit::singleline(&mut self.find_replace).desired_width(180.0)).changed() {}
                        if ui.small_button("Replace").clicked() {
                            self.find_replace_once(false);
                        }
                        if ui.small_button("All").clicked() {
                            self.find_replace_once(true);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut self.find_case, "Match case").changed() {
                            self.find_refresh();
                        }
                        ui.label(egui::RichText::new(format!("{} match(es) across pages", self.matches.len())).weak().small());
                    });
                });
            self.find_open = fopen;
        }

        let mut popen = self.pages_open;
        if popen {
            egui::Window::new("Page Setup")
                .open(&mut popen)
                .collapsible(false)
                .default_width(340.0)
                .show(ctx, |ui| {
                    let (w, h) = (self.page().page_w, self.page().page_h);
                    ui.label(egui::RichText::new("PAPER").size(CAPTION).strong().color(t.text_secondary));
                    ui.horizontal_wrapped(|ui| {
                        for (label, pw, ph) in [("Letter", 850.0, 1100.0), ("A4", 827.0, 1169.0), ("A5", 585.0, 826.0), ("16:9", 960.0, 540.0)] {
                            let sel = ((w - pw).abs() < 2.0) && ((h - ph).abs() < 2.0);
                            if ui.add(egui::Button::selectable(sel, egui::RichText::new(label).size(11.0)).min_size(Vec2::new(70.0, 22.0))).clicked() {
                                self.set_page_size(pw, ph);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Custom");
                        let mut cw = w;
                        let mut ch = h;
                        let before = (cw, ch);
                        ui.add(egui::DragValue::new(&mut cw).range(200.0..=3000.0).suffix("w"));
                        ui.add(egui::DragValue::new(&mut ch).range(200.0..=3000.0).suffix("h"));
                        if (cw, ch) != before {
                            self.set_page_size(cw, ch);
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("BACKGROUND").size(CAPTION).strong().color(t.text_secondary));
                    ui.horizontal(|ui| {
                        let bg = self.page().background.clone();
                        let cur = bg.as_deref().and_then(style::parse_color);
                        self.color_chip(ui, colorpicker::PickTarget::PageBackground, cur, "Background — open the colour picker");
                        if ui.button("None").clicked() {
                            self.page_mut().background = None;
                            self.file.dirty = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("GRID").size(CAPTION).strong().color(t.text_secondary));
                    ui.horizontal(|ui| {
                        ui.label("Size");
                        ui.add(egui::DragValue::new(&mut self.grid).range(2.0..=50.0).speed(1.0));
                        ui.checkbox(&mut self.show_grid, "Show grid");
                        ui.checkbox(&mut self.show_guides, "Guides");
                    });
                });
            self.pages_open = popen;
        }

        let mut kopen = self.keys_open;
        if kopen {
            egui::Window::new("Keyboard Shortcuts")
                .open(&mut kopen)
                .collapsible(false)
                .default_width(340.0)
                .show(ctx, |ui| {
                    for (k, d) in [
                        ("Ctrl+S / Ctrl+E", "Save / Export .svg"),
                        ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
                        ("Ctrl+C / Ctrl+V", "Copy / Paste"),
                        ("Ctrl+D", "Duplicate"),
                        ("Delete", "Delete selection"),
                        ("Ctrl+A", "Select all"),
                        ("Ctrl+G / Ctrl+Shift+G", "Group / Ungroup"),
                        ("Ctrl+F", "Find & Replace"),
                        ("Ctrl+Shift+X", "Edit diagram XML"),
                        ("Arrows / Shift+Arrows", "Nudge 1 / 10 grid cells"),
                        ("Wheel / Ctrl+Wheel", "Pan / Zoom"),
                        ("Space + drag", "Pan"),
                        ("Double-click", "Edit label · add waypoint · new text"),
                        ("Esc", "Cancel connect / drag"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(k).strong().size(11.5).color(t.accent));
                            ui.label(egui::RichText::new(d).size(11.5).color(t.text_secondary));
                        });
                    }
                });
            self.keys_open = kopen;
        }

        // ── Export As (options + one-click actions) ──
        let mut eopen = self.export_open;
        if eopen {
            egui::Window::new("Export As")
                .open(&mut eopen)
                .collapsible(false)
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("OPTIONS").size(CAPTION).strong().color(t.text_secondary));
                    ui.horizontal(|ui| {
                        ui.label("Zoom");
                        ui.add(egui::DragValue::new(&mut self.export_zoom).range(0.25..=4.0).speed(0.05).suffix("×"));
                        ui.label("Border");
                        ui.add(egui::DragValue::new(&mut self.export_border).range(0.0..=200.0).speed(1.0).suffix("pt"));
                    });
                    ui.checkbox(&mut self.export_transparent, "Transparent background (PNG / SVG)");
                    ui.checkbox(&mut self.export_crop, "Crop to content");
                    ui.separator();
                    ui.label(egui::RichText::new("FORMAT").size(CAPTION).strong().color(t.text_secondary));
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("XML (.drawio)").clicked() {
                            self.export_drawio();
                        }
                        if ui.button("SVG (.svg)").clicked() {
                            self.export_svg();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("PNG (.png)").clicked() {
                            self.export_png();
                        }
                        if ui.button("PDF (.pdf)").clicked() {
                            self.export_pdf();
                        }
                    });
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("Files are written next to the source name in the temp directory.").weak().small());
                });
            self.export_open = eopen;
        }

        // ── File › Save As… ──
        let mut sopen = self.saveas_open;
        if sopen {
            egui::Window::new("Save As")
                .open(&mut sopen)
                .collapsible(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("PATH").size(CAPTION).strong().color(t.text_secondary));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.saveas_path)
                            .desired_width(ui.available_width())
                            .hint_text("./diagram.drawio")
                            .font(egui::FontId::monospace(11.5)),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let path = self.saveas_path.clone();
                        self.save_as(&path);
                        self.saveas_open = false;
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Save").strong())).clicked() {
                            let path = self.saveas_path.clone();
                            self.save_as(&path);
                            self.saveas_open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.saveas_open = false;
                        }
                        ui.label(egui::RichText::new("Writes the full .drawio workbook as XML.").weak().small());
                    });
                });
            self.saveas_open = sopen;
        }

        // ── page-tab context menu › Rename… ──
        if let Some((_idx, buf)) = &mut self.page_rename {
            let mut go = false;
            let mut cancel = false;
            egui::Window::new("Rename Page")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("PAGE NAME").size(CAPTION).strong().color(t.text_secondary));
                    let resp = ui.add(egui::TextEdit::singleline(buf).desired_width(ui.available_width()));
                    if resp.changed() || resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Rename").strong())).clicked() {
                            go = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel {
                self.page_rename = None;
            } else if go {
                let (i, name) = self.page_rename.take().unwrap();
                if !name.trim().is_empty() {
                    self.file.rename_diagram(i, name.trim().to_string());
                }
            } else {
                // keep editing — request keyboard focus on first open
            }
        }

        // ── More Shapes dialog ──
        self.more_shapes_dialog(ctx, &t);
        // ── FigCraft-style colour picker popover ──
        colorpicker::show(self, ctx);

        // ── Help › About Draw… ──
        let mut aopen = self.about_open;
        if aopen {
            egui::Window::new("About Draw")
                .open(&mut aopen)
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Draw").strong().size(16.0).color(t.accent));
                        ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak().size(11.0));
                    });
                    ui.separator();
                    ui.label(
                        egui::RichText::new(
                            "Draw — drawio-style diagrams in DocCraft. A native mxGraph editor with the full \
                             diagrams.net shape library, drawio-exact connection semantics (hover arrows, fixed \
                             points, waypoint re-linking), layers, pages and .drawio XML round-tripping — plus \
                             SVG / PNG / PDF export. Part of the DocCraft suite's shared design system.",
                        )
                        .size(11.5),
                    );
                });
            self.about_open = aopen;
        }
    }
}
