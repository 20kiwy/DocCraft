//! The drawing canvas: world↔screen mapping, hit tests, drag interactions
//! (move / resize / rubber-band / connect / pan / waypoints), alignment
//! guides, edge routing with line jumps, inline label editing, the outline
//! minimap and the right-click context menu.
//!
//! State lives on [`crate::ui::DrawApp`]; this module is the engine that
//! reads and mutates it every frame.

use crate::model::{Cell, Diagram};
use crate::shapes::{self, Xf};
use crate::style::{self, Look};
use crate::ui::{Drag, DrawApp, Tool};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(0xf0, 0x87, 0x2d);
const GUIDE: Color32 = Color32::from_rgb(0xff, 0x44, 0xd0);
const CANVAS_BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x22);
const GRID_MINOR: Color32 = Color32::from_rgb(0x27, 0x27, 0x2d);
const GRID_MAJOR: Color32 = Color32::from_rgb(0x30, 0x30, 0x38);
const HANDLE_FILL: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const HANDLE: f32 = 7.0;

// ── mapping ──────────────────────────────────────────────────────────────────

pub fn to_screen(pan: Vec2, zoom: f32, origin: Pos2, p: Pos2) -> Pos2 {
    origin + pan + p.to_vec2() * zoom
}

pub fn to_world(pan: Vec2, zoom: f32, origin: Pos2, p: Pos2) -> Pos2 {
    let v = (p - origin - pan) / zoom;
    Pos2::new(v.x, v.y)
}

/// The world-space rectangle currently visible on the canvas.
pub fn viewport(app: &DrawApp, canvas: Rect) -> Rect {
    let a = to_world(app.pan, app.zoom, canvas.min, canvas.left_top());
    let b = to_world(app.pan, app.zoom, canvas.min, canvas.right_bottom());
    Rect::from_two_pos(a, b)
}

// ── entry point ──────────────────────────────────────────────────────────────

pub fn show(app: &mut DrawApp, ui: &mut Ui) {
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    app.view_rect = Some(rect);
    let painter = ui.painter_at(rect.expand(1.0));
    painter.rect_filled(rect, CornerRadius::ZERO, CANVAS_BG);

    let page_setup = app.page();
    let (page_w, page_h, page_bg) = (page_setup.page_w, page_setup.page_h, page_setup.background.clone());
    let page_bg = page_bg.as_deref().and_then(style::parse_color).unwrap_or(Color32::from_rgb(0xf7, 0xf7, 0xf7));

    if app.show_grid {
        draw_grid(&painter, rect, app.pan, app.zoom, app.grid);
    }

    // page sheet with shadow
    let page = Rect::from_min_size(to_screen(app.pan, app.zoom, rect.min, Pos2::ZERO), Vec2::new(page_w * app.zoom, page_h * app.zoom));
    painter.rect_filled(page.translate(Vec2::new(4.0, 5.0)), CornerRadius::ZERO, Color32::from_black_alpha(70));
    painter.rect_filled(page, CornerRadius::ZERO, page_bg);
    painter.rect_stroke(page, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x5c)), StrokeKind::Outside);

    let pan = app.pan;
    let zoom = app.zoom;
    let w2s = move |p: Pos2| to_screen(pan, zoom, rect.min, p);
    let s2w = move |p: Pos2| to_world(pan, zoom, rect.min, p);

    // ── routes (screen space) for every visible edge ──
    let page = app.page();
    let routes: Vec<(String, style::EdgeRoute, Vec<Pos2>)> = page
        .cells
        .iter()
        .filter(|c| c.edge && page.layer_visible(c))
        .map(|c| {
            let world = route_points(page, c);
            (c.id.clone(), style::edge_route(&c.style), world.iter().map(|p| w2s(*p)).collect())
        })
        .collect();

    // ── cells in paint order ──
    let sel = app.selected.clone();
    let mut labels: Vec<(String, Pos2, f32)> = Vec::new(); // edge labels (id, screen midpoint, size)
    {
        let page = app.page();
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
                    paint_polyline(&painter, pts, &Stroke::new(4.0 * zoom.max(0.5), ACCENT.linear_multiply(0.35)), &[]);
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
                    painter.rect_filled(r, CornerRadius::same(2), ACCENT.linear_multiply(0.12));
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
            painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, Color32::from_rgb(0x40, 0xc0, 0x60)), StrokeKind::Outside);
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
            painter.rect_stroke(bg, CornerRadius::same(2), Stroke::new(1.0, ACCENT), StrokeKind::Outside);
        }
    }

    // ── selection chrome ──
    if let Some(id) = sel.first().cloned()
        && let Some(c) = app.page().cell(&id)
        && c.vertex
    {
        let r = Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.2, ACCENT), StrokeKind::Outside);
        let hs = HANDLE * zoom.max(0.6).min(1.4);
        for i in 0..8u8 {
            let hr = handle_rect(r, i, hs);
            painter.rect_filled(hr, CornerRadius::same(1), HANDLE_FILL);
            painter.rect_stroke(hr, CornerRadius::same(1), Stroke::new(1.0, ACCENT), StrokeKind::Outside);
        }
    }
    if sel.len() > 1
        && let Some((bx, by, bw, bh)) = app.page().bbox(&sel)
    {
        let r = Rect::from_min_size(w2s(Pos2::new(bx, by)), Vec2::new(bw * zoom, bh * zoom));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, ACCENT.linear_multiply(0.8)), StrokeKind::Outside);
    }
    // single selected edge: waypoint handles
    if let Some(id) = sel.first().cloned()
        && let Some(c) = app.page().cell(&id)
        && c.edge
        && let Some((_, _, pts)) = routes.iter().find(|(rid, _, _)| rid == &id)
    {
        for (i, w) in c.geom.points.iter().enumerate() {
            let p = w2s(Pos2::from(*w));
            let r = Rect::from_center_size(p, Vec2::splat(7.0));
            painter.rect_filled(r, CornerRadius::same(1), HANDLE_FILL);
            painter.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.0, ACCENT), StrokeKind::Outside);
            let _ = i;
        }
    }

    // hover connection arrows (drawio-style) for a single selected vertex
    if app.tool == Tool::Select && app.drag == Drag::None && sel.len() == 1 && app.edge_from.is_none() && app.edit_label.is_none() {
        let sel_rect = sel
            .first()
            .and_then(|id| app.page().cell(id))
            .filter(|c| c.vertex)
            .map(|c| (c.id.clone(), Rect::from_min_size(w2s(Pos2::new(c.geom.x, c.geom.y)), Vec2::new(c.geom.w * zoom, c.geom.h * zoom))));
        if let Some((id, r)) = sel_rect {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                let dir = Vec2::new(dx, dy);
                let center = Pos2::new(r.center().x + dir.x * (r.width() / 2.0 + 13.0), r.center().y + dir.y * (r.height() / 2.0 + 13.0));
                let ar = Rect::from_center_size(center, Vec2::splat(15.0));
                if !rect.contains(center) {
                    continue;
                }
                let hresp = ui.allocate_rect(ar, Sense::click_and_drag());
                let hov = hresp.hovered();
                painter.circle_filled(center, 7.0, if hov { ACCENT } else { ACCENT.linear_multiply(0.85) });
                arrow_glyph(&painter, center, dir, HANDLE_FILL, 0.8, false);
                if hresp.drag_started() {
                    app.push_undo();
                    app.drag = Drag::Edge { src: id.clone(), cur: s2w(ui.input(|i| i.pointer.latest_pos()).unwrap_or(center)) };
                } else if hresp.clicked() {
                    app.edge_from = Some(id.clone());
                    app.status = "Connect: click the target shape (Esc to cancel)".into();
                }
            }
        }
    }

    // ── overlays: guides, rubber band, edge preview ──
    for g in &app.guides {
        let (v, pos) = *g;
        let vp = viewport(app, rect);
        let (a, b) = if v {
            (Pos2::new(pos, vp.top()), Pos2::new(pos, vp.bottom()))
        } else {
            (Pos2::new(vp.left(), pos), Pos2::new(vp.right(), pos))
        };
        painter.line_segment([w2s(a), w2s(b)], Stroke::new(1.0, GUIDE));
    }
    if let Drag::Rubber { start, cur } = app.drag.clone() {
        let a = w2s(start);
        let b = w2s(cur);
        let r = Rect::from_two_pos(a, b);
        painter.rect_filled(r, CornerRadius::ZERO, ACCENT.linear_multiply(0.10));
        painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    }
    if let Drag::Edge { src, cur } = app.drag.clone() {
        let page = app.page();
        if let Some(c) = page.cell(&src) {
            let a = w2s(anchor(page, c, cur));
            let b = w2s(cur);
            painter.line_segment([a, b], Stroke::new(1.6, ACCENT));
            arrow_glyph(&painter, b, (b - a).normalized(), ACCENT, 0.9, true);
            // highlight potential target
            if let Some(t) = pick_vertex(app.page(), cur, false)
                && t.id != src
            {
                let tr = Rect::from_min_size(w2s(Pos2::new(t.geom.x, t.geom.y)), Vec2::new(t.geom.w * zoom, t.geom.h * zoom));
                painter.rect_stroke(tr, CornerRadius::same(2), Stroke::new(2.0, ACCENT), StrokeKind::Outside);
            }
        }
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
        minimap(app, ui, rect, &painter);
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
                let picked = pick_vertex(app.page(), world, false).map(|c| (c.id.clone(), c.geom.x, c.geom.y));
                if let Some((id, px, py)) = picked {
                    if !app.selected.contains(&id) {
                        let mods = ui.input(|i| i.modifiers);
                        if mods.shift {
                            app.selected.push(id.clone());
                        } else {
                            app.selected = vec![id.clone()];
                        }
                    }
                    let grab = Vec2::new(world.x - px, world.y - py);
                    let ids = app.selected.clone();
                    let orig = positions(app, &ids);
                    app.push_undo();
                    app.drag = Drag::Move { ids, grab, orig, moved: false };
                } else if let Some(id) = pick_edge(routes, world, app.zoom) {
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
            Drag::Edge { src, .. } => {
                app.drag = Drag::Edge { src, cur: world };
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
            Drag::Edge { src, cur } => {
                app.drag = Drag::None;
                let target = pick_vertex(app.page(), cur, false).map(|c| c.id.clone()).filter(|id| id.as_str() != src);
                let estyle = "edgeStyle=orthogonalEdgeStyle;rounded=0;html=1;endArrow=classic;";
                let id = match target {
                    Some(t) => app.edit_page(|page| page.add_edge(estyle, Some(src), Some(t))).unwrap_or_default(),
                    None => {
                        let id = app.edit_page(|page| page.add_edge(estyle, Some(src), None)).unwrap_or_default();
                        app.edit_page(|page| {
                            if let Some(c) = page.cell_mut(&id) {
                                c.geom.source_pt = None;
                                c.geom.target_pt = Some((cur.x, cur.y));
                            }
                        });
                        id
                    }
                };
                if !id.is_empty() {
                    app.selected = vec![id];
                }
                app.status = "Connected".into();
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
                    let estyle = "edgeStyle=orthogonalEdgeStyle;rounded=0;html=1;endArrow=classic;";
                    let id = app.edit_page(|page| page.add_edge(estyle, Some(from), Some(target))).unwrap_or_default();
                    app.edge_from = None;
                    app.selected = vec![id];
                    app.tool = Tool::Select;
                    app.status = "Edge added".into();
                }
            }
            Tool::Select => {
                // plain click without drag on empty: clear selection
                if pick_vertex(app.page(), world, false).is_none() && pick_edge(routes, world, app.zoom).is_none() {
                    app.selected.clear();
                }
                app.ctx_menu = None;
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
        } else if let Some(id) = pick_edge(routes, world, app.zoom) {
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
        let cell = pick_vertex(app.page(), world, true).map(|c| c.id.clone()).or_else(|| pick_edge(routes, world, app.zoom));
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
    let open_area = egui::Area::new(egui::Id::new("draw_ctx_menu"))
        .fixed_pos(menu.pos)
        .order(egui::Order::Foreground);
    let mut action: Option<CtxAction> = None;
    open_area.show(ui.ctx(), |ui| {
        egui::Frame::default()
            .fill(Color32::from_rgb(0x2a, 0x2a, 0x30))
            .stroke(Stroke::new(1.0, Color32::from_rgb(0x45, 0x45, 0x4e)))
            .corner_radius(5.0)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.set_min_width(150.0);
                let item = |ui: &mut Ui, label: &str, en: bool| -> bool {
                    let b = ui.add_enabled(en, egui::Button::new(egui::RichText::new(label).size(12.5)).min_size(Vec2::new(ui.available_width(), 22.0)));
                    b.clicked()
                };
                let has_cell = menu.cell.is_some();
                let page = app.page();
                let locked = menu.cell.as_deref().and_then(|id| page.cell(id)).map(|c| page.layer_locked(c)).unwrap_or(false);
                if item(ui, "✏ Edit Label", has_cell && !locked) {
                    action = Some(CtxAction::EditLabel);
                }
                if item(ui, "⧉ Copy", has_cell) {
                    action = Some(CtxAction::Copy);
                }
                if item(ui, "⧉⧉ Duplicate", has_cell && !locked) {
                    action = Some(CtxAction::Duplicate);
                }
                if item(ui, "🗑 Delete", has_cell && !locked) {
                    action = Some(CtxAction::Delete);
                }
                ui.separator();
                if item(ui, "⤒ To Front", has_cell && !locked) {
                    action = Some(CtxAction::ToFront);
                }
                if item(ui, "⤓ To Back", has_cell && !locked) {
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

fn pick_edge(routes: &[(String, style::EdgeRoute, Vec<Pos2>)], world: Pos2, zoom: f32) -> Option<String> {
    let tol = 6.0 / zoom.max(0.3);
    routes.iter().rev().find_map(|(id, _, pts)| {
        pts.windows(2).any(|w| dist_to_seg(world, w[0], w[1]) < tol).then(|| id.clone())
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
    let page = app.page();
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

// ── edge routing ─────────────────────────────────────────────────────────────

fn anchor(page: &Diagram, cell: &Cell, toward: Pos2) -> Pos2 {
    let r = Rect::from_min_size(Pos2::new(cell.geom.x, cell.geom.y), Vec2::new(cell.geom.w, cell.geom.h));
    shapes::anchor_on_rect(r, toward)
}

fn endpoint(page: &Diagram, id: &Option<String>, pt: &Option<(f32, f32)>, toward: Pos2) -> Option<Pos2> {
    if let Some(id) = id
        && let Some(c) = page.cell(id)
        && c.vertex
    {
        return Some(anchor(page, c, toward));
    }
    pt.map(Pos2::from)
}

/// World-space routing points for one edge.
pub fn route_points(page: &Diagram, cell: &Cell) -> Vec<Pos2> {
    let center = |id: &Option<String>, fallback: &Option<(f32, f32)>| -> Option<Pos2> {
        id.as_deref()
            .and_then(|id| page.cell(id))
            .map(|c| Pos2::new(c.geom.x + c.geom.w / 2.0, c.geom.y + c.geom.h / 2.0))
            .or_else(|| fallback.map(Pos2::from))
    };
    let a_center = center(&cell.source, &cell.geom.source_pt);
    let b_center = center(&cell.target, &cell.geom.target_pt);
    let (Some(ac), Some(bc)) = (a_center, b_center) else { return Vec::new() };
    let a = endpoint(page, &cell.source, &cell.geom.source_pt, bc).unwrap_or(ac);
    let b = endpoint(page, &cell.target, &cell.geom.target_pt, ac).unwrap_or(bc);
    let wps: Vec<Pos2> = cell.geom.points.iter().map(|w| Pos2::from(*w)).collect();
    match route_kind(&cell.style) {
        style::EdgeRoute::Straight | style::EdgeRoute::Curved => {
            let mut pts = vec![a];
            pts.extend(wps);
            pts.push(b);
            pts
        }
        style::EdgeRoute::Orthogonal => {
            let mut pts = vec![a];
            let mut cur = a;
            for w in wps {
                pts.extend(elbow(cur, w));
                pts.push(w);
                cur = w;
            }
            pts.extend(elbow(cur, b));
            pts.push(b);
            pts
        }
    }
}

fn route_kind(st: &str) -> style::EdgeRoute {
    match style::get(st, "edgeStyle") {
        Some("straightEdgeStyle") | Some("none") => style::EdgeRoute::Straight,
        Some("orthogonalEdgeStyle") | Some("elbowEdgeStyle") => {
            if style::get_bool(st, "curved", false) {
                style::EdgeRoute::Curved
            } else {
                style::EdgeRoute::Orthogonal
            }
        }
        _ => {
            if style::get_bool(st, "curved", false) {
                style::EdgeRoute::Curved
            } else {
                style::EdgeRoute::Orthogonal
            }
        }
    }
}

fn elbow(p: Pos2, q: Pos2) -> Vec<Pos2> {
    if (p.x - q.x).abs() < 0.5 || (p.y - q.y).abs() < 0.5 {
        Vec::new()
    } else if (q.x - p.x).abs() >= (q.y - p.y).abs() {
        vec![Pos2::new(q.x, p.y)]
    } else {
        vec![Pos2::new(p.x, q.y)]
    }
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
            // path with jump arcs
            let mut path: Vec<Pos2> = Vec::with_capacity(pts.len() + jumps.len() * 6);
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
            // arrowheads
            let end_arrow = style::get(st, "endArrow").unwrap_or("classic");
            if end_arrow != "none" && path.len() >= 2 {
                let b = *path.last().unwrap();
                let prev = path[path.len() - 2];
                arrow_head(p, b, (b - prev).normalized(), look.stroke, end_arrow, zoom);
            }
            if let Some(start_arrow) = style::get(st, "startArrow")
                && start_arrow != "none"
                && path.len() >= 2
            {
                let a = path[0];
                arrow_head(p, a, (path[0] - path[1]).normalized(), look.stroke, start_arrow, zoom);
            }
        }
    }
}

/// Smooth (Catmull-Rom sampled) curve through the route points.
fn paint_curve(p: &egui::Painter, pts: &[Pos2], look: &Look, dash: &[f32], st: &str) {
    if pts.len() < 3 {
        paint_polyline(p, pts, &look.stroke_w(), dash);
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
    let b = *out.last().unwrap();
    let prev = out[out.len() - 2];
    let end_arrow = style::get(st, "endArrow").unwrap_or("classic");
    if end_arrow != "none" {
        arrow_head(p, b, (b - prev).normalized(), look.stroke, end_arrow, 1.0);
    }
}

fn arrow_head(p: &egui::Painter, tip: Pos2, dir: Vec2, color: Color32, kind: &str, zoom: f32) {
    let s = 8.0 * zoom.max(0.5);
    let side = Vec2::new(-dir.y, dir.x);
    let back = tip - dir * s;
    match kind {
        "open" => {
            p.line_segment([back + side * s * 0.6, tip], Stroke::new(1.4 * zoom.max(0.5), color));
            p.line_segment([back - side * s * 0.6, tip], Stroke::new(1.4 * zoom.max(0.5), color));
        }
        "oval" => {
            p.circle_filled(tip - dir * s * 0.4, s * 0.4, color);
        }
        "diamond" => {
            p.add(Shape::convex_polygon(
                vec![tip, tip - dir * s * 0.55 + side * s * 0.4, tip - dir * s, tip - dir * s * 0.55 - side * s * 0.4],
                color,
                Stroke::NONE,
            ));
        }
        "ERone" => {
            p.line_segment([tip - dir * s * 0.5 + side * s * 0.5, tip - dir * s * 0.5 - side * s * 0.5], Stroke::new(1.6 * zoom.max(0.5), color));
        }
        "ERmany" => {
            for f in [-0.6, 0.0, 0.6] {
                p.line_segment([tip - dir * s * 0.6, tip + side * s * f], Stroke::new(1.4 * zoom.max(0.5), color));
            }
        }
        "ERmandOne" => {
            for t in [0.35, 0.65] {
                let bt = tip - dir * s * t;
                p.line_segment([bt + side * s * 0.5, bt - side * s * 0.5], Stroke::new(1.6 * zoom.max(0.5), color));
            }
        }
        "ERoneToMany" => {
            let bt = tip - dir * s * 0.4;
            p.line_segment([bt + side * s * 0.5, bt - side * s * 0.5], Stroke::new(1.6 * zoom.max(0.5), color));
            for f in [-0.6, 0.0, 0.6] {
                p.line_segment([tip - dir * s * 0.2, tip + side * s * f], Stroke::new(1.4 * zoom.max(0.5), color));
            }
        }
        "ERzeroToOne" => {
            p.circle_stroke(tip - dir * s * 0.75, s * 0.22, Stroke::new(1.2 * zoom.max(0.5), color));
            let bt = tip - dir * s * 0.3;
            p.line_segment([bt + side * s * 0.5, bt - side * s * 0.5], Stroke::new(1.6 * zoom.max(0.5), color));
        }
        // "classic" and anything else: filled triangle
        _ => {
            p.add(Shape::convex_polygon(
                vec![tip, back + side * s * 0.5, back - side * s * 0.5],
                color,
                Stroke::NONE,
            ));
        }
    }
}

fn arrow_glyph(p: &egui::Painter, c: Pos2, dir: Vec2, color: Color32, scale: f32, open: bool) {
    let s = 8.0 * scale;
    let side = Vec2::new(-dir.y, dir.x);
    if open {
        p.line_segment([c - dir * s + side * s * 0.7, c], Stroke::new(1.5 * scale, color));
        p.line_segment([c - dir * s - side * s * 0.7, c], Stroke::new(1.5 * scale, color));
    } else {
        p.add(Shape::convex_polygon(vec![c + dir * s * 0.6, c - dir * s * 0.5 + side * s * 0.5, c - dir * s * 0.5 - side * s * 0.5], color, Stroke::NONE));
    }
}

// ── chrome ───────────────────────────────────────────────────────────────────

fn draw_grid(p: &egui::Painter, rect: Rect, pan: Vec2, zoom: f32, grid: f32) {
    let step = grid * zoom;
    if step < 4.0 {
        return;
    }
    let major_every = 5;
    let mut i = 0;
    let mut x = rect.min.x + pan.x % step;
    while x < rect.max.x {
        let col = if i % major_every == 0 { GRID_MAJOR } else { GRID_MINOR };
        p.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(0.6, col));
        x += step;
        i += 1;
    }
    let mut i = 0;
    let mut y = rect.min.y + pan.y % step;
    while y < rect.max.y {
        let col = if i % major_every == 0 { GRID_MAJOR } else { GRID_MINOR };
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
        let page = app.page();
        (page.page_w, page.page_h)
    };
    let zw = (rect.width() - 60.0) / pw.max(1.0);
    let zh = (rect.height() - 60.0) / ph.max(1.0);
    app.zoom = zw.min(zh).clamp(0.2, 5.0);
    let center = Pos2::new(pw / 2.0, ph / 2.0);
    app.pan = rect.center() - rect.min - center.to_vec2() * app.zoom;
}

// ── minimap (outline) ────────────────────────────────────────────────────────

fn minimap(app: &mut DrawApp, ui: &mut Ui, canvas: Rect, painter: &egui::Painter) {
    let size = Vec2::new(160.0, 120.0);
    let mm = Rect::from_min_size(canvas.right_bottom() - Vec2::new(size.x + 12.0, size.y + 12.0), size);
    // sense drag for viewport control, click to jump
    let resp = ui.allocate_rect(mm, Sense::click_and_drag());
    painter.rect_filled(mm, CornerRadius::same(4), Color32::from_rgba_premultiplied(0x26, 0x26, 0x2b, 235));
    painter.rect_stroke(mm, CornerRadius::same(4), Stroke::new(1.0, Color32::from_rgb(0x45, 0x45, 0x4e)), StrokeKind::Inside);

    let page = app.page();
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
    painter.rect_stroke(pg, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(0x60, 0x60, 0x6a)), StrokeKind::Inside);

    for c in page.cells.iter().filter(|c| page.layer_visible(c)) {
        if c.vertex {
            let r = Rect::from_two_pos(map(Pos2::new(c.geom.x, c.geom.y)), map(Pos2::new(c.geom.x + c.geom.w, c.geom.y + c.geom.h)));
            painter.rect_filled(r, CornerRadius::ZERO, Color32::from_rgba_premultiplied(0xd0, 0xd0, 0xd8, 140));
        } else {
            let pts = route_points(page, c);
            if pts.len() >= 2 {
                let sp: Vec<Pos2> = pts.iter().map(|p| map(*p)).collect();
                painter.add(Shape::line(sp, Stroke::new(1.0, Color32::from_rgba_premultiplied(0xd0, 0xd0, 0xd8, 120))));
            }
        }
    }

    // viewport
    let vp = viewport(app, canvas);
    let v1 = map(vp.min);
    let v2 = map(vp.max);
    painter.rect_stroke(Rect::from_two_pos(v1, v2), CornerRadius::ZERO, Stroke::new(1.2, ACCENT), StrokeKind::Inside);

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
