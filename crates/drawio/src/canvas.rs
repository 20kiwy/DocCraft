//! The drawing canvas: world↔screen mapping, hit tests, drag interactions
//! (move / resize / rubber-band / connect / pan / waypoints), alignment
//! guides, edge routing with line jumps, inline label editing, the outline
//! minimap and the right-click context menu.
//!
//! State lives on [`crate::ui::DrawApp`]; this module is the engine that
//! reads and mutates it every frame.

use crate::connection::{self, ConnectState};
use crate::model::{ArrowKind, Cell, Diagram};
use crate::shapes::{self, Xf};
use crate::style::{self, Look};
use crate::ui::{Drag, DrawApp, Tool};
use doccraft_theme::Tokens;
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

const HANDLE_FILL: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const HANDLE: f32 = 7.0;
// drawio connection colors (spec §1 — three different "blues/greens")
pub const CONNECT_BLUE: Color32 = Color32::from_rgb(0x29, 0xb6, 0xf2); // hover arrows, × markers, endpoint handles
pub const TARGET_BLUE: Color32 = Color32::from_rgb(0x00, 0xa8, 0xff); // valid drop-target highlight
const SNAP_GREEN: Color32 = Color32::from_rgb(0x00, 0xff, 0x00); // mxConstants.VALID_COLOR
const INVALID_RED: Color32 = Color32::from_rgb(0xff, 0x00, 0x00); // mxConstants.INVALID_COLOR
const MARKER_BLUE: Color32 = Color32::from_rgb(0x00, 0x88, 0xcf); // mxConstants.HANDLE_STROKECOLOR

// ── drawio HoverIcons + connection spec (Graph.js / mxConnectionHandler v32.5.5) ──
/// HoverIcons.arrowSpacing — the shape bbox is grown by this before arrows sit on it.
const ARROW_SPACING: f32 = 2.0;
/// HoverIcons.inactiveOpacity = 15 (% of 255 ≈ 38) — arrows fade when not approached.
const ARROW_INACTIVE_ALPHA: u8 = 38;
/// Editor.hintOffset — the "[x%, y%]" hint sits this far below the cursor.
const HINT_OFFSET: f32 = 24.0;
/// HoverIcons arrow SVG polygons, 1:1 px (arrowFill #29b6f2, stroke #fff).
const ARROW_UP_PTS: [Pos2; 7] = [
    Pos2::new(6.0, 26.0), Pos2::new(12.0, 26.0), Pos2::new(12.0, 12.0), Pos2::new(18.0, 12.0),
    Pos2::new(9.0, 1.0), Pos2::new(1.0, 12.0), Pos2::new(6.0, 12.0),
];
const ARROW_RIGHT_PTS: [Pos2; 7] = [
    Pos2::new(1.0, 6.0), Pos2::new(14.0, 6.0), Pos2::new(14.0, 1.0), Pos2::new(26.0, 9.0),
    Pos2::new(14.0, 18.0), Pos2::new(14.0, 12.0), Pos2::new(1.0, 12.0),
];
const ARROW_DOWN_PTS: [Pos2; 7] = [
    Pos2::new(6.0, 1.0), Pos2::new(6.0, 14.0), Pos2::new(1.0, 14.0), Pos2::new(9.0, 26.0),
    Pos2::new(18.0, 14.0), Pos2::new(12.0, 14.0), Pos2::new(12.0, 1.0),
];
const ARROW_LEFT_PTS: [Pos2; 7] = [
    Pos2::new(1.0, 9.0), Pos2::new(12.0, 1.0), Pos2::new(12.0, 6.0), Pos2::new(26.0, 6.0),
    Pos2::new(26.0, 12.0), Pos2::new(12.0, 12.0), Pos2::new(12.0, 18.0),
];
const ARROW_UP_SIZE: (f32, f32) = (18.0, 28.0);
const ARROW_RIGHT_SIZE: (f32, f32) = (26.0, 18.0);
const ARROW_DOWN_SIZE: (f32, f32) = (18.0, 26.0);
const ARROW_LEFT_SIZE: (f32, f32) = (28.0, 18.0);

// ── mapping ──────────────────────────────────────────────────────────────────

pub fn to_screen(pan: Vec2, zoom: f32, origin: Pos2, p: Pos2) -> Pos2 {
    origin + pan + p.to_vec2() * zoom
}

pub fn to_world(pan: Vec2, zoom: f32, origin: Pos2, p: Pos2) -> Pos2 {
    let v = (p - origin - pan) / zoom;
    Pos2::new(v.x, v.y)
}

/// Screen position → canvas world position (palette drag & drop, overlay hit tests).
/// Uses the canvas rect recorded on the last [`show`] frame.
pub fn screen_to_world(app: &DrawApp, pos: Pos2) -> Pos2 {
    let Some(r) = app.view_rect else { return Pos2::ZERO };
    to_world(app.pan, app.zoom, r.min, pos)
}

/// Linear mix a→b in sRGB space (`t` = 0 gives `a`, 1 gives `b`).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// Grid colors derived from the tokens: subtle in both themes, major lines
/// slightly stronger than minor ones.
fn grid_colors(t: &Tokens) -> (Color32, Color32) {
    if t.dark {
        (mix(t.border, t.canvas, 0.55), mix(t.border, t.canvas, 0.2))
    } else {
        (mix(t.border, t.canvas, 0.15), mix(t.border, t.canvas, 0.45))
    }
}

/// The world-space rectangle currently visible on the canvas.
pub fn viewport(app: &DrawApp, canvas: Rect) -> Rect {
    let a = to_world(app.pan, app.zoom, canvas.min, canvas.left_top());
    let b = to_world(app.pan, app.zoom, canvas.min, canvas.right_bottom());
    Rect::from_two_pos(a, b)
}

// ── entry point ──────────────────────────────────────────────────────────────

pub fn show(app: &mut DrawApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    app.view_rect = Some(rect);
    let painter = ui.painter_at(rect.expand(1.0));
    painter.rect_filled(rect, CornerRadius::ZERO, t.canvas);

    let page_setup = app.page();
    let (page_w, page_h, page_bg) = (page_setup.page_w, page_setup.page_h, page_setup.background.clone());
    // explicit page background wins; otherwise pure white in light theme,
    // dark-sheet gray in the dark theme
    let page_bg = page_bg
        .as_deref()
        .and_then(style::parse_color)
        .unwrap_or(if t.dark { Color32::from_rgb(0x25, 0x25, 0x26) } else { Color32::WHITE });

    if app.show_grid {
        draw_grid(&painter, rect, app.pan, app.zoom, app.grid, &t);
    }

    // page sheet with a soft theme shadow
    let page = Rect::from_min_size(to_screen(app.pan, app.zoom, rect.min, Pos2::ZERO), Vec2::new(page_w * app.zoom, page_h * app.zoom));
    painter.add(doccraft_theme::card_shadow(&t).as_shape(page, CornerRadius::ZERO));
    painter.rect_filled(page, CornerRadius::ZERO, page_bg);
    painter.rect_stroke(page, CornerRadius::ZERO, Stroke::new(1.0, t.border_strong), StrokeKind::Outside);

    let pan = app.pan;
    let zoom = app.zoom;
    let w2s = move |p: Pos2| to_screen(pan, zoom, rect.min, p);
    let s2w = move |p: Pos2| to_world(pan, zoom, rect.min, p);

    // ── routes (screen space) for every visible edge, via the connection engine ──
    let page = app.page().clone();
    let routes: Vec<(String, style::EdgeRoute, Vec<Pos2>)> = page
        .cells
        .iter()
        .filter(|c| c.edge && page.layer_visible(c))
        .map(|c| {
            let world = connection::route_points(&page, c);
            (c.id.clone(), style::edge_route(&c.style), world.iter().map(|p| w2s(*p)).collect())
        })
        .collect();

    // ── cells in paint order ──
    let sel = app.selected.clone();
    let mut labels: Vec<(String, Pos2, f32)> = Vec::new(); // edge labels (id, screen midpoint, size)
    {
        let page = app.page().clone();
        for c in &page.cells {
            if !page.layer_visible(c) {
                continue;
            }
            let selected = sel.contains(&c.id);
            if c.edge {
                let Some((_, route, pts)) = routes.iter().find(|(id, _, _)| id == &c.id) else { continue };
                if pts.len() < 2 {
                    continue;
                }
                let mut look = style::edge_look(&c.style, zoom);
                apply_opacity(&mut look, c);
                if selected {
                    paint_polyline(&painter, pts, &Stroke::new(4.0 * zoom.max(0.5), t.accent.linear_multiply(0.35)), &[]);
                }
                let jumps = if style::line_jump(&c.style) && *route == style::EdgeRoute::Orthogonal && zoom >= 0.4 {
                    compute_jumps(&c.id, pts, &routes)
                } else {
                    Vec::new()
                };
                paint_edge(&painter, pts, &c.style, &look, &jumps, zoom);
                if !c.value.is_empty() {
                    let mid = edge_label_pos(pts);
                    labels.push((c.id.clone(), mid, 12.0 * zoom.max(0.4)));
                }
            } else {
                let r = Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom));
                if !rect.intersects(r) {
                    continue;
                }
                let mut look = style::vertex_look(&c.style, zoom);
                apply_opacity(&mut look, c);
                if selected {
                    painter.rect_filled(r, CornerRadius::same(2), t.accent.linear_multiply(0.12));
                }
                let xf = Xf::from_style(&c.style);
                shapes::paint_vertex(&painter, r, &c.style, &look, &c.value, &xf);
            }
        }
    }
    // find/replace highlights
    for (pg, id) in &app.matches {
        if *pg != app.file.active {
            continue;
        }
        if let Some(c) = app.page().cell(id)
            && c.vertex
        {
            let r = Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom)).expand(3.0);
            painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, t.success), StrokeKind::Outside);
        }
    }

    // edge labels (after shapes so they sit on top)
    for (id, mid, fs) in labels {
        let Some(cell) = app.page().cell(&id) else { continue };
        let look = style::edge_look(&cell.style, zoom);
        let font = FontId::new(fs, style::font_family(&cell.style));
        let galley = painter.layout(cell.value.clone(), font, look.text, 160.0);
        let sz = galley.size();
        let bg = Rect::from_center_size(mid, sz + Vec2::new(8.0, 4.0));
        painter.rect_filled(bg, CornerRadius::same(2), Color32::from_white_alpha(215));
        painter.galley(mid - sz / 2.0, galley, look.text);
        if sel.contains(&id) {
            painter.rect_stroke(bg, CornerRadius::same(2), Stroke::new(1.0, t.accent), StrokeKind::Outside);
        }
    }

    // ── selection chrome ──
    if let Some(id) = sel.first().cloned()
        && let Some(c) = app.page().cell(&id)
        && c.vertex
    {
        let r = Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.2, t.accent), StrokeKind::Outside);
        let hs = HANDLE * zoom.max(0.6).min(1.4);
        for i in 0..8u8 {
            let hr = handle_rect(r, i, hs);
            painter.rect_filled(hr, CornerRadius::same(1), HANDLE_FILL);
            painter.rect_stroke(hr, CornerRadius::same(1), Stroke::new(1.0, t.accent), StrokeKind::Outside);
        }
    }
    if sel.len() > 1
        && let Some((bx, by, bw, bh)) = app.page().bbox(&sel)
    {
        let r = Rect::from_min_size(w2s(Pos2::new(bx, by)), Vec2::new(bw * zoom, bh * zoom));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, t.accent.linear_multiply(0.8)), StrokeKind::Outside);
    }
    // single selected edge: waypoint handles + endpoint handles
    if let Some(id) = sel.first().cloned()
        && let Some(c) = app.page().cell(&id)
        && c.edge
        && let Some((_, _, pts)) = routes.iter().find(|(rid, _, _)| rid == &id)
        && pts.len() >= 2
    {
        for (i, w) in c.geom.points.iter().enumerate() {
            let p = w2s(Pos2::from(*w));
            let r = Rect::from_center_size(p, Vec2::splat(7.0));
            painter.rect_filled(r, CornerRadius::same(1), HANDLE_FILL);
            painter.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.0, t.accent), StrokeKind::Outside);
            let _ = i;
        }
        // endpoint handles: r=6 #29b6f2 with white stroke; × inside when the
        // end is pinned to a fixed connection point (spec §5)
        let dragging_this = matches!(
            app.connect,
            ConnectState::EndpointDrag { ref edge, .. } if *edge == id
        );
        if !dragging_this && app.edit_label.is_none() {
            let ends = [
                (true, *pts.first().unwrap(), c.exit().is_some()),
                (false, *pts.last().unwrap(), c.entry().is_some()),
            ];
            for (is_source, sp, pinned) in ends {
                let er = Rect::from_center_size(sp, Vec2::splat(12.0));
                let hresp = ui.allocate_rect(er, Sense::click_and_drag());
                let hot = hresp.hovered() || hresp.dragged();
                painter.circle_filled(sp, 6.0, CONNECT_BLUE);
                painter.circle_stroke(sp, 6.0, Stroke::new(1.5, HANDLE_FILL));
                if pinned {
                    paint_cross(&painter, sp, 3.0, HANDLE_FILL, Stroke::new(1.2, HANDLE_FILL));
                }
                if hot {
                    painter.circle_stroke(sp, 9.0, Stroke::new(1.0, CONNECT_BLUE.linear_multiply(0.6)));
                }
                // start on the raw press — drawio connects on mousedown, and
                // egui 0.36 postpones drag_started for click_and_drag widgets
                // (the arrows vanish once the pointer leaves the hover zone,
                // which would kill a pending drag before the decision)
                let press_on_handle = ui.input(|i| i.pointer.primary_down()) && hresp.hovered();
                if press_on_handle {
                    let cur = s2w(ui.input(|i| i.pointer.latest_pos()).unwrap_or(sp));
                    app.connect = ConnectState::EndpointDrag { edge: id.clone(), is_source, cur, snapped: None, press: cur };
                    app.status = "Drag to re-link · drop on empty to dangle · Esc to cancel".into();
                }
            }
        }
    }

    // ── connection overlays: preview edge, × markers, snap ring, highlights ──
    connect_overlay(app, ui, rect, &painter, &w2s, &s2w);

    // ── hover connection arrows (drawio HoverIcons) ──
    hover_arrows(app, ui, rect, &painter, &w2s, &s2w);

    // ── overlays: guides, rubber band, edge preview ──
    for g in &app.guides {
        let (v, pos) = *g;
        let vp = viewport(app, rect);
        let (a, b) = if v {
            (Pos2::new(pos, vp.top()), Pos2::new(pos, vp.bottom()))
        } else {
            (Pos2::new(vp.left(), pos), Pos2::new(vp.right(), pos))
        };
        painter.line_segment([w2s(a), w2s(b)], Stroke::new(1.0, t.guide));
    }
    if let Drag::Rubber { start, cur } = app.drag.clone() {
        let a = w2s(start);
        let b = w2s(cur);
        let r = Rect::from_two_pos(a, b);
        painter.rect_filled(r, CornerRadius::ZERO, t.accent.linear_multiply(0.10));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, t.accent), StrokeKind::Inside);
    }

    // ── inline label editor ──
    if let Some((id, buf, _orig)) = app.edit_label.clone() {
        if let Some(c) = app.page().cell(&id) {
            let (r, fs) = if c.edge {
                let mid = routes.iter().find(|(rid, _, _)| rid == &id).map(|(_, _, p)| edge_label_pos(p)).unwrap_or(rect.center());
                (Rect::from_center_size(mid, Vec2::new(120.0 * zoom.max(0.6), 24.0)), 12.0 * zoom.max(0.6))
            } else {
                let cr = Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom));
                (cr, style::font_size(&c.style, zoom).max(11.0))
            };
            let mut text = buf;
            let er = ui.put(r.expand(2.0), egui::TextEdit::singleline(&mut text).font(egui::FontId::proportional(fs)));
            if app.edit_just_opened {
                er.request_focus();
                app.edit_just_opened = false;
            }
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let cancel = er.has_focus() && esc;
            let commit = !cancel && (er.lost_focus() || (er.has_focus() && enter));
            if cancel {
                app.edit_label = None;
            } else if commit {
                app.edit_label = None;
                let new_text = text.clone();
                app.edit_page(|page| {
                    if let Some(c) = page.cell_mut(&id) {
                        c.value = new_text;
                    }
                });
            }
        } else {
            app.edit_label = None;
        }
    }

    // ── minimap ──
    if app.show_outline {
        minimap(app, ui, rect, &painter, &t);
    }

    // ── interactions ──
    canvas_input(app, ui, rect, resp, &routes, s2w);
    context_menu(app, ui, rect);
}

// ── input handling ───────────────────────────────────────────────────────────

fn canvas_input(
    app: &mut DrawApp,
    ui: &mut Ui,
    rect: Rect,
    resp: egui::Response,
    routes: &[(String, style::EdgeRoute, Vec<Pos2>)],
    s2w: impl Fn(Pos2) -> Pos2,
) {
    // wheel: zoom (ctrl) / pan
    let scroll = ui.input(|i| i.smooth_scroll_delta);
    if resp.hovered() && scroll != Vec2::ZERO {
        let mods = ui.input(|i| i.modifiers);
        let ptr = ui.input(|i| i.pointer.latest_pos()).unwrap_or(rect.center());
        if mods.ctrl {
            let f = if scroll.y > 0.0 { 1.1 } else { 1.0 / 1.1 };
            zoom_at(app, rect, ptr, f);
        } else {
            app.pan += Vec2::new(scroll.x, scroll.y);
        }
    }

    // space / middle mouse pan
    let space = ui.input(|i| i.key_down(egui::Key::Space));
    if resp.drag_started() {
        let p = resp.interact_pointer_pos().unwrap_or(rect.center());
        let world = s2w(p);
        if space || resp.drag_started_by(egui::PointerButton::Middle) {
            app.drag = Drag::Pan { last: p };
            return;
        }
        match app.tool {
            Tool::Select => {
                // resize handles
                let sel_rect = app
                    .selected
                    .first()
                    .and_then(|id| app.page().cell(id))
                    .filter(|c| c.vertex)
                    .map(|c| (
                        c.id.clone(),
                        Rect::from_min_size(
                            to_screen(app.pan, app.zoom, rect.min, Pos2::new(c.geom.x, c.geom.y)),
                            Vec2::new(c.geom.w * app.zoom, c.geom.h * app.zoom),
                        ),
                    ));
                if let Some((id, r)) = sel_rect {
                    if let Some(i) = (0..8u8).find(|i| handle_rect(r, *i, HANDLE).contains(p)) {
                        let orig = app.page().cell(&id).map(|c| (c.geom.x, c.geom.y, c.geom.w, c.geom.h)).unwrap_or_default();
                        app.push_undo();
                        app.drag = Drag::Resize { id, handle: i, orig };
                        return;
                    }
                }
                // waypoint grab (selected edge)
                let wp_hit = app
                    .selected
                    .first()
                    .and_then(|id| app.page().cell(id))
                    .filter(|c| c.edge)
                    .and_then(|c| {
                        c.geom
                            .points
                            .iter()
                            .position(|w| to_screen(app.pan, app.zoom, rect.min, Pos2::from(*w)).distance(p) < 6.0)
                            .map(|i| (c.id.clone(), i))
                    });
                if let Some((id, i)) = wp_hit {
                    app.push_undo();
                    app.drag = Drag::Waypoint { id, idx: i };
                    return;
                }
                // vertex / edge pick
                let picked = pick_vertex(app.page(), world, false).map(|c| c.id.clone());
                if let Some(id) = picked {
                    if !app.selected.contains(&id) {
                        let mods = ui.input(|i| i.modifiers);
                        if mods.shift {
                            app.selected.push(id.clone());
                        } else {
                            app.selected = vec![id.clone()];
                        }
                    }
                    let ids = app.selected.clone();
                    let orig = positions(app, &ids);
                    // press-world reference: the drag delta is world − press
                    // (grab used to store the shape-origin offset, which
                    // double-counted it and teleported nodes by their own x/y)
                    let grab = Vec2::new(world.x, world.y);
                    app.push_undo();
                    app.drag = Drag::Move { ids, grab, orig, moved: false };
                } else if let Some(id) = pick_edge(routes, p, app.zoom) {
                    if !app.selected.contains(&id) {
                        app.selected = vec![id];
                    }
                    app.drag = Drag::None;
                } else {
                    if !ui.input(|i| i.modifiers.shift) {
                        app.selected.clear();
                    }
                    app.drag = Drag::Rubber { start: world, cur: world };
                }
            }
            Tool::Connect => {
                if app.edge_from.is_none() {
                    if let Some(c) = pick_vertex(app.page(), world, false) {
                        app.edge_from = Some(c.id.clone());
                        app.status = "Connect: click the target shape".into();
                    }
                }
            }
        }
    }
    if resp.dragged() {
        let Some(p) = resp.interact_pointer_pos() else { return };
        let world = s2w(p);
        match app.drag.clone() {
            Drag::Pan { last } => {
                app.pan += p - last;
                app.drag = Drag::Pan { last: p };
            }
            Drag::Move { ids, grab, orig, moved } => {
                let mut dx = world.x - grab.x;
                let mut dy = world.y - grab.y;
                let bw = app.page().bbox(&ids).unwrap_or((dx, dy, 0.0, 0.0));
                if app.show_guides {
                    let (gx, gy, guides) = align_guides(app.page(), &ids, bw, 4.0 / app.zoom);
                    dx += gx;
                    dy += gy;
                    app.guides = guides;
                }
                if app.show_grid {
                    dx = (dx / app.grid).round() * app.grid;
                    dy = (dy / app.grid).round() * app.grid;
                }
                apply_positions(app, &orig, dx, dy);
                app.drag = Drag::Move { ids, grab, orig, moved: true };
                app.file.dirty = true;
            }
            Drag::Resize { id, handle, orig } => {
                let locked = app.page().cell(&id).map(|c| app.page().layer_locked(c)).unwrap_or(true);
                if !locked {
                    let sg = app.show_grid;
                    let g = app.grid;
                    app.edit_page(|page| {
                        if let Some(c) = page.cell_mut(&id) {
                            let (ox, oy, ow, oh) = orig;
                            let snap = |v: f32| if sg { (v / g).round() * g } else { v };
                            let right = snap(world.x.max(ox + 10.0));
                            let bottom = snap(world.y.max(oy + 10.0));
                            let left = snap(world.x.min(ox + ow - 10.0));
                            let top = snap(world.y.min(oy + oh - 10.0));
                            match handle {
                                0 => { c.geom.x = left; c.geom.y = top; c.geom.w = ox + ow - left; c.geom.h = oy + oh - top; }
                                1 => { c.geom.y = top; c.geom.h = oy + oh - top; }
                                2 => { c.geom.y = top; c.geom.h = oy + oh - top; c.geom.w = right - ox; }
                                3 => { c.geom.w = right - ox; }
                                4 => { c.geom.w = right - ox; c.geom.h = bottom - oy; }
                                5 => { c.geom.h = bottom - oy; }
                                6 => { c.geom.x = left; c.geom.w = ox + ow - left; c.geom.h = bottom - oy; }
                                _ => { c.geom.x = left; c.geom.w = ox + ow - left; }
                            }
                        }
                    });
                }
            }
            Drag::Waypoint { id, idx } => {
                let g = app.grid.max(1.0);
                let sg = app.show_grid;
                app.edit_page(|page| {
                    if let Some(c) = page.cell_mut(&id) {
                        let (x, y) = if sg { ((world.x / g).round() * g, (world.y / g).round() * g) } else { (world.x, world.y) };
                        if let Some(w) = c.geom.points.get_mut(idx) {
                            *w = (x, y);
                        }
                    }
                });
            }
            Drag::Rubber { start, .. } => {
                app.drag = Drag::Rubber { start, cur: world };
            }
            Drag::None | Drag::Minimap => {}
        }
    }
    if resp.drag_stopped() {
        match app.drag.clone() {
            Drag::Move { moved, .. } => {
                app.guides.clear();
                app.drag = Drag::None;
                let _ = moved;
            }
            Drag::Rubber { start, cur } => {
                let r = Rect::from_two_pos(start, cur);
                app.selected = app
                    .page()
                    .cells
                    .iter()
                    .filter(|c| c.vertex && app.page().layer_visible(c) && !app.page().layer_locked(c))
                    .filter(|c| r.contains(Pos2::new(c.geom.x + c.geom.w / 2.0, c.geom.y + c.geom.h / 2.0)))
                    .map(|c| c.id.clone())
                    .collect();
                app.drag = Drag::None;
            }
            _ => app.drag = Drag::None,
        }
    }

    // clicks
    if resp.clicked() {
        let Some(p) = resp.interact_pointer_pos() else { return };
        let world = s2w(p);
        match app.tool {
            Tool::Connect => {
                let target = pick_vertex(app.page(), world, false)
                    .filter(|c| app.edge_from.as_deref() != Some(c.id.as_str()))
                    .map(|c| c.id.clone());
                if let (Some(from), Some(target)) = (app.edge_from.clone(), target) {
                    app.push_undo();
                    let id = app
                        .edit_page(|page| page.add_edge(crate::model::DEFAULT_EDGE_STYLE, Some(from), Some(target)))
                        .unwrap_or_default();
                    app.edge_from = None;
                    app.selected = vec![id];
                    app.tool = Tool::Select;
                    app.status = "Edge added".into();
                }
            }
            Tool::Select => {
                // drawio selects on mouse-up. egui 0.36 postpones drag_started
                // for motionless presses, so a plain click owns edge selection
                // (vertex selection happens on press; keep it here).
                if pick_vertex(app.page(), world, false).is_some() {
                    // keep the press-time selection
                } else if let Some(id) = pick_edge(routes, p, app.zoom) {
                    if !app.selected.contains(&id) {
                        app.selected = vec![id];
                    }
                } else {
                    app.selected.clear();
                }
                app.ctx_menu = None;
            }
        }
    }

    // ── connection drags: follow the cursor, snap, commit on release ──
    if app.connect.active() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.connect = ConnectState::Idle;
            app.status = "Connection cancelled".into();
        } else {
            let world = ui.input(|i| i.pointer.latest_pos()).map(|pp| s2w(pp));
            let tol = 4.0 / app.zoom.max(0.2);
            if let Some(world) = world {
                let snap = connection::find_snap(app.page(), world, tol, None);
                match &mut app.connect {
                    ConnectState::Connecting { cur, snapped, .. } => {
                        *cur = world;
                        *snapped = snap;
                    }
                    ConnectState::EndpointDrag { cur, snapped, .. } => {
                        *cur = world;
                        *snapped = snap;
                    }
                    _ => {}
                }
                // live drawio-style status while a new connection is dragged
                if matches!(app.connect, ConnectState::Connecting { .. }) {
                    app.status = match &app.connect {
                        ConnectState::Connecting { snapped: Some(s), .. } if s.constraint.is_some() => {
                            let (fx, fy) = s.constraint.unwrap();
                            format!(
                                "Release to connect at fixed point [{}%, {}%] · Esc cancels",
                                (fx * 100.0).round(),
                                (fy * 100.0).round()
                            )
                        }
                        _ => "Release on a shape to connect · drop on empty for a dangling end · Esc cancels".into(),
                    };
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                commit_connect(app);
            }
        }
    }

    // double click: label editing / waypoint add / new text
    if resp.double_clicked() {
        let Some(p) = resp.interact_pointer_pos() else { return };
        let world = s2w(p);
        if let Some(c) = pick_vertex(app.page(), world, false) {
            let v = c.value.clone();
            app.edit_label = Some((c.id.clone(), v.clone(), v));
            app.edit_just_opened = true;
        } else if let Some(id) = pick_edge(routes, p, app.zoom) {
            // near an existing waypoint? remove it. near the midpoint? edit label. else add waypoint.
            let near_wp = app
                .page()
                .cell(&id)
                .and_then(|c| c.geom.points.iter().position(|w| Pos2::from(*w).distance(world) < 8.0 / app.zoom));
            if let Some(i) = near_wp {
                app.push_undo();
                app.edit_page(|page| {
                    if let Some(c) = page.cell_mut(&id) {
                        c.geom.points.remove(i);
                    }
                });
            } else {
                let (_, _, pts) = routes.iter().find(|(rid, _, _)| rid == &id).cloned().unwrap_or((String::new(), style::EdgeRoute::Orthogonal, Vec::new()));
                let mid_screen = edge_label_pos(&pts);
                let mid_world = s2w(mid_screen);
                let val = app.page().cell(&id).map(|c| c.value.clone()).unwrap_or_default();
                if mid_world.distance(world) < 24.0 / app.zoom.max(0.5) {
                    app.edit_label = Some((id.clone(), val.clone(), val));
                    app.edit_just_opened = true;
                } else {
                    app.push_undo();
                    let w = (world.x, world.y);
                    app.edit_page(|page| {
                        if let Some(c) = page.cell_mut(&id) {
                            let idx = nearest_segment(&c.geom.points, world);
                            c.geom.points.insert(idx, w);
                        }
                    });
                    app.status = "Waypoint added — drag to move, double-click to remove".into();
                }
            }
        } else {
            // drawio behavior: double-click on empty canvas creates a text label
            app.push_undo();
            let st = "text;strokeColor=none;fillColor=none;align=left;verticalAlign=middle;whiteSpace=wrap;html=1;";
            let g = app.grid;
            let x = ((world.x - 40.0) / g).round() * g;
            let y = ((world.y - 15.0) / g).round() * g;
            let id = app.edit_page(|page| page.add_vertex("", st, x, y, 80.0, 30.0)).unwrap_or_default();
            app.selected = vec![id.clone()];
            app.edit_label = Some((id, String::new(), String::new()));
            app.edit_just_opened = true;
        }
    }
    // right click: context menu
    if resp.secondary_clicked() {
        let Some(p) = resp.interact_pointer_pos() else { return };
        let world = s2w(p);
        let cell = pick_vertex(app.page(), world, true).map(|c| c.id.clone()).or_else(|| pick_edge(routes, p, app.zoom));
        if let Some(id) = cell.clone()
            && !app.selected.contains(&id)
        {
            app.selected = vec![id];
        }
        app.ctx_menu = Some(CtxMenu { pos: p, cell, world });
    }
}

// ── context menu ─────────────────────────────────────────────────────────────

/// Right-click menu state (`cell` = the picked cell, if any; `world` = menu origin in world space).
#[derive(Debug, Clone, PartialEq)]
pub struct CtxMenu {
    pub pos: Pos2,
    pub cell: Option<String>,
    pub world: Pos2,
}

fn context_menu(app: &mut DrawApp, ui: &mut Ui, _rect: Rect) {
    let Some(menu) = app.ctx_menu.clone() else { return };
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ctx_menu = None;
        return;
    }
    let t = Tokens::get(ui.ctx());
    let open_area = egui::Area::new(egui::Id::new("draw_ctx_menu"))
        .fixed_pos(menu.pos)
        .order(egui::Order::Foreground);
    let mut action: Option<CtxAction> = None;
    open_area.show(ui.ctx(), |ui| {
        egui::Frame::default()
            .fill(t.panel)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(5.0)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.set_min_width(150.0);
                let item = |ui: &mut Ui, label: &str, en: bool| -> bool {
                    let b = ui.add_enabled(en, egui::Button::new(egui::RichText::new(label).size(12.5)).min_size(Vec2::new(ui.available_width(), 22.0)));
                    b.clicked()
                };
                let has_cell = menu.cell.is_some();
                let page = app.page().clone();
                let locked = menu.cell.as_deref().and_then(|id| page.cell(id)).map(|c| page.layer_locked(c)).unwrap_or(false);
                if item(ui, "Edit Label", has_cell && !locked) {
                    action = Some(CtxAction::EditLabel);
                }
                if item(ui, "Copy", has_cell) {
                    action = Some(CtxAction::Copy);
                }
                if item(ui, "Duplicate", has_cell && !locked) {
                    action = Some(CtxAction::Duplicate);
                }
                if item(ui, "Delete", has_cell && !locked) {
                    action = Some(CtxAction::Delete);
                }
                ui.separator();
                if item(ui, "To Front", has_cell && !locked) {
                    action = Some(CtxAction::ToFront);
                }
                if item(ui, "To Back", has_cell && !locked) {
                    action = Some(CtxAction::ToBack);
                }
                ui.separator();
                if item(ui, "Paste Here", !app.clip.is_empty()) {
                    action = Some(CtxAction::PasteHere);
                }
                if item(ui, "Select All", true) {
                    action = Some(CtxAction::SelectAll);
                }
            });
    });
    // dismiss on any primary click anywhere (menu item clicks are handled above)
    let clicked_elsewhere = ui.input(|i| i.pointer.primary_clicked());
    if let Some(a) = action {
        app.ctx_menu = None;
        match a {
            CtxAction::EditLabel => {
                if let Some(id) = menu.cell {
                    let v = app.page().cell(&id).map(|c| c.value.clone()).unwrap_or_default();
                    app.edit_label = Some((id, v.clone(), v));
                    app.edit_just_opened = true;
                }
            }
            CtxAction::Copy => app.action_copy(),
            CtxAction::Duplicate => app.action_duplicate(),
            CtxAction::Delete => app.action_delete(),
            CtxAction::ToFront => app.action_to_front(),
            CtxAction::ToBack => app.action_to_back(),
            CtxAction::PasteHere => app.action_paste_at(menu.world),
            CtxAction::SelectAll => app.action_select_all(),
        }
    } else if clicked_elsewhere {
        app.ctx_menu = None;
    }
}

enum CtxAction {
    EditLabel,
    Copy,
    Duplicate,
    Delete,
    ToFront,
    ToBack,
    PasteHere,
    SelectAll,
}

// ── picking ──────────────────────────────────────────────────────────────────

fn pick_vertex(page: &Diagram, world: Pos2, include_locked: bool) -> Option<&Cell> {
    page.cells.iter().rev().find(|c| {
        c.vertex
            && page.layer_visible(c)
            && (include_locked || !page.layer_locked(c))
            && world.x >= c.geom.x
            && world.x <= c.geom.x + c.geom.w
            && world.y >= c.geom.y
            && world.y <= c.geom.y + c.geom.h
    })
}

fn pick_edge(routes: &[(String, style::EdgeRoute, Vec<Pos2>)], screen_p: Pos2, zoom: f32) -> Option<String> {
    // routes are in SCREEN space: hit-test with the screen pointer, not world
    let tol = 6.0 / zoom.max(0.3);
    routes.iter().rev().find_map(|(id, _, pts)| {
        pts.windows(2).any(|w| dist_to_seg(screen_p, w[0], w[1]) < tol).then(|| id.clone())
    })
}

fn dist_to_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 0.001 {
        return p.distance(a);
    }
    let t = (((p - a).dot(ab)) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Index in `points` after which a world waypoint should be inserted
/// (closest point on the waypoint polyline; ends map to 0 / len).
fn nearest_segment(points: &[(f32, f32)], world: Pos2) -> usize {
    let mut best = (f32::MAX, points.len());
    let chain: Vec<Pos2> = points.iter().map(|w| Pos2::from(*w)).collect();
    if chain.is_empty() {
        return 0;
    }
    for i in 0..chain.len() - 1 {
        let d = dist_to_seg(world, chain[i], chain[i + 1]);
        if d < best.0 {
            best = (d, i + 1);
        }
    }
    let d_first = world.distance(chain[0]);
    let d_last = world.distance(*chain.last().unwrap());
    if d_first < best.0 && d_first <= d_last {
        0
    } else if d_last < best.0 {
        points.len()
    } else {
        best.1
    }
}

// ── moving + guides ──────────────────────────────────────────────────────────

fn positions(app: &DrawApp, ids: &[String]) -> Vec<(String, f32, f32)> {
    let page = app.page().clone();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        for did in std::iter::once(id.clone()).chain(page.descendants(id)) {
            if seen.insert(did.clone())
                && let Some(c) = page.cell(&did)
                && c.vertex
            {
                out.push((did, c.geom.x, c.geom.y));
            }
        }
    }
    out
}

fn apply_positions(app: &mut DrawApp, orig: &[(String, f32, f32)], dx: f32, dy: f32) {
    app.edit_page(|page| {
        for (id, ox, oy) in orig {
            if let Some(c) = page.cell_mut(id) {
                c.geom.x = ox + dx;
                c.geom.y = oy + dy;
            }
        }
    });
}

/// Alignment guides: snap the moving bbox to other shapes' edges/centers.
/// Returns (dx_extra, dy_extra, guide lines in world space).
fn align_guides(page: &Diagram, moving: &[String], bbox: (f32, f32, f32, f32), tol: f32) -> (f32, f32, Vec<(bool, f32)>) {
    let (bx, by, bw, bh) = bbox;
    let xs = [bx, bx + bw / 2.0, bx + bw];
    let ys = [by, by + bh / 2.0, by + bh];
    let mut best_x: Option<(f32, f32, f32)> = None; // (|delta|, delta, guide_pos)
    let mut best_y: Option<(f32, f32, f32)> = None;
    for c in &page.cells {
        if !c.vertex || moving.contains(&c.id) || !page.layer_visible(c) {
            continue;
        }
        let (cx, cy, cw, ch) = (c.geom.x, c.geom.y, c.geom.w, c.geom.h);
        for vx in [cx, cx + cw / 2.0, cx + cw] {
            for mx in xs {
                let d = vx - mx;
                if d.abs() < tol && best_x.map(|(bd, _, _)| d.abs() < bd).unwrap_or(true) {
                    best_x = Some((d.abs(), d, vx));
                }
            }
        }
        for vy in [cy, cy + ch / 2.0, cy + ch] {
            for my in ys {
                let d = vy - my;
                if d.abs() < tol && best_y.map(|(bd, _, _)| d.abs() < bd).unwrap_or(true) {
                    best_y = Some((d.abs(), d, vy));
                }
            }
        }
    }
    let mut guides = Vec::new();
    let mut dx = 0.0;
    let mut dy = 0.0;
    if let Some((_, d, g)) = best_x {
        dx = d;
        guides.push((true, g));
    }
    if let Some((_, d, g)) = best_y {
        dy = d;
        guides.push((false, g));
    }
    (dx, dy, guides)
}

// ── connection UX (hover arrows, overlays, commit) ──────────────────────────

fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i32)
    } else {
        format!("{v:.3}")
    }
}

/// 5px "×" cross with an optional halo (fixed-point markers).
fn paint_cross(p: &egui::Painter, c: Pos2, half: f32, color: Color32, stroke: Stroke) {
    p.line_segment([Pos2::new(c.x - half, c.y - half), Pos2::new(c.x + half, c.y + half)], stroke);
    p.line_segment([Pos2::new(c.x - half, c.y + half), Pos2::new(c.x + half, c.y - half)], stroke);
    let _ = color;
}

/// Track the hovered vertex and draw the 4 drawio hover arrows (#29b6f2
/// triangles just outside the bounds, after ~140ms hover, or immediately for
/// the single selected shape). Drag from one starts a connection; click
/// creates a new rectangle connected in that direction.
fn hover_arrows(
    app: &mut DrawApp,
    ui: &mut Ui,
    rect: Rect,
    painter: &egui::Painter,
    w2s: &impl Fn(Pos2) -> Pos2,
    s2w: &impl Fn(Pos2) -> Pos2,
) {
    if app.tool != Tool::Select || app.edit_label.is_some() || app.drag != Drag::None || app.connect.active() || app.ctx_menu.is_some() {
        return;
    }
    let page = app.page().clone();
    let ptr = ui.input(|i| i.pointer.latest_pos());
    let hov_id = ptr.and_then(|p| pick_vertex(&page, s2w(p), false)).map(|c| c.id.clone());
    // keep hover alive while the pointer is on the arrows themselves
    let keep = match (&app.hover, ptr) {
        (Some((id, _)), Some(p)) => page.cell(id).map(|c| c.rect().expand(30.0).contains(s2w(p))).unwrap_or(false),
        _ => false,
    };
    // wall-clock hover timing (drawio: HoverIcons.activationDelay = 140 ms)
    let now = ui.input(|i| i.time) as f32;
    match (&app.hover, &hov_id) {
        (Some((id, t0)), Some(h)) if id == h => app.hover = Some((id.clone(), *t0)),
        (_, Some(h)) => app.hover = Some((h.clone(), now)),
        _ if keep => {}
        _ => app.hover = None,
    }
    if app.hover.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    // drawio shows arrows on the hovered vertex only, after 140 ms
    let Some((tid, _)) = app.hover.clone().filter(|(_, t0)| now - *t0 >= 0.14) else {
        return;
    };
    let Some(cell) = page.cell(&tid).cloned() else { return };
    if page.layer_locked(&cell) || !page.layer_visible(&cell) {
        return;
    }
    let shape_r = Rect::from_min_size(
        w2s(Pos2::new(cell.geom.x, cell.geom.y)),
        Vec2::new(cell.geom.w * app.zoom, cell.geom.h * app.zoom),
    );
    // HoverIcons.repaint: bds = state bbox grown by tolerance(0) + arrowSpacing(2);
    // Up sits at (cx − w/2, bds.top − h), Right at (bds.right, cy − h/2), …
    let b = shape_r.expand(ARROW_SPACING);
    let (cx, cy) = (shape_r.center().x, shape_r.center().y);
    let arrows: [(&[Pos2], (f32, f32), Pos2, (f32, f32)); 4] = [
        (&ARROW_UP_PTS, ARROW_UP_SIZE, Pos2::new(cx - ARROW_UP_SIZE.0 / 2.0, b.top() - ARROW_UP_SIZE.1), (0.5, 0.0)),
        (&ARROW_RIGHT_PTS, ARROW_RIGHT_SIZE, Pos2::new(b.right(), cy - ARROW_RIGHT_SIZE.1 / 2.0), (1.0, 0.5)),
        (&ARROW_DOWN_PTS, ARROW_DOWN_SIZE, Pos2::new(cx - ARROW_DOWN_SIZE.0 / 2.0, b.bottom()), (0.5, 1.0)),
        (&ARROW_LEFT_PTS, ARROW_LEFT_SIZE, Pos2::new(b.left() - ARROW_LEFT_SIZE.0, cy - ARROW_LEFT_SIZE.1 / 2.0), (0.0, 0.5)),
    ];
    let inside_shape = ptr.map(|p| shape_r.expand(8.0).contains(p)).unwrap_or(false);
    // "touch on the edge": arrows stay bright while the pointer is on the
    // shape or within 8 px of its border (drawio keeps HoverIcons active
    // across the whole shape bbox + tolerance, not only the interior)
    // drawio draws edge-endpoint handles ABOVE the hover icons — when a
    // selected edge's endpoint sits at a fixed point, its handle must win the
    // press over the shape's arrow (mxConstraintHandler/overlay pane order).
    let endpoint_handles: Vec<Rect> = app
        .selected
        .first()
        .and_then(|id| page.cell(id))
        .filter(|c| c.edge)
        .map(|c| connection::route_points(&page, c))
        .map(|pts| {
            pts.first()
                .into_iter()
                .chain(pts.last().into_iter())
                .map(|p| Rect::from_center_size(w2s(*p), Vec2::splat(14.0)))
                .collect()
        })
        .unwrap_or_default();
    for (pts, (aw, ah), pos, side_n) in arrows {
        let box_r = Rect::from_min_size(pos, Vec2::new(aw, ah));
        if !rect.intersects(box_r.expand(1.0)) {
            continue;
        }
        if endpoint_handles.iter().any(|h| h.intersects(box_r)) {
            continue;
        }
        // drawio issue #1029 ("Connection and arrows misbehave in tight spaces"):
        // skip an arrow that would sit on top of another cell
        let overlapped = page.cells.iter().any(|o| {
            o.vertex
                && o.id != cell.id
                && page.layer_visible(o)
                && box_r.intersects(Rect::from_min_size(
                    w2s(Pos2::new(o.geom.x, o.geom.y)),
                    Vec2::new(o.geom.w * app.zoom, o.geom.h * app.zoom),
                ))
        });
        if overlapped {
            continue;
        }
        let hresp = ui.allocate_rect(box_r.expand(1.0), Sense::click_and_drag());
        // inactiveOpacity 15 until the pointer reaches the shape or the arrow
        let alpha = if hresp.hovered() || hresp.dragged() || inside_shape { 255u8 } else { ARROW_INACTIVE_ALPHA };
        let fill = Color32::from_rgba_unmultiplied(0x29, 0xb6, 0xf2, alpha);
        let stroke_c = Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, alpha);
        let poly: Vec<Pos2> = pts.iter().map(|p| pos + p.to_vec2()).collect();
        painter.add(Shape::convex_polygon(poly, fill, Stroke::new(1.0, stroke_c)));
        // drawio starts the connection on mousedown on the arrow; egui 0.36
        // postpones drag_started for click_and_drag widgets and the arrows
        // disappear once the pointer leaves the hover zone — so read the raw
        // press instead of waiting for the drag decision.
        let press_on_arrow = ui.input(|i| i.pointer.primary_down()) && hresp.hovered();
        if press_on_arrow && !app.connect.active() {
            let cur = ui.input(|i| i.pointer.latest_pos().map(s2w).unwrap_or(box_r.center()));
            app.connect = ConnectState::Connecting {
                source: cell.id.clone(),
                side: Some(side_n),
                cur,
                snapped: None,
                press: cur,
            };
            app.status = "Release on a shape to connect · click for a new node · Esc cancels".into();
        }
    }
}

/// Click on a hover arrow: new default rectangle ~defaultEdgeLength away,
/// connected to the source with the default edge (exit pinned to that side).
fn create_connected_vertex(app: &mut DrawApp, src: &Cell, dir: Vec2) {
    app.push_undo();
    let (nw, nh) = (120.0f32, 60.0f32);
    let border = if dir.x != 0.0 { src.geom.w / 2.0 } else { src.geom.h / 2.0 };
    let gap = 120.0f32; // mxGraph defaultEdgeLength
    let mut cx = src.geom.x + src.geom.w / 2.0 + dir.x * (border + gap + nw / 2.0);
    let mut cy = src.geom.y + src.geom.h / 2.0 + dir.y * (border + gap + nh / 2.0);
    let g = app.grid.max(1.0);
    cx = (cx / g).round() * g;
    cy = (cy / g).round() * g;
    let side = if dir.x > 0.0 {
        (1.0, 0.5)
    } else if dir.x < 0.0 {
        (0.0, 0.5)
    } else if dir.y > 0.0 {
        (0.5, 1.0)
    } else {
        (0.5, 0.0)
    };
    let src_id = src.id.clone();
    let id = app
        .edit_page(|page| {
            let nid = page.add_vertex("", crate::model::DEFAULT_VERTEX_STYLE, cx - nw / 2.0, cy - nh / 2.0, nw, nh);
            let eid = page.add_edge(crate::model::DEFAULT_EDGE_STYLE, Some(src_id), Some(nid.clone()));
            if let Some(e) = page.cell_mut(&eid) {
                e.style = style::set(&e.style, "exitX", &fmt_num(side.0));
                e.style = style::set(&e.style, "exitY", &fmt_num(side.1));
            }
            nid
        })
        .unwrap_or_default();
    app.selected = vec![id];
    app.status = "Shape added + connected (endArrow=classic)".into();
}

/// During Connecting / EndpointDrag: preview edge with the REAL edge style,
/// fixed-point × markers, #00FF00 snap ring and the #00a8ff drop highlight.
fn connect_overlay(
    app: &DrawApp,
    ui: &Ui,
    rect: Rect,
    painter: &egui::Painter,
    w2s: &impl Fn(Pos2) -> Pos2,
    s2w: &impl Fn(Pos2) -> Pos2,
) {
    let _ = (ui, s2w);
    if !app.connect.active() {
        return;
    }
    let page = app.page().clone();
    let zoom = app.zoom;
    // live drag facts, used by every layer below
    let (cur_world, snap_now) = match &app.connect {
        ConnectState::Connecting { cur, snapped, .. } | ConnectState::EndpointDrag { cur, snapped, .. } => (*cur, snapped.clone()),
        _ => return,
    };
    let target_under = connection::hovering_vertex(&page, cur_world);
    let pinned = snap_now.as_ref().and_then(|s| s.constraint).is_some();
    let valid = pinned || target_under.is_some();
    let (style, a, b) = match &app.connect {
        ConnectState::Connecting { source, side, .. } => {
            let end = snap_now.as_ref().map(|s| s.pos).unwrap_or(cur_world);
            let a = match (*side, page.cell(source)) {
                (Some((fx, fy)), Some(c)) => {
                    let r = c.rect();
                    Pos2::new(r.min.x + fx * r.width(), r.min.y + fy * r.height())
                }
                (_, Some(c)) => connection::perimeter(&c.style, c.rect(), end),
                _ => end,
            };
            (crate::model::DEFAULT_EDGE_STYLE.to_string(), a, end)
        }
        ConnectState::EndpointDrag { edge, is_source, .. } => {
            let Some(ec) = page.cell(edge) else { return };
            let end = snap_now.as_ref().map(|s| s.pos).unwrap_or(cur_world);
            let Some(other) = connection::resolve_endpoint(&page, ec, !*is_source, end) else { return };
            let style = ec.style.clone();
            if *is_source {
                (style, end, other.pos)
            } else {
                (style, other.pos, end)
            }
        }
        _ => return,
    };

    // drawio mxConnectionHandler.createShape/getEdgeColor/getEdgeWidth: the
    // preview is a DASHED polyline — GREEN #00FF00 w3 over a valid target,
    // RED #FF0000 w1 over nothing; plain mxPolyline carries no arrow markers.
    let pts = connection::preview_route(&style, a, b);
    if pts.len() >= 2 {
        let spts: Vec<Pos2> = pts.iter().map(|p| w2s(*p)).collect();
        let (w, col) = if valid {
            ((3.0 * zoom).max(1.5), SNAP_GREEN)
        } else {
            ((1.0 * zoom).max(1.0), INVALID_RED)
        };
        painter.add(Shape::dashed_line(
            &spts,
            (w, col),
            6.0 * zoom.max(0.5),
            4.0 * zoom.max(0.5),
        ));
    }

    // drop-target outline (mxConnectionHandler updateCurrentState): GREEN w5
    // while snapped to a fixed connection point (OUTLINE_HIGHLIGHT_COLOR),
    // else BLUE #00a8ff w5 at 50% on a plain valid hover.
    let outline_of = |id: &str| -> Option<Rect> {
        page.cell(id).map(|c| {
            Rect::from_min_size(
                w2s(Pos2::new(c.geom.x, c.geom.y)),
                Vec2::new(c.geom.w * zoom, c.geom.h * zoom),
            )
        })
    };
    if pinned {
        if let Some(s) = &snap_now
            && let Some(r) = outline_of(&s.shape)
        {
            painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(5.0, SNAP_GREEN.gamma_multiply(0.5)), StrokeKind::Outside);
        }
    } else if let Some(h) = &target_under
        && let Some(r) = outline_of(h)
    {
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(5.0, TARGET_BLUE.gamma_multiply(0.5)), StrokeKind::Outside);
    }

    // fixed-point × markers on the shape the cursor is working on (drawio
    // constraint-handler focus points; #5627/#5683 keep them exact + stable)
    let mut marked: Vec<&str> = Vec::new();
    if let Some(s) = &snap_now {
        marked.push(s.shape.as_str());
    }
    if let Some(h) = &target_under {
        marked.push(h.as_str());
    }
    marked.dedup();
    for id in marked {
        let Some(c) = page.cell(id) else { continue };
        for p in connection::fixed_points_world(c) {
            let sp = w2s(p);
            if !rect.contains(sp) {
                continue;
            }
            paint_cross(painter, sp, 3.5, MARKER_BLUE, Stroke::new(3.0, MARKER_BLUE.gamma_multiply(0.35)));
            paint_cross(painter, sp, 2.5, MARKER_BLUE, Stroke::new(1.6, MARKER_BLUE));
        }
    }

    // snap dot: green circle exactly at the fixed point …
    if pinned
        && let Some(s) = &snap_now
    {
        let sp = w2s(s.pos);
        painter.circle_filled(sp, 6.0, Color32::from_rgba_unmultiplied(0x00, 0xff, 0x00, 128));
        painter.circle_stroke(sp, 6.0, Stroke::new(1.2, SNAP_GREEN.gamma_multiply(0.9)));
        // … plus drawio's edge-handler hint "«[x%, y%]»" (Graph.js updateHint)
        // in the geHint style: light box, 1px #ddd border, 9pt text, 80%.
        let (fx, fy) = s.constraint.unwrap();
        let text = format!("[{}%, {}%]", (fx * 100.0).round(), (fy * 100.0).round());
        let galley = painter.layout_no_wrap(text, FontId::proportional(10.5), Color32::from_rgb(0x33, 0x33, 0x33));
        let cs = w2s(cur_world);
        let pos = Pos2::new(cs.x - galley.size().x / 2.0 - 16.0, cs.y.max(sp.y) + HINT_OFFSET);
        let hr = Rect::from_min_size(pos, galley.size() + Vec2::new(32.0, 8.0));
        painter.rect_filled(hr, CornerRadius::same(3), Color32::from_white_alpha(204));
        painter.rect_stroke(hr, CornerRadius::same(3), Stroke::new(1.0, Color32::from_rgb(0xdd, 0xdd, 0xdd)), StrokeKind::Inside);
        painter.galley(hr.min + Vec2::new(16.0, 4.0), galley, Color32::from_rgb(0x33, 0x33, 0x33));
    }
}

/// Commit a connection drag: link / re-link / dangle per the snap state.
fn commit_connect(app: &mut DrawApp) {
    match app.connect.clone() {
        ConnectState::Connecting { source, side, cur, snapped, press } => {
            // drawio: press+release on an arrow without moving creates a new
            // connected node (mxGraph defaultEdgeLength away) — resolve the
            // click-vs-drag ambiguity here, at release time.
            if cur.distance(press) < 6.0
                && let Some(c) = app.page().cell(&source).cloned()
            {
                let (fx, fy) = side.unwrap_or((0.5, 0.5));
                let dir = Vec2::new(
                    if fx > 0.5 { 1.0 } else if fx < 0.5 { -1.0 } else { 0.0 },
                    if fy > 0.5 { 1.0 } else if fy < 0.5 { -1.0 } else { 0.0 },
                );
                create_connected_vertex(app, &c, dir);
                app.connect = ConnectState::Idle;
                return;
            }
            app.push_undo();
            let hover = connection::hovering_vertex(app.page(), cur);
            let sid = source.clone();
            let id = app
                .edit_page(|page| {
                    let (target, entry) = match &snapped {
                        Some(s) => (Some(s.shape.clone()), s.constraint),
                        None => (hover.clone(), None),
                    };
                    let id = page.add_edge(crate::model::DEFAULT_EDGE_STYLE, Some(sid), target.clone());
                    if let Some(c) = page.cell_mut(&id) {
                        if let Some((fx, fy)) = side {
                            c.style = style::set(&c.style, "exitX", &fmt_num(fx));
                            c.style = style::set(&c.style, "exitY", &fmt_num(fy));
                        }
                        if let Some((fx, fy)) = entry {
                            c.style = style::set(&c.style, "entryX", &fmt_num(fx));
                            c.style = style::set(&c.style, "entryY", &fmt_num(fy));
                        }
                        if target.is_none() {
                            c.geom.target_pt = Some((cur.x, cur.y));
                        }
                    }
                    id
                })
                .unwrap_or_default();
            if !id.is_empty() {
                app.selected = vec![id];
            }
            app.status = if snapped.map(|s| s.constraint.is_some()).unwrap_or(false) {
                "Connected at fixed point".into()
            } else {
                "Connected".into()
            };
        }
        ConnectState::EndpointDrag { edge, is_source, cur, snapped, press } => {
            if cur.distance(press) < 6.0 {
                // a plain click on an endpoint changes nothing
                app.status = "Endpoint unchanged".into();
            } else {
                app.push_undo();
                let hover = connection::hovering_vertex(app.page(), cur);
                app.edit_page(|page| {
                    if let Some(c) = page.cell_mut(&edge) {
                        match (&snapped, &hover) {
                            (Some(s), _) => c.link(is_source, Some(s.shape.clone()), s.constraint, None),
                            (None, Some(h)) => c.link(is_source, Some(h.clone()), None, None),
                            (None, None) => c.link(is_source, None, None, Some((cur.x, cur.y))),
                        }
                    }
                });
                app.status = if snapped.is_some() { "Endpoint re-linked".into() } else { "Endpoint released".into() };
            }
        }
        _ => {}
    }
    app.connect = ConnectState::Idle;
}

/// Screen midpoint of an edge path (label anchor).
fn edge_label_pos(pts: &[Pos2]) -> Pos2 {
    let mut total = 0.0;
    let segs: Vec<f32> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
    let half = segs.iter().sum::<f32>() / 2.0;
    for (i, len) in segs.iter().enumerate() {
        if total + *len >= half {
            let t = if *len > 0.0 { (half - total) / *len } else { 0.0 };
            return pts[i] + (pts[i + 1] - pts[i]) * t;
        }
        total += *len;
    }
    pts.last().copied().unwrap_or(Pos2::ZERO)
}

// ── edge painting (dash, jumps, arrows) ──────────────────────────────────────

fn apply_opacity(look: &mut Look, c: &Cell) {
    let o = style::get_f32(&c.style, "opacity", 100.0);
    if (0.0..100.0).contains(&o) {
        let f = (o / 100.0).clamp(0.05, 1.0);
        look.fill = look.fill.map(|c| c.gamma_multiply(f));
        look.stroke = look.stroke.gamma_multiply(f);
        look.text = look.text.gamma_multiply(f);
    }
}

fn paint_polyline(p: &egui::Painter, pts: &[Pos2], stroke: &Stroke, dash: &[f32]) {
    if dash.is_empty() {
        p.add(Shape::line(pts.to_vec(), *stroke));
    } else {
        for w in pts.windows(2) {
            dashed_segment(p, w[0], w[1], dash, *stroke);
        }
    }
}

fn dashed_segment(p: &egui::Painter, a: Pos2, b: Pos2, pattern: &[f32], stroke: Stroke) {
    let d = b - a;
    let len = d.length();
    if len < 0.5 {
        return;
    }
    let dir = d / len;
    let mut t = 0.0;
    let mut i = 0;
    let pat: Vec<f32> = if pattern.is_empty() { vec![6.0, 4.0] } else { pattern.to_vec() };
    let mut on = true;
    while t < len {
        let step = pat[i % pat.len()].max(0.5);
        let end = (t + step).min(len);
        if on {
            p.line_segment([a + dir * t, a + dir * end], stroke);
        }
        t = end;
        i += 1;
        on = !on;
    }
}

fn compute_jumps(id: &str, pts: &[Pos2], routes: &[(String, style::EdgeRoute, Vec<Pos2>)]) -> Vec<(usize, f32)> {
    let mut jumps = Vec::new();
    for (oid, _, opts) in routes {
        if oid == id || opts.len() < 2 {
            continue;
        }
        for (si, w) in pts.windows(2).enumerate() {
            for ow in opts.windows(2) {
                if let Some(t) = seg_t(w[0], w[1], ow[0], ow[1])
                    && (0.03..0.97).contains(&t)
                {
                    jumps.push((si, t));
                }
            }
        }
    }
    jumps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    jumps
}

/// Intersection parameter t of segment a1→a2 with b1→b2.
fn seg_t(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> Option<f32> {
    let r = a2 - a1;
    let s = b2 - b1;
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-6 {
        return None;
    }
    let qp = b1 - a1;
    let t = (qp.x * s.y - qp.y * s.x) / den;
    let u = (qp.x * r.y - qp.y * r.x) / den;
    (t > 0.001 && t < 0.999 && u > 0.001 && u < 0.999).then_some(t)
}

fn paint_edge(p: &egui::Painter, pts: &[Pos2], st: &str, look: &Look, jumps: &[(usize, f32)], zoom: f32) {
    let dash: Vec<f32> = if look.dashed { look.dash.iter().map(|d| d * zoom).collect() } else { Vec::new() };
    match style::edge_route(st) {
        style::EdgeRoute::Curved => paint_curve(p, pts, look, &dash, st),
        _ => {
            // path with jump arcs. Seed with the FIRST point: each window only
            // pushes its end, so without the seed a 2-point edge collapses to a
            // single point and paints nothing (and longer routes lose their
            // first segment).
            let mut path: Vec<Pos2> = Vec::with_capacity(pts.len() + jumps.len() * 6 + 1);
            if let Some(first) = pts.first() {
                path.push(*first);
            }
            let jr = (4.0 * zoom.max(0.5)).min(9.0);
            for (si, w) in pts.windows(2).enumerate() {
                let seg_jumps: Vec<f32> = jumps.iter().filter(|(s, _)| *s == si).map(|(_, t)| *t).collect();
                let seg_len = w[0].distance(w[1]);
                let dir = if seg_len > 0.001 { (w[1] - w[0]) / seg_len } else { Vec2::X };
                let mut last_t = 0.0f32;
                for t in seg_jumps {
                    let j = w[0] + (w[1] - w[0]) * t;
                    let dt = jr / seg_len.max(1.0);
                    let t0 = (t - dt).max(last_t);
                    let t1 = (t + dt).min(1.0);
                    if t0 > last_t {
                        path.push(w[0] + (w[1] - w[0]) * t0);
                    }
                    // semicircle bump over the crossing (bulge toward travel side)
                    let n = Vec2::new(-dir.y, dir.x);
                    for k in 1..6 {
                        let a = -std::f32::consts::FRAC_PI_2 + k as f32 / 6.0 * std::f32::consts::PI;
                        let p_on = j + dir * (a.cos() * jr) + n * (a.sin() * jr);
                        path.push(p_on);
                    }
                    last_t = t1;
                }
                if last_t < 1.0 {
                    path.push(w[1]);
                }
            }
            paint_polyline(p, &path, &look.stroke_w(), &dash);
            paint_edge_markers(p, st, look, &path, zoom);
        }
    }
}

/// Start/end markers read from the style string (kind, fill flag, size) and
/// painted with the full ArrowKind geometry.
fn paint_edge_markers(p: &egui::Painter, st: &str, look: &Look, path: &[Pos2], zoom: f32) {
    if path.len() < 2 {
        return;
    }
    let sw = look.width;
    let end = crate::style::get(st, "endArrow").and_then(ArrowKind::from_str).unwrap_or(ArrowKind::Classic);
    if end != ArrowKind::None {
        let b = *path.last().unwrap();
        let prev = path[path.len() - 2];
        let d0 = b - prev; let dl0 = d0.length(); let dir = if dl0 > 1e-6 { d0 / dl0 } else { Vec2::ZERO };
        if dir != Vec2::ZERO {
            let fill = style::get_bool(st, "endFill", true);
            let size = style::get_f32(st, "endSize", 6.0);
            paint_marker(p, b, dir, look.stroke, end, fill, size, sw, zoom);
        }
    }
    let start = style::get(st, "startArrow").and_then(ArrowKind::from_str);
    if let Some(start) = start.filter(|k| *k != ArrowKind::None) {
        let a = path[0];
        let d1 = a - path[1]; let dl1 = d1.length(); let dir = if dl1 > 1e-6 { d1 / dl1 } else { Vec2::ZERO };
        if dir != Vec2::ZERO {
            let fill = style::get_bool(st, "startFill", true);
            let size = style::get_f32(st, "startSize", 6.0);
            paint_marker(p, a, dir, look.stroke, start, fill, size, sw, zoom);
        }
    }
}

/// Smooth (Catmull-Rom sampled) curve through the route points.
fn paint_curve(p: &egui::Painter, pts: &[Pos2], look: &Look, dash: &[f32], st: &str) {
    if pts.len() < 3 {
        paint_polyline(p, pts, &look.stroke_w(), dash);
        paint_edge_markers(p, st, look, pts, 1.0);
        return;
    }
    let mut out = Vec::new();
    let ext = |i: usize| pts[i.clamp(0, pts.len() - 1)].to_vec2();
    for i in 0..pts.len() - 1 {
        let (p0, p1, p2, p3) = (ext(i.wrapping_sub(1)), pts[i].to_vec2(), pts[i + 1].to_vec2(), ext(i + 2));
        for k in 0..8 {
            let t = k as f32 / 8.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let v = 0.5
                * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
            out.push(Pos2::new(v.x, v.y));
        }
    }
    out.push(*pts.last().unwrap());
    paint_polyline(p, &out, &look.stroke_w(), dash);
    paint_edge_markers(p, st, look, &out, 1.0);
}

/// Paint one arrow marker of any [`ArrowKind`] at `tip` pointing along `dir`.
/// `size` is the style's `startSize`/`endSize` (default 6, model units),
/// `fill` picks filled vs outline geometry, `stroke_w` is the edge width.
pub fn paint_marker(p: &egui::Painter, tip: Pos2, dir: Vec2, color: Color32, kind: ArrowKind, fill: bool, size: f32, stroke_w: f32, zoom: f32) {
    let z = zoom.max(0.3);
    let s = (size * 1.5 + stroke_w * 2.0) * z; // overall length in screen px
    let sw = (stroke_w * z).max(1.0);
    let side = Vec2::new(-dir.y, dir.x);
    let back = tip - dir * s;
    let stroke = Stroke::new(sw, color);
    let poly = |pts: Vec<Pos2>, fill_color: Option<Color32>| {
        if let Some(fc) = fill_color {
            p.add(Shape::convex_polygon(pts.clone(), fc, Stroke::NONE));
        }
        let mut outline = pts.clone();
        if let Some(first) = pts.first() {
            outline.push(*first);
        }
        p.add(Shape::line(outline, stroke));
    };
    match kind {
        ArrowKind::None => {}
        ArrowKind::Classic | ArrowKind::ClassicThin => {
            let w = if kind == ArrowKind::Classic { 0.5 } else { 0.3 };
            poly(vec![tip, back + side * s * w, back - side * s * w], fill.then_some(color));
        }
        ArrowKind::Block | ArrowKind::BlockThin => {
            let w = if kind == ArrowKind::Block { 0.55 } else { 0.32 };
            poly(vec![tip, back + side * s * w, back - side * s * w], fill.then_some(color));
        }
        ArrowKind::Open | ArrowKind::OpenThin => {
            let w = if kind == ArrowKind::Open { 0.65 } else { 0.45 };
            let lw = if kind == ArrowKind::Open { sw } else { sw * 0.8 };
            let st2 = Stroke::new(lw, color);
            p.line_segment([tip, back + side * s * w], st2);
            p.line_segment([tip, back - side * s * w], st2);
        }
        ArrowKind::OpenAsync => {
            p.line_segment([tip, back + side * s * 0.9], stroke);
            p.line_segment([tip, back - side * s * 0.5], stroke);
        }
        ArrowKind::Async => {
            poly(vec![tip, back + side * s * 0.85, tip - dir * s * 0.55 - side * s * 0.45], fill.then_some(color));
        }
        ArrowKind::Oval | ArrowKind::Circle => {
            let c = tip - dir * s * 0.5;
            if fill {
                p.circle_filled(c, s * 0.5, color);
            } else {
                p.circle_stroke(c, s * 0.5, stroke);
            }
        }
        ArrowKind::Diamond | ArrowKind::DiamondThin => {
            let w = if kind == ArrowKind::Diamond { 0.4 } else { 0.25 };
            poly(
                vec![tip, tip - dir * s * 0.5 + side * s * w, tip - dir * s, tip - dir * s * 0.5 - side * s * w],
                fill.then_some(color),
            );
        }
        ArrowKind::Box => {
            poly(
                vec![tip, tip + side * s * 0.7, back + side * s * 0.7, back - side * s * 0.7, tip - side * s * 0.7],
                fill.then_some(color),
            );
        }
        ArrowKind::HalfCircle => {
            let r = s * 0.6;
            let pts: Vec<Pos2> = (0..=12)
                .map(|k| {
                    let th = -std::f32::consts::FRAC_PI_2 + k as f32 / 12.0 * std::f32::consts::PI;
                    tip + side * (r * th.sin()) - dir * (r * th.cos())
                })
                .collect();
            p.add(Shape::line(pts, stroke));
        }
        ArrowKind::Dash => {
            let bt = tip - dir * s * 0.5;
            p.line_segment([bt + side * s * 0.6, bt - side * s * 0.6], stroke);
        }
        ArrowKind::BaseDash => {
            let bt = tip - dir * s * 0.2;
            p.line_segment([bt + side * s * 0.4, bt - side * s * 0.4], stroke);
        }
        ArrowKind::Cross => {
            let c = tip - dir * s * 0.5;
            p.line_segment([c + side * s * 0.5, c - side * s * 0.5], stroke);
            p.line_segment([c + dir * s * 0.5, c - dir * s * 0.5], stroke);
        }
        ArrowKind::CirclePlus => {
            let c = tip - dir * s * 0.6;
            p.circle_stroke(c, s * 0.5, stroke);
            p.line_segment([c + side * s * 0.3, c - side * s * 0.3], stroke);
            p.line_segment([c + dir * s * 0.3, c - dir * s * 0.3], stroke);
        }
        ArrowKind::ErOne => {
            let bt = tip - dir * s * 0.5;
            p.line_segment([bt + side * s * 0.55, bt - side * s * 0.55], stroke);
        }
        ArrowKind::ErMandOne => {
            for t in [0.35, 0.65] {
                let bt = tip - dir * s * t;
                p.line_segment([bt + side * s * 0.5, bt - side * s * 0.5], stroke);
            }
        }
        ArrowKind::ErMany => {
            for f in [-0.6, 0.0, 0.6] {
                p.line_segment([tip - dir * s, tip + side * s * f], stroke);
            }
        }
        ArrowKind::ErOneToMany => {
            let bt = tip - dir * s * 0.75;
            p.line_segment([bt + side * s * 0.55, bt - side * s * 0.55], stroke);
            for f in [-0.6, 0.0, 0.6] {
                p.line_segment([tip - dir * s, tip + side * s * f], stroke);
            }
        }
        ArrowKind::ErZeroToOne => {
            p.circle_stroke(tip - dir * s * 0.85, s * 0.3, stroke);
            let bt = tip - dir * s * 0.35;
            p.line_segment([bt + side * s * 0.5, bt - side * s * 0.5], stroke);
        }
        ArrowKind::ErZeroToMany => {
            p.circle_stroke(tip - dir * s * 0.85, s * 0.3, stroke);
            for f in [-0.55, 0.0, 0.55] {
                p.line_segment([tip - dir * s * 0.45, tip + side * s * f], stroke);
            }
        }
        ArrowKind::DoubleBlock => {
            poly(vec![tip, back + side * s * 0.5, back - side * s * 0.5], fill.then_some(color));
            let b2 = back - dir * s * 0.9;
            poly(vec![back, b2 + side * s * 0.5, b2 - side * s * 0.5], fill.then_some(color));
        }
    }
}

// ── chrome ───────────────────────────────────────────────────────────────────

fn draw_grid(p: &egui::Painter, rect: Rect, pan: Vec2, zoom: f32, grid: f32, t: &Tokens) {
    let step = grid * zoom;
    if step < 4.0 {
        return;
    }
    let (minor, major) = grid_colors(t);
    let major_every = 5;
    let mut i = 0;
    let mut x = rect.min.x + pan.x % step;
    while x < rect.max.x {
        let col = if i % major_every == 0 { major } else { minor };
        p.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(0.6, col));
        x += step;
        i += 1;
    }
    let mut i = 0;
    let mut y = rect.min.y + pan.y % step;
    while y < rect.max.y {
        let col = if i % major_every == 0 { major } else { minor };
        p.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], Stroke::new(0.6, col));
        y += step;
        i += 1;
    }
}

pub fn handle_rect(r: Rect, i: u8, size: f32) -> Rect {
    let (x, y) = match i {
        0 => (r.left(), r.top()),
        1 => (r.center().x, r.top()),
        2 => (r.right(), r.top()),
        3 => (r.right(), r.center().y),
        4 => (r.right(), r.bottom()),
        5 => (r.center().x, r.bottom()),
        6 => (r.left(), r.bottom()),
        _ => (r.left(), r.center().y),
    };
    Rect::from_center_size(Pos2::new(x, y), Vec2::splat(size))
}

pub fn zoom_at(app: &mut DrawApp, canvas: Rect, pointer: Pos2, factor: f32) {
    let old = app.zoom;
    let new = (old * factor).clamp(0.2, 5.0);
    if (new - old).abs() < f32::EPSILON {
        return;
    }
    let world = to_world(app.pan, old, canvas.min, pointer);
    app.zoom = new;
    app.pan = pointer - canvas.min - world.to_vec2() * new;
}

/// Fit the page into the canvas view.
pub fn fit_page(app: &mut DrawApp) {
    let Some(rect) = app.view_rect else { return };
    let (pw, ph) = {
        let page = app.page().clone();
        (page.page_w, page.page_h)
    };
    let zw = (rect.width() - 60.0) / pw.max(1.0);
    let zh = (rect.height() - 60.0) / ph.max(1.0);
    app.zoom = zw.min(zh).clamp(0.2, 5.0);
    let center = Pos2::new(pw / 2.0, ph / 2.0);
    app.pan = rect.center() - rect.min - center.to_vec2() * app.zoom;
}

// ── minimap (outline) ────────────────────────────────────────────────────────

fn minimap(app: &mut DrawApp, ui: &mut Ui, canvas: Rect, painter: &egui::Painter, t: &Tokens) {
    let size = Vec2::new(160.0, 120.0);
    let mm = Rect::from_min_size(canvas.right_bottom() - Vec2::new(size.x + 12.0, size.y + 12.0), size);
    // sense drag for viewport control, click to jump
    let resp = ui.allocate_rect(mm, Sense::click_and_drag());
    painter.rect_filled(mm, CornerRadius::same(4), t.panel.gamma_multiply(0.92));
    painter.rect_stroke(mm, CornerRadius::same(4), Stroke::new(1.0, t.border), StrokeKind::Inside);

    let page = app.page().clone();
    // world bounds: page ∪ content
    let mut bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(page.page_w, page.page_h));
    for c in page.cells.iter().filter(|c| c.vertex && page.layer_visible(c)) {
        bounds = bounds.union(Rect::from_min_size(Pos2::new(c.geom.x, c.geom.y), Vec2::new(c.geom.w, c.geom.h)));
    }
    let pad = 8.0;
    let inner = mm.shrink(pad);
    let scale = (inner.width() / bounds.width().max(1.0)).min(inner.height() / bounds.height().max(1.0));
    let map = |p: Pos2| Pos2::new(mm.center().x + (p.x - bounds.center().x) * scale, mm.center().y + (p.y - bounds.center().y) * scale);

    // page outline
    let pg = Rect::from_two_pos(map(Pos2::ZERO), map(Pos2::new(page.page_w, page.page_h)));
    painter.rect_stroke(pg, CornerRadius::ZERO, Stroke::new(1.0, t.border_strong), StrokeKind::Inside);

    for c in page.cells.iter().filter(|c| page.layer_visible(c)) {
        if c.vertex {
            let r = Rect::from_two_pos(map(Pos2::new(c.geom.x, c.geom.y)), map(Pos2::new(c.geom.x + c.geom.w, c.geom.y + c.geom.h)));
            painter.rect_filled(r, CornerRadius::ZERO, t.text.gamma_multiply(0.55));
        } else {
            let pts = connection::route_points(&page, c);
            if pts.len() >= 2 {
                let sp: Vec<Pos2> = pts.iter().map(|p| map(*p)).collect();
                painter.add(Shape::line(sp, Stroke::new(1.0, t.text.gamma_multiply(0.47))));
            }
        }
    }

    // viewport
    let vp = viewport(app, canvas);
    let v1 = map(vp.min);
    let v2 = map(vp.max);
    painter.rect_stroke(Rect::from_two_pos(v1, v2), CornerRadius::ZERO, Stroke::new(1.2, t.accent), StrokeKind::Inside);

    // drag → recenter
    let jump = |app: &mut DrawApp, p: Pos2| {
        let world_x = bounds.center().x + (p.x - mm.center().x) / scale;
        let world_y = bounds.center().y + (p.y - mm.center().y) / scale;
        app.pan = canvas.center() - canvas.min - Vec2::new(world_x, world_y) * app.zoom;
    };
    if resp.dragged() {
        if let Some(p) = resp.interact_pointer_pos() {
            jump(app, p);
        }
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            jump(app, p);
        }
    }
}
