//! Minimal `.svg` I/O for the Vector craft.
//!
//! The writer is hand-rolled string building (rect / ellipse / line /
//! polygon / path / polyline / text, with two-stop `linearGradient` defs).
//! The reader walks the tree with `roxmltree` and understands the same
//! subset: plain colors (`#rgb`, `#rrggbb`, `rgb(...)`, a few names,
//! `none`), `fill="url(#id)"` gradients, and path data with M/L/H/V/C/S/Q/T/Z
//! commands (arcs `A` are flattened to lines).

use crate::model::*;
use egui::{pos2, vec2, Color32, Pos2, Vec2};

// ---------------------------------------------------------------------------
// writer
// ---------------------------------------------------------------------------

fn col(c: Color32) -> String {
    if c.a() == 0 {
        return "none".into();
    }
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Number formatting without trailing zeros (keeps files compact).
fn n(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn paint_attrs(fill: &str, s: &Shape) -> String {
    let mut a = String::new();
    if fill != "none" && s.fill.a() != 255 {
        a.push_str(&format!(" fill-opacity=\"{}\"", (s.fill.a() as f32 / 255.0).round() / 100.0 * 100.0));
    }
    if s.stroke.a() == 0 || s.stroke_w <= 0.0 {
        a.push_str(" stroke=\"none\"");
    } else {
        a.push_str(&format!(" stroke=\"{}\" stroke-width=\"{}\"", col(s.stroke), n(s.stroke_w)));
    }
    if s.opacity < 0.999 {
        a.push_str(&format!(" opacity=\"{}\"", (s.opacity * 100.0).round() / 100.0));
    }
    a
}

/// Escape text for XML.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Serialize a document as a standalone `.svg` file.
pub fn write(doc: &Doc) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        n(doc.artboard.x),
        n(doc.artboard.y),
        n(doc.artboard.x),
        n(doc.artboard.y)
    ));
    // gradient defs
    let mut defs = String::new();
    for node in &doc.nodes {
        let Node::Shape(s) = node else { continue };
        if let Some((top, bot)) = s.gradient {
            let id = grad_id(s.id);
            defs.push_str(&format!(
                "  <linearGradient id=\"{}\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">\n    <stop offset=\"0\" stop-color=\"{}\"/>\n    <stop offset=\"1\" stop-color=\"{}\"/>\n  </linearGradient>\n",
                id,
                col(top),
                col(bot)
            ));
        }
    }
    if !defs.is_empty() {
        out.push_str(" <defs>\n");
        out.push_str(&defs);
        out.push_str(" </defs>\n");
    }
    for node in &doc.nodes {
        match node {
            Node::Shape(s) if !s.hidden => {
                let fill = match s.gradient {
                    Some(_) => format!("url(#{})", grad_id(s.id)),
                    None => col(s.fill),
                };
                match s.kind {
                    Kind::Rect => out.push_str(&format!(
                        " <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{}/>\n",
                        n(s.x), n(s.y), n(s.w), n(s.h), paint_attrs(&fill, s), stroke_dash(s)
                    )),
                    Kind::Ellipse => {
                        let (c, r) = (s.rect().center(), vec2(s.w / 2.0, s.h / 2.0));
                        out.push_str(&format!(
                            " <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{}{}/>\n",
                            n(c.x), n(c.y), n(r.x), n(r.y), paint_attrs(&fill, s), stroke_dash(s)
                        ));
                    }
                    Kind::Poly => {
                        let pts = poly_points(s.rect(), s.sides);
                        out.push_str(&format!(
                            " <polygon points=\"{}\"{} fill=\"{}\"{}/>\n",
                            pts.iter().map(|p| format!("{},{}", n(p.x), n(p.y))).collect::<Vec<_>>().join(" "),
                            paint_attrs(&fill, s),
                            fill,
                            stroke_dash(s)
                        ));
                    }
                    Kind::Line => {
                        let (a, b) = match (s.anchors.first(), s.anchors.get(1)) {
                            (Some(a), Some(b)) => (*a, *b),
                            _ => continue,
                        };
                        out.push_str(&format!(
                            " <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"{}/>\n",
                            n(a.p.x), n(a.p.y), n(b.p.x), n(b.p.y), col(s.stroke), n(s.stroke_w), stroke_dash(s)
                        ));
                    }
                    Kind::Path => {
                        let d = path_d(&s.anchors, s.closed);
                        out.push_str(&format!(
                            " <path d=\"{d}\"{} fill=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}/>\n",
                            paint_attrs(&fill, s),
                            if s.closed { fill } else { "none".to_string() },
                            stroke_dash(s)
                        ));
                    }
                }
            }
            Node::Stroke(st) if !st.hidden => {
                out.push_str(&format!(
                    " <polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" opacity=\"{}\"/>\n",
                    st.pts.iter().map(|p| format!("{},{}", n(p.x), n(p.y))).collect::<Vec<_>>().join(" "),
                    col(st.color),
                    n(st.width),
                    (st.opacity * 100.0).round() / 100.0
                ));
            }
            Node::Text(t) if !t.hidden => {
                out.push_str(&format!(
                    " <text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"{}\" fill=\"{}\" opacity=\"{}\">{}</text>\n",
                    n(t.x),
                    n(t.y + t.size),
                    n(t.size),
                    col(t.color),
                    (t.opacity * 100.0).round() / 100.0,
                    esc(&t.text)
                ));
            }
            _ => {}
        }
    }
    out.push_str("</svg>\n");
    out
}

fn stroke_dash(_s: &Shape) -> &'static str {
    ""
}

fn grad_id(id: u64) -> String {
    format!("vg{id}")
}

/// Path data from anchors: M + (C|L)* + optional Z.
fn path_d(anchors: &[Anchor], closed: bool) -> String {
    let mut d = String::new();
    let Some(a0) = anchors.first() else { return d };
    d.push_str(&format!("M {} {}", n(a0.p.x), n(a0.p.y)));
    let segs = if closed { anchors.len() } else { anchors.len() - 1 };
    for i in 0..segs {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % anchors.len()];
        if a.h_out == Vec2::ZERO && b.h_in == Vec2::ZERO {
            d.push_str(&format!(" L {} {}", n(b.p.x), n(b.p.y)));
        } else {
            let c1 = a.p + a.h_out;
            let c2 = b.p + b.h_in;
            d.push_str(&format!(" C {} {} {} {} {} {}", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(b.p.x), n(b.p.y)));
        }
    }
    if closed {
        d.push_str(" Z");
    }
    d
}

// ---------------------------------------------------------------------------
// reader
// ---------------------------------------------------------------------------

/// Parse an SVG into a document. Returns the doc plus a warning count for
/// skipped elements (transforms are ignored, arcs flattened).
pub fn parse(bytes: &[u8]) -> Result<(Doc, usize), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "not UTF-8".to_string())?;
    let tree = roxmltree::Document::parse(text).map_err(|e| format!("XML: {e}"))?;
    let root = tree.root_element();
    if root.tag_name().name() != "svg" {
        return Err("root element is not <svg>".into());
    }
    let (mut w, mut h) = (640.0f32, 480.0f32);
    if let Some(vb) = root.attribute("viewBox") {
        let nums: Vec<f32> = vb.split([' ', ',']).filter_map(|t| t.parse().ok()).collect();
        if nums.len() == 4 {
            w = nums[2];
            h = nums[3];
        }
    }
    if let Some(v) = root.attribute("width").and_then(|t| t.parse::<f32>().ok()) {
        w = v;
    }
    if let Some(v) = root.attribute("height").and_then(|t| t.parse::<f32>().ok()) {
        h = v;
    }
    let mut doc = Doc::new(w, h);
    let mut skipped = 0usize;

    // gradients first
    let mut grads: Vec<(String, (Color32, Color32))> = Vec::new();
    for g in tree.descendants().filter(|e| e.tag_name().name() == "linearGradient") {
        let Some(id) = g.attribute("id").map(str::to_string) else { continue };
        let stops: Vec<Color32> = g
            .children()
            .filter(|e| e.tag_name().name() == "stop")
            .filter_map(|e| e.attribute("stop-color").and_then(parse_color))
            .collect();
        if let (Some(a), Some(b)) = (stops.first(), stops.last()) {
            grads.push((id, (*a, *b)));
        }
    }
    let grad_of = |s: &str| grads.iter().find(|(id, _)| s.contains(id)).map(|(_, g)| *g);

    for e in root.descendants() {
        let tag = e.tag_name().name();
        if matches!(tag, "defs" | "linearGradient" | "stop" | "svg" | "title" | "desc" | "style" | "metadata") {
            continue;
        }
        if e.has_attribute("transform") {
            skipped += 1;
        }
        let fill = e.attribute("fill");
        let stroke = e.attribute("stroke");
        let sw = e.attribute("stroke-width").and_then(|t| t.parse::<f32>().ok()).unwrap_or(1.0);
        let opacity = e.attribute("opacity").and_then(|t| t.parse::<f32>().ok()).unwrap_or(1.0);
        let grad = fill.and_then(grad_of);
        let fill_col = if grad.is_some() {
            Color32::WHITE
        } else {
            fill.map(parse_color).flatten().unwrap_or(Color32::TRANSPARENT)
        };
        let stroke_col = stroke.map(parse_color).flatten().unwrap_or_else(|| {
            // SVG default: no stroke
            Color32::TRANSPARENT
        });
        match tag {
            "rect" => {
                let mut s = Shape::new(doc.alloc(), Kind::Rect, f(e, "x"), f(e, "y"), f(e, "width").max(1.0), f(e, "height").max(1.0));
                s.fill = fill_col;
                s.gradient = grad;
                s.stroke = stroke_col;
                s.stroke_w = sw;
                s.opacity = opacity.clamp(0.0, 1.0);
                if let (Some(rx), Some(_ry)) = (e.attribute("rx"), e.attribute("ry")) {
                    let _ = rx; // rounded corners are ignored (kept as plain rects)
                }
                doc.nodes.push(Node::Shape(s));
            }
            "ellipse" | "circle" => {
                let (cx, cy) = (f(e, "cx"), f(e, "cy"));
                let (rx, ry) = if tag == "circle" {
                    let r = f(e, "r").max(1.0);
                    (r, r)
                } else {
                    (f(e, "rx").max(1.0), f(e, "ry").max(1.0))
                };
                let mut s = Shape::new(doc.alloc(), Kind::Ellipse, cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                s.fill = fill_col;
                s.gradient = grad;
                s.stroke = stroke_col;
                s.stroke_w = sw;
                s.opacity = opacity.clamp(0.0, 1.0);
                doc.nodes.push(Node::Shape(s));
            }
            "line" => {
                let mut s = Shape::new(doc.alloc(), Kind::Line, 0.0, 0.0, 0.0, 0.0);
                s.anchors = vec![
                    Anchor::corner(pos2(f(e, "x1"), f(e, "y1"))),
                    Anchor::corner(pos2(f(e, "x2"), f(e, "y2"))),
                ];
                s.fill = Color32::TRANSPARENT;
                s.stroke = if stroke_col == Color32::TRANSPARENT { Color32::from_rgb(0x26, 0x26, 0x26) } else { stroke_col };
                s.stroke_w = sw;
                s.opacity = opacity.clamp(0.0, 1.0);
                s.sync_bounds();
                doc.nodes.push(Node::Shape(s));
            }
            "polyline" | "polygon" => {
                let Some(pts) = parse_points(e.attribute("points").unwrap_or("")) else {
                    skipped += 1;
                    continue;
                };
                if pts.len() < 2 {
                    skipped += 1;
                    continue;
                }
                let mut s = Shape::new(doc.alloc(), Kind::Path, 0.0, 0.0, 0.0, 0.0);
                s.closed = tag == "polygon" && fill_col != Color32::TRANSPARENT;
                s.anchors = pts.iter().map(|p| Anchor::corner(*p)).collect();
                s.fill = fill_col;
                s.stroke = if stroke_col == Color32::TRANSPARENT && !s.closed { Color32::from_rgb(0x26, 0x26, 0x26) } else { stroke_col };
                s.stroke_w = sw;
                s.opacity = opacity.clamp(0.0, 1.0);
                s.sync_bounds();
                doc.nodes.push(Node::Shape(s));
            }
            "path" => {
                let Some(d) = e.attribute("d") else { skipped += 1; continue };
                match parse_path(d) {
                    Some((pts, closed)) if pts.len() >= 2 => {
                        let mut s = Shape::new(doc.alloc(), Kind::Path, 0.0, 0.0, 0.0, 0.0);
                        s.closed = closed && fill_col != Color32::TRANSPARENT;
                        s.anchors = pts;
                        s.fill = fill_col;
                        s.stroke = stroke_col;
                        s.stroke_w = if stroke_col == Color32::TRANSPARENT { 0.0 } else { sw };
                        s.opacity = opacity.clamp(0.0, 1.0);
                        s.sync_bounds();
                        doc.nodes.push(Node::Shape(s));
                    }
                    _ => skipped += 1,
                }
            }
            "text" => {
                let content = e.text().unwrap_or("").trim().to_string();
                if content.is_empty() {
                    skipped += 1;
                    continue;
                }
                let tid = doc.alloc();
                doc.nodes.push(Node::Text(TextItem {
                    id: tid,
                    x: f(e, "x"),
                    y: f(e, "y") - f(e, "font-size").max(8.0),
                    w: 240.0,
                    text: content,
                    size: f(e, "font-size").max(8.0),
                    color: fill_col,
                    opacity: opacity.clamp(0.0, 1.0),
                    hidden: false,
                    locked: false,
                }));
            }
            _ => skipped += 1,
        }
    }
    Ok((doc, skipped))
}

fn f(e: roxmltree::Node, attr: &str) -> f32 {
    e.attribute(attr).and_then(|t| t.trim().trim_end_matches("px").parse::<f32>().ok()).unwrap_or(0.0)
}

/// `#rgb` / `#rrggbb` / `rgb(r,g,b)` / a few names / `none`.
pub fn parse_color(s: &str) -> Option<Color32> {
    let s = s.trim();
    if s == "none" || s.is_empty() {
        return Some(Color32::TRANSPARENT);
    }
    if let Some(hex) = s.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let v: Vec<u8> = hex.chars().filter_map(|c| u8::from_str_radix(&c.to_string(), 16).ok()).collect();
                if v.len() == 3 {
                    Some(Color32::from_rgb(v[0] * 17, v[1] * 17, v[2] * 17))
                } else {
                    None
                }
            }
            6 => u32::from_str_radix(hex, 16).ok().map(|v| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
            _ => None,
        };
    }
    if let Some(rest) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let nums: Vec<f32> = rest.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if nums.len() == 3 {
            return Some(Color32::from_rgb(nums[0] as u8, nums[1] as u8, nums[2] as u8));
        }
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "white" => Color32::WHITE,
        "black" => Color32::BLACK,
        "red" => Color32::from_rgb(0xff, 0, 0),
        "green" => Color32::from_rgb(0, 0x80, 0),
        "blue" => Color32::from_rgb(0, 0, 0xff),
        "gray" | "grey" => Color32::GRAY,
        "yellow" => Color32::from_rgb(0xff, 0xff, 0),
        "orange" => Color32::from_rgb(0xff, 0xa5, 0x00),
        "purple" => Color32::from_rgb(0x80, 0, 0x80),
        "transparent" => Color32::TRANSPARENT,
        _ => return None,
    })
}

/// `x,y x,y …` or `x y, x y`.
fn parse_points(s: &str) -> Option<Vec<Pos2>> {
    let nums: Vec<f32> = s.split([' ', ',', '\t', '\n']).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect();
    if nums.len() < 4 || nums.len() % 2 != 0 {
        return None;
    }
    Some(nums.chunks(2).map(|c| pos2(c[0], c[1])).collect())
}

/// Path data → flat anchors (curves keep their handle offsets). Arcs are
/// treated as lines.
fn parse_path(d: &str) -> Option<(Vec<Anchor>, bool)> {
    let toks = tokenize(d);
    if toks.is_empty() {
        return None;
    }
    let mut cur = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut last_cmd = ' ';
    let mut anchors: Vec<Anchor> = Vec::new();
    let mut closed = false;
    let mut i = 0usize;
    while i < toks.len() {
        // a letter starts a command; a number repeats the last command
        // (implicit M → L, per the SVG spec)
        let (cmd, _is_cmd) = match toks[i] {
            (true, c) => (c as u8 as char, true),
            (false, _) => (if last_cmd == 'M' { 'L' } else { last_cmd }, false),
        };
        let rel = cmd.is_lowercase();
        let up = cmd.to_ascii_uppercase();
        match up {
            'M' => {
                let (x, y) = (take_num(&toks, &mut i), take_num(&toks, &mut i));
                cur = if rel { pos2(cur.x + x, cur.y + y) } else { pos2(x, y) };
                start = cur;
                anchors.push(Anchor::corner(cur));
                last_cmd = 'M';
            }
            'L' => {
                let (x, y) = (take_num(&toks, &mut i), take_num(&toks, &mut i));
                cur = if rel { pos2(cur.x + x, cur.y + y) } else { pos2(x, y) };
                anchors.push(Anchor::corner(cur));
                last_cmd = 'L';
            }
            'H' => {
                let x = take_num(&toks, &mut i);
                cur = if rel { pos2(cur.x + x, cur.y) } else { pos2(x, cur.y) };
                anchors.push(Anchor::corner(cur));
                last_cmd = 'H';
            }
            'V' => {
                let y = take_num(&toks, &mut i);
                cur = if rel { pos2(cur.x, cur.y + y) } else { pos2(cur.x, y) };
                anchors.push(Anchor::corner(cur));
                last_cmd = 'V';
            }
            'C' => {
                let c1 = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let c2 = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let p = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let (c1, c2, p) = if rel { (cur + c1.to_vec2(), cur + c2.to_vec2(), cur + p.to_vec2()) } else { (c1, c2, p) };
                if let Some(a) = anchors.last_mut() {
                    a.h_out = c1 - a.p;
                }
                cur = p;
                anchors.push(Anchor { p: cur, h_in: c2 - cur, h_out: Vec2::ZERO });
                last_cmd = 'C';
            }
            'S' => {
                // smooth cubic: reflect the previous outgoing handle
                let c2 = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let p = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let (c2, p) = if rel { (cur + c2.to_vec2(), cur + p.to_vec2()) } else { (c2, p) };
                let c1 = match (anchors.last(), last_cmd) {
                    (Some(a), 'C' | 'S') => a.p - a.h_in,
                    (Some(a), _) => a.p,
                    _ => cur,
                };
                if let Some(a) = anchors.last_mut() {
                    a.h_out = c1 - a.p;
                }
                cur = p;
                anchors.push(Anchor { p: cur, h_in: c2 - cur, h_out: Vec2::ZERO });
                last_cmd = 'S';
            }
            'Q' => {
                // quadratic → cubic
                let q = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let p = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                let (q, p) = if rel { (cur + q.to_vec2(), cur + p.to_vec2()) } else { (q, p) };
                let p0 = anchors.last().map(|a| a.p).unwrap_or(cur);
                let c1 = p0 + (q - p0) * (2.0 / 3.0);
                let c2 = p + (q - p) * (2.0 / 3.0);
                if let Some(a) = anchors.last_mut() {
                    a.h_out = c1 - a.p;
                }
                cur = p;
                anchors.push(Anchor { p: cur, h_in: c2 - cur, h_out: Vec2::ZERO });
                last_cmd = 'Q';
            }
            'T' => {
                let p = pos2(take_num(&toks, &mut i), take_num(&toks, &mut i));
                cur = if rel { cur + p.to_vec2() } else { p };
                anchors.push(Anchor::corner(cur));
                last_cmd = 'T';
            }
            'A' => {
                // arc: skip the 7 parameters, land as a straight line
                for _ in 0..7 {
                    take_num(&toks, &mut i);
                }
                last_cmd = 'A';
            }
            'Z' => {
                closed = true;
                if anchors.len() > 1 {
                    cur = start;
                }
                last_cmd = 'Z';
            }
            _ => {}
        }
    }
    if anchors.len() < 2 {
        return None;
    }
    Some((anchors, closed))
}

/// Split path data into command tokens (letters) and numbers.
fn tokenize(d: &str) -> Vec<(bool, f32)> {
    let mut out = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, out: &mut Vec<(bool, f32)>| {
        if !num.is_empty() {
            if let Ok(v) = num.parse::<f32>() {
                out.push((false, v));
            }
            num.clear();
        }
    };
    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            flush(&mut num, &mut out);
            out.push((true, c as u8 as f32));
        } else if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
            // handle "-": it can start a new number after digits
            if (c == '-' || c == '+') && (num.ends_with(|ch: char| ch.is_ascii_digit()) || num.ends_with('.')) && !num.ends_with('e') && !num.ends_with('E') {
                flush(&mut num, &mut out);
            }
            num.push(c);
        } else {
            flush(&mut num, &mut out);
        }
    }
    flush(&mut num, &mut out);
    out
}

fn take_num(toks: &[(bool, f32)], i: &mut usize) -> f32 {
    while *i < toks.len() && toks[*i].0 {
        *i += 1; // skip command tokens when numbers are expected
    }
    let v = toks.get(*i).map(|t| t.1).unwrap_or(0.0);
    *i += 1;
    v
}
