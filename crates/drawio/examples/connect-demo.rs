//! Headless-testable harness for the drawio connection engine.
//!
//! Opens a borderless window hosting [`DrawApp`] with two seeded shapes and
//! dumps the live connection state to `/tmp/drawio-edges.json` every frame:
//!
//! ```json
//! { "view": [x, y], "pan": [x, y], "zoom": 1.0,
//!   "shapes": [{ "id": "2", "x": 140.0, … }],
//!   "edges":  [{ "id": "3", "source": "2", "target": "4",
//!                "exitX": null, "exitY": null, "entryX": null, "entryY": null }] }
//! ```
//!
//! Driven by `shots/connect-test.sh`: xdotool drags from a hover arrow to the
//! other shape; the JSON asserts that the edge really LINKED (source/target +
//! exit/entry pinned), i.e. the drawio connection flow works end to end.

use doccraft_drawio::model::DEFAULT_VERTEX_STYLE;
use doccraft_drawio::ui::DrawApp;

struct Demo(DrawApp);

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // deterministic mapping for the driver: zoom 1, pan 0
        self.0.zoom = 1.0;
        self.0.pan = egui::Vec2::ZERO;
        let ctx = ui.ctx().clone();
        let _ = ctx;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(0x1e, 0x1e, 0x22)))
            .show(ui, |ui| self.0.ui(ui));

        // dump state for the test driver
        let page = self.0.page();
        let mut dump = String::from("{");
        if let Some(r) = self.0.view_rect {
            dump += &format!("\"view\":[{:.1},{:.1}],", r.min.x, r.min.y);
        }
        dump += &format!(
            "\"pan\":[{:.1},{:.1}],\"zoom\":{:.2},\"connect\":\"{}\",\"hover\":{},\"selected\":{},",
            self.0.pan.x, self.0.pan.y, self.0.zoom,
            match &self.0.connect {
                doccraft_drawio::connection::ConnectState::Idle => "Idle".into(),
                doccraft_drawio::connection::ConnectState::Connecting { source, .. } => format!("Connecting({source})"),
                doccraft_drawio::connection::ConnectState::EndpointDrag { edge, .. } => format!("EndpointDrag({edge})"),
            },
            self.0.hover.as_ref().map(|(id, _)| format!("\"{id}\"")).unwrap_or("null".into()),
            format!("[{}]", self.0.selected.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(",")),
        );
        dump += "\"shapes\":[";
        let mut first = true;
        for c in page.cells.iter().filter(|c| c.vertex) {
            if !first { dump += ","; }
            first = false;
            dump += &format!(
                "{{\"id\":\"{}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1}}}",
                c.id, c.geom.x, c.geom.y, c.geom.w, c.geom.h
            );
        }
        dump += "],\"edges\":[";
        first = true;
        let (pan, zoom) = (self.0.pan, self.0.zoom);
        for c in page.cells.iter().filter(|c| c.edge) {
            if !first { dump += ","; }
            first = false;
            // world-space route points (screen = view + pan + p*zoom)
            let pts = doccraft_drawio::connection::route_points(&page, c);
            let pts_s: Vec<String> = pts.iter()
                .map(|p| format!("[{:?},{:?}]", p.x, p.y))
                .collect();
            dump += &format!(
                "{{\"id\":\"{}\",\"source\":{},\"target\":{},\"exitX\":{},\"exitY\":{},\"entryX\":{},\"entryY\":{},\"route\":[{}],\"style\":\"{}\"}}",
                c.id,
                opt(&c.source),
                opt(&c.target),
                js(&style_num(&c.style, "exitX")),
                js(&style_num(&c.style, "exitY")),
                js(&style_num(&c.style, "entryX")),
                js(&style_num(&c.style, "entryY")),
                pts_s.join(","),
                c.style.replace('"', "'"),
            );
        }
        dump += "]}";
        // atomic publish: readers never see a torn write
        let tmp = std::path::Path::new("/tmp/drawio-edges.json.tmp");
        if std::fs::write(tmp, dump.as_bytes()).is_ok() {
            let _ = std::fs::rename(tmp, "/tmp/drawio-edges.json");
        }

        // DEBUG overlay: magenta route lines to discriminate engine-vs-paint.
        if std::env::var("DEMO_DEBUG").is_ok()
            && let Some(r) = self.0.view_rect
        {
            let (pan, zoom) = (self.0.pan, self.0.zoom);
            let w2s = |p: egui::Pos2| r.min + pan + p.to_vec2() * zoom;
            let painter = ui.painter_at(r);
            for c in page.cells.iter().filter(|c| c.edge) {
                let pts: Vec<egui::Pos2> =
                    doccraft_drawio::connection::route_points(&page, c).into_iter().map(w2s).collect();
                if pts.len() >= 2 {
                    painter.add(egui::Shape::line(
                        pts.clone(),
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(0xff, 0x00, 0xff)),
                    ));
                }
                for p in &pts {
                    painter.circle_filled(*p, 3.0, egui::Color32::from_rgb(0x00, 0xff, 0x00));
                }
            }
        }
    }
}

fn opt(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("\"{s}\""),
        None => "null".into(),
    }
}

fn style_num(style: &str, key: &str) -> Option<f32> {
    let v = doccraft_drawio::style::get_f32(style, key, f32::NAN);
    if v.is_finite() { Some(v) } else { None }
}

fn js(v: &Option<f32>) -> String {
    match v {
        Some(x) => format!("{x}"),
        None => "null".into(),
    }
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_decorations(false),
        ..Default::default()
    };
    // DEMO_PAGE=sample → the plain sample flowchart (user-scenario repro);
    // default → the two-shape connect-parity harness page.
    let sample = std::env::var("DEMO_PAGE").as_deref() == Ok("sample");
    eframe::run_native(
        "drawio connect demo",
        opts,
        Box::new(move |_cc| {
            let mut app = DrawApp::new();
            if sample {
                app.edit_page(|page| *page = DrawApp::new().file.diagrams[0].clone());
            } else {
                app.edit_page(|page| {
                    // two rectangles that both fit the default canvas viewport
                    page.add_vertex("Start", DEFAULT_VERTEX_STYLE, 120.0, 120.0, 200.0, 70.0);
                    page.add_vertex("End", DEFAULT_VERTEX_STYLE, 400.0, 320.0, 200.0, 70.0);
                });
            }
            Ok(Box::new(Demo(app)))
        }),
    )
}
