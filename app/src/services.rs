//! All-`None` host services for the embedded crafts.
//!
//! The real apps wire these to native file dialogs, printing and the system
//! clipboard. Inside DocCraft the crafts run hosted: the host implements the
//! hooks it can (clipboard text) and leaves the rest `None`, which every
//! upstream UI already handles (built-in file browser, egui clipboard, …).

use deckcraft_ui_egui as deck_ui;
use gridcraft_ui_egui as grid_ui;
use wordcraft_ui_egui as word_ui;

pub fn word_services() -> word_ui::Services {
    word_ui::Services {
        printers: None,
        print_native: None,
        pick_open: None,
        pick_save: None,
        file_dialog: None,
        open_async: None,
        inbox: None,
        pasted: None,
        download: None,
        async_saves: None,
        on_dirty: None,
        print: None,
        clipboard_picture: None,
        animations_enabled: None,
        clipboard_text: None,
        clipboard_html: None,
        set_clipboard_html: None,
        clipboard_rtf: None,
        clipboard_formats: None,
        store_font: None,
        forget_font: None,
        load_system_fonts: None,
        make_default_app: None,
    }
}

pub fn grid_services() -> grid_ui::Services {
    grid_ui::Services {
        printers: None,
        print_native: None,
        pick_open: None,
        pick_save: None,
        pick_picture: None,
        pick_picture_async: None,
        open_url: None,
        copy_html: None,
        paste_html: None,
        paste_text: None,
        download: None,
        open_async: None,
        inbox: None,
        print: None,
    }
}

pub fn deck_services() -> deck_ui::Services {
    deck_ui::Services {
        pick_open: None,
        pick_save: None,
        read: None,
        write: None,
        open_async: None,
        download: None,
        inbox: None,
        clipboard_image: None,
        clipboard_html: None,
        clipboard_set_html: None,
        audio_out: None,
    }
}

/// FigCraft's Services is a trait with default methods: hosted, every hook
/// stays at its default (built-in dialogs, egui clipboard, no native reveal).
pub struct FigServices;

impl figcraft_ui_egui::Services for FigServices {}
