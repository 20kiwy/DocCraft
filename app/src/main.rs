//! DocCraft 3.0 — one binary, the real storytold crafts inside it.
//!
//! WordCraft (Word), GridCraft (Excel), DeckCraft (PowerPoint) and PdfCraft
//! (Acrobat) are the upstream apps from github.com/storytold, linked as path
//! dependencies from git submodules and embedded in this shell, unchanged.
//! Draw (drawio), Markdown and Typeset are DocCraft's own native crafts,
//! written in the same pattern (model + engine + egui front end).

mod crafts;
mod host;
mod services;

use host::HostApp;

fn main() -> eframe::Result {
    env_logger_ok();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DocCraft")
            .with_app_id("ai.storyteller.doccraft")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DocCraft",
        options,
        Box::new(|cc| {
            // Craft-accurate typography: register the fonts the sandbox has so
            // the crafts' FontDb probes find something real.
            install_system_fonts(&cc.egui_ctx);
            Ok(Box::new(HostApp::new(cc)))
        }),
    )
}

fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dirs = ["/usr/share/fonts", "/usr/local/share/fonts"];
    let mut installed = 0;
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttf")) {
                if let Some(name) = p.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(bytes) = std::fs::read(&p) {
                        fonts.font_data.insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                        fonts.families.entry(egui::FontFamily::Proportional).or_default().push(name.to_string());
                        installed += 1;
                    }
                }
            }
        }
    }
    ctx.set_fonts(fonts);
    log::info!("doccraft: installed {installed} system font faces");
}

fn env_logger_ok() {
    if std::env::var_os("RUST_LOG").is_none() {
        // SAFETY: single-threaded startup, before any threads read the env.
        unsafe { std::env::set_var("RUST_LOG", "warn") };
    }
}
