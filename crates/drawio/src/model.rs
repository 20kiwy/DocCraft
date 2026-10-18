//! The mxGraph document model (`mxfile › diagram › mxGraphModel › root`).
//!
//! Everything the editor touches lives here: pages ([`Diagram`]), cells
//! ([`Cell`]), layers ([`Layer`]) and geometry ([`Geom`]). Cells keep their
//! mxGraph `style` string verbatim so import/export round-trips losslessly;
//! [`crate::style`] reads/writes individual keys inside that string.

use serde::{Deserialize, Serialize};

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
            style: style.to_string(),
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
