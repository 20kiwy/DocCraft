//! DocCraft Suite — one binary, the real storytold crafts inside it.
//!
//! WordCraft (Word), GridCraft (Excel), DeckCraft (PowerPoint), PdfCraft
//! (Acrobat) and FigCraft (Design) are the upstream apps from
//! github.com/storytold, linked as path dependencies from git submodules and
//! embedded in this shell, unchanged. Draw (drawio), Markdown,
//! Typeset and Notes are DocCraft's own native crafts, written in the same
//! pattern (model + engine + egui front end).
//!
//! The window uses a custom header (logo + window buttons) styled after the
//! storytold craft-launcher. `--register-associations` (Windows) registers
//! every supported file type so DocCraft appears in "Open with" on any PC.

mod crafts;
mod host;
mod register_associations;
mod services;

use host::HostApp;

fn main() -> eframe::Result {
    env_logger_ok();

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/doccraft-icon-256.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DocCraft")
        .with_app_id("ai.storyteller.doccraft")
        .with_inner_size([1440.0, 920.0])
        .with_min_inner_size([900.0, 560.0])
        .with_decorations(false); // custom header IS the title bar
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
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
