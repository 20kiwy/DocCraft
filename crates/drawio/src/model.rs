//! The mxGraph document model (`mxfile › diagram › mxGraphModel › root`).
//!
//! Everything the editor touches lives here: pages ([`Diagram`]), cells
//! ([`Cell`]), layers ([`Layer`]) and geometry ([`Geom`]). Cells keep their
//! mxGraph `style` string verbatim so import/export round-trips losslessly;
//! [`crate::style`] reads/writes individual keys inside that string.

use serde::{Deserialize, Serialize};

// ── edge vocabulary (mxGraph style keys, typed for the editor) ───────────────

/// mxGraph arrow/marker kinds — the exact set behind the Format panel's
/// "Line start" / "Line end" dropdowns (`startArrow`/`endArrow` keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowKind {
    None,
    Classic,
    ClassicThin,
    Open,
    OpenThin,
    OpenAsync,
    Block,
    BlockThin,
    Async,
    Oval,
    Diamond,
    DiamondThin,
    Box,
    HalfCircle,
    Dash,
    Cross,
    CirclePlus,
    Circle,
    BaseDash,
    ErOne,
    ErMandOne,
    ErMany,
    ErOneToMany,
    ErZeroToOne,
    ErZeroToMany,
    DoubleBlock,
}

impl ArrowKind {
    pub const ALL: [ArrowKind; 26] = [
        ArrowKind::None,
        ArrowKind::Classic,
        ArrowKind::ClassicThin,
        ArrowKind::Open,
        ArrowKind::OpenThin,
        ArrowKind::OpenAsync,
        ArrowKind::Block,
        ArrowKind::BlockThin,
        ArrowKind::Async,
        ArrowKind::Oval,
        ArrowKind::Diamond,
        ArrowKind::DiamondThin,
        ArrowKind::Box,
        ArrowKind::HalfCircle,
        ArrowKind::Dash,
        ArrowKind::Cross,
        ArrowKind::CirclePlus,
        ArrowKind::Circle,
        ArrowKind::BaseDash,
        ArrowKind::ErOne,
        ArrowKind::ErMandOne,
        ArrowKind::ErMany,
        ArrowKind::ErOneToMany,
        ArrowKind::ErZeroToOne,
        ArrowKind::ErZeroToMany,
        ArrowKind::DoubleBlock,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ArrowKind::None => "none",
            ArrowKind::Classic => "classic",
            ArrowKind::ClassicThin => "classicThin",
            ArrowKind::Open => "open",
            ArrowKind::OpenThin => "openThin",
            ArrowKind::OpenAsync => "openAsync",
            ArrowKind::Block => "block",
            ArrowKind::BlockThin => "blockThin",
            ArrowKind::Async => "async",
            ArrowKind::Oval => "oval",
            ArrowKind::Diamond => "diamond",
            ArrowKind::DiamondThin => "diamondThin",
            ArrowKind::Box => "box",
            ArrowKind::HalfCircle => "halfCircle",
            ArrowKind::Dash => "dash",
            ArrowKind::Cross => "cross",
            ArrowKind::CirclePlus => "circlePlus",
            ArrowKind::Circle => "circle",
            ArrowKind::BaseDash => "baseDash",
            ArrowKind::ErOne => "ERone",
            ArrowKind::ErMandOne => "ERmandOne",
            ArrowKind::ErMany => "ERmany",
            ArrowKind::ErOneToMany => "ERoneToMany",
            ArrowKind::ErZeroToOne => "ERzeroToOne",
            ArrowKind::ErZeroToMany => "ERzeroToMany",
            ArrowKind::DoubleBlock => "doubleBlock",
        }
    }

    pub fn from_str(v: &str) -> Option<Self> {
        ArrowKind::ALL.iter().copied().find(|k| k.as_str() == v)
    }

    /// mxGraph default fill for the kind when `xFill` is absent.
    pub fn default_fill(&self) -> bool {
        matches!(
            self,
            ArrowKind::Classic
                | ArrowKind::ClassicThin
                | ArrowKind::Block
                | ArrowKind::BlockThin
                | ArrowKind::Async
                | ArrowKind::Oval
                | ArrowKind::Diamond
                | ArrowKind::DiamondThin
                | ArrowKind::DoubleBlock
        )
    }
}

/// Edge line-jump style (`jumpStyle` key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpStyle {
    None,
    Arc,
    Gap,
    Sharp,
    Line,
}

impl JumpStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            JumpStyle::None => "none",
            JumpStyle::Arc => "arc",
            JumpStyle::Gap => "gap",
            JumpStyle::Sharp => "sharp",
            JumpStyle::Line => "line",
        }
    }
    pub fn from_str(v: &str) -> Option<Self> {
        Some(match v {
            "arc" => JumpStyle::Arc,
            "gap" => JumpStyle::Gap,
            "sharp" => JumpStyle::Sharp,
            "line" => JumpStyle::Line,
            _ => return None,
        })
    }
}

/// Edge routing flavor (`edgeStyle` + `elbow` + `curved` keys).
///
/// Lives here so the model has the full typed vocabulary; [`crate::style`]
/// re-exports it for painters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeRoute {
    Orthogonal,
    Straight,
    ElbowH,
    ElbowV,
    EntityRelation,
    Curved,
}

impl EdgeRoute {
    /// (edgeStyle value, elbow value or `None`) for writing back. `Straight`
    /// has no `edgeStyle` key (mxGraph default routing = straight).
    pub fn style_kv(&self) -> (Option<&'static str>, Option<&'static str>) {
        match self {
            EdgeRoute::Straight => (None, None),
            EdgeRoute::Orthogonal => (Some("orthogonalEdgeStyle"), None),
            EdgeRoute::ElbowH => (Some("elbowEdgeStyle"), Some("vertical")),
            EdgeRoute::ElbowV => (Some("elbowEdgeStyle"), None),
            EdgeRoute::EntityRelation => (Some("entityRelationEdgeStyle"), None),
            EdgeRoute::Curved => (Some("orthogonalEdgeStyle"), None),
        }
    }
}

/// Style of the default new edge (`endArrow=classic;endFill=1`, orthogonal).
pub const DEFAULT_EDGE_STYLE: &str = "edgeStyle=orthogonalEdgeStyle;rounded=0;html=1;endArrow=classic;endFill=1;";

/// Style of the default new vertex.
pub const DEFAULT_VERTEX_STYLE: &str = "rounded=1;whiteSpace=wrap;html=1;";

/// One opened `.drawio` file — a workbook of named diagram pages.
#[derive(Debug, Clone, Default)]
pub struct MxFile {
    pub diagrams: Vec<Diagram>,
    pub active: usize,
    pub path: Option<String>,
    pub dirty: bool,
}

/// One page (drawio shows page tabs along the bottom).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    pub name: String,
    pub cells: Vec<Cell>,
    pub next_id: usize,
    /// Page setup (drawio `mxGraphModel pageWidth/pageHeight`).
    pub page_w: f32,
    pub page_h: f32,
    /// Page background color (`background` attr), `None` = transparent/white.
    pub background: Option<String>,
    /// Drawing layers, in paint order (bottom first).
    pub layers: Vec<Layer>,
}

/// One drawing layer. Cells point at a layer by id via `Cell::layer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
}

/// A vertex (shape) or an edge (connector) on the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub id: String,
    pub value: String,
    pub style: String,
    pub vertex: bool,
    pub edge: bool,
    /// Edge endpoints (drawio keeps ids; empty when floating).
    pub source: Option<String>,
    pub target: Option<String>,
    /// Parent group vertex id (mxGraph `parent` attribute), if grouped.
    pub parent: Option<String>,
    /// Owning layer id (ours; maps to the mxGraph parent for top-level cells).
    pub layer: String,
    pub geom: Geom,
}

/// `mxGeometry` — position/size for vertices, waypoints for edges.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Geom {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Edge waypoints in world coordinates (mxGraph `Array as="points"`).
    pub points: Vec<(f32, f32)>,
    /// Floating edge endpoints (mxPoint `as="sourcePoint"/"targetPoint"`).
    pub source_pt: Option<(f32, f32)>,
    pub target_pt: Option<(f32, f32)>,
    pub relative: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            id: String::new(),
            value: String::new(),
            style: String::new(),
            vertex: false,
            edge: false,
            source: None,
            target: None,
            parent: None,
            layer: "1".into(),
            geom: Geom::default(),
        }
    }
}

impl Cell {
    pub fn is_group(&self) -> bool {
        self.vertex && self.style.contains("shape=group")
    }

    /// Center of the vertex in world coordinates.
    pub fn center(&self) -> (f32, f32) {
        (self.geom.x + self.geom.w / 2.0, self.geom.y + self.geom.h / 2.0)
    }

    /// Bounding rect of a vertex in world coordinates.
    pub fn rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::new(self.geom.x, self.geom.y), egui::Vec2::new(self.geom.w, self.geom.h))
    }

    // ── typed edge view (the mxGraph style string stays canonical storage) ──
    //
    // drawio stores every one of these as style keys / geometry; these
    // accessors + setters expose them typed so the connection engine and the
    // format panel don't re-parse strings everywhere. `Cell::link` keeps
    // `source`/`target`/constraints/dangling points in sync in one call.

    /// Fixed source anchor constraint (`exitX`,`exitY` in 0..1 shape coords).
    pub fn exit(&self) -> Option<(f32, f32)> {
        let x = crate::style::get_f32(&self.style, "exitX", f32::NAN);
        let y = crate::style::get_f32(&self.style, "exitY", f32::NAN);
        (x.is_finite() && y.is_finite()).then_some((x, y))
    }

    /// Fixed target anchor constraint (`entryX`,`entryY` in 0..1 shape coords).
    pub fn entry(&self) -> Option<(f32, f32)> {
        let x = crate::style::get_f32(&self.style, "entryX", f32::NAN);
        let y = crate::style::get_f32(&self.style, "entryY", f32::NAN);
        (x.is_finite() && y.is_finite()).then_some((x, y))
    }

    pub fn start_arrow(&self) -> ArrowKind {
        crate::style::get(&self.style, "startArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::None)
    }
    pub fn end_arrow(&self) -> ArrowKind {
        crate::style::get(&self.style, "endArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::Classic)
    }
    pub fn start_fill(&self) -> bool {
        crate::style::get_bool(&self.style, "startFill", true)
    }
    pub fn end_fill(&self) -> bool {
        crate::style::get_bool(&self.style, "endFill", true)
    }
    pub fn start_size(&self) -> f32 {
        crate::style::get_f32(&self.style, "startSize", 6.0)
    }
    pub fn end_size(&self) -> f32 {
        crate::style::get_f32(&self.style, "endSize", 6.0)
    }
    pub fn route(&self) -> EdgeRoute {
        crate::style::edge_route(&self.style)
    }
    pub fn jump(&self) -> JumpStyle {
        crate::style::get(&self.style, "jumpStyle").and_then(JumpStyle::from_str).unwrap_or(JumpStyle::None)
    }
    pub fn jump_size(&self) -> f32 {
        crate::style::get_f32(&self.style, "jumpSize", 6.0)
    }
    /// Waypoints in world coordinates (mxGraph `Array as="points"`).
    pub fn waypoints(&self) -> Vec<(f32, f32)> {
        self.geom.points.clone()
    }

    /// Set `startArrow`/`endArrow` + matching `xFill` in one call.
    pub fn set_arrow(&mut self, start: bool, kind: ArrowKind, fill: bool) {
        let side = if start { "start" } else { "end" };
        let fill_key = format!("{side}Fill");
        self.style = crate::style::set(&self.style, &format!("{side}Arrow"), kind.as_str());
        self.style = crate::style::set(&self.style, &fill_key, if fill { "1" } else { "0" });
    }

    pub fn set_start_size(&mut self, v: f32) {
        self.style = crate::style::set(&self.style, "startSize", &fmt_num(v));
    }
    pub fn set_end_size(&mut self, v: f32) {
        self.style = crate::style::set(&self.style, "endSize", &fmt_num(v));
    }

    /// Wire one end: cell id + fixed constraint, or a dangling world point.
    /// Clears the complementary state on that end (constraints vs point).
    pub fn link(
        &mut self,
        source: bool,
        cell: Option<String>,
        constraint: Option<(f32, f32)>,
        point: Option<(f32, f32)>,
    ) {
        let (ck_x, ck_y, side) = if source {
            ("exitX", "exitY", &mut self.source)
        } else {
            ("entryX", "entryY", &mut self.target)
        };
        *side = cell;
        match constraint {
            Some((x, y)) => {
                self.style = crate::style::set(&self.style, ck_x, &fmt_num(x));
                self.style = crate::style::set(&self.style, ck_y, &fmt_num(y));
            }
            None => {
                self.style = crate::style::remove(&self.style, ck_x);
                self.style = crate::style::remove(&self.style, ck_y);
            }
        }
        if source {
            self.geom.source_pt = point;
        } else {
            self.geom.target_pt = point;
        }
    }
}

fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i32)
    } else {
        format!("{v:.3}")
    }
}

impl Diagram {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: Vec::new(),
            next_id: 1,
            page_w: 850.0,
            page_h: 1100.0,
            background: None,
            layers: vec![Layer { id: "1".into(), name: "Background".into(), visible: true, locked: false }],
        }
    }

    pub fn add_vertex(&mut self, value: &str, style: &str, x: f32, y: f32, w: f32, h: f32) -> String {
        let id = format!("v{}", self.next_id);
        self.next_id += 1;
        self.cells.push(Cell {
            id: id.clone(),
            value: value.to_string(),
            style: style.to_string(),
            vertex: true,
            edge: false,
            source: None,
            target: None,
            parent: None,
            layer: self.default_layer_id(),
            geom: Geom { x, y, w, h, points: Vec::new(), source_pt: None, target_pt: None, relative: false },
        });
        id
    }

    pub fn add_edge(&mut self, style: &str, source: Option<String>, target: Option<String>) -> String {
        let id = format!("e{}", self.next_id);
        self.next_id += 1;
        self.cells.push(Cell {
            id: id.clone(),
            value: String::new(),
            style: if style.is_empty() { DEFAULT_EDGE_STYLE.to_string() } else { style.to_string() },
            vertex: false,
            edge: true,
            source,
            target,
            parent: None,
            layer: self.default_layer_id(),
            geom: Geom::default(),
        });
        id
    }

    // ── lookups ──────────────────────────────────────────────────────────────

    pub fn cell(&self, id: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.id == id)
    }

    pub fn cell_mut(&mut self, id: &str) -> Option<&mut Cell> {
        self.cells.iter_mut().find(|c| c.id == id)
    }

    pub fn vertex_at(&self, x: f32, y: f32) -> Option<&Cell> {
        self.cells.iter().rev().find(|c| {
            c.vertex
                && self.layer_visible(c)
                && x >= c.geom.x
                && x <= c.geom.x + c.geom.w
                && y >= c.geom.y
                && y <= c.geom.y + c.geom.h
        })
    }

    pub fn vertex_at_mut(&mut self, x: f32, y: f32) -> Option<&mut Cell> {
        self.cells.iter_mut().rev().find(|c| {
            c.vertex
                && x >= c.geom.x
                && x <= c.geom.x + c.geom.w
                && y >= c.geom.y
                && y <= c.geom.y + c.geom.h
        })
    }

    pub fn children_of<'a>(&'a self, id: &str) -> impl Iterator<Item = &'a Cell> {
        self.cells.iter().filter(move |c| c.parent.as_deref() == Some(id))
    }

    /// All descendants of a group cell (children, grandchildren, …).
    pub fn descendants(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        for c in self.children_of(id) {
            out.push(c.id.clone());
            out.extend(self.descendants(&c.id));
        }
        out
    }

    /// Bounding box (x, y, w, h) of the given cells.
    pub fn bbox(&self, ids: &[String]) -> Option<(f32, f32, f32, f32)> {
        let mut r: Option<(f32, f32, f32, f32)> = None;
        for c in &self.cells {
            if !ids.contains(&c.id) {
                continue;
            }
            let (x0, y0, x1, y1) = if c.vertex {
                (c.geom.x, c.geom.y, c.geom.x + c.geom.w, c.geom.y + c.geom.h)
            } else {
                continue;
            };
            r = Some(match r {
                None => (x0, y0, x1 - x0, y1 - y0),
                Some((bx, by, bw, bh)) => (bx.min(x0), by.min(y0), bx.max(x1), by.max(y1)),
            });
        }
        r.map(|(x, y, x2, y2)| (x, y, x2 - x, y2 - y))
    }

    /// Delete cells (and their descendants, and edges that touch them).
    /// Returns how many cells were removed.
    pub fn delete_cascade(&mut self, ids: &[String]) -> usize {
        let mut doomed: Vec<String> = Vec::new();
        for id in ids {
            if self.cell(id).is_some() {
                doomed.push(id.clone());
                doomed.extend(self.descendants(id));
            }
        }
        let before = self.cells.len();
        self.cells.retain(|c| {
            !doomed.contains(&c.id)
                && c.source.as_deref().map(|s| !doomed.contains(&s.to_string())).unwrap_or(true)
                && c.target.as_deref().map(|t| !doomed.contains(&t.to_string())).unwrap_or(true)
        });
        before - self.cells.len()
    }

    // ── layers ───────────────────────────────────────────────────────────────

    pub fn default_layer_id(&self) -> String {
        self.layers.first().map(|l| l.id.clone()).unwrap_or_else(|| "1".into())
    }

    pub fn layer_index(&self, id: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer_visible(&self, cell: &Cell) -> bool {
        match self.layer_index(&cell.layer) {
            Some(i) => self.layers[i].visible,
            None => true,
        }
    }

    pub fn layer_locked(&self, cell: &Cell) -> bool {
        match self.layer_index(&cell.layer) {
            Some(i) => self.layers[i].locked,
            None => false,
        }
    }

    pub fn new_layer_id(&self) -> String {
        let mut n = self.layers.len() + 1;
        while self.layers.iter().any(|l| l.id == format!("L{n}")) {
            n += 1;
        }
        format!("L{n}")
    }

    pub fn add_layer(&mut self, name: &str) -> String {
        let id = self.new_layer_id();
        self.layers.push(Layer {
            id: id.clone(),
            name: name.to_string(),
            visible: true,
            locked: false,
        });
        id
    }

    pub fn next_cell_id(&mut self) -> String {
        loop {
            let id = format!("c{}", self.next_id);
            self.next_id += 1;
            if self.cell(&id).is_none() {
                return id;
            }
        }
    }
}

impl MxFile {
    /// A fresh single-page, empty document (File › New / File › Close).
    pub fn blank() -> Self {
        Self { diagrams: vec![Diagram::new("Page-1")], active: 0, path: None, dirty: false }
    }

    /// Clone page `index` and insert the copy right after it ("Duplicate").
    /// Returns the new page index. Cell ids stay per-diagram, so a whole-page
    /// clone (including its internal source/target references) is consistent.
    pub fn duplicate_diagram(&mut self, index: usize) -> usize {
        let Some(mut copy) = self.diagrams.get(index).cloned() else {
            return self.active;
        };
        let base = if copy.name.is_empty() { format!("Page-{}", index + 1) } else { copy.name.clone() };
        copy.name = format!("{base} copy");
        let at = (index + 1).min(self.diagrams.len());
        self.diagrams.insert(at, copy);
        self.active = at;
        self.dirty = true;
        at
    }

    /// Insert a new empty page after `index` ("Insert Page After").
    /// Returns the new page index.
    pub fn insert_diagram_after(&mut self, index: usize) -> usize {
        let at = (index + 1).min(self.diagrams.len());
        let mut n = self.diagrams.len() + 1;
        while self.diagrams.iter().any(|d| d.name == format!("Page-{n}")) {
            n += 1;
        }
        self.diagrams.insert(at, Diagram::new(format!("Page-{n}")));
        self.active = at;
        self.dirty = true;
        at
    }

    /// Remove page `index` (never removes the last page). Returns whether the
    /// page was removed.
    pub fn remove_diagram(&mut self, index: usize) -> bool {
        if self.diagrams.len() <= 1 || index >= self.diagrams.len() {
            return false;
        }
        self.diagrams.remove(index);
        self.active = self.active.min(self.diagrams.len() - 1);
        self.dirty = true;
        true
    }

    /// Rename page `index` (page-tab context menu).
    pub fn rename_diagram(&mut self, index: usize, name: String) {
        if let Some(d) = self.diagrams.get_mut(index) {
            d.name = name;
            self.dirty = true;
        }
    }

    /// A small flowchart sample so the craft opens with something alive and
    /// showing off the shape library.
    pub fn sample() -> Self {
        let mut page = Diagram::new("Page-1");
        let t = "rounded=1;arcSize=50;whiteSpace=wrap;html=1;fillColor=#d5e8d4;strokeColor=#82b366;";
        let p = "rounded=0;whiteSpace=wrap;html=1;fillColor=#dae8fc;strokeColor=#6c8ebf;";
        let d = "rhombus;whiteSpace=wrap;html=1;fillColor=#ffe6cc;strokeColor=#d79b00;";
        let g = "rounded=1;arcSize=50;whiteSpace=wrap;html=1;fillColor=#f8cecc;strokeColor=#b85450;";
        let s1 = page.add_vertex("Start", t, 80.0, 60.0, 120.0, 40.0);
        let pr = page.add_vertex("Collect report data", p, 60.0, 150.0, 160.0, 60.0);
        let de = page.add_vertex("Data valid?", d, 60.0, 260.0, 160.0, 80.0);
        let pr2 = page.add_vertex("Generate summary", p, 300.0, 270.0, 160.0, 60.0);
        let s2 = page.add_vertex("End", g, 330.0, 400.0, 120.0, 40.0);
        let e = "edgeStyle=orthogonalEdgeStyle;rounded=0;html=1;endArrow=classic;";
        page.add_edge(e, Some(s1), Some(pr.clone()));
        page.add_edge(e, Some(pr), Some(de.clone()));
        let y = page.add_edge(e, Some(de.clone()), Some(pr2.clone()));
        page.cell_mut(&y).unwrap().value = "yes".into();
        let n = page.add_edge(e, Some(de.clone()), Some(s2.clone()));
        page.cell_mut(&n).unwrap().value = "no".into();
        page.cell_mut(&n).unwrap().geom.points = vec![(140.0, 420.0)];
        page.add_edge(e, Some(pr2), Some(s2));
        Self { diagrams: vec![page], active: 0, path: None, dirty: false }
    }
}
