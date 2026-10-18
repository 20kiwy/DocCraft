//! The diagrams.net-style editor shell: menu bar, toolbar, shapes palette,
//! layers panel, the Style/Text/Arrange format panel, page tabs, status bar
//! and the dialogs (Edit XML, Find & Replace, Page Setup, Shortcuts).
//!
//! The drawing surface itself lives in [`crate::canvas`]; the model in
//! [`crate::model`]; file I/O in [`crate::xmlio`].

use crate::canvas::{self, CtxMenu};
use crate::model::{Cell, Diagram, MxFile};
use crate::shapes;
use crate::style;
use crate::xmlio;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

// ── dark chrome palette (drawio accent, same family as the other crafts) ────
pub const ACCENT: Color32 = Color32::from_rgb(0xf0, 0x87, 0x2d);
const PANEL: Color32 = Color32::from_rgb(0x23, 0x23, 0x27);
const PANEL2: Color32 = Color32::from_rgb(0x26, 0x26, 0x2c);
const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x22);
const BORDER: Color32 = Color32::from_rgb(0x34, 0x34, 0x3c);
const TEXT: Color32 = Color32::from_rgb(0xd8, 0xd8, 0xde);
const DIM: Color32 = Color32::from_rgb(0x9a, 0x9a, 0xa4);
const FAINT: Color32 = Color32::from_rgb(0x6e, 0x6e, 0x78);
const CAPTION: f32 = 10.0;

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
    Edge { src: String, cur: Pos2 },
    Rubber { start: Pos2, cur: Pos2 },
    Minimap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmtTab {
    Style,
    Text,
    Arrange,
}

pub struct DrawApp {
    pub file: MxFile,
    pub tool: Tool,
    pub pan: Vec2,
    pub zoom: f32,
    pub selected: Vec<String>,
    pub edge_from: Option<String>,
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

    // ── shell state ──
    palette_open: usize,
    palette_query: String,
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
            palette_open: 0,
            palette_query: String::new(),
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
        if self.page().layer_locked(&Cell { layer: self.active_layer(), vertex: true, ..Cell::default() }) {
            self.status = "The active layer is locked".into();
            return;
        }
        self.push_undo();
        let center = self.view_center_world();
        let x = ((center.x - def.w / 2.0) / self.grid).round() * self.grid;
        let y = ((center.y - def.h / 2.0) / self.grid).round() * self.grid;
        let layer = self.active_layer();
        let id = self.edit_page(|page| {
            let id = page.add_vertex(def.name, def.style, x, y, def.w, def.h);
            if let Some(c) = page.cell_mut(&id) {
                c.layer = layer.clone();
            }
            id
        })
        .unwrap_or_default();
        self.selected = vec![id];
        self.tool = Tool::Select;
        self.status = format!("Added {}", def.name);
    }

    fn view_center_world(&self) -> Pos2 {
        if let Some(r) = self.view_rect {
            let vp = canvas::viewport(self, r);
            return Pos2::new((vp.center().x / self.grid).round() * self.grid, (vp.center().y / self.grid).round() * self.grid);
        }
        Pos2::new(80.0, 80.0)
    }

    // ── file ops ─────────────────────────────────────────────────────────────

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

    pub fn export_svg(&mut self) {
        let stem = self
            .file
            .path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
            .and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string()))
            .unwrap_or_else(|| "doccraft-draw".into());
        let svg = xmlio::write_svg(self.page());
        let p = std::env::temp_dir().join(format!("{stem}.svg"));
        match std::fs::write(&p, svg) {
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
        ui.style_mut().visuals = egui::Visuals::dark();
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
            .frame(egui::Frame::default().fill(BG).inner_margin(0))
            .show_inside(ui, |ui| {
                canvas::show(self, ui);
            });

        self.dialogs(&ctx);
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("draw_menu")
            .exact_size(26.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(6, 2)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Draw").strong().size(12.5).color(ACCENT));
                    ui.separator();
                    for name in ["File", "Edit", "View", "Arrange", "Extras", "Help"] {
                        ui.label(egui::RichText::new(name).size(11.5).color(DIM));
                    }
                    if ui.link(egui::RichText::new("Shortcuts").size(11.0)).clicked() {
                        self.keys_open = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let name = self.file.path.clone().unwrap_or_else(|| "Untitled.drawio".into());
                        let dot = if self.file.dirty { " ●" } else { "" };
                        ui.label(egui::RichText::new(format!("{name}{dot}")).weak().size(10.5));
                    });
                });
            });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        egui::Panel::top("draw_toolbar")
            .exact_size(32.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(6, 3)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 4.0;
                let tb = |ui: &mut Ui, label: &str, tip: &str| -> bool {
                    ui.add(egui::Button::new(egui::RichText::new(label).size(13.0)).min_size(Vec2::new(26.0, 24.0)))
                        .on_hover_text(tip)
                        .clicked()
                };
                // file cluster
                if tb(ui, "💾", "Save (Ctrl+S)") {
                    self.save();
                }
                if tb(ui, "🖼", "Export .svg (Ctrl+E)") {
                    self.export_svg();
                }
                ui.separator();
                if tb(ui, "↶", "Undo (Ctrl+Z)") {
                    self.undo();
                }
                if tb(ui, "↷", "Redo (Ctrl+Y)") {
                    self.redo();
                }
                ui.separator();
                if tb(ui, "⧉⧉", "Duplicate (Ctrl+D)") {
                    self.action_duplicate();
                }
                if tb(ui, "🗑", "Delete (Del)") {
                    self.action_delete();
                }
                ui.separator();
                if ui
                    .add(egui::Button::selectable(self.tool == Tool::Connect, egui::RichText::new("➔").size(13.0)).min_size(Vec2::new(26.0, 24.0)))
                    .on_hover_text("Connect tool: click source, then target")
                    .clicked()
                {
                    self.tool = if self.tool == Tool::Connect { Tool::Select } else { Tool::Connect };
                    self.edge_from = None;
                }
                if ui
                    .add(egui::Button::selectable(self.tool == Tool::Select, egui::RichText::new("⬚").size(13.0)).min_size(Vec2::new(26.0, 24.0)))
                    .on_hover_text("Select tool")
                    .clicked()
                {
                    self.tool = Tool::Select;
                }
                ui.separator();
                if tb(ui, "＋", "Zoom in") {
                    if let Some(r) = self.view_rect {
                        canvas::zoom_at(self, r, r.center(), 1.25);
                    }
                }
                if tb(ui, "－", "Zoom out") {
                    if let Some(r) = self.view_rect {
                        canvas::zoom_at(self, r, r.center(), 1.0 / 1.25);
                    }
                }
                if tb(ui, "⤢", "Fit page") {
                    canvas::fit_page(self);
                }
                ui.separator();
                if tb(ui, "🔍", "Find & Replace (Ctrl+F)") {
                    self.find_open = !self.find_open;
                }
                if tb(ui, "⟩⟩", "Edit Diagram XML (Ctrl+Shift+X)") {
                    self.open_xml_editor();
                }
                if tb(ui, "🗎", "Page Setup") {
                    self.pages_open = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).weak().size(11.0));
                });
                });
            });
    }

    fn palette_panel(&mut self, ui: &mut Ui) {
        egui::Panel::left("draw_palette")
            .resizable(true)
            .default_size(216.0)
            .min_size(180.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::same(6)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                self.palette(ui);
                ui.add_space(4.0);
                ui.separator();
                self.layers_panel(ui);
            });
    }

    fn format_panel_dock(&mut self, ui: &mut Ui) {
        egui::Panel::right("draw_format")
            .resizable(true)
            .default_size(248.0)
            .min_size(210.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::same(6)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                self.format_panel(ui);
            });
    }

    // ── palette ──────────────────────────────────────────────────────────────

    fn palette(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("SHAPES").size(CAPTION).strong().color(DIM));
        });
        ui.add_space(2.0);
        ui.add(egui::TextEdit::singleline(&mut self.palette_query).hint_text("Search shapes…").desired_width(ui.available_width()));
        ui.add_space(4.0);

        let q = self.palette_query.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("draw_palette_scroll").show(ui, |ui| {
            if q.is_empty() {
                for (gi, cat) in shapes::LIB.iter().enumerate() {
                    let open = self.palette_open == gi;
                    let header = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click())
                        .1;
                    let (rect, resp) = (header.interact_rect, header.clone());
                    let _ = rect;
                    let paint = ui.painter();
                    paint.rect_filled(header.interact_rect, CornerRadius::same(3), if resp.hovered() { PANEL2 } else { PANEL });
                    paint.text(
                        Pos2::new(header.interact_rect.left() + 6.0, header.interact_rect.center().y),
                        Align2::LEFT_CENTER,
                        if open { "▾ " } else { "▸ " },
                        FontId::proportional(11.0),
                        DIM,
                    );
                    paint.text(
                        Pos2::new(header.interact_rect.left() + 20.0, header.interact_rect.center().y),
                        Align2::LEFT_CENTER,
                        cat.name,
                        FontId::proportional(12.0),
                        TEXT,
                    );
                    if resp.clicked() {
                        self.palette_open = if open { usize::MAX } else { gi };
                    }
                    if open {
                        let refs: Vec<&shapes::ShapeDef> = cat.items.iter().collect();
                        self.shape_grid(ui, &refs);
                    }
                }
            } else {
                // search across all categories
                let mut n = 0;
                for cat in shapes::LIB {
                    let hits: Vec<&shapes::ShapeDef> =
                        cat.items.iter().filter(|d| d.name.to_lowercase().contains(&q)).collect();
                    if hits.is_empty() {
                        continue;
                    }
                    ui.label(egui::RichText::new(cat.name).size(CAPTION).strong().color(FAINT));
                    self.shape_grid(ui, &hits);
                    n += hits.len();
                }
                if n == 0 {
                    ui.label(egui::RichText::new("No shapes match.").small().color(DIM));
                }
            }
        });
    }

    fn shape_grid(&mut self, ui: &mut Ui, items: &[&shapes::ShapeDef]) {
        ui.style_mut().spacing.item_spacing = Vec2::splat(4.0);
        let cols = 3;
        let cw = (ui.available_width() - (cols - 1) as f32 * 4.0) / cols as f32;
        let ch = 44.0;
        let mut i = 0;
        while i < items.len() {
            ui.horizontal(|ui| {
                for d in items.iter().skip(i).take(cols) {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cw, ch), Sense::click());
                    let fill = if resp.hovered() { PANEL2 } else { PANEL };
                    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
                    ui.painter().rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                    // mini preview through the real painter
                    let look = style::vertex_look(d.style, 1.0);
                    let inner = rect.shrink(6.0);
                    let aspect = d.w / d.h.max(1.0);
                    let (pw, ph) = if aspect >= 1.0 {
                        (inner.width(), (inner.width() / aspect).min(inner.height()))
                    } else {
                        ((inner.height() * aspect).min(inner.width()), inner.height())
                    };
                    let pr = Rect::from_center_size(rect.center(), Vec2::new(pw, ph));
                    let xf = shapes::Xf::from_style(d.style);
                    shapes::paint_vertex(ui.painter(), pr, d.style, &look, "", &xf);
                    if resp.clicked() {
                        self.insert_shape(d);
                    }
                    resp.on_hover_text(d.name);
                }
            });
            i += cols;
        }
        ui.add_space(2.0);
    }

    // ── layers ───────────────────────────────────────────────────────────────

    fn layers_panel(&mut self, ui: &mut Ui) {
        let open = self.show_layers;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(3), if resp.hovered() { PANEL2 } else { PANEL });
        p.text(Pos2::new(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER, if open { "▾ " } else { "▸ " }, FontId::proportional(11.0), DIM);
        p.text(Pos2::new(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, "LAYERS", FontId::proportional(10.0), DIM);
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
                let eye = if l.visible { "👁" } else { "–" };
                let lock = if l.locked { "🔒" } else { "·" };
                if ui.add_sized(Vec2::new(20.0, 18.0), egui::Button::new(egui::RichText::new(eye).size(10.0)).frame(false)).clicked() {
                    ops.push((l.id.clone(), 0));
                }
                if ui.add_sized(Vec2::new(20.0, 18.0), egui::Button::new(egui::RichText::new(lock).size(10.0)).frame(false)).clicked() {
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
            if ui.add_sized(Vec2::new(24.0, 18.0), egui::Button::new(egui::RichText::new("＋").size(10.0))).on_hover_text("Add layer").clicked() {
                let n = count + 1;
                let id = self.page_mut().add_layer(&format!("Layer {n}"));
                self.set_active_layer(id);
                self.file.dirty = true;
            }
            if ui.add_sized(Vec2::new(24.0, 18.0), egui::Button::new(egui::RichText::new("🗑").size(10.0))).on_hover_text("Delete active layer").clicked()
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
            if ui.add_sized(Vec2::new(24.0, 18.0), egui::Button::new(egui::RichText::new("↑").size(10.0))).on_hover_text("Move layer up").clicked() {
                let id = self.active_layer();
                if let Some(ix) = self.page().layer_index(&id)
                    && ix > 0
                {
                    self.push_undo();
                    self.page_mut().layers.swap(ix, ix - 1);
                    self.file.dirty = true;
                }
            }
            if ui.add_sized(Vec2::new(24.0, 18.0), egui::Button::new(egui::RichText::new("↓").size(10.0))).on_hover_text("Move layer down").clicked() {
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

    // ── format panel (Style / Text / Arrange) ────────────────────────────────

    fn format_panel(&mut self, ui: &mut Ui) {
        // tabs
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 2.0;
            for (tab, label) in [(FmtTab::Style, "Style"), (FmtTab::Text, "Text"), (FmtTab::Arrange, "Arrange")] {
                if ui.add(egui::Button::selectable(self.fmt_tab == tab, egui::RichText::new(label).size(11.5)).min_size(Vec2::new(ui.available_width() / 3.0 - 2.0, 22.0)))
                    .clicked()
                {
                    self.fmt_tab = tab;
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();

        if self.selected.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Nothing selected").size(12.0).color(DIM));
            ui.label(egui::RichText::new("Click a shape, drag on the canvas, or pick one from the Shapes palette.").small().color(FAINT));
            return;
        }

        egui::ScrollArea::vertical().id_salt("draw_fmt_scroll").show(ui, |ui| {
            match self.fmt_tab {
                FmtTab::Style => self.style_tab(ui),
                FmtTab::Text => self.text_tab(ui),
                FmtTab::Arrange => self.arrange_tab(ui),
            }
        });
    }

    /// Apply one style-key change to every selected cell (undo-deduped).
    fn apply_style(&mut self, f: impl Fn(&mut String)) {
        self.push_undo();
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
                    Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { TEXT } else { BORDER }),
                    egui::StrokeKind::Outside,
                );
                if resp.clicked() {
                    out = Some(Some(hex.to_string()));
                }
            }
        });
        out
    }

    fn style_tab(&mut self, ui: &mut Ui) {
        let is_edge = self.page().cell(&self.selected[0]).map(|c| c.edge).unwrap_or(false);
        let cur_style = self.page().cell(&self.selected[0]).map(|c| c.style.clone()).unwrap_or_default();

        ui.label(egui::RichText::new("FILL").size(CAPTION).strong().color(DIM));
        let cur_fill = if style::is_none_color(&cur_style, "fillColor") { None } else { Some(style::color_or(&cur_style, "fillColor", Color32::WHITE)) };
        if let Some(pick) = Self::swatches(ui, &style::FILL_PRESETS, cur_fill) {
            let hex = pick.unwrap_or_else(|| "none".into());
            self.apply_style(|s| *s = style::set(s, "fillColor", &hex));
        }
        ui.horizontal(|ui| {
            let mut rgb = cur_fill.unwrap_or(Color32::WHITE);
            if ui.color_edit_button_srgb(&mut [rgb.r(), rgb.g(), rgb.b()]).changed() {
                let hex = style::color_hex(Color32::from_rgb(rgb.r(), rgb.g(), rgb.b()));
                self.apply_style(|s| *s = style::set(s, "fillColor", &hex));
            }
            ui.label(egui::RichText::new("custom").small().color(FAINT));
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
        ui.label(egui::RichText::new("LINE").size(CAPTION).strong().color(DIM));
        let cur_stroke = Some(style::color_or(&cur_style, "strokeColor", Color32::BLACK));
        if let Some(pick) = Self::swatches(ui, &style::STROKE_PRESETS, cur_stroke) {
            let hex = pick.unwrap_or_else(|| "#000000".into());
            self.apply_style(|s| *s = style::set(s, "strokeColor", &hex));
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Width").small().color(DIM));
            let w = style::get_f32(&cur_style, "strokeWidth", 1.0);
            let mut w = w;
            if ui.add(egui::DragValue::new(&mut w).range(0.5..=8.0).speed(0.25).max_decimals(1)).changed() {
                let v = format!("{w}");
                self.apply_style(|s| *s = style::set(s, "strokeWidth", &v));
            }
            ui.separator();
            ui.label(egui::RichText::new("Opacity").small().color(DIM));
            let mut op = style::get_f32(&cur_style, "opacity", 100.0);
            if ui.add(egui::Slider::new(&mut op, 10.0..=100.0).show_value(false)).changed() {
                let v = format!("{op}");
                self.apply_style(|s| *s = style::set(s, "opacity", &v));
            }
            ui.label(egui::RichText::new(format!("{op:.0}%")).small().color(FAINT));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Dash").small().color(DIM));
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
            ui.add_space(4.0);
            ui.label(egui::RichText::new("ROUTE").size(CAPTION).strong().color(DIM));
            ui.horizontal(|ui| {
                let route = style::edge_route(&cur_style);
                for (label, kind, estyle) in [
                    ("Orthogonal", style::EdgeRoute::Orthogonal, "orthogonalEdgeStyle"),
                    ("Straight", style::EdgeRoute::Straight, "straightEdgeStyle"),
                    ("Curved", style::EdgeRoute::Curved, "orthogonalEdgeStyle"),
                ] {
                    let active = route == kind;
                    if ui.add(egui::Button::selectable(active, egui::RichText::new(label).size(10.5)).min_size(Vec2::new(74.0, 20.0))).clicked() {
                        self.apply_style(|s| {
                            *s = style::set(s, "edgeStyle", estyle);
                            if kind == style::EdgeRoute::Curved {
                                *s = style::set(s, "curved", "1");
                            } else {
                                *s = style::remove(s, "curved");
                            }
                        });
                    }
                }
            });
            ui.horizontal(|ui| {
                let mut jump = style::line_jump(&cur_style);
                if ui.toggle_value(&mut jump, "Line jumps").changed() {
                    let v = if jump { "arc" } else { "none" };
                    self.apply_style(|s| *s = style::set(s, "jumpStyle", v));
                }
            });
            for (title, key) in [("Start arrow", "startArrow"), ("End arrow", "endArrow")] {
                ui.label(egui::RichText::new(title).small().color(DIM));
                let cur = style::get(&cur_style, key).unwrap_or(if key == "endArrow" { "classic" } else { "none" }).to_string();
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::splat(3.0);
                    for (label, val) in style::ARROWS {
                        if ui.add(egui::Button::selectable(cur == val, egui::RichText::new(label).size(9.5)).min_size(Vec2::new(58.0, 17.0))).clicked() {
                            let k = key.to_string();
                            let v = val.to_string();
                            self.apply_style(move |s| *s = style::set(s, &k, &v));
                        }
                    }
                });
            }
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
        let cur_style = self.page().cell(&self.selected[0]).map(|c| c.style.clone()).unwrap_or_default();
        let has_value = self.page().cell(&self.selected[0]).map(|c| !c.value.is_empty()).unwrap_or(false);

        ui.label(egui::RichText::new("LABEL").size(CAPTION).strong().color(DIM));
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

        ui.label(egui::RichText::new("FONT").size(CAPTION).strong().color(DIM));
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
        ui.label(egui::RichText::new("FONT COLOR").size(CAPTION).strong().color(DIM));
        let cur_fc = Some(style::color_or(&cur_style, "fontColor", Color32::BLACK));
        if let Some(pick) = Self::swatches(ui, &style::STROKE_PRESETS, cur_fc) {
            let hex = pick.unwrap_or_else(|| "#000000".into());
            self.apply_style(move |s| *s = style::set(s, "fontColor", &hex));
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ALIGN").size(CAPTION).strong().color(DIM));
        ui.horizontal(|ui| {
            let h = style::halign(&cur_style);
            for (label, a, v) in [("⯇", style::Align::Min, "left"), ("≡", style::Align::Mid, "center"), ("⯈", style::Align::Max, "right")] {
                if ui.add(egui::Button::selectable(h == a, egui::RichText::new(label).size(11.0)).min_size(Vec2::new(26.0, 20.0))).clicked() {
                    let v = v.to_string();
                    self.apply_style(move |s| *s = style::set(s, "align", &v));
                }
            }
            ui.separator();
            let v_al = style::valign(&cur_style);
            for (label, a, v) in [("⤒", style::Align::Min, "top"), ("↕", style::Align::Mid, "middle"), ("⤓", style::Align::Max, "bottom")] {
                if ui.add(egui::Button::selectable(v_al == a, egui::RichText::new(label).size(11.0)).min_size(Vec2::new(26.0, 20.0))).clicked() {
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
                ui.label(egui::RichText::new("Double-click the shape to edit in place").small().color(FAINT));
            }
        });
    }

    fn arrange_tab(&mut self, ui: &mut Ui) {
        // geometry of the first selected vertex
        let first = self.selected.iter().find_map(|id| self.page().cell(id)).filter(|c| c.vertex).map(|c| (c.geom.x, c.geom.y, c.geom.w, c.geom.h));
        if let Some((mut x, mut y, mut w, mut h)) = first {
            ui.label(egui::RichText::new("POSITION & SIZE").size(CAPTION).strong().color(DIM));
            let id0 = self.selected[0].clone();
            let before = (x, y, w, h);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("X").small().color(DIM));
                ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(0));
                ui.label(egui::RichText::new("Y").small().color(DIM));
                ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(0));
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("W").small().color(DIM));
                ui.add(egui::DragValue::new(&mut w).speed(1.0).range(8.0..=4000.0).max_decimals(0));
                ui.label(egui::RichText::new("H").small().color(DIM));
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
        ui.label(egui::RichText::new("ROTATE & FLIP").size(CAPTION).strong().color(DIM));
        ui.horizontal(|ui| {
            if ui.button("↻ 90°").on_hover_text("Rotate 90° clockwise").clicked() {
                self.rotate_selection(90.0);
            }
            if ui.button("↺ 90°").on_hover_text("Rotate 90° counter-clockwise").clicked() {
                self.rotate_selection(-90.0);
            }
            if ui.button("↔").on_hover_text("Flip horizontal").clicked() {
                self.flip_selection(true);
            }
            if ui.button("↕").on_hover_text("Flip vertical").clicked() {
                self.flip_selection(false);
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ORDER").size(CAPTION).strong().color(DIM));
        ui.horizontal(|ui| {
            if ui.button("⤒ To Front").clicked() {
                self.action_to_front();
            }
            if ui.button("⤓ To Back").clicked() {
                self.action_to_back();
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("ALIGN (PAGE)").size(CAPTION).strong().color(DIM));
        ui.horizontal(|ui| {
            for (i, (label, tip)) in [("⯇", "Left"), ("≡", "Center"), ("⯈", "Right"), ("⤒", "Top"), ("↕", "Middle"), ("⤓", "Bottom")].iter().enumerate() {
                if ui.add_sized(Vec2::new(28.0, 20.0), egui::Button::new(*label)).on_hover_text(*tip).clicked() {
                    self.align_selection(i as u8);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Distribute").small().color(DIM));
            if ui.small_button("⟷").on_hover_text("Distribute horizontally (3+ shapes)").clicked() {
                self.distribute_selection(true);
            }
            if ui.small_button("⇕").on_hover_text("Distribute vertically (3+ shapes)").clicked() {
                self.distribute_selection(false);
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("GROUP").size(CAPTION).strong().color(DIM));
        ui.horizontal(|ui| {
            if ui.button("Group (Ctrl+G)").clicked() {
                self.group_selection();
            }
            if ui.button("Ungroup").clicked() {
                self.ungroup_selection();
            }
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new("LAYER").size(CAPTION).strong().color(DIM));
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
        egui::Panel::bottom("draw_pages")
            .exact_size(26.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(6, 2)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, d) in self.file.diagrams.iter().enumerate() {
                        let name = if d.name.is_empty() { format!("Page-{}", i + 1) } else { d.name.clone() };
                        if ui.add(egui::Button::selectable(i == self.file.active, egui::RichText::new(format!(" {name} ")).size(11.0))).clicked() {
                            self.file.active = i;
                            self.selected.clear();
                        }
                    }
                    if ui.add_sized(Vec2::new(24.0, 20.0), egui::Button::new(egui::RichText::new("＋").size(11.0))).on_hover_text("Add page").clicked() {
                        self.add_page();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(path) = &self.file.path {
                            ui.label(egui::RichText::new(path).weak().size(10.5));
                        }
                        if self.file.dirty {
                            ui.label(egui::RichText::new("●").size(10.5).color(ACCENT));
                        }
                    });
                });
            });
    }

    // ── status bar ───────────────────────────────────────────────────────────

    fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("draw_status")
            .exact_size(24.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(egui::Margin::symmetric(8, 2)).stroke(Stroke::new(1.0, BORDER)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = if self.tool == Tool::Connect {
                        "Connect: click the source shape, then the target"
                    } else {
                        "Drag from a shape's arrow to connect · Double-click to label · Right-click for the menu"
                    };
                    ui.label(egui::RichText::new(hint).weak().size(10.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let n = self.page().cells.len();
                        ui.label(egui::RichText::new(format!("{n} cells")).weak().size(10.5));
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
                            .add(egui::Button::new(egui::RichText::new(format!("{}%", (self.zoom * 100.0) as i32)).size(10.5)).frame(false))
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
                        ui.label(egui::RichText::new(format!("✗ {err}")).color(Color32::from_rgb(0xe0, 0x60, 0x60)).small());
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
                        if ui.small_button("⏎ Next").clicked() {
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
                    ui.label(egui::RichText::new("PAPER").size(CAPTION).strong().color(DIM));
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
                    ui.label(egui::RichText::new("BACKGROUND").size(CAPTION).strong().color(DIM));
                    ui.horizontal(|ui| {
                        let bg = self.page().background.clone();
                        let mut col = bg.as_deref().and_then(style::parse_color).unwrap_or(Color32::WHITE);
                        if ui.color_edit_button_srgb(&mut [col.r(), col.g(), col.b()]).changed() {
                            let hex = style::color_hex(col);
                            self.page_mut().background = Some(hex);
                            self.file.dirty = true;
                        }
                        if ui.button("None").clicked() {
                            self.page_mut().background = None;
                            self.file.dirty = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("GRID").size(CAPTION).strong().color(DIM));
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
                            ui.label(egui::RichText::new(k).strong().size(11.5).color(ACCENT));
                            ui.label(egui::RichText::new(d).size(11.5).color(DIM));
                        });
                    }
                });
            self.keys_open = kopen;
        }
    }
}
