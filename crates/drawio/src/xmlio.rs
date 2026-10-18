//! Lossless `.drawio` (mxGraph XML) reading and writing.
//!
//! Round-trips: pages, cells, mxGraph style strings, edge waypoints
//! (`Array as="points"`), floating endpoints (`sourcePoint`/`targetPoint`),
//! page setup (`pageWidth/pageHeight/background`), layers (written as
//! `parent="0"` container cells) and groups (mxGraph `parent` + child
//! coordinates stored relative to the group origin, like real drawio).

use crate::model::{Cell, Diagram, Geom, Layer, MxFile};

/// Parse an `.drawio` file (uncompressed mxGraph XML; the format drawio writes
/// when "URL auto-save"/compressed export is off, and what drawio stores on
/// disk by default). A bare `<mxGraphModel>` also parses (single page).
pub fn parse(bytes: &[u8]) -> Result<MxFile, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("bad XML: {e}"))?;
    let mut file = MxFile::default();
    for d in doc.descendants().filter(|n| n.has_tag_name("diagram")) {
        let mut diagram = Diagram::new(d.attribute("name").unwrap_or("Page-1"));
        if let Some(model) = d.children().find(|n| n.has_tag_name("mxGraphModel")) {
            diagram.page_w = model.attribute("pageWidth").and_then(|v| v.parse().ok()).unwrap_or(850.0);
            diagram.page_h = model.attribute("pageHeight").and_then(|v| v.parse().ok()).unwrap_or(1100.0);
            diagram.background = model.attribute("background").map(str::to_string);
            if let Some(root) = model.children().find(|n| n.has_tag_name("root")) {
                parse_root(root, &mut diagram);
            }
        }
        diagram.next_id = diagram
            .cells
            .iter()
            .filter_map(|c| c.id.trim_start_matches(['v', 'e', 'c']).parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        if diagram.layers.is_empty() {
            diagram.layers = vec![Layer { id: "1".into(), name: "Background".into(), visible: true, locked: false }];
        }
        absolutize_groups(&mut diagram);
        file.diagrams.push(diagram);
    }
    if file.diagrams.is_empty() {
        file.diagrams.push(Diagram::new("Page-1"));
    }
    Ok(file)
}

fn parse_root(root: roxmltree::Node<'_, '_>, diagram: &mut Diagram) {
    for mc in root.children().filter(|n| n.has_tag_name("mxCell")) {
        let id = mc.attribute("id").unwrap_or("").to_string();
        let parent = mc.attribute("parent").map(str::to_string);
        let vertex = mc.attribute("vertex").map(|v| v == "1").unwrap_or(false);
        let edge = mc.attribute("edge").map(|v| v == "1").unwrap_or(false);
        let style = mc.attribute("style").unwrap_or("").to_string();

        // mxGraph layers are container cells parented to the root (id "0");
        // id "1" is the implicit default layer we always have.
        if !vertex && !edge && (parent.as_deref() == Some("0")) && id != "0" && id != "1" && !id.is_empty() {
            let visible = mc.attribute("visible").map(|v| v != "0").unwrap_or(true);
            let locked = style.contains("locked=1");
            diagram.layers.push(Layer {
                id,
                name: {
                    let v = mc.attribute("value").unwrap_or("");
                    if v.is_empty() { format!("Layer {}", diagram.layers.len()) } else { v.to_string() }
                },
                visible,
                locked,
            });
            continue;
        }
        if id == "0" || id == "1" {
            continue; // root + default layer
        }

        let geom_node = mc.children().find(|n| n.has_tag_name("mxGeometry"));
        let geom = geom_node
            .map(|g| Geom {
                x: g.attribute("x").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                y: g.attribute("y").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                w: g.attribute("width").and_then(|v| v.parse().ok()).unwrap_or(80.0),
                h: g.attribute("height").and_then(|v| v.parse().ok()).unwrap_or(40.0),
                points: g
                    .children()
                    .filter(|n| n.has_tag_name("Array") || n.has_tag_name("array"))
                    .flat_map(|a| a.children())
                    .filter(|n| n.has_tag_name("mxPoint"))
                    .filter_map(|p| Some((p.attribute("x")?.parse().ok()?, p.attribute("y")?.parse().ok()?)))
                    .collect(),
                source_pt: point_of(g, "sourcePoint"),
                target_pt: point_of(g, "targetPoint"),
                relative: g.attribute("relative").map(|v| v == "1").unwrap_or(false),
            })
            .unwrap_or_default();
        let cell = Cell {
            id,
            value: mc.attribute("value").unwrap_or("").to_string(),
            style,
            vertex,
            edge,
            source: mc.attribute("source").map(str::to_string),
            target: mc.attribute("target").map(str::to_string),
            parent,
            layer: String::new(), // resolved below
            geom,
        };
        diagram.cells.push(cell);
    }
    // Resolve layer membership: a cell whose parent names a layer id.
    let layer_ids: Vec<String> = diagram.layers.iter().map(|l| l.id.clone()).collect();
    let default = diagram.default_layer_id();
    for c in &mut diagram.cells {
        c.layer = match &c.parent {
            Some(p) if layer_ids.contains(p) => p.clone(),
            _ => default.clone(),
        };
    }
}

fn point_of(g: roxmltree::Node<'_, '_>, as_attr: &str) -> Option<(f32, f32)> {
    let p = g.children().find(|n| n.has_tag_name("mxPoint") && n.attribute("as") == Some(as_attr))?;
    Some((p.attribute("x")?.parse().ok()?, p.attribute("y")?.parse().ok()?))
}

/// mxGraph stores group children with coordinates relative to the group
/// origin; the model keeps absolute coordinates, so add the origins back.
fn absolutize_groups(d: &mut Diagram) {
    for _ in 0..8 {
        let mut changed = false;
        let snapshot = d.cells.clone();
        for c in d.cells.iter_mut() {
            if let Some(pid) = c.parent.clone()
                && let Some(p) = snapshot.iter().find(|p| p.id == pid && p.is_group())
            {
                let (dx, dy) = (p.geom.x, p.geom.y);
                if (dx.abs() > f32::EPSILON || dy.abs() > f32::EPSILON) && c.vertex {
                    c.geom.x += dx;
                    c.geom.y += dy;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Cumulative shift from ancestor group origins (mxGraph children are
/// relative to their immediate parent, so nested groups add up).
fn parent_shift(d: &Diagram, id: &str) -> (f32, f32) {
    let mut dx = 0.0;
    let mut dy = 0.0;
    let mut cur = d.cell(id).and_then(|c| c.parent.clone());
    for _ in 0..8 {
        let Some(pid) = cur else { break };
        let Some(p) = d.cell(&pid) else { break };
        if p.is_group() {
            dx += p.geom.x;
            dy += p.geom.y;
        }
        cur = p.parent.clone();
    }
    (dx, dy)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ── SVG export (File › Export .svg) ─────────────────────────────────────────

/// Export one page as a plain SVG. Shape geometry is approximated with SVG
/// primitives (rect/ellipse/polygon/path); edge ends keep arrow markers.
pub fn write_svg(d: &Diagram) -> String {
    let (w, h) = (d.page_w.max(1.0), d.page_h.max(1.0));
    let bg = d.background.as_deref().and_then(|b| if b == "none" { None } else { Some(b.to_string()) });
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n\
         <defs><marker id=\"a\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\
         <path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"context-stroke\"/></marker></defs>\n"
    );
    s.push_str(&match &bg {
        Some(c) => format!("<rect width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>\n"),
        None => String::new(),
    });
    let vcol = |c: &Cell, k: &str, dflt: &str| crate::style::get(&c.style, k).unwrap_or(dflt).to_string();
    // edges first (like the canvas), then vertices in paint order
    for c in d.cells.iter().filter(|c| c.edge && d.layer_visible(c)) {
        let Some((_, _, pts)) = edge_svg_points(d, c) else { continue };
        let col = vcol(c, "strokeColor", "#000000");
        let sw = crate::style::get_f32(&c.style, "strokeWidth", 1.0);
        let dash = if crate::style::get_bool(&c.style, "dashed", false) { " stroke-dasharray=\"6 4\"" } else { "" };
        let path = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect::<Vec<_>>().join(" ");
        let marker = if crate::style::get(&c.style, "endArrow").map(|v| v != "none").unwrap_or(true) {
            " marker-end=\"url(#a)\""
        } else {
            ""
        };
        s.push_str(&format!("<polyline points=\"{path}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"{sw}\"{dash}{marker}/>\n"));
    }
    for c in d.cells.iter().filter(|c| c.vertex && d.layer_visible(c)) {
        let fill = vcol(c, "fillColor", "#ffffff");
        let fill = if fill == "none" { "none".to_string() } else { fill };
        let stroke = vcol(c, "strokeColor", "#000000");
        let sw = crate::style::get_f32(&c.style, "strokeWidth", 1.0);
        let dash = if crate::style::get_bool(&c.style, "dashed", false) { " stroke-dasharray=\"6 4\"" } else { "" };
        let g = &c.geom;
        let common = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"{dash}");
        let shape = crate::style::get(&c.style, "shape").unwrap_or("");
        let el = if shape == "umlActor" {
            String::new() // stick figure: approximate with a plain rect frame
        } else if c.style.contains("ellipse") && shape.is_empty() || shape == "doubleEllipse" {
            format!("<ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" {common}/>", g.x + g.w / 2.0, g.y + g.h / 2.0, g.w / 2.0, g.h / 2.0)
        } else if c.style.contains("rhombus") {
            format!(
                "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" {common}/>",
                g.x + g.w / 2.0, g.y, g.x + g.w, g.y + g.h / 2.0, g.x + g.w / 2.0, g.y + g.h, g.x, g.y + g.h / 2.0
            )
        } else if c.style.contains("triangle") {
            format!("<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" {common}/>", g.x + g.w / 2.0, g.y, g.x + g.w, g.y + g.h, g.x, g.y + g.h)
        } else {
            let rx = if crate::style::get_bool(&c.style, "rounded", false) { 10 } else { 0 };
            format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{rx}\" {common}/>", g.x, g.y, g.w, g.h)
        };
        if !el.is_empty() {
            s.push_str(&el);
            s.push('\n');
        }
        if !c.value.is_empty() {
            let fc = vcol(c, "fontColor", "#000000");
            let fs = crate::style::get_f32(&c.style, "fontSize", 12.0);
            let weight = if crate::style::font_style(&c.style) & 1 != 0 { " font-weight=\"bold\"" } else { "" };
            s.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Helvetica\" font-size=\"{fs}\"{weight} fill=\"{fc}\">{}</text>\n",
                g.x + g.w / 2.0,
                g.y + g.h / 2.0,
                esc(&c.value)
            ));
        }
    }
    s.push_str("</svg>\n");
    s
}

/// First and last routing point of an edge (world coords), if computable.
fn edge_ends(d: &Diagram, c: &Cell) -> (Option<(f32, f32)>, Option<(f32, f32)>) {
    let src = c.source.as_deref().and_then(|id| d.cell(id)).map(|v| (v.geom.x + v.geom.w / 2.0, v.geom.y + v.geom.h / 2.0));
    let tgt = c.target.as_deref().and_then(|id| d.cell(id)).map(|v| (v.geom.x + v.geom.w / 2.0, v.geom.y + v.geom.h / 2.0));
    (src.or(c.geom.source_pt), tgt.or(c.geom.target_pt))
}

/// Waypoint list including endpoints (world), or `None` without both ends.
fn edge_svg_points(d: &Diagram, c: &Cell) -> Option<(f32, f32, Vec<(f32, f32)>)> {
    let (src, tgt) = edge_ends(d, c);
    let mut pts = Vec::new();
    if let Some(p) = src {
        pts.push(p);
    }
    pts.extend(c.geom.points.iter().copied());
    if let Some(p) = tgt {
        pts.push(p);
    }
    if pts.len() < 2 {
        return None;
    }
    Some((pts[0].0, pts[0].1, pts))
}

/// Serialize one page to a standalone string (Extras › Edit Diagram…).
pub fn write_diagram_to_string(d: &Diagram) -> String {
    let mut out = String::new();
    write_diagram(&mut out, d);
    out
}

/// Serialize back to mxGraph XML drawio can open (round-trip safe).
pub fn write(file: &MxFile) -> String {
    let mut out = String::from(
        "<mxfile host=\"doccraft\" modified=\"2026-10-18T00:00:00.000Z\" agent=\"DocCraft\" version=\"24.7.17\">\n",
    );
    for d in &file.diagrams {
        write_diagram(&mut out, d);
    }
    out.push_str("</mxfile>\n");
    out
}

/// Serialize one page (used by Extras › Edit Diagram…, which edits a page).
pub fn write_diagram(out: &mut String, d: &Diagram) {
    out.push_str(&format!("  <diagram id=\"{}\" name=\"{}\">\n", esc(&d.name.replace(' ', "")), esc(&d.name)));
    out.push_str(&format!(
        "    <mxGraphModel dx=\"800\" dy=\"600\" grid=\"1\" gridSize=\"10\" guides=\"1\" tooltips=\"1\" connect=\"1\" arrows=\"1\" fold=\"1\" page=\"1\" pageScale=\"1\" math=\"0\" shadow=\"0\" pageWidth=\"{}\" pageHeight=\"{}\"{}>\n",
        d.page_w,
        d.page_h,
        match &d.background {
            Some(b) => format!(" background=\"{b}\""),
            None => String::new(),
        }
    ));
    out.push_str("      <root>\n");
    out.push_str("        <mxCell id=\"0\"/>\n");
    out.push_str("        <mxCell id=\"1\" parent=\"0\"/>\n");
    for l in &d.layers {
        if l.id == "1" {
            continue; // default layer is implicit (id "1")
        }
        let vis = if l.visible { "" } else { " visible=\"0\"" };
        out.push_str(&format!(
            "        <mxCell id=\"{}\" value=\"{}\" style=\"layer;locked={};\" parent=\"0\"{vis}/>\n",
            esc(&l.id),
            esc(&l.name),
            if l.locked { 1 } else { 0 },
        ));
    }
    for c in &d.cells {
        let mut attrs = format!("id=\"{}\"", esc(&c.id));
        if !c.value.is_empty() {
            attrs.push_str(&format!(" value=\"{}\"", esc(&c.value)));
        }
        if !c.style.is_empty() {
            attrs.push_str(&format!(" style=\"{}\"", esc(&c.style)));
        }
        // Parent: group membership first, else the owning layer.
        let parent = if let Some(p) = &c.parent {
            p.clone()
        } else if c.layer != "1" && d.layer_index(&c.layer).is_some() {
            c.layer.clone()
        } else {
            "1".to_string()
        };
        attrs.push_str(&format!(" parent=\"{}\"", esc(&parent)));
        if let (Some(s), Some(t)) = (&c.source, &c.target) {
            attrs.push_str(&format!(" edge=\"1\" source=\"{}\" target=\"{}\"", esc(s), esc(t)));
        } else if c.vertex {
            attrs.push_str(" vertex=\"1\"");
        } else if c.edge {
            attrs.push_str(" edge=\"1\"");
        }
        if c.vertex || c.edge {
            out.push_str(&format!("        <mxCell {attrs}>\n"));
            if c.vertex {
                let (dx, dy) = parent_shift(d, &c.id);
                out.push_str(&format!(
                    "          <mxGeometry x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" as=\"geometry\"/>\n",
                    c.geom.x - dx,
                    c.geom.y - dy,
                    c.geom.w,
                    c.geom.h
                ));
            } else {
                let has_pts = !c.geom.points.is_empty() || c.geom.source_pt.is_some() || c.geom.target_pt.is_some();
                if has_pts {
                    out.push_str("          <mxGeometry relative=\"1\" as=\"geometry\">\n");
                    if let Some((x, y)) = c.geom.source_pt {
                        out.push_str(&format!("            <mxPoint x=\"{x}\" y=\"{y}\" as=\"sourcePoint\"/>\n"));
                    }
                    if let Some((x, y)) = c.geom.target_pt {
                        out.push_str(&format!("            <mxPoint x=\"{x}\" y=\"{y}\" as=\"targetPoint\"/>\n"));
                    }
                    if !c.geom.points.is_empty() {
                        out.push_str("            <Array as=\"points\">\n");
                        for (x, y) in &c.geom.points {
                            out.push_str(&format!("              <mxPoint x=\"{x}\" y=\"{y}\"/>\n"));
                        }
                        out.push_str("            </Array>\n");
                    }
                    out.push_str("          </mxGeometry>\n");
                } else {
                    out.push_str("          <mxGeometry relative=\"1\" as=\"geometry\"/>\n");
                }
            }
            out.push_str("        </mxCell>\n");
        } else {
            out.push_str(&format!("        <mxCell {attrs}/>\n"));
        }
    }
    out.push_str("      </root>\n    </mxGraphModel>\n  </diagram>\n");
}
