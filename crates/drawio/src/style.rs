//! mxGraph style-string toolkit.
//!
//! A drawio style is `key=value;key=value;…` (bare keys allowed). Cells keep
//! that string verbatim; these helpers read and rewrite single keys so the
//! format panel can edit without destroying anything it doesn't know about.

use egui::{Color32, Stroke};

// ── key access ───────────────────────────────────────────────────────────────

/// Value of `key` in the style string (`None` when absent).
pub fn get<'a>(style: &'a str, key: &str) -> Option<&'a str> {
    for part in style.split(';') {
        let t = part.trim();
        if let Some(eq) = t.find('=') {
            if &t[..eq] == key {
                return Some(&t[eq + 1..]);
            }
        }
    }
    None
}

pub fn get_f32(style: &str, key: &str, default: f32) -> f32 {
    get(style, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Presence means true (`key` or `key=1`), `key=0` means false.
pub fn get_bool(style: &str, key: &str, default: bool) -> bool {
    match get(style, key) {
        Some("0") => false,
        Some(_) => true,
        None => default,
    }
}

/// Set (or append) `key=value`, preserving everything else.
pub fn set(style: &str, key: &str, value: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut found = false;
    for part in style.split(';') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        let is_key = t.find('=').map(|eq| &t[..eq] == key).unwrap_or(false);
        if is_key {
            parts.push(format!("{key}={value}"));
            found = true;
        } else {
            parts.push(t.to_string());
        }
    }
    if !found {
        parts.push(format!("{key}={value}"));
    }
    parts.join(";")
}

/// Remove `key` from the style string.
pub fn remove(style: &str, key: &str) -> String {
    style
        .split(';')
        .filter_map(|p| {
            let t = p.trim();
            if t.is_empty() {
                return None;
            }
            let is_key = t.find('=').map(|eq| &t[..eq] == key).unwrap_or(false);
            if is_key { None } else { Some(t.to_string()) }
        })
        .collect::<Vec<_>>()
        .join(";")
}

// ── colors ───────────────────────────────────────────────────────────────────

/// Parse `#rgb`, `#rrggbb`, `#rrggbbaa` or `none`→`None`.
pub fn parse_color(s: &str) -> Option<Color32> {
    let s = s.trim().strip_prefix('#')?;
    let byte = |v: &str| u8::from_str_radix(v, 16).ok();
    match s.len() {
        3 => Some(Color32::from_rgb(
            byte(&s[0..1])? * 17,
            byte(&s[1..2])? * 17,
            byte(&s[2..3])? * 17,
        )),
        6 => Some(Color32::from_rgb(byte(&s[0..2])?, byte(&s[2..4])?, byte(&s[4..6])?)),
        8 => Some(Color32::from_rgb(byte(&s[2..4])?, byte(&s[4..6])?, byte(&s[6..8])?)),
        _ => None,
    }
}

pub fn color_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Value of a color key; `none` and absence fall back differently:
/// `fallback` is used when the key is absent or unparsable.
pub fn color_or(style: &str, key: &str, fallback: Color32) -> Color32 {
    get(style, key).and_then(parse_color).unwrap_or(fallback)
}

/// `key=none` present?
pub fn is_none_color(style: &str, key: &str) -> bool {
    get(style, key) == Some("none")
}

pub fn lighten(c: Color32, f: f32) -> Color32 {
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * f) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

pub fn darken(c: Color32, f: f32) -> Color32 {
    let mix = |v: u8| (v as f32 * (1.0 - f)) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

// ── presets (drawio's classic style-panel palette) ──────────────────────────

/// 12 fill presets (hex).
pub const FILL_PRESETS: [&str; 12] = [
    "#ffffff", "#f5f5f5", "#dae8fc", "#d5e8d4", "#ffe6cc", "#fff2cc", "#f8cecc", "#e1d5e7", "#b0e3e6",
    "#bac8d3", "#ffd966", "#2d3339",
];

/// 12 line presets (hex).
pub const STROKE_PRESETS: [&str; 12] = [
    "#000000", "#333333", "#666666", "#999999", "#ffffff", "#6c8ebf", "#82b366", "#d79b00", "#b85450",
    "#9673a6", "#2d3339", "#ae4132",
];

pub const FONT_SIZES: [f32; 14] = [6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 28.0, 32.0, 48.0, 64.0];

pub const FONTS: [&str; 4] = ["Helvetica", "Times New Roman", "Courier New", "Georgia"];

/// Arrow-end presets: (label, mxGraph value for `startArrow`/`endArrow`).
pub const ARROWS: [(&str, &str); 10] = [
    ("None", "none"),
    ("Classic", "classic"),
    ("Open", "open"),
    ("Oval", "oval"),
    ("Diamond", "diamond"),
    ("ER one", "ERone"),
    ("ER many", "ERmany"),
    ("ER 1-to-N", "ERoneToMany"),
    ("ER 0-to-1", "ERzeroToOne"),
    ("ER mandatory one", "ERmandOne"),
];

/// Dash presets: (label, `dashPattern` value; `None` = solid).
pub const DASHES: [(&str, Option<&str>); 4] = [
    ("Solid", None),
    ("Dash", Some("4 4")),
    ("Dot", Some("1 3")),
    ("Dash-Dot", Some("8 4 1 4")),
];

// ── resolved look used by painters ──────────────────────────────────────────

/// Everything a painter needs, resolved from a style string.
#[derive(Debug, Clone)]
pub struct Look {
    /// Fill color; `None` = no fill (also used by edges).
    pub fill: Option<Color32>,
    pub stroke: Color32,
    pub width: f32,
    pub dashed: bool,
    /// Dash on/off lengths in world points (empty = egui default solid).
    pub dash: Vec<f32>,
    pub text: Color32,
    /// Zoom factor, for scaling dash patterns and fonts.
    pub scale: f32,
}

impl Look {
    pub fn stroke_w(&self) -> Stroke {
        Stroke::new((self.width * self.scale).max(0.5), self.stroke)
    }
}

const DEFAULT_FILL: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const DEFAULT_STROKE: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);

pub fn vertex_look(style: &str, zoom: f32) -> Look {
    let fill = if is_none_color(style, "fillColor") {
        None
    } else {
        Some(color_or(style, "fillColor", DEFAULT_FILL))
    };
    let dash = dash_of(style);
    Look {
        fill,
        stroke: color_or(style, "strokeColor", DEFAULT_STROKE),
        width: get_f32(style, "strokeWidth", 1.0),
        dashed: dash.is_some(),
        dash: dash.unwrap_or_default(),
        text: color_or(style, "fontColor", DEFAULT_STROKE),
        scale: zoom,
    }
}

pub fn edge_look(style: &str, zoom: f32) -> Look {
    Look {
        fill: None,
        stroke: color_or(style, "strokeColor", DEFAULT_STROKE),
        width: get_f32(style, "strokeWidth", 1.0),
        dashed: get_bool(style, "dashed", false),
        dash: dash_of(style).unwrap_or_default(),
        text: color_or(style, "fontColor", DEFAULT_STROKE),
        scale: zoom,
    }
}

fn dash_of(style: &str) -> Option<Vec<f32>> {
    if !get_bool(style, "dashed", false) {
        return None;
    }
    Some(match get(style, "dashPattern") {
        Some(p) => p.split_whitespace().filter_map(|v| v.parse().ok()).collect(),
        None => vec![6.0, 4.0],
    })
}

/// egui font family for an mxGraph `fontFamily` value.
pub fn font_family(style: &str) -> egui::FontFamily {
    match get(style, "fontFamily").unwrap_or("Helvetica") {
        "Times New Roman" | "Georgia" | "serif" => egui::FontFamily::Name("serif".into()),
        "Courier New" | "Courier" | "mono" => egui::FontFamily::Monospace,
        _ => egui::FontFamily::Proportional,
    }
}

pub fn font_size(style: &str, zoom: f32) -> f32 {
    get_f32(style, "fontSize", 12.0) * zoom
}

/// `fontStyle` bitmask: 1 bold, 2 italic, 4 underline.
pub fn font_style(style: &str) -> u32 {
    get_f32(style, "fontStyle", 0.0) as u32
}

pub fn set_font_bits(style: &mut String, bit: u32, on: bool) {
    let cur = font_style(style);
    let next = if on { cur | bit } else { cur & !bit };
    *style = if next == 0 { remove(style, "fontStyle") } else { set(style, "fontStyle", &next.to_string()) };
}

/// Horizontal label alignment (mxGraph `align`).
pub fn halign(style: &str) -> Align {
    match get(style, "align").unwrap_or("center") {
        "left" => Align::Min,
        "right" => Align::Max,
        _ => Align::Mid,
    }
}

/// Vertical label alignment (mxGraph `verticalAlign`).
pub fn valign(style: &str) -> Align {
    match get(style, "verticalAlign").unwrap_or("middle") {
        "top" => Align::Min,
        "bottom" => Align::Max,
        _ => Align::Mid,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Min,
    Mid,
    Max,
}

/// Edge routing flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeRoute {
    Orthogonal,
    Straight,
    Curved,
}

pub fn edge_route(style: &str) -> EdgeRoute {
    match get(style, "edgeStyle") {
        Some("orthogonalEdgeStyle") | Some("elbowEdgeStyle") => {
            if get_bool(style, "curved", false) {
                EdgeRoute::Curved
            } else {
                EdgeRoute::Orthogonal
            }
        }
        Some("straightEdgeStyle") | Some("none") => EdgeRoute::Straight,
        _ => {
            if get_bool(style, "curved", false) {
                EdgeRoute::Curved
            } else {
                EdgeRoute::Orthogonal
            }
        }
    }
}

/// Per-edge line-jump style (`jumpStyle=arc`).
pub fn line_jump(style: &str) -> bool {
    get(style, "jumpStyle").map(|v| v != "none").unwrap_or(false)
}

/// A human name for the shape behind a style (status bar / context menus).
pub fn shape_label(style: &str) -> String {
    match get(style, "shape") {
        Some(s) => s.to_string(),
        None => {
            if get_bool(style, "edge", false) || get(style, "endArrow").is_some() {
                "edge".into()
            } else {
                "rectangle".into()
            }
        }
    }
}
