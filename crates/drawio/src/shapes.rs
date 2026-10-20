//! The drawio shape library: catalog (14 categories, ~140 shapes) and the
//! geometry painters that draw each shape's distinctive outline with egui
//! primitives.
//!
//! Every palette entry carries a real mxGraph style string + default size.
//! Painting dispatches on the style (`shape=` key first, then base flags like
//! `ellipse`/`rhombus`), so imported files keep their look and palette entries
//! render exactly like what lands on the canvas.

use crate::style::{self, Look};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use egui::epaint::PathShape;

// ── transform: rotation + flip ───────────────────────────────────────────────

/// Maps unit-space points (u, v ∈ 0..1 across the shape rect) to screen,
/// applying `flipH`/`flipV` and `rotation`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Xf {
    pub rot: f32, // radians
    pub fh: bool,
    pub fv: bool,
}

impl Xf {
    pub fn from_style(style: &str) -> Self {
        Self {
            rot: style::get_f32(style, "rotation", 0.0).to_radians(),
            fh: style::get_bool(style, "flipH", false),
            fv: style::get_bool(style, "flipV", false),
        }
    }

    pub fn straight(&self) -> bool {
        self.rot.abs() < 0.001 && !self.fh && !self.fv
    }

    pub fn map(&self, r: Rect, u: f32, v: f32) -> Pos2 {
        let u = if self.fh { 1.0 - u } else { u };
        let v = if self.fv { 1.0 - v } else { v };
        let p = Pos2::new(r.min.x + u * r.width(), r.min.y + v * r.height());
        if self.rot.abs() < 0.001 {
            p
        } else {
            rotate(p, r.center(), self.rot)
        }
    }
}

pub fn rotate(p: Pos2, c: Pos2, a: f32) -> Pos2 {
    let (s, co) = a.sin_cos();
    let d = p - c;
    Pos2::new(c.x + d.x * co - d.y * s, c.y + d.x * s + d.y * co)
}

// ── painting primitives (dashed-aware, transform-aware) ─────────────────────

fn stroke_path(p: &Painter, pts: &[Pos2], look: &Look, closed: bool) {
    if pts.len() < 2 {
        return;
    }
    if !look.dashed {
        p.add(Shape::line(pts.to_vec(), look.stroke_w()));
        return;
    }
    let pat: &[f32] = if look.dash.is_empty() { &[6.0, 4.0] } else { &look.dash };
    let pat: Vec<f32> = pat.iter().map(|v| v * look.scale.max(0.2)).collect();
    let mut segs: Vec<[Pos2; 2]> = Vec::new();
    for w in pts.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let len = (b - a).length();
        if len < 0.01 {
            continue;
        }
        let dir = (b - a) / len;
        let mut d = 0.0;
        let mut i = 0;
        while d < len {
            let run = pat[i % pat.len()];
            let next = (d + run).min(len);
            if i % 2 == 0 {
                segs.push([a + dir * d, a + dir * next]);
            }
            d = next;
            i += 1;
        }
    }
    if closed
        && let (Some(last), Some(first)) = (pts.last(), pts.first())
    {
        let _ = (last, first); // closing dash segment omitted; visual detail
    }
    for s in segs {
        p.line_segment(s, look.stroke_w());
    }
}

fn poly(p: &Painter, pts: Vec<Pos2>, look: &Look) {
    if pts.len() < 3 {
        return;
    }
    if let Some(fill) = look.fill {
        p.add(Shape::Path(PathShape { points: pts.clone(), closed: true, fill, stroke: Stroke::NONE.into() }));
    }
    let mut outline = pts.clone();
    outline.push(pts[0]);
    stroke_path(p, &outline, look, true);
}

fn ellipse_pts(r: Rect, xf: &Xf, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let (u, v) = (0.5 + a.cos() / 2.0, 0.5 + a.sin() / 2.0);
            xf.map(r, u, v)
        })
        .collect()
}

fn ellipse(p: &Painter, r: Rect, xf: &Xf, look: &Look) {
    poly(p, ellipse_pts(r, xf, 40), look);
}

/// Border-intersection anchor used for edges: point on the rect border along
/// the line from the center toward `toward`.
pub fn border_point(r: Rect, toward: Pos2) -> Pos2 {
    let c = r.center();
    let dx = toward.x - c.x;
    let dy = toward.y - c.y;
    if dx.abs() < 0.001 && dy.abs() < 0.001 {
        return c;
    }
    let sx = if dx.abs() > 0.001 { r.width() / 2.0 / dx.abs() } else { f32::INFINITY };
    let sy = if dy.abs() > 0.001 { r.height() / 2.0 / dy.abs() } else { f32::INFINITY };
    let s = sx.min(sy);
    Pos2::new(c.x + dx * s, c.y + dy * s)
}

/// Draw a cell label (value) honoring align/verticalAlign/wrap/fonts.
pub fn label(p: &Painter, r: Rect, value: &str, st: &str, look: &Look, xf: &Xf) {
    if value.is_empty() {
        return;
    }
    let bits = style::font_style(st);
    let font = FontId::new(style::font_size(st, look.scale), style::font_family(st));
    let color = look.text;
    let h = style::halign(st);
    let v = style::valign(st);
    let wrap = style::get(st, "whiteSpace").map(|w| w == "wrap").unwrap_or(true);
    let inner = r.shrink(3.0 * look.scale.max(0.5));
    let wrap_w = if wrap { inner.width() } else { f32::INFINITY };
    let galley = p.layout(value.to_string(), font, color, wrap_w);
    let sz = galley.size();
    let mut ax = match h {
        style::Align::Min => inner.left(),
        style::Align::Mid => inner.center().x - sz.x / 2.0,
        style::Align::Max => inner.right() - sz.x,
    };
    let mut ay = match v {
        style::Align::Min => inner.top(),
        style::Align::Mid => inner.center().y - sz.y / 2.0,
        style::Align::Max => inner.bottom() - sz.y,
    };
    if !xf.straight() {
        let c = r.center();
        let anchor = rotate(Pos2::new(ax + sz.x / 2.0, ay + sz.y / 2.0), c, xf.rot);
        ax = anchor.x - sz.x / 2.0;
        ay = anchor.y - sz.y / 2.0;
    }
    p.galley(Pos2::new(ax, ay), galley.clone(), color);
    // underline (fontStyle bit 4)
    if bits & 4 != 0 {
        let mut y = ay;
        for row in &galley.rows {
            y = row.rect().bottom() + 1.0;
            let x0 = match h {
                style::Align::Min => ax,
                style::Align::Mid => ax + (sz.x - row.rect().width()) / 2.0,
                style::Align::Max => ax + sz.x - row.rect().width(),
            };
            p.line_segment([Pos2::new(x0, y), Pos2::new(x0 + row.rect().width(), y)], Stroke::new(1.0, color));
        }
    }
}

/// Vertical (stacked-character) text, used for pool/lane title bands.
fn vtext(p: &Painter, r: Rect, value: &str, look: &Look) {
    if value.is_empty() {
        return;
    }
    let font = FontId::proportional(11.0 * look.scale.max(0.5));
    let mut y = r.center().y - value.chars().count() as f32 * 7.0 * look.scale.max(0.5);
    for c in value.chars() {
        p.text(Pos2::new(r.center().x, y), Align2::CENTER_TOP, c.to_string(), font.clone(), look.text);
        y += 13.0 * look.scale.max(0.5);
    }
}

fn arrow_glyph(p: &Painter, tip: Pos2, dir: Vec2, color: Color32, scale: f32, open: bool) {
    let s = 7.0 * scale;
    let side = Vec2::new(-dir.y, dir.x);
    if open {
        p.line_segment([tip - dir * s + side * s * 0.7, tip], Stroke::new(1.5 * scale, color));
        p.line_segment([tip - dir * s - side * s * 0.7, tip], Stroke::new(1.5 * scale, color));
    } else {
        p.add(Shape::convex_polygon(
            vec![tip, tip - dir * s + side * s * 0.55, tip - dir * s - side * s * 0.55],
            color,
            Stroke::NONE,
        ));
    }
}

// ── catalog ──────────────────────────────────────────────────────────────────

pub struct ShapeDef {
    pub name: &'static str,
    pub style: &'static str,
    pub w: f32,
    pub h: f32,
}

pub struct Cat {
    pub name: &'static str,
    pub items: &'static [ShapeDef],
}

macro_rules! sh {
    ($n:literal, $s:literal, $w:literal, $h:literal) => {
        ShapeDef { name: $n, style: $s, w: $w, h: $h }
    };
}

pub const LIB: &[Cat] = &[
    Cat {
        name: "General",
        items: &[
            sh!("Rectangle", "rounded=0;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Rounded Rectangle", "rounded=1;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Ellipse", "ellipse;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Circle", "ellipse;aspect=fixed;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Diamond", "rhombus;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Hexagon", "shape=hexagon;perimeter=hexagonPerimeter2;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 80.0),
            sh!("Parallelogram", "shape=parallelogram;perimeter=parallelogramPerimeter;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 60.0),
            sh!("Triangle", "triangle;whiteSpace=wrap;html=1;", 60.0, 80.0),
            sh!("Cylinder", "shape=cylinder3;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Text", "text;strokeColor=none;fillColor=none;align=left;verticalAlign=middle;whiteSpace=wrap;html=1;", 80.0, 40.0),
        ],
    },
    Cat {
        name: "Basic",
        items: &[
            sh!("Cube", "shape=cube;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;darkOpacity2=0.1;", 140.0, 100.0),
            sh!("Cylinder", "shape=cylinder3;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Cone", "shape=cone;whiteSpace=wrap;html=1;", 60.0, 80.0),
            sh!("Star", "shape=star;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Heart", "shape=heart;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Lightning", "shape=lightning;whiteSpace=wrap;html=1;", 60.0, 80.0),
            sh!("Cloud", "ellipse;shape=cloud;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Speech Bubble", "shape=speech;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Note", "shape=note;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 80.0, 80.0),
            sh!("Brace", "shape=brace;whiteSpace=wrap;html=1;", 20.0, 120.0),
            sh!("Cross", "shape=cross;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Ring", "shape=ring;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Half-Rounded Rect", "rounded=1;arcSize=50;whiteSpace=wrap;html=1;", 120.0, 40.0),
            sh!("Trapezoid", "shape=trapezoid;perimeter=trapezoidPerimeter;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 60.0),
        ],
    },
    Cat {
        name: "Arrows",
        items: &[
            sh!("Right Arrow", "shape=singleArrow;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 60.0),
            sh!("Left Arrow", "shape=singleArrow;direction=west;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 60.0),
            sh!("Up Arrow", "shape=singleArrow;direction=north;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 60.0, 100.0),
            sh!("Down Arrow", "shape=singleArrow;direction=south;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 60.0, 100.0),
            sh!("Double Arrow", "shape=doubleArrow;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 60.0),
            sh!("Striped Arrow", "shape=stripedArrow;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Tape Arrow", "shape=tape;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Callout", "shape=rectCallout;whiteSpace=wrap;html=1;", 120.0, 80.0),
        ],
    },
    Cat {
        name: "UML",
        items: &[
            sh!("Actor", "shape=umlActor;verticalLabelPosition=bottom;labelBackgroundColor=#ffffff;verticalAlign=top;html=1;outlineConnect=0;", 30.0, 60.0),
            sh!("Use Case", "ellipse;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Class", "verticalAlign=top;align=left;overflow=fill;fontSize=12;whiteSpace=wrap;html=1;", 160.0, 100.0),
            sh!("Interface", "ellipse;fillColor=#ffffff;whiteSpace=wrap;html=1;", 24.0, 24.0),
            sh!("Component", "shape=component;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Package", "shape=folder;tabWidth=60;tabHeight=20;tabPosition=left;whiteSpace=wrap;html=1;", 140.0, 70.0),
            sh!("Node (3D)", "shape=cube;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;darkOpacity2=0.1;", 100.0, 80.0),
            sh!("Artifact", "shape=note;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 90.0),
            sh!("UML Note", "shape=note;fillColor=#fff9e6;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 80.0, 80.0),
            sh!("Lifeline", "shape=umlLifeline;perimeter=lifelinePerimeter;whiteSpace=wrap;html=1;container=1;collapsible=0;recursiveResize=0;outlineConnect=0;", 100.0, 300.0),
            sh!("Activation", "rounded=1;fillColor=#ffffff;whiteSpace=wrap;html=1;", 12.0, 120.0),
            sh!("Frame", "shape=umlFrame;whiteSpace=wrap;html=1;", 180.0, 120.0),
            sh!("System Boundary", "rounded=0;whiteSpace=wrap;html=1;dashed=1;verticalAlign=top;fontSize=12;", 240.0, 160.0),
        ],
    },
    Cat {
        name: "ER",
        items: &[
            sh!("Entity", "verticalAlign=top;align=center;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Weak Entity", "shape=weakEntity;verticalAlign=top;align=center;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Attribute", "ellipse;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Key Attribute", "ellipse;fontStyle=4;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Multivalued", "shape=doubleEllipse;whiteSpace=wrap;html=1;", 90.0, 50.0),
            sh!("Derived Attribute", "ellipse;dashed=1;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Relationship", "rhombus;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Identifying Rel.", "shape=doubleRhombus;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("ISA", "triangle;direction=north;whiteSpace=wrap;html=1;", 80.0, 60.0),
        ],
    },
    Cat {
        name: "Flowchart",
        items: &[
            sh!("Terminator", "rounded=1;arcSize=50;whiteSpace=wrap;html=1;", 120.0, 40.0),
            sh!("Process", "rounded=0;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Decision", "rhombus;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Data", "shape=parallelogram;perimeter=parallelogramPerimeter;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 60.0),
            sh!("Document", "shape=document;whiteSpace=wrap;html=1;boundedLbl=1;", 120.0, 60.0),
            sh!("Multi-Document", "shape=multiDocument;whiteSpace=wrap;html=1;boundedLbl=1;", 120.0, 80.0),
            sh!("Manual Input", "shape=manualInput;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Manual Operation", "shape=trapezoid;perimeter=trapezoidPerimeter;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 60.0),
            sh!("Delay", "shape=delay;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Display", "shape=display;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Preparation", "shape=hexagon;perimeter=hexagonPerimeter2;whiteSpace=wrap;html=1;fixedSize=1;", 120.0, 60.0),
            sh!("Connector", "ellipse;whiteSpace=wrap;html=1;", 30.0, 30.0),
            sh!("Off-Page Connector", "shape=offPageConnector;whiteSpace=wrap;html=1;", 90.0, 70.0),
            sh!("Or", "ellipse;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Summing Junction", "ellipse;shape=summingJunction;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Merge", "triangle;direction=south;whiteSpace=wrap;html=1;", 70.0, 60.0),
            sh!("Extract", "triangle;direction=north;whiteSpace=wrap;html=1;", 70.0, 60.0),
            sh!("Internal Storage", "shape=internalStorage;whiteSpace=wrap;html=1;backgroundOutline=1;", 100.0, 80.0),
            sh!("Database", "shape=cylinder3;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Sequential Data", "shape=dataStorage;whiteSpace=wrap;html=1;", 90.0, 70.0),
            sh!("Direct Access", "shape=directData;whiteSpace=wrap;html=1;", 90.0, 70.0),
            sh!("Loop Limit", "shape=loopLimit;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Sort", "shape=sortShape;perimeter=parallelogramPerimeter;whiteSpace=wrap;html=1;fixedSize=1;", 90.0, 60.0),
            sh!("Collate", "shape=collate;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Predefined Process", "shape=process;whiteSpace=wrap;html=1;backgroundOutline=1;", 120.0, 60.0),
        ],
    },
    Cat {
        name: "BPMN",
        items: &[
            sh!("Start Event", "ellipse;shape=eventStart;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("End Event", "ellipse;shape=eventEnd;fillColor=#ffe6cc;strokeColor=#d79b00;strokeWidth=3;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Intermediate Event", "ellipse;shape=eventInter;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Message Start", "ellipse;shape=eventMsg;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Timer Start", "ellipse;shape=eventTimer;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Error Event", "ellipse;shape=eventError;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;aspect=fixed;", 30.0, 30.0),
            sh!("Task", "rounded=1;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("User Task", "rounded=1;shape=taskUser;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Service Task", "rounded=1;shape=taskService;whiteSpace=wrap;html=1;", 120.0, 60.0),
            sh!("Subprocess", "rounded=1;shape=subprocess;whiteSpace=wrap;html=1;", 140.0, 70.0),
            sh!("Exclusive Gateway", "rhombus;shape=gatewayX;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 50.0, 50.0),
            sh!("Parallel Gateway", "rhombus;shape=gatewayPlus;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 50.0, 50.0),
            sh!("Inclusive Gateway", "rhombus;shape=gatewayO;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 50.0, 50.0),
            sh!("Event Gateway", "rhombus;shape=gatewayPent;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 50.0, 50.0),
            sh!("Pool", "swimlane;fontStyle=1;align=center;verticalAlign=middle;horizontal=0;startSize=20;horizontalStack=0;resizeParent=1;collapsible=0;html=1;", 400.0, 160.0),
            sh!("Lane", "swimlane;horizontal=0;startSize=20;html=1;", 160.0, 200.0),
        ],
    },
    Cat {
        name: "Software",
        items: &[
            sh!("Cloud", "ellipse;shape=cloud;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Server", "shape=server;whiteSpace=wrap;html=1;", 70.0, 80.0),
            sh!("Database", "shape=cylinder3;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Client", "shape=client;whiteSpace=wrap;html=1;", 80.0, 60.0),
            sh!("Mobile App", "shape=phone;whiteSpace=wrap;html=1;", 40.0, 80.0),
            sh!("Queue", "shape=queue;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Cache", "shape=cylinder3;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 70.0),
            sh!("API Endpoint", "rounded=1;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Firewall", "shape=firewall;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Storage", "shape=storage;whiteSpace=wrap;html=1;", 70.0, 70.0),
        ],
    },
    Cat {
        name: "Networks",
        items: &[
            sh!("Router", "shape=router;whiteSpace=wrap;html=1;", 80.0, 50.0),
            sh!("Switch", "shape=switchDev;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Workstation", "shape=monitor;whiteSpace=wrap;html=1;", 80.0, 70.0),
            sh!("Laptop", "shape=laptop;whiteSpace=wrap;html=1;", 80.0, 60.0),
            sh!("Firewall", "shape=firewall;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Wireless AP", "shape=wifi;whiteSpace=wrap;html=1;", 60.0, 50.0),
            sh!("Printer", "shape=printer;whiteSpace=wrap;html=1;", 70.0, 60.0),
            sh!("Monitor", "shape=monitor;fillColor=#bac8d3;whiteSpace=wrap;html=1;", 80.0, 70.0),
        ],
    },
    Cat {
        name: "Mockups",
        items: &[
            sh!("Browser Window", "shape=browser;whiteSpace=wrap;html=1;verticalAlign=top;", 200.0, 140.0),
            sh!("Dialog Box", "shape=dialog;whiteSpace=wrap;html=1;verticalAlign=top;", 180.0, 110.0),
            sh!("Button", "rounded=1;arcSize=50;whiteSpace=wrap;html=1;", 80.0, 30.0),
            sh!("Text Field", "rounded=0;fillColor=#ffffff;whiteSpace=wrap;html=1;", 100.0, 26.0),
            sh!("Phone Frame", "rounded=1;arcSize=20;shape=phone;whiteSpace=wrap;html=1;verticalAlign=top;", 60.0, 120.0),
            sh!("Tablet Frame", "rounded=1;arcSize=12;shape=tablet;whiteSpace=wrap;html=1;verticalAlign=top;", 100.0, 140.0),
            sh!("List Item", "shape=listItem;whiteSpace=wrap;html=1;", 120.0, 30.0),
            sh!("Table Grid", "shape=tableGrid;whiteSpace=wrap;html=1;verticalAlign=top;", 140.0, 100.0),
        ],
    },
    Cat {
        name: "Tables",
        items: &[
            sh!("Class Table", "shape=classTable;verticalAlign=top;align=left;overflow=fill;fontSize=11;whiteSpace=wrap;html=1;", 160.0, 120.0),
            sh!("Entity Table", "shape=entityTable;verticalAlign=top;align=left;overflow=fill;fontSize=11;whiteSpace=wrap;html=1;", 160.0, 120.0),
            sh!("Row List", "shape=tableGrid;verticalAlign=top;align=left;whiteSpace=wrap;html=1;", 120.0, 90.0),
        ],
    },
    Cat {
        name: "Titles",
        items: &[
            sh!("Text", "text;strokeColor=none;fillColor=none;align=left;verticalAlign=middle;whiteSpace=wrap;html=1;", 80.0, 40.0),
            sh!("Heading", "text;fontSize=20;fontStyle=1;align=center;strokeColor=none;fillColor=none;whiteSpace=wrap;html=1;", 160.0, 40.0),
            sh!("Title Bar", "shape=titleBar;verticalAlign=middle;fontStyle=1;whiteSpace=wrap;html=1;", 160.0, 30.0),
            sh!("Divider H", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 120.0, 10.0),
            sh!("Divider V", "line;direction=north;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 10.0, 120.0),
        ],
    },
    Cat {
        name: "Signs",
        items: &[
            sh!("Warning", "triangle;direction=north;fillColor=#ffe6cc;strokeColor=#d79b00;shape=warning;whiteSpace=wrap;html=1;", 70.0, 60.0),
            sh!("Forbidden", "ellipse;shape=forbidden;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;aspect=fixed;", 50.0, 50.0),
            sh!("Info", "ellipse;shape=info;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Question", "ellipse;shape=question;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Check", "ellipse;shape=checkMark;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Cross Mark", "ellipse;shape=crossMark;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("First Aid", "ellipse;shape=firstAid;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 50.0, 50.0),
            sh!("Plus Sign", "shape=cross;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
        ],
    },
    Cat {
        name: "Misc",
        items: &[
            sh!("Container", "rounded=0;dashed=1;fillColor=none;verticalAlign=top;whiteSpace=wrap;html=1;", 200.0, 140.0),
            sh!("Dashed Group", "shape=group;dashed=1;fillColor=none;strokeColor=#999999;html=1;whiteSpace=wrap;", 160.0, 120.0),
            sh!("Placeholder Frame", "rounded=0;dashed=1;fillColor=none;strokeColor=#b3b3b3;fontColor=#b3b3b3;whiteSpace=wrap;html=1;", 160.0, 100.0),
            sh!("Venn Two", "shape=venn2;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Venn Three", "shape=venn3;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 120.0, 100.0),
        ],
    },
    // ── More Shapes dialog libraries (off by default, toggled in the dialog) ──
    Cat {
        name: "OpenStack",
        items: &[
            sh!("Compute", "shape=cube;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Object Storage", "shape=cylinder3;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Block Storage", "shape=cylinder3;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Image Service", "shape=note;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 70.0),
            sh!("Identity", "shape=folder;fillColor=#f8cecc;strokeColor=#b85450;tabWidth=60;tabHeight=20;tabPosition=left;whiteSpace=wrap;html=1;", 90.0, 70.0),
            sh!("Dashboard", "shape=monitor;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;", 80.0, 70.0),
            sh!("Networking", "shape=router;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;", 80.0, 50.0),
            sh!("Orchestration", "rounded=1;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Telemetry", "ellipse;shape=eventTimer;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Database", "shape=cylinder3;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
        ],
    },
    Cat {
        name: "Rack",
        items: &[
            sh!("Rack Frame", "rounded=0;fillColor=#f5f5f5;strokeColor=#666666;verticalAlign=top;whiteSpace=wrap;html=1;", 60.0, 160.0),
            sh!("Rack Server", "shape=server;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;", 70.0, 80.0),
            sh!("Storage Array", "shape=storage;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 70.0, 70.0),
            sh!("ToR Switch", "shape=switchDev;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Rack Router", "shape=router;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;", 80.0, 50.0),
            sh!("Patch Panel", "rounded=0;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 120.0, 20.0),
            sh!("UPS", "shape=cube;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 80.0, 60.0),
            sh!("Blade Chassis", "shape=browser;fillColor=#f5f5f5;strokeColor=#666666;verticalAlign=top;whiteSpace=wrap;html=1;", 120.0, 80.0),
        ],
    },
    Cat {
        name: "SAP",
        items: &[
            sh!("SAP System", "rounded=1;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("SAP Database", "shape=cylinder3;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("SAP Portal", "shape=monitor;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 80.0, 70.0),
            sh!("SAP Gateway", "shape=hexagon;perimeter=hexagonPerimeter2;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;fixedSize=1;", 110.0, 60.0),
            sh!("SAP Mobile", "shape=phone;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 40.0, 80.0),
            sh!("SAP Cloud", "ellipse;shape=cloud;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("SAP Integration", "shape=doubleArrow;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 110.0, 60.0),
            sh!("SAP Analytics", "shape=display;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 70.0),
            sh!("SAP Fiori", "shape=browser;fillColor=#dae8fc;strokeColor=#6c8ebf;verticalAlign=top;whiteSpace=wrap;html=1;", 130.0, 90.0),
        ],
    },
    Cat {
        name: "Veeam",
        items: &[
            sh!("Backup Job", "rounded=1;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Backup Repository", "shape=cylinder3;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Backup Proxy", "shape=server;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 70.0, 80.0),
            sh!("vSphere Host", "shape=monitor;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 80.0, 70.0),
            sh!("Agent", "shape=phone;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 40.0, 80.0),
            sh!("Tape Library", "shape=storage;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 70.0, 70.0),
            sh!("Cloud Repository", "ellipse;shape=cloud;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Instant Recovery", "ellipse;shape=eventEnd;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Replication", "shape=doubleArrow;fillColor=#d5e8d4;strokeColor=#82b366;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 110.0, 60.0),
        ],
    },
    Cat {
        name: "VMware",
        items: &[
            sh!("ESXi Host", "shape=server;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;", 70.0, 80.0),
            sh!("vCenter Server", "shape=monitor;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;", 80.0, 70.0),
            sh!("Datastore", "shape=cylinder3;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Virtual Machine", "shape=browser;fillColor=#b0e3e6;strokeColor=#106e8b;verticalAlign=top;whiteSpace=wrap;html=1;", 120.0, 80.0),
            sh!("Cluster", "shape=group;fillColor=none;strokeColor=#106e8b;dashed=1;whiteSpace=wrap;html=1;", 130.0, 90.0),
            sh!("vSwitch", "shape=switchDev;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("VM Template", "shape=note;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 70.0),
            sh!("Port Group", "shape=router;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;", 80.0, 50.0),
            sh!("vSAN", "shape=cube;fillColor=#b0e3e6;strokeColor=#106e8b;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 80.0, 70.0),
        ],
    },
    Cat {
        name: "ArchiMate 3.2",
        items: &[
            sh!("Actor", "shape=umlActor;verticalLabelPosition=bottom;labelBackgroundColor=#ffffff;verticalAlign=top;html=1;outlineConnect=0;", 30.0, 60.0),
            sh!("Business Role", "rounded=0;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;verticalAlign=top;", 100.0, 60.0),
            sh!("Business Service", "ellipse;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Business Process", "rounded=1;shape=subprocess;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Business Event", "ellipse;shape=eventInter;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Business Object", "shape=note;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 70.0),
            sh!("Application Component", "shape=component;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Application Function", "rounded=1;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Data Object", "shape=note;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 70.0),
            sh!("Node", "shape=cube;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Device", "shape=cube;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Artifact", "shape=note;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 90.0),
        ],
    },
    Cat {
        name: "ArchiMate 2.1",
        items: &[
            sh!("Actor", "shape=umlActor;verticalLabelPosition=bottom;labelBackgroundColor=#ffffff;verticalAlign=top;html=1;outlineConnect=0;", 30.0, 60.0),
            sh!("Role", "rounded=0;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;verticalAlign=top;", 100.0, 60.0),
            sh!("Service", "ellipse;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Process", "rounded=1;shape=subprocess;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Component", "shape=component;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Node", "shape=cube;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Product", "shape=folder;fillColor=#ffe6cc;strokeColor=#d79b00;tabWidth=60;tabHeight=20;tabPosition=left;whiteSpace=wrap;html=1;", 100.0, 70.0),
        ],
    },
    Cat {
        name: "BPMN 2.0",
        items: &[
            sh!("Message Throw", "ellipse;shape=eventMsg;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Signal Throw", "ellipse;shape=warning;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Escalation", "ellipse;shape=warning;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Terminate", "ellipse;shape=eventEnd;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Conditional", "shape=display;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 100.0, 60.0),
            sh!("Link Event", "ellipse;shape=eventInter;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Compensation", "ellipse;shape=eventInter;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Data Object", "shape=note;fillColor=#ffffff;strokeColor=#000000;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 60.0, 70.0),
            sh!("Data Store", "shape=cylinder3;fillColor=#ffffff;strokeColor=#000000;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 70.0),
            sh!("Pool", "swimlane;fontStyle=1;align=center;verticalAlign=middle;horizontal=0;startSize=20;horizontalStack=0;resizeParent=1;collapsible=0;html=1;", 400.0, 160.0),
            sh!("Lane", "swimlane;horizontal=0;startSize=20;html=1;", 160.0, 200.0),
            sh!("Group", "shape=group;dashed=1;fillColor=none;strokeColor=#999999;whiteSpace=wrap;html=1;", 140.0, 110.0),
            sh!("Text Annotation", "text;strokeColor=none;fillColor=none;align=left;verticalAlign=middle;whiteSpace=wrap;html=1;", 90.0, 40.0),
        ],
    },
    Cat {
        name: "SysML",
        items: &[
            sh!("Block", "shape=titleBar;verticalAlign=middle;fontStyle=1;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Requirement", "shape=note;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Package", "shape=folder;fillColor=#dae8fc;strokeColor=#6c8ebf;tabWidth=60;tabHeight=20;tabPosition=left;whiteSpace=wrap;html=1;", 100.0, 70.0),
            sh!("Constraint", "shape=parallelogram;perimeter=parallelogramPerimeter;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;fixedSize=1;", 110.0, 60.0),
            sh!("Activity", "rounded=1;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("State", "rounded=1;arcSize=40;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Port", "rounded=0;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;aspect=fixed;", 20.0, 20.0),
            sh!("Item Flow", "shape=singleArrow;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Part Property", "shape=cube;fillColor=#dae8fc;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("BDD Frame", "shape=umlFrame;fillColor=none;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;", 150.0, 110.0),
        ],
    },
    Cat {
        name: "Value Stream Mapping",
        items: &[
            sh!("Supplier", "rounded=0;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;verticalAlign=top;", 90.0, 60.0),
            sh!("Process Box", "rounded=0;fillColor=#ffffff;strokeColor=#000000;whiteSpace=wrap;html=1;verticalAlign=top;", 90.0, 70.0),
            sh!("Inventory", "triangle;direction=south;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 70.0, 50.0),
            sh!("Shipment", "shape=doubleArrow;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Push Arrow", "shape=singleArrow;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Electronic Info", "shape=doubleArrow;dashed=1;fillColor=none;strokeColor=#666666;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Manual Info", "line;strokeWidth=2;fillColor=none;dashed=1;align=left;verticalAlign=middle;html=1;", 100.0, 10.0),
            sh!("Data Box", "shape=internalStorage;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;backgroundOutline=1;", 90.0, 70.0),
            sh!("Kanban", "shape=note;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;backgroundOutline=1;darkOpacity=0.05;", 70.0, 60.0),
            sh!("Operator", "shape=umlActor;verticalLabelPosition=bottom;labelBackgroundColor=#ffffff;verticalAlign=top;html=1;outlineConnect=0;", 30.0, 60.0),
            sh!("Timeline", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 120.0, 10.0),
        ],
    },
    Cat {
        name: "Cabinets",
        items: &[
            sh!("Base Cabinet", "shape=cube;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Wall Cabinet", "shape=cube;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 60.0),
            sh!("Drawer Unit", "shape=internalStorage;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;backgroundOutline=1;", 90.0, 80.0),
            sh!("Shelf", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 110.0, 10.0),
            sh!("Wardrobe", "rounded=0;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;verticalAlign=top;", 70.0, 140.0),
            sh!("Nightstand", "shape=cube;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 60.0, 50.0),
            sh!("Desk", "rounded=0;fillColor=#ffe6cc;strokeColor=#d79b00;whiteSpace=wrap;html=1;", 130.0, 50.0),
            sh!("Cupboard", "shape=browser;fillColor=#ffe6cc;strokeColor=#d79b00;verticalAlign=top;whiteSpace=wrap;html=1;", 100.0, 110.0),
            sh!("Pantry", "rounded=0;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;verticalAlign=top;", 70.0, 130.0),
        ],
    },
    Cat {
        name: "Infographic",
        items: &[
            sh!("Pie Chart", "shape=venn2;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 110.0, 80.0),
            sh!("Ring Percent", "shape=ring;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 70.0, 70.0),
            sh!("Bar Chart", "shape=tableGrid;fillColor=#ffd966;strokeColor=#d6b656;verticalAlign=top;whiteSpace=wrap;html=1;", 110.0, 80.0),
            sh!("Funnel", "shape=trapezoid;perimeter=trapezoidPerimeter;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;fixedSize=1;", 110.0, 60.0),
            sh!("Pyramid", "triangle;direction=north;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 90.0, 70.0),
            sh!("Step Arrow", "shape=singleArrow;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Milestone", "ellipse;shape=eventStart;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Trend Line", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 110.0, 10.0),
            sh!("Target", "shape=ring;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;", 70.0, 70.0),
            sh!("Star", "shape=star;fillColor=#ffd966;strokeColor=#d6b656;whiteSpace=wrap;html=1;", 70.0, 70.0),
        ],
    },
    Cat {
        name: "EIP",
        items: &[
            sh!("Message Channel", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 110.0, 10.0),
            sh!("Messaging Gateway", "shape=component;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;", 110.0, 60.0),
            sh!("Content Filter", "shape=hexagon;perimeter=hexagonPerimeter2;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;fixedSize=1;", 110.0, 60.0),
            sh!("Message Router", "shape=router;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;", 80.0, 50.0),
            sh!("Message Translator", "shape=parallelogram;perimeter=parallelogramPerimeter;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;fixedSize=1;", 110.0, 60.0),
            sh!("Point-to-Point Channel", "shape=queue;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;", 100.0, 50.0),
            sh!("Splitter", "shape=doubleArrow;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;arrowWidth=0.5;arrowSize=0.4;", 100.0, 50.0),
            sh!("Aggregator", "shape=collate;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;", 80.0, 80.0),
            sh!("Message Envelope", "ellipse;shape=eventMsg;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
            sh!("Endpoint", "ellipse;fillColor=#e1d5e7;strokeColor=#9673a6;whiteSpace=wrap;html=1;aspect=fixed;", 40.0, 40.0),
        ],
    },
    Cat {
        name: "Electrical",
        items: &[
            sh!("Battery", "line;strokeWidth=3;fillColor=none;align=left;verticalAlign=middle;html=1;", 90.0, 12.0),
            sh!("Resistor", "shape=parallelogram;perimeter=parallelogramPerimeter;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;fixedSize=1;", 100.0, 40.0),
            sh!("Capacitor", "line;strokeWidth=3;fillColor=none;align=left;verticalAlign=middle;html=1;direction=north;", 12.0, 90.0),
            sh!("Switch", "shape=switchDev;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;", 90.0, 40.0),
            sh!("Lamp", "ellipse;fillColor=#fff2cc;strokeColor=#d6b656;whiteSpace=wrap;html=1;aspect=fixed;", 50.0, 50.0),
            sh!("Motor", "shape=cylinder3;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 70.0),
            sh!("Ground", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 70.0, 10.0),
            sh!("Generator", "shape=cylinder3;fillColor=#bac8d3;strokeColor=#6c8ebf;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;size=15;", 60.0, 80.0),
            sh!("Transformer", "shape=cube;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;boundedLbl=1;backgroundOutline=1;darkOpacity=0.05;", 90.0, 70.0),
            sh!("Fuse", "rounded=0;fillColor=#f8cecc;strokeColor=#b85450;whiteSpace=wrap;html=1;", 100.0, 30.0),
            sh!("Wire", "line;strokeWidth=2;fillColor=none;align=left;verticalAlign=middle;html=1;", 100.0, 10.0),
            sh!("Terminal", "ellipse;fillColor=#f5f5f5;strokeColor=#666666;whiteSpace=wrap;html=1;aspect=fixed;", 24.0, 24.0),
        ],
    },
];

/// More-Shapes dialog layout: `(group header, [category names])`. An empty
/// header renders as the top-level section (no group label).
pub const DIALOG_GROUPS: &[(&str, &[&str])] = &[
    (
        "",
        &[
            "General", "Basic", "Arrows", "UML", "ER", "Flowchart", "BPMN", "Software", "Networks", "Mockups", "Tables", "Titles", "Signs", "Misc",
            "OpenStack", "Rack", "SAP", "Veeam", "VMware",
        ],
    ),
    ("Business", &["ArchiMate 3.2", "ArchiMate 2.1", "BPMN 2.0", "SysML", "Value Stream Mapping"]),
    ("Other", &["Cabinets", "Infographic", "EIP", "Electrical"]),
];

/// Categories enabled on the palette by default: the original 14, exactly.
pub const DEFAULT_ENABLED: usize = 14;

/// LIB index of a category by name (More-Shapes dialog wiring).
pub fn cat_index(name: &str) -> Option<usize> {
    LIB.iter().position(|c| c.name == name)
}

/// Count of categories (for fixed-size open/close state arrays).
pub const CATS: usize = LIB.len();

// ── painting dispatch ────────────────────────────────────────────────────────

/// Paint one vertex: distinctive geometry + label.
pub fn paint_vertex(p: &Painter, r: Rect, st: &str, look: &Look, value: &str, xf: &Xf) {
    let shape = style::get(st, "shape").unwrap_or("");
    match shape {
        // ── basics ──
        "cube" => cube(p, r, look, xf),
        "cylinder3" | "cylinder" => cylinder(p, r, look, xf),
        "cone" => cone(p, r, look, xf),
        "star" => star(p, r, look, xf),
        "heart" => heart(p, r, look, xf),
        "lightning" => lightning(p, r, look, xf),
        "cloud" => cloud(p, r, look, xf),
        "speech" => speech(p, r, look, xf),
        "note" => note(p, r, look, xf),
        "brace" => brace(p, r, look, xf),
        "cross" => cross(p, r, look, xf),
        "ring" => ring(p, r, look, xf),
        "umlActor" => actor(p, r, look, xf),
        "umlLifeline" => lifeline(p, r, look, xf, value),
        "umlFrame" => uml_frame(p, r, look, xf, value),
        "component" => component(p, r, look, xf),
        "folder" => package(p, r, look, xf),
        "weakEntity" => weak_entity(p, r, look, xf),
        "doubleEllipse" => double_ellipse(p, r, look, xf),
        "doubleRhombus" => double_rhombus(p, r, look, xf),
        "parallelogram" => parallelogram(p, r, look, xf),
        "trapezoid" => trapezoid(p, r, look, xf),
        "hexagon" => hexagon(p, r, look, xf),
        "document" => document(p, r, look, xf),
        "multiDocument" => multi_document(p, r, look, xf),
        "manualInput" => manual_input(p, r, look, xf),
        "delay" => delay(p, r, look, xf),
        "display" => display(p, r, look, xf),
        "internalStorage" => internal_storage(p, r, look, xf),
        "dataStorage" => data_storage(p, r, look, xf),
        "directData" => direct_data(p, r, look, xf),
        "loopLimit" => loop_limit(p, r, look, xf),
        "sortShape" => sort_shape(p, r, look, xf),
        "collate" => collate(p, r, look, xf),
        "process" => predefined(p, r, look, xf),
        "offPageConnector" => off_page(p, r, look, xf),
        "summingJunction" => summing_junction(p, r, look, xf),
        "singleArrow" => single_arrow(p, r, st, look, xf),
        "doubleArrow" => double_arrow(p, r, st, look, xf),
        "stripedArrow" => striped_arrow(p, r, look, xf),
        "tape" => tape(p, r, look, xf),
        "rectCallout" => rect_callout(p, r, look, xf),
        // ── bpmn ──
        "eventStart" => event_circle(p, r, look, xf, 1.2),
        "eventEnd" => event_circle(p, r, look, xf, 3.0),
        "eventInter" => event_inter(p, r, look, xf),
        "eventMsg" => event_glyph(p, r, look, xf, Glyph::Envelope),
        "eventTimer" => event_glyph(p, r, look, xf, Glyph::Clock),
        "eventError" => event_glyph(p, r, look, xf, Glyph::Bolt),
        "taskUser" => task_icon(p, r, look, xf, Glyph::Person),
        "taskService" => task_icon(p, r, look, xf, Glyph::Gear),
        "subprocess" => subprocess(p, r, look, xf),
        "gatewayX" => gateway(p, r, look, xf, Glyph::X),
        "gatewayPlus" => gateway(p, r, look, xf, Glyph::Plus),
        "gatewayO" => gateway(p, r, look, xf, Glyph::O),
        "gatewayPent" => gateway(p, r, look, xf, Glyph::Pent),
        "pool" | "swimlanePool" => swimlane(p, r, st, look, value),
        "lane" | "swimlane" => swimlane(p, r, st, look, value),
        // ── software / networks ──
        "server" => server(p, r, look, xf),
        "storage" => storage(p, r, look, xf),
        "client" => monitor(p, r, look, xf),
        "monitor" => monitor(p, r, look, xf),
        "phone" => phone(p, r, look, xf),
        "tablet" => tablet(p, r, look, xf),
        "queue" => queue(p, r, look, xf),
        "api" => rounded_rect(p, r, st, look, xf),
        "firewall" => firewall(p, r, look, xf),
        "router" => router(p, r, look, xf),
        "switchDev" => switch_dev(p, r, look, xf),
        "laptop" => laptop(p, r, look, xf),
        "wifi" => wifi(p, r, look, xf),
        "printer" => printer(p, r, look, xf),
        // ── mockups / tables ──
        "browser" => browser(p, r, look, xf, value),
        "dialog" => dialog(p, r, look, xf, value),
        "listItem" => list_item(p, r, look, xf, value),
        "tableGrid" => table_grid(p, r, look, xf, value),
        "classTable" => class_table(p, r, look, xf, value),
        "entityTable" => entity_table(p, r, look, xf, value),
        "titleBar" => title_bar(p, r, look, xf, value),
        // ── misc / signs ──
        "group" => group_box(p, r, look, xf),
        "placeholder" => plain_rect(p, r, st, look, xf),
        "venn2" => venn2(p, r, look, xf),
        "venn3" => venn3(p, r, look, xf),
        "warning" => warning(p, r, look, xf),
        "forbidden" => forbidden(p, r, look, xf),
        "info" => info_sign(p, r, look, "i"),
        "question" => info_sign(p, r, look, "?"),
        "checkMark" => info_sign(p, r, look, "✓"),
        "crossMark" => cross_mark(p, r, look),
        "firstAid" => first_aid(p, r, look, xf),
        "divider" | "" => {}
        _ => plain_rect(p, r, st, look, xf),
    }
    // base-geometry fallbacks for shapes without an explicit `shape=` key
    if shape.is_empty() {
        if st.contains("ellipse") {
            ellipse(p, r, xf, look);
        } else if st.contains("rhombus") {
            diamond(p, r, look, xf);
        } else if st.contains("triangle") {
            triangle(p, r, st, look, xf);
        } else if st.contains("swimlane") {
            swimlane(p, r, st, look, value);
        } else if st.contains("shape=line") || st.starts_with("line") {
            line_shape(p, r, st, look);
        } else {
            plain_rect(p, r, st, look, xf);
        }
    }
    label(p, r, value, st, look, xf);
}

/// Small fixed glyph drawn inside shapes (BPMN events, tasks, gateways…).
#[derive(Debug, Clone, Copy)]
enum Glyph {
    Envelope,
    Clock,
    Bolt,
    Person,
    Gear,
    X,
    Plus,
    O,
    Pent,
}

fn glyph(p: &Painter, c: Pos2, s: f32, look: &Look, g: Glyph) {
    let st = Stroke::new((1.2 * look.scale).max(0.8), look.stroke);
    match g {
        Glyph::Envelope => {
            let w = s * 1.6;
            let h = s;
            let r = Rect::from_center_size(c, Vec2::new(w, h));
            p.rect_stroke(r, 1.0, st, egui::StrokeKind::Inside);
            p.line_segment([r.left_top(), r.center()], st);
            p.line_segment([r.right_top(), r.center()], st);
        }
        Glyph::Clock => {
            p.circle_stroke(c, s / 2.0, st);
            p.line_segment([c, c + Vec2::new(0.0, -s * 0.32)], st);
            p.line_segment([c, c + Vec2::new(s * 0.22, 0.0)], st);
        }
        Glyph::Bolt => {
            let s2 = s * 0.9;
            p.add(Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - s2 * 0.2, c.y - s2 / 2.0),
                    Pos2::new(c.x + s2 * 0.25, c.y - s2 / 2.0),
                    Pos2::new(c.x - s2 * 0.05, c.y),
                    Pos2::new(c.x + s2 * 0.25, c.y),
                    Pos2::new(c.x - s2 * 0.3, c.y + s2 / 2.0),
                    Pos2::new(c.x - s2 * 0.08, c.y + s2 * 0.05),
                    Pos2::new(c.x - s2 * 0.35, c.y + s2 * 0.05),
                ],
                look.stroke,
                Stroke::NONE,
            ));
        }
        Glyph::Person => {
            p.circle_filled(Pos2::new(c.x, c.y - s * 0.3), s * 0.22, look.stroke);
            p.add(Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - s * 0.3, c.y + s * 0.45),
                    Pos2::new(c.x, c.y - s * 0.05),
                    Pos2::new(c.x + s * 0.3, c.y + s * 0.45),
                ],
                look.stroke,
                Stroke::NONE,
            ));
        }
        Glyph::Gear => {
            p.circle_stroke(c, s * 0.3, st);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let d = Vec2::new(a.cos(), a.sin()) * s * 0.42;
                p.line_segment([c + d * 0.6, c + d], st);
            }
        }
        Glyph::X => {
            let k = s * 0.3;
            p.line_segment([c - Vec2::new(k, k), c + Vec2::new(k, k)], Stroke::new(2.2 * look.scale.max(0.5), look.stroke));
            p.line_segment([c + Vec2::new(k, -k), c - Vec2::new(k, -k)], Stroke::new(2.2 * look.scale.max(0.5), look.stroke));
        }
        Glyph::Plus => {
            let k = s * 0.3;
            p.line_segment([c - Vec2::new(k, 0.0), c + Vec2::new(k, 0.0)], Stroke::new(2.2 * look.scale.max(0.5), look.stroke));
            p.line_segment([c - Vec2::new(0.0, k), c + Vec2::new(0.0, k)], Stroke::new(2.2 * look.scale.max(0.5), look.stroke));
        }
        Glyph::O => {
            p.circle_stroke(c, s * 0.3, Stroke::new(1.6 * look.scale.max(0.5), look.stroke));
        }
        Glyph::Pent => {
            p.circle_stroke(c, s * 0.34, Stroke::new(1.2 * look.scale.max(0.5), look.stroke));
            let pts: Vec<Pos2> = (0..5)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 / 5.0 * std::f32::consts::TAU;
                    c + Vec2::new(a.cos(), a.sin()) * s * 0.24
                })
                .collect();
            p.add(Shape::convex_polygon(pts, Color32::TRANSPARENT, st));
        }
    }
}

// ── individual painters ─────────────────────────────────────────────────────

fn plain_rect(p: &Painter, r: Rect, st: &str, look: &Look, xf: &Xf) {
    let rounded = style::get_bool(st, "rounded", false);
    if xf.straight() && rounded {
        let rad = if style::get(st, "arcSize").is_some() { (r.width().min(r.height()) / 2.0).min(18.0 * look.scale) } else { 8.0 * look.scale };
        if let Some(fill) = look.fill {
            p.rect_filled(r, rad, fill);
        }
        if !look.dashed {
            p.rect_stroke(r, rad, look.stroke_w(), egui::StrokeKind::Inside);
        } else {
            dashed_rect(p, r, look);
        }
        return;
    }
    if xf.straight() && !rounded && !look.dashed && look.fill.is_some() {
        // fast crisp path
        p.rect_filled(r, 0.0, look.fill.unwrap());
        p.rect_stroke(r, 0.0, look.stroke_w(), egui::StrokeKind::Inside);
        return;
    }
    let pts = rect_pts(r, xf);
    poly(p, pts, look);
}

fn dashed_rect(p: &Painter, r: Rect, look: &Look) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    stroke_path(p, &pts, look, true);
}

fn rect_pts(r: Rect, xf: &Xf) -> Vec<Pos2> {
    vec![xf.map(r, 0.0, 0.0), xf.map(r, 1.0, 0.0), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0)]
}

fn diamond(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(p, vec![xf.map(r, 0.5, 0.0), xf.map(r, 1.0, 0.5), xf.map(r, 0.5, 1.0), xf.map(r, 0.0, 0.5)], look);
}

fn double_rhombus(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    diamond(p, r, look, xf);
    let inner = r.shrink(r.width().min(r.height()) * 0.12);
    diamond(p, inner, &Look { fill: None, ..look.clone() }, xf);
}

fn triangle(p: &Painter, r: Rect, st: &str, look: &Look, xf: &Xf) {
    let dir = style::get(st, "direction").unwrap_or("east");
    let pts = match dir {
        "north" => vec![xf.map(r, 0.5, 0.0), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0)],
        "south" => vec![xf.map(r, 0.0, 0.0), xf.map(r, 1.0, 0.0), xf.map(r, 0.5, 1.0)],
        "west" => vec![xf.map(r, 0.0, 0.5), xf.map(r, 1.0, 0.0), xf.map(r, 1.0, 1.0)],
        _ => vec![xf.map(r, 0.0, 0.0), xf.map(r, 1.0, 0.5), xf.map(r, 0.0, 1.0)],
    };
    poly(p, pts, look);
}

fn line_shape(p: &Painter, r: Rect, st: &str, look: &Look) {
    let vertical = style::get(st, "direction").map(|d| d == "north" || d == "south").unwrap_or(false);
    let (a, b) = if vertical {
        (Pos2::new(r.center().x, r.top()), Pos2::new(r.center().x, r.bottom()))
    } else {
        (Pos2::new(r.left(), r.center().y), Pos2::new(r.right(), r.center().y))
    };
    stroke_path(p, &[a, b], look, false);
}

fn cube(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let d = (r.width().min(r.height()) * 0.22).min(30.0 * look.scale);
    let (lt, rt, rb, lb) = (r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom());
    let off = Vec2::new(d * 0.6, -d * 0.6);
    let top = vec![lt, lt + off, rt + off, rt];
    let side = vec![rt, rt + off, rb + off, rb];
    // main face
    poly(p, rect_pts(r, xf), look);
    if let Some(fill) = look.fill {
        p.add(Shape::convex_polygon(top.clone(), style::darken(fill, 0.12), Stroke::NONE));
        p.add(Shape::convex_polygon(side.clone(), style::darken(fill, 0.22), Stroke::NONE));
    }
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[lt, lt + off, rt + off, rt + off, rb + off], &dim, false);
    stroke_path(p, &top, &dim, true);
    stroke_path(p, &side, &dim, true);
    let _ = (lb,);
}

fn cylinder(p: &Painter, r: Rect, look: &Look, _xf: &Xf) {
    let ry = (r.height() * 0.14).min(r.width() / 2.0);
    let c = r.center();
    let n = 24;
    let wall_pts = || -> Vec<Pos2> {
        let mut pts = vec![
            Pos2::new(r.left(), r.top() + ry),
            Pos2::new(r.left(), r.bottom() - ry),
        ];
        for i in 0..=n {
            let a = std::f32::consts::PI + i as f32 / n as f32 * std::f32::consts::PI;
            pts.push(Pos2::new(c.x + a.cos() * r.width() / 2.0, r.bottom() - ry + a.sin().abs() * ry / 2.0));
        }
        pts.push(Pos2::new(r.right(), r.top() + ry));
        pts
    };
    if let Some(fill) = look.fill {
        p.add(Shape::Path(PathShape { points: wall_pts(), closed: true, fill, stroke: Stroke::NONE.into() }));
    }
    stroke_path(p, &[Pos2::new(r.left(), r.top() + ry), Pos2::new(r.left(), r.bottom() - ry)], look, false);
    stroke_path(p, &[Pos2::new(r.right(), r.top() + ry), Pos2::new(r.right(), r.bottom() - ry)], look, false);
    // bottom arc
    let bottom: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = std::f32::consts::PI + i as f32 / n as f32 * std::f32::consts::PI;
            Pos2::new(c.x + a.cos() * r.width() / 2.0, r.bottom() - ry + a.sin().abs() * ry / 2.0)
        })
        .collect();
    stroke_path(p, &bottom, look, false);
    // top ellipse
    let top: Vec<Pos2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            Pos2::new(c.x + a.cos() * r.width() / 2.0, r.top() + ry + a.sin() * ry / 2.0)
        })
        .collect();
    poly(p, top, &Look { fill: look.fill, ..look.clone() });
}

fn cone(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let ry = r.height() * 0.14;
    let apex = xf.map(r, 0.5, 0.0);
    let mut pts: Vec<Pos2> = (0..=20)
        .map(|i| {
            let a = std::f32::consts::PI + i as f32 / 20.0 * std::f32::consts::PI;
            Pos2::new(r.center().x + a.cos() * r.width() / 2.0, r.bottom() - ry + a.sin().abs() * ry / 2.0)
        })
        .collect();
    pts.insert(0, apex);
    poly(p, pts, look);
    let base: Vec<Pos2> = (0..=20)
        .map(|i| {
            let a = i as f32 / 20.0 * std::f32::consts::TAU;
            Pos2::new(r.center().x + a.cos() * r.width() / 2.0, r.bottom() - ry + a.sin() * ry / 2.0)
        })
        .collect();
    stroke_path(p, &base, &Look { fill: None, ..look.clone() }, false);
}

fn star(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 5;
    let (ro, ri) = (0.5, 0.21);
    let mut pts = Vec::new();
    for i in 0..n * 2 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / (n * 2) as f32 * std::f32::consts::TAU;
        let rad = if i % 2 == 0 { ro } else { ri };
        pts.push(xf.map(r, 0.5 + a.cos() * rad, 0.5 + a.sin() * rad));
    }
    poly(p, pts, look);
}

fn heart(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let mut pts = Vec::new();
    let n = 40;
    for i in 0..=n {
        let t = i as f32 / n as f32 * std::f32::consts::TAU;
        let x = 16.0 * t.sin().powi(3);
        let y = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());
        let u = 0.5 + x / 34.0;
        let v = y / 32.0;
        let u = if xf.fh { 1.0 - u } else { u };
        let v = if xf.fv { 1.0 - v } else { v };
        pts.push(xf.map(r, u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)));
    }
    poly(p, pts, look);
}

fn lightning(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(
        p,
        vec![
            xf.map(r, 0.55, 0.0),
            xf.map(r, 1.0, 0.0),
            xf.map(r, 0.62, 0.38),
            xf.map(r, 0.9, 0.38),
            xf.map(r, 0.35, 1.0),
            xf.map(r, 0.52, 0.55),
            xf.map(r, 0.2, 0.55),
        ],
        look,
    );
}

fn cloud_blobs(r: Rect) -> Vec<(Pos2, f32)> {
    let w = r.width();
    let h = r.height();
    vec![
        (Pos2::new(r.left() + w * 0.28, r.top() + h * 0.55), h * 0.34),
        (Pos2::new(r.left() + w * 0.5, r.top() + h * 0.38), h * 0.42),
        (Pos2::new(r.left() + w * 0.72, r.top() + h * 0.55), h * 0.33),
        (Pos2::new(r.left() + w * 0.6, r.top() + h * 0.72), h * 0.3),
        (Pos2::new(r.left() + w * 0.38, r.top() + h * 0.74), h * 0.28),
    ]
}

fn cloud(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    // fill the blob cluster, then stroke the union silhouette approximated
    // by two arcs + base line
    if let Some(fill) = look.fill {
        for (c, rad) in cloud_blobs(r) {
            p.circle_filled(rotate(c, r.center(), xf.rot), rad, fill);
        }
    }
    let dim = Look { fill: None, ..look.clone() };
    let w = r.width();
    let h = r.height();
    let mut pts: Vec<Pos2> = Vec::new();
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        pts.push(xf.map(r, 0.18 + t * 0.3, 0.12 + (1.0 - (t * std::f32::consts::PI).sin().abs()) * 0.1));
    }
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        pts.push(xf.map(r, 0.48 + t * 0.34, 0.06 + (1.0 - (t * std::f32::consts::PI).sin().abs()) * 0.14));
    }
    pts.push(xf.map(r, 0.96, 0.62));
    pts.push(xf.map(r, 0.8, 0.9));
    pts.push(xf.map(r, 0.2, 0.9));
    pts.push(xf.map(r, 0.04, 0.62));
    stroke_path(p, &pts, &dim, false);
    let _ = (w, h);
}

fn speech(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let bubble = Rect::from_min_size(r.min, Vec2::new(r.width(), r.height() * 0.75));
    let pts = rect_pts(bubble, xf);
    poly(p, pts, look);
    let a = xf.map(r, 0.25, 0.75);
    let b = xf.map(r, 0.35, 0.75);
    let c = xf.map(r, 0.15, 1.0);
    if let Some(fill) = look.fill {
        p.add(Shape::convex_polygon(vec![a, b, c], fill, Stroke::NONE));
    }
    stroke_path(p, &[a, c, b], &Look { fill: None, ..look.clone() }, false);
}

fn note(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let fold = (r.width().min(r.height()) * 0.22).min(24.0 * look.scale);
    let pts = vec![
        xf.map(r, 0.0, 0.0),
        xf.map(r, 1.0, 0.0),
        xf.map(r, 1.0, 1.0),
        xf.map(r, 0.0, 1.0),
    ];
    // body without the folded corner: draw polygon with 5 points
    let fold_u = 1.0 - fold / r.width().max(1.0);
    let fold_v = fold / r.height().max(1.0);
    let body = vec![
        xf.map(r, 0.0, 0.0),
        xf.map(r, fold_u, 0.0),
        xf.map(r, 1.0, fold_v),
        xf.map(r, 1.0, 1.0),
        xf.map(r, 0.0, 1.0),
    ];
    if xf.straight() {
        if let Some(fill) = look.fill {
            p.add(Shape::convex_polygon(body.clone(), fill, Stroke::NONE));
        }
        stroke_path(p, &body, look, true);
        let a = xf.map(r, fold_u, 0.0);
        let b = xf.map(r, 1.0, fold_v);
        stroke_path(p, &[a, b], &Look { fill: None, ..look.clone() }, false);
    } else {
        poly(p, pts, look);
    }
}

fn brace(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let a = xf.map(r, 0.5, 0.0);
    let b = xf.map(r, 0.5, 1.0);
    let c = xf.map(r, 0.5, 0.5);
    let m1 = xf.map(r, 1.0, 0.5);
    let d = Look { fill: None, ..look.clone() };
    // curly approximation: outer arcs + middle nub
    stroke_path(
        p,
        &[a, Pos2::new(r.center().x + (m1.x - c.x) * 0.4, a.y + (c.y - a.y) * 0.4), c, m1, c, Pos2::new(c.x + (m1.x - c.x) * 0.4, b.y - (b.y - c.y) * 0.4), b],
        &d,
        false,
    );
}

fn cross(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let t = 0.3;
    poly(
        p,
        vec![
            xf.map(r, (1.0 - t) / 2.0, 0.0),
            xf.map(r, (1.0 + t) / 2.0, 0.0),
            xf.map(r, (1.0 + t) / 2.0, (1.0 - t) / 2.0),
            xf.map(r, 1.0, (1.0 - t) / 2.0),
            xf.map(r, 1.0, (1.0 + t) / 2.0),
            xf.map(r, (1.0 + t) / 2.0, (1.0 + t) / 2.0),
            xf.map(r, (1.0 + t) / 2.0, 1.0),
            xf.map(r, (1.0 - t) / 2.0, 1.0),
            xf.map(r, (1.0 - t) / 2.0, (1.0 + t) / 2.0),
            xf.map(r, 0.0, (1.0 + t) / 2.0),
            xf.map(r, 0.0, (1.0 - t) / 2.0),
            xf.map(r, (1.0 - t) / 2.0, (1.0 - t) / 2.0),
        ],
        look,
    );
}

fn ring(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    let inner = r.shrink(r.width().min(r.height()) * 0.18);
    ellipse(p, inner, xf, &Look { fill: None, ..look.clone() });
}

fn actor(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let dim = Look { fill: look.fill, ..look.clone() };
    let cx = r.center().x;
    let head_r = r.width().min(16.0 * look.scale) * 0.35;
    let head = Pos2::new(cx, r.top() + head_r + 2.0 * look.scale);
    p.circle_filled(head, head_r, look.fill.unwrap_or(Color32::WHITE));
    p.circle_stroke(head, head_r, look.stroke_w());
    let neck = Pos2::new(cx, head.y + head_r);
    let hip = Pos2::new(cx, r.top() + r.height() * 0.55);
    stroke_path(p, &[neck, hip], &dim, false);
    stroke_path(p, &[Pos2::new(cx - r.width() / 2.0, r.top() + r.height() * 0.32), Pos2::new(cx + r.width() / 2.0, r.top() + r.height() * 0.32)], &dim, false);
    stroke_path(p, &[hip, Pos2::new(cx - r.width() * 0.4, r.bottom())], &dim, false);
    stroke_path(p, &[hip, Pos2::new(cx + r.width() * 0.4, r.bottom())], &dim, false);
}

fn lifeline(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    // head box + dashed stem
    let head = Rect::from_min_size(r.min, Vec2::new(r.width(), 34.0 * look.scale));
    plain_rect(p, head, "rounded=0;whiteSpace=wrap;", look, xf);
    label(p, head, value, "whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let cx = r.center().x;
    stroke_path(p, &[Pos2::new(cx, head.bottom()), Pos2::new(cx, r.bottom())], &Look { dashed: true, dash: vec![4.0, 3.0], ..dim }, false);
}

fn uml_frame(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let tab_w = 90.0 * look.scale;
    let tab_h = 20.0 * look.scale;
    let tab = Rect::from_min_size(r.min, Vec2::new(tab_w, tab_h));
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(tab.right(), tab.top()), Pos2::new(tab.right(), tab.bottom()), Pos2::new(tab.left(), tab.bottom())], &dim, false);
    if !value.is_empty() {
        let font = FontId::proportional(11.0 * look.scale.max(0.5));
        p.text(Pos2::new(r.left() + 6.0, r.top() + 4.0), Align2::LEFT_TOP, value, font, look.text);
    }
}

fn component(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let s = (r.height() * 0.28).min(14.0 * look.scale);
    let dim = Look { fill: None, ..look.clone() };
    // two tabs on the left edge
    for i in 0..2 {
        let y = r.top() + r.height() * (0.25 + 0.5 * i as f32);
        let tab_r = Rect::from_min_size(Pos2::new(r.left(), y - s / 2.0), Vec2::new(s, s));
        stroke_path(
            p,
            &[
                Pos2::new(tab_r.left(), tab_r.top()),
                Pos2::new(tab_r.right(), tab_r.top()),
                Pos2::new(tab_r.right(), tab_r.bottom()),
                Pos2::new(tab_r.left(), tab_r.bottom()),
            ],
            &dim,
            true,
        );
        stroke_path(p, &[Pos2::new(tab_r.right(), tab_r.center().y), Pos2::new(tab_r.right() + s, tab_r.center().y)], &dim, false);
    }
}

fn package(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let tab_h = (r.height() * 0.3).min(20.0 * look.scale);
    let tab_w = r.width() * 0.4;
    let tab = Rect::from_min_size(r.min, Vec2::new(tab_w, tab_h));
    if let Some(fill) = look.fill {
        p.rect_filled(tab, 0.0, fill);
    }
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[tab.left_top(), tab.right_top(), tab.right_bottom(), tab.left_bottom(), tab.left_top()], &dim, true);
    let body = Rect::from_min_size(Pos2::new(r.left(), r.top() + tab_h), Vec2::new(r.width(), r.height() - tab_h));
    plain_rect(p, body, "rounded=0;whiteSpace=wrap;", look, xf);
    let _ = xf;
}

fn weak_entity(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    plain_rect(p, r.shrink(3.0 * look.scale), "rounded=0;whiteSpace=wrap;", &Look { fill: None, ..look.clone() }, xf);
}

fn double_ellipse(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    ellipse(p, r.shrink(3.0 * look.scale), xf, &Look { fill: None, ..look.clone() });
}

fn parallelogram(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let sk = r.width() * 0.2;
    let su = (sk / r.width().max(1.0)).min(0.4);
    poly(
        p,
        vec![xf.map(r, su, 0.0), xf.map(r, 1.0, 0.0), xf.map(r, 1.0 - su, 1.0), xf.map(r, 0.0, 1.0)],
        look,
    );
}

fn trapezoid(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let su = 0.2;
    poly(p, vec![xf.map(r, su, 0.0), xf.map(r, 1.0 - su, 0.0), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0)], look);
}

fn hexagon(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let su = (r.height() / r.width().max(1.0) * 0.5).min(0.3);
    poly(
        p,
        vec![xf.map(r, su, 0.0), xf.map(r, 1.0 - su, 0.0), xf.map(r, 1.0, 0.5), xf.map(r, 1.0 - su, 1.0), xf.map(r, su, 1.0), xf.map(r, 0.0, 0.5)],
        look,
    );
}

fn document(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let wave = r.height() * 0.15;
    let mut pts = vec![xf.map(r, 0.0, 0.0), xf.map(r, 1.0, 0.0), xf.map(r, 1.0, 1.0 - wave / r.height().max(1.0))];
    let n = 14;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let w = wave * (1.0 - (t * std::f32::consts::PI * 2.0).sin().abs() * 0.9) / r.height().max(1.0);
        pts.push(xf.map(r, u, 1.0 - w));
    }
    poly(p, pts, look);
}

fn multi_document(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let off = (r.width().min(r.height()) * 0.1).min(12.0 * look.scale);
    for i in (0..3).rev() {
        let shift = Vec2::new(off * i as f32, -off * i as f32);
        let rr = Rect::from_min_size(r.min + shift, Vec2::new(r.width() - off * 2.0, r.height() - off * 2.0));
        let dim = if i == 0 { look.clone() } else { Look { fill: None, ..look.clone() } };
        document(p, rr, &dim, xf);
    }
}

fn manual_input(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(p, vec![xf.map(r, 0.0, 0.25), xf.map(r, 1.0, 0.0), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0)], look);
}

fn delay(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 20;
    let mut pts = vec![xf.map(r, 0.0, 0.0), xf.map(r, 0.5, 0.0)];
    for i in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::PI;
        let u = 0.5 + a.cos() * 0.5;
        let v = 0.5 + a.sin() * 0.5;
        pts.push(xf.map(r, u, v));
    }
    pts.push(xf.map(r, 0.0, 1.0));
    poly(p, pts, look);
}

fn display(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 20;
    let mut pts = vec![xf.map(r, 0.2, 0.0), xf.map(r, 0.85, 0.0)];
    for i in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::PI;
        let u = 0.85 + a.cos() * 0.15;
        let v = 0.5 + a.sin() * 0.5;
        pts.push(xf.map(r, u, v));
    }
    pts.push(xf.map(r, 0.2, 1.0));
    pts.push(xf.map(r, 0.0, 0.5));
    poly(p, pts, look);
}

fn internal_storage(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let y1 = r.top() + r.height() * 0.25;
    let y2 = r.top() + r.height() * 0.75;
    stroke_path(p, &[Pos2::new(r.left(), y1), Pos2::new(r.right(), y1)], &dim, false);
    stroke_path(p, &[Pos2::new(r.left(), y2), Pos2::new(r.right(), y2)], &dim, false);
}

fn data_storage(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 20;
    let mut pts = vec![xf.map(r, 0.0, 0.0)];
    for i in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::PI;
        let u = 1.0 - 0.3 * (1.0 + a.cos());
        let v = 0.5 + a.sin() * 0.5;
        pts.push(xf.map(r, u, v));
    }
    pts.push(xf.map(r, 0.0, 1.0));
    for i in 0..=n {
        let a = std::f32::consts::FRAC_PI_2 - i as f32 / n as f32 * std::f32::consts::PI;
        let u = 0.3 + 0.3 * (1.0 - a.cos());
        let v = 0.5 + a.sin() * 0.5;
        pts.push(xf.map(r, u, v));
    }
    poly(p, pts, look);
}

fn direct_data(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    cylinder(p, r, look, xf);
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(r.left(), r.center().y), Pos2::new(r.right(), r.center().y)], &dim, false);
}

fn loop_limit(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(p, vec![xf.map(r, 0.25, 0.0), xf.map(r, 0.75, 0.0), xf.map(r, 1.0, 0.3), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0), xf.map(r, 0.0, 0.3)], look);
}

fn sort_shape(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    parallelogram(p, r, look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let sk = 0.2;
    let y1 = r.top() + r.height() * 0.33;
    let y2 = r.top() + r.height() * 0.66;
    let u1 = sk * (1.0 - 0.33);
    let u2 = sk * (1.0 - 0.66);
    stroke_path(p, &[Pos2::new(r.left() + u1 * r.width(), y1), Pos2::new(r.right() - (sk - u1) * r.width(), y1)], &dim, false);
    stroke_path(p, &[Pos2::new(r.left() + u2 * r.width(), y2), Pos2::new(r.right() - (sk - u2) * r.width(), y2)], &dim, false);
}

fn collate(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let dim = Look { fill: None, ..look.clone() };
    let (a, b, c, d) = (xf.map(r, 0.0, 0.0), xf.map(r, 1.0, 0.0), xf.map(r, 1.0, 1.0), xf.map(r, 0.0, 1.0));
    let m = r.center();
    let rot = xf.rot;
    if let Some(fill) = look.fill {
        p.add(Shape::convex_polygon(vec![a, b, m], fill, Stroke::NONE));
        p.add(Shape::convex_polygon(vec![c, d, m], style::darken(fill, 0.15), Stroke::NONE));
    }
    stroke_path(p, &[a, m, c], &dim, false);
    stroke_path(p, &[b, m, d], &dim, false);
    let _ = rot;
}

fn predefined(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    plain_rect(p, r.shrink(4.0 * look.scale), "rounded=0;whiteSpace=wrap;", &Look { fill: None, ..look.clone() }, xf);
}

fn off_page(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(p, rect_pts(r, xf), look);
    let dim = Look { fill: None, ..look.clone() };
    let y = r.top() + r.height() * 0.75;
    stroke_path(p, &[r.left_top(), Pos2::new(r.center().x, y), r.right_top()], &dim, false);
}

fn summing_junction(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    let dim = Look { fill: None, ..look.clone() };
    let c = r.center();
    let k = r.width().min(r.height()) * 0.3;
    stroke_path(p, &[c - Vec2::new(k, k), c + Vec2::new(k, k)], &dim, false);
    stroke_path(p, &[c + Vec2::new(k, -k), c - Vec2::new(k, -k)], &dim, false);
}

fn rounded_rect(p: &Painter, r: Rect, st: &str, look: &Look, xf: &Xf) {
    plain_rect(p, r, &format!("{st}rounded=1;"), look, xf);
}

fn single_arrow(p: &Painter, r: Rect, st: &str, look: &Look, xf: &Xf) {
    let dir = style::get(st, "direction").unwrap_or("east");
    let pts = |flip: bool| {
        let pts: Vec<(f32, f32)> = if flip {
            vec![(0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.8), (0.0, 0.8), (0.0, 0.2), (0.6, 0.2)]
        } else {
            vec![(0.0, 0.0), (0.4, 0.0), (0.4, 0.2), (1.0, 0.2), (1.0, 0.8), (0.4, 0.8), (0.4, 1.0)]
        };
        // rotate point list for N/S/W orientations by re-mapping through a
        // swapped-axes rect
        pts.iter()
            .map(|&(u, v)| match dir {
                "west" => xf.map(r, 1.0 - u, v),
                "north" => xf.map(r, v, u),
                "south" => xf.map(r, v, 1.0 - u),
                _ => xf.map(r, u, v),
            })
            .collect::<Vec<_>>()
    };
    poly(p, pts(dir == "west" || dir == "south"), look);
}

fn double_arrow(p: &Painter, r: Rect, st: &str, look: &Look, xf: &Xf) {
    let dir = style::get(st, "direction").unwrap_or("east");
    let base = vec![(0.35, 0.0), (0.5, 0.0), (0.5, 0.2), (1.0, 0.2), (1.0, 0.8), (0.5, 0.8), (0.5, 1.0), (0.35, 1.0), (0.15, 0.5)];
    let pts: Vec<Pos2> = base
        .iter()
        .map(|&(u, v)| match dir {
            "west" => xf.map(r, 1.0 - u, v),
            "north" => xf.map(r, v, u),
            "south" => xf.map(r, v, 1.0 - u),
            _ => xf.map(r, u, v),
        })
        .collect();
    poly(p, pts, look);
}

fn striped_arrow(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    poly(
        p,
        vec![xf.map(r, 0.3, 0.0), xf.map(r, 0.3, 0.25), xf.map(r, 0.0, 0.25), xf.map(r, 0.0, 0.75), xf.map(r, 0.3, 0.75), xf.map(r, 0.3, 1.0), xf.map(r, 1.0, 0.5)],
        look,
    );
}

fn tape(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 16;
    let mut pts = vec![xf.map(r, 0.0, 0.35), xf.map(r, 0.75, 0.35)];
    for i in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::PI;
        let u = 0.75 + a.cos() * 0.25;
        let v = 0.5 + a.sin() * 0.35;
        pts.push(xf.map(r, u, v));
    }
    pts.push(xf.map(r, 0.0, 0.65));
    poly(p, pts, look);
}

fn rect_callout(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let body = Rect::from_min_size(r.min, Vec2::new(r.width(), r.height() * 0.72));
    poly(p, rect_pts(body, xf), look);
    let a = xf.map(r, 0.35, 0.72);
    let b = xf.map(r, 0.5, 0.72);
    let c = xf.map(r, 0.55, 1.0);
    let d = xf.map(r, 0.6, 0.72);
    if let Some(fill) = look.fill {
        p.add(Shape::convex_polygon(vec![a, b, c, d], fill, Stroke::NONE));
    }
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[a, c, d], &dim, false);
}

// ── bpmn ────────────────────────────────────────────────────────────────────

fn event_circle(p: &Painter, r: Rect, look: &Look, xf: &Xf, width: f32) {
    let mut lk = look.clone();
    lk.width = width;
    ellipse(p, r, xf, &lk);
}

fn event_inter(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    ellipse(p, r.shrink(3.0 * look.scale), xf, &Look { fill: None, ..look.clone() });
}

fn event_glyph(p: &Painter, r: Rect, look: &Look, xf: &Xf, g: Glyph) {
    ellipse(p, r, xf, look);
    glyph(p, r.center(), r.width().min(r.height()) * 0.8, look, g);
}

fn task_icon(p: &Painter, r: Rect, look: &Look, xf: &Xf, g: Glyph) {
    let rounded = true;
    let st = if rounded { "rounded=1;whiteSpace=wrap;" } else { "rounded=0;whiteSpace=wrap;" };
    plain_rect(p, r, st, look, xf);
    let s = r.height().min(r.width()) * 0.45;
    let c = Pos2::new(r.right() - s * 0.9, r.bottom() - s * 0.9);
    glyph(p, c, s, look, g);
}

fn subprocess(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=1;whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let c = Pos2::new(r.center().x, r.bottom() - 9.0 * look.scale);
    let k = 5.0 * look.scale;
    stroke_path(p, &[c - Vec2::new(k, 0.0), c + Vec2::new(k, 0.0)], &dim, false);
    stroke_path(p, &[c - Vec2::new(0.0, k), c + Vec2::new(0.0, k)], &dim, false);
}

fn gateway(p: &Painter, r: Rect, look: &Look, xf: &Xf, g: Glyph) {
    diamond(p, r, look, xf);
    glyph(p, r.center(), r.width().min(r.height()) * 0.9, &Look { fill: None, ..look.clone() }, g);
}

/// Swimlane / pool / lane: outer frame + title band on the leading edge
/// (`horizontal=0` = vertical band on the left, like real drawio).
fn swimlane(p: &Painter, r: Rect, st: &str, look: &Look, value: &str) {
    let vertical = style::get(st, "horizontal").map(|h| h == "0").unwrap_or(true);
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, &Xf::default());
    let band = if vertical {
        Rect::from_min_size(r.min, Vec2::new(20.0 * look.scale, r.height()))
    } else {
        Rect::from_min_size(r.min, Vec2::new(r.width(), 20.0 * look.scale))
    };
    if let Some(fill) = look.fill {
        p.rect_filled(band, 0.0, style::darken(fill, 0.08));
    }
    let dim = Look { fill: None, ..look.clone() };
    if vertical {
        stroke_path(p, &[Pos2::new(band.right(), band.top()), Pos2::new(band.right(), band.bottom())], &dim, false);
        vtext(p, band, value, look);
    } else {
        stroke_path(p, &[Pos2::new(band.left(), band.bottom()), Pos2::new(band.right(), band.bottom())], &dim, false);
        label(p, band, value, "whiteSpace=wrap;verticalAlign=middle;fontStyle=1;", look, &Xf::default());
    }
    let _ = st;
}

// ── software / networks ─────────────────────────────────────────────────────

fn server(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    for i in 1..3 {
        let y = r.top() + r.height() * i as f32 / 3.0;
        stroke_path(p, &[Pos2::new(r.left(), y), Pos2::new(r.right(), y)], &dim, false);
    }
    let dot = Pos2::new(r.left() + 6.0 * look.scale, r.top() + r.height() / 6.0);
    p.circle_filled(dot, 2.5 * look.scale, look.stroke);
}

fn storage(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    cylinder(p, r, look, xf);
}

fn monitor(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let screen = Rect::from_min_size(r.min, Vec2::new(r.width(), r.height() * 0.72));
    plain_rect(p, screen, "rounded=0;whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let stand_w = r.width() * 0.3;
    stroke_path(p, &[Pos2::new(r.center().x, screen.bottom()), Pos2::new(r.center().x, r.bottom() - 3.0)], &dim, false);
    stroke_path(p, &[Pos2::new(r.center().x - stand_w / 2.0, r.bottom()), Pos2::new(r.center().x + stand_w / 2.0, r.bottom())], &dim, false);
}

fn laptop(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let screen = Rect::from_min_size(Pos2::new(r.left() + r.width() * 0.15, r.top()), Vec2::new(r.width() * 0.7, r.height() * 0.62));
    plain_rect(p, screen, "rounded=0;whiteSpace=wrap;", look, xf);
    let base = Rect::from_min_size(r.left_top(), Vec2::new(r.width(), r.height() * 0.18)).translate(Vec2::new(0.0, r.height() * 0.72));
    poly(p, rect_pts(base, xf), &Look { fill: look.fill.map(|f| style::darken(f, 0.1)), ..look.clone() });
}

fn phone(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=1;arcSize=20;whiteSpace=wrap;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let sp = Rect::from_center_size(Pos2::new(r.center().x, r.top() + 6.0 * look.scale), Vec2::new(r.width() * 0.4, 2.5 * look.scale));
    stroke_path(p, &[sp.left_center(), sp.right_center()], &dim, false);
    p.circle_stroke(Pos2::new(r.center().x, r.bottom() - 7.0 * look.scale), 3.5 * look.scale, dim.stroke_w());
}

fn tablet(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=1;arcSize=12;whiteSpace=wrap;", look, xf);
    p.circle_stroke(Pos2::new(r.center().x, r.bottom() - 7.0 * look.scale), 3.5 * look.scale, Look { fill: None, ..look.clone() }.stroke_w());
}

fn queue(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let n = 16;
    let mut pts = vec![xf.map(r, 0.0, 0.0), xf.map(r, 0.65, 0.0)];
    for i in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::PI;
        pts.push(xf.map(r, 0.65 + a.cos() * 0.35, 0.5 + a.sin() * 0.5));
    }
    pts.push(xf.map(r, 0.0, 1.0));
    poly(p, pts, look);
}

fn firewall(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    // brick pattern
    let dim = Look { fill: None, ..look.clone() };
    let rows = 3;
    for i in 1..rows {
        let y = r.top() + r.height() * i as f32 / rows as f32;
        stroke_path(p, &[Pos2::new(r.left(), y), Pos2::new(r.right(), y)], &dim, false);
    }
    for i in 0..rows {
        let y0 = r.top() + r.height() * i as f32 / rows as f32;
        let y1 = r.top() + r.height() * (i + 1) as f32 / rows as f32;
        let off = if i % 2 == 0 { 0.25 } else { 0.75 };
        stroke_path(p, &[Pos2::new(r.left() + r.width() * off, y0), Pos2::new(r.left() + r.width() * off, y1)], &dim, false);
    }
}

fn router(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    let c = r.center();
    let dim = Look { fill: None, ..look.clone() };
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0)] {
        stroke_path(p, &[c + Vec2::new(dx * r.width() * 0.25, dy * r.height() * 0.15), c + Vec2::new(dx * r.width() * 0.4, dy * r.height() * 0.15)], &dim, false);
    }
    arrow_glyph(p, Pos2::new(c.x + r.width() * 0.12, c.y - r.height() * 0.15), Vec2::new(1.0, 0.0), look.stroke, look.scale.max(0.5), false);
    arrow_glyph(p, Pos2::new(c.x - r.width() * 0.12, c.y + r.height() * 0.15), Vec2::new(-1.0, 0.0), look.stroke, look.scale.max(0.5), false);
}

fn switch_dev(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let c = r.center();
    arrow_glyph(p, Pos2::new(c.x - r.width() * 0.15, c.y), Vec2::new(1.0, 0.0), look.stroke, look.scale.max(0.5), false);
    arrow_glyph(p, Pos2::new(c.x + r.width() * 0.15, c.y), Vec2::new(1.0, 0.0), look.stroke, look.scale.max(0.5), false);
}

fn wifi(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let dim = Look { fill: None, ..look.clone() };
    let c = Pos2::new(r.center().x, r.bottom() - r.height() * 0.15);
    for i in 1..=3 {
        let rad = r.width() * 0.5 * i as f32 / 3.0;
        let n = 12;
        let pts: Vec<Pos2> = (0..=n)
            .map(|j| {
                let a = -std::f32::consts::PI * 0.75 + j as f32 / n as f32 * std::f32::consts::PI * 0.5;
                Pos2::new(c.x + a.cos() * rad, c.y + a.sin() * rad)
            })
            .collect();
        stroke_path(p, &pts, &dim, false);
    }
    p.circle_filled(c, 2.5 * look.scale, look.stroke);
    let _ = xf;
}

fn printer(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let body = Rect::from_min_size(Pos2::new(r.left(), r.top() + r.height() * 0.3), Vec2::new(r.width(), r.height() * 0.45));
    plain_rect(p, body, "rounded=0;whiteSpace=wrap;", look, xf);
    let paper = Rect::from_min_size(Pos2::new(r.left() + r.width() * 0.2, r.top()), Vec2::new(r.width() * 0.6, r.height() * 0.35));
    poly(p, rect_pts(paper, xf), &Look { fill: Some(Color32::WHITE), ..look.clone() });
    let out = Rect::from_min_size(Pos2::new(r.left() + r.width() * 0.25, r.bottom() - r.height() * 0.18), Vec2::new(r.width() * 0.5, r.height() * 0.18));
    poly(p, rect_pts(out, xf), &Look { fill: Some(Color32::WHITE), ..look.clone() });
}

// ── mockups / tables ────────────────────────────────────────────────────────

fn browser(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let bar_h = 22.0 * look.scale;
    let bar = Rect::from_min_size(r.min, Vec2::new(r.width(), bar_h));
    if let Some(fill) = look.fill {
        p.rect_filled(bar, 0.0, style::darken(fill, 0.12));
    }
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(r.left(), bar.bottom()), Pos2::new(r.right(), bar.bottom())], &dim, false);
    // url field + 3 window dots
    let url = Rect::from_min_size(Pos2::new(r.left() + 30.0 * look.scale, r.top() + 4.0 * look.scale), Vec2::new(r.width() - 38.0 * look.scale, bar_h - 8.0 * look.scale));
    if xf.straight() {
        p.rect_stroke(url, 3.0, dim.stroke_w(), egui::StrokeKind::Inside);
    }
    for i in 0..3 {
        p.circle_filled(Pos2::new(r.left() + 8.0 * look.scale + i as f32 * 7.0 * look.scale, r.top() + bar_h / 2.0), 2.2 * look.scale, look.stroke);
    }
    label(p, Rect::from_min_size(Pos2::new(r.left(), bar.bottom()), Vec2::new(r.width(), r.height() - bar_h)), value, "whiteSpace=wrap;verticalAlign=top;", look, xf);
}

fn dialog(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;", look, xf);
    let bar_h = 20.0 * look.scale;
    let bar = Rect::from_min_size(r.min, Vec2::new(r.width(), bar_h));
    if let Some(fill) = look.fill {
        p.rect_filled(bar, 0.0, style::darken(fill, 0.15));
    }
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(r.left(), bar.bottom()), Pos2::new(r.right(), bar.bottom())], &dim, false);
    label(p, bar, value, "whiteSpace=wrap;verticalAlign=middle;align=left;", look, xf);
}

fn list_item(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;align=left;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(r.left() + 14.0 * look.scale, r.center().y), Pos2::new(r.left() + 22.0 * look.scale, r.center().y)], &dim, false);
    let _ = value;
}

fn table_grid(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    let rows = value.lines().count().max(3).min(8) as usize;
    let cols = 3;
    let head_h = r.height() / rows.max(1) as f32;
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;verticalAlign=top;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    if let Some(fill) = look.fill {
        let head = Rect::from_min_size(r.min, Vec2::new(r.width(), head_h));
        p.rect_filled(head, 0.0, style::darken(fill, 0.15));
    }
    for i in 1..rows {
        let y = r.top() + head_h * i as f32;
        stroke_path(p, &[Pos2::new(r.left(), y), Pos2::new(r.right(), y)], &dim, false);
    }
    for i in 1..cols {
        let x = r.left() + r.width() * i as f32 / cols as f32;
        stroke_path(p, &[Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], &dim, false);
    }
    label(p, r, value, "whiteSpace=wrap;verticalAlign=top;", look, xf);
}

/// Shared "title band + rows separated by lines" painter for class boxes and
/// table shapes. `value` line 1 = title, remaining lines = rows.
fn stacked_table(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str, title_st: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;verticalAlign=top;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    let lines: Vec<&str> = value.lines().collect();
    let band_h = (18.0 * look.scale).max(r.height() / (lines.len().max(3) + 1) as f32);
    let band = Rect::from_min_size(r.min, Vec2::new(r.width(), band_h));
    if let Some(fill) = look.fill {
        p.rect_filled(band, 0.0, style::darken(fill, 0.18));
    }
    stroke_path(p, &[Pos2::new(r.left(), band.bottom()), Pos2::new(r.right(), band.bottom())], &dim, false);
    let title_look = Look { text: look.text, ..look.clone() };
    label(p, band, lines.first().copied().unwrap_or(""), title_st, &title_look, xf);
    let rest = lines.iter().skip(1).copied().collect::<Vec<_>>().join("\n");
    let body = Rect::from_min_size(Pos2::new(r.left(), band.bottom()), Vec2::new(r.width(), r.height() - band_h));
    if !rest.is_empty() {
        let n = rest.lines().count().max(1) as f32;
        let row_h = body.height() / n;
        let mut y = body.top();
        for line in rest.lines() {
            let row = Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), row_h));
            label(p, row, line, "whiteSpace=wrap;verticalAlign=middle;align=left;", look, xf);
            y += row_h;
            stroke_path(p, &[Pos2::new(body.left(), y), Pos2::new(body.right(), y)], &dim, false);
        }
    }
}

fn class_table(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    stacked_table(p, r, look, xf, value, "whiteSpace=wrap;verticalAlign=middle;fontStyle=1;");
}

fn entity_table(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    stacked_table(p, r, look, xf, value, "whiteSpace=wrap;verticalAlign=middle;fontStyle=1;");
}

fn title_bar(p: &Painter, r: Rect, look: &Look, xf: &Xf, value: &str) {
    plain_rect(p, r, "rounded=0;whiteSpace=wrap;align=left;", look, xf);
    let dim = Look { fill: None, ..look.clone() };
    stroke_path(p, &[Pos2::new(r.left() + 6.0 * look.scale, r.bottom() - 4.0 * look.scale), Pos2::new(r.right() - 6.0 * look.scale, r.bottom() - 4.0 * look.scale)], &dim, false);
    let _ = value;
}

// ── misc / signs ────────────────────────────────────────────────────────────

fn group_box(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let dim = Look { fill: None, dashed: true, dash: vec![5.0, 4.0], ..look.clone() };
    dashed_rect(p, r, &dim);
}

fn venn2(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let d = r.width().min(r.height());
    let cy = r.center().y;
    let a = Rect::from_center_size(Pos2::new(r.center().x - d * 0.22, cy), Vec2::new(d * 0.85, d * 0.85));
    let b = Rect::from_center_size(Pos2::new(r.center().x + d * 0.22, cy), Vec2::new(d * 0.85, d * 0.85));
    if let Some(fill) = look.fill {
        ellipse(p, a, xf, &Look { fill: Some(fill), ..look.clone() });
        ellipse(p, b, xf, &Look { fill: Some(style::darken(fill, 0.15)), ..look.clone() });
    } else {
        ellipse(p, a, xf, look);
        ellipse(p, b, xf, look);
    }
}

fn venn3(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    let d = r.width().min(r.height());
    let cs = [
        Pos2::new(r.center().x - d * 0.18, r.center().y - d * 0.18),
        Pos2::new(r.center().x + d * 0.18, r.center().y - d * 0.18),
        Pos2::new(r.center().x, r.center().y + d * 0.2),
    ];
    for (i, c) in cs.iter().enumerate() {
        let rr = Rect::from_center_size(*c, Vec2::new(d * 0.8, d * 0.8));
        let fill = look.fill.map(|f| if i == 0 { f } else { style::darken(f, 0.1 * i as f32) });
        ellipse(p, rr, xf, &Look { fill, ..look.clone() });
    }
}

fn warning(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    triangle(p, r, "direction=north;", look, xf);
    let font = FontId::proportional(r.height() * 0.4);
    p.text(r.center() + Vec2::new(0.0, r.height() * 0.15), Align2::CENTER_CENTER, "!", font, look.text);
}

fn forbidden(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    let dim = Look { fill: None, ..look.clone() };
    let c = r.center();
    let k = r.width().min(r.height()) * 0.32;
    stroke_path(p, &[c - Vec2::new(k, -k), c + Vec2::new(k, -k)], &dim, false);
}

fn info_sign(p: &Painter, r: Rect, look: &Look, mark: &str) {
    ellipse(p, r, &Xf::default(), look);
    let font = FontId::proportional(r.height() * 0.55);
    p.text(r.center(), Align2::CENTER_CENTER, mark, font, look.text);
}

/// Circle with a hand-drawn X — vector strokes instead of the tofu-prone
/// ✕ (U+2715) text glyph, which the installed fonts can't render.
fn cross_mark(p: &Painter, r: Rect, look: &Look) {
    ellipse(p, r, &Xf::default(), look);
    let k = r.width().min(r.height()) * 0.2;
    let st = Stroke::new((r.width().min(r.height()) * 0.08).max(1.6), look.text);
    let c = r.center();
    p.line_segment([c - Vec2::new(k, k), c + Vec2::new(k, k)], st);
    p.line_segment([c + Vec2::new(k, -k), c - Vec2::new(k, -k)], st);
}

fn first_aid(p: &Painter, r: Rect, look: &Look, xf: &Xf) {
    ellipse(p, r, xf, look);
    let c = r.center();
    let t = r.width().min(r.height()) * 0.18;
    let th = t * 0.66;
    let cross_pts = |horiz: bool| {
        vec![
            if horiz { c + Vec2::new(-t, -th) } else { c + Vec2::new(-th, -t) },
            if horiz { c + Vec2::new(t, -th) } else { c + Vec2::new(th, -t) },
            if horiz { c + Vec2::new(t, th) } else { c + Vec2::new(th, t) },
            if horiz { c + Vec2::new(-t, th) } else { c + Vec2::new(-th, t) },
        ]
    };
    if let Some(fill) = look.fill {
        p.add(Shape::convex_polygon(cross_pts(true), fill, Stroke::NONE));
        p.add(Shape::convex_polygon(cross_pts(false), fill, Stroke::NONE));
    }
}

// re-exported for canvas edge painting
pub use border_point as anchor_on_rect;
