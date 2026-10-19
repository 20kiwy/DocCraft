//! Optional `serde_json` serialization for the Vector document model
//! (enabled by the default `json` feature; built by hand so the crate does
//! not need `serde` derive).

use crate::model::*;
use egui::{pos2, vec2, Color32, Pos2, Vec2};

fn col(c: Color32) -> serde_json::Value {
    serde_json::json!([c.r(), c.g(), c.b(), c.a()])
}

fn parse_col(v: &serde_json::Value) -> Option<Color32> {
    let a = v.as_array()?;
    Some(Color32::from_rgba_unmultiplied(
        a.first()?.as_u64()? as u8,
        a.get(1)?.as_u64()? as u8,
        a.get(2)?.as_u64()? as u8,
        a.get(3).and_then(|x| x.as_u64()).unwrap_or(255) as u8,
    ))
}

impl Doc {
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "artboard": [self.artboard.x, self.artboard.y],
            "nodes": self.nodes.iter().map(|n| match n {
                Node::Shape(s) => serde_json::json!({
                    "id": s.id, "type": "shape", "kind": format!("{:?}", s.kind).to_lowercase(),
                    "x": s.x, "y": s.y, "w": s.w, "h": s.h,
                    "anchors": s.anchors.iter().map(|a| serde_json::json!([
                        a.p.x, a.p.y, a.h_in.x, a.h_in.y, a.h_out.x, a.h_out.y
                    ])).collect::<Vec<_>>(),
                    "closed": s.closed, "sides": s.sides,
                    "fill": col(s.fill),
                    "gradient": s.gradient.map(|(t, b)| serde_json::json!([col(t), col(b)])),
                    "stroke": col(s.stroke), "stroke_w": s.stroke_w, "opacity": s.opacity,
                    "hidden": s.hidden, "locked": s.locked,
                }),
                Node::Stroke(s) => serde_json::json!({
                    "id": s.id, "type": "stroke",
                    "pts": s.pts.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
                    "color": col(s.color), "width": s.width, "opacity": s.opacity,
                    "hidden": s.hidden, "locked": s.locked,
                }),
                Node::Text(t) => serde_json::json!({
                    "id": t.id, "type": "text",
                    "x": t.x, "y": t.y, "w": t.w, "text": t.text, "size": t.size,
                    "color": col(t.color), "opacity": t.opacity,
                    "hidden": t.hidden, "locked": t.locked,
                }),
            }).collect::<Vec<_>>(),
        })
    }

    pub fn from_json_value(v: &serde_json::Value) -> Result<Self, String> {
        let ab = v.get("artboard").and_then(|a| a.as_array()).ok_or("missing artboard")?;
        let mut doc = Doc::new(
            ab.first().and_then(|x| x.as_f64()).unwrap_or(640.0) as f32,
            ab.get(1).and_then(|x| x.as_f64()).unwrap_or(480.0) as f32,
        );
        doc.next_id = 1;
        let nodes = v.get("nodes").and_then(|n| n.as_array()).ok_or("missing nodes")?;
        for n in nodes {
            let id = n.get("id").and_then(|x| x.as_u64()).unwrap_or_else(|| doc.alloc());
            doc.next_id = doc.next_id.max(id + 1);
            match n.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                "shape" => {
                    let kind = match n.get("kind").and_then(|k| k.as_str()).unwrap_or("rect") {
                        "ellipse" => Kind::Ellipse,
                        "line" => Kind::Line,
                        "poly" | "polygon" => Kind::Poly,
                        "path" => Kind::Path,
                        _ => Kind::Rect,
                    };
                    let g = |k: &str, d: f32| n.get(k).and_then(|x| x.as_f64()).unwrap_or(d as f64) as f32;
                    let mut s = Shape::new(id, kind, g("x", 0.0), g("y", 0.0), g("w", 10.0), g("h", 10.0));
                    if let Some(list) = n.get("anchors").and_then(|a| a.as_array()) {
                        for a in list {
                            if let Some(q) = a.as_array() {
                                let num = |i: usize| q.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                                s.anchors.push(Anchor {
                                    p: pos2(num(0), num(1)),
                                    h_in: vec2(num(2), num(3)),
                                    h_out: vec2(num(4), num(5)),
                                });
                            }
                        }
                    }
                    s.closed = n.get("closed").and_then(|x| x.as_bool()).unwrap_or(false);
                    s.sides = n.get("sides").and_then(|x| x.as_u64()).unwrap_or(6) as usize;
                    s.fill = n.get("fill").and_then(parse_col).unwrap_or(Color32::WHITE);
                    s.gradient = n.get("gradient").and_then(|g| g.as_array()).and_then(|g| {
                        Some((
                            parse_col(g.first()?)?,
                            parse_col(g.get(1)?)?,
                        ))
                    });
                    s.stroke = n.get("stroke").and_then(parse_col).unwrap_or(Color32::BLACK);
                    s.stroke_w = g("stroke_w", 1.0);
                    s.opacity = g("opacity", 1.0);
                    s.hidden = n.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false);
                    s.locked = n.get("locked").and_then(|x| x.as_bool()).unwrap_or(false);
                    doc.nodes.push(Node::Shape(s));
                }
                "stroke" => {
                    let pts: Vec<Pos2> = n
                        .get("pts")
                        .and_then(|p| p.as_array())
                        .map(|list| {
                            list.iter()
                                .filter_map(|q| {
                                    let c = q.as_array()?;
                                    Some(pos2(c.first()?.as_f64()? as f32, c.get(1)?.as_f64()? as f32))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    doc.nodes.push(Node::Stroke(FreeStroke {
                        id,
                        pts,
                        color: n.get("color").and_then(parse_col).unwrap_or(Color32::BLACK),
                        width: n.get("width").and_then(|x| x.as_f64()).unwrap_or(2.0) as f32,
                        opacity: n.get("opacity").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
                        hidden: n.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false),
                        locked: n.get("locked").and_then(|x| x.as_bool()).unwrap_or(false),
                    }));
                }
                "text" => {
                    doc.nodes.push(Node::Text(TextItem {
                        id,
                        x: n.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                        y: n.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                        w: n.get("w").and_then(|x| x.as_f64()).unwrap_or(240.0) as f32,
                        text: n.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        size: n.get("size").and_then(|x| x.as_f64()).unwrap_or(14.0) as f32,
                        color: n.get("color").and_then(parse_col).unwrap_or(Color32::BLACK),
                        opacity: n.get("opacity").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
                        hidden: n.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false),
                        locked: n.get("locked").and_then(|x| x.as_bool()).unwrap_or(false),
                    }));
                }
                _ => {}
            }
        }
        Ok(doc)
    }
}
