//! Register DocCraft as an "Open with" handler for every supported file type.
//!
//! On Windows this writes HKCU\Software\Classes entries (no admin rights
//! needed) so the user can right-click any .docx/.xlsx/.pptx/.pdf/.drawio/… file
//! and choose DocCraft. Run once from the installer, from the About panel, or
//! with `doccraft.exe --register-associations`.

/// Every extension DocCraft can open, with the craft that claims it.
pub const ASSOCIATIONS: &[(&str, &str)] = &[
    ("docx", "Writer"), ("doc", "Writer"), ("odt", "Writer"), ("rtf", "Writer"),
    ("xlsx", "Sheets"), ("xlsm", "Sheets"), ("xls", "Sheets"), ("ods", "Sheets"), ("csv", "Sheets"), ("tsv", "Sheets"),
    ("pptx", "Slides"), ("potx", "Slides"), ("ppsx", "Slides"),
    ("pdf", "Reader"),
    ("fig", "Design"), ("figcraft", "Design"), ("svg", "Design"),
    ("drawio", "Draw"), ("xml", "Draw"),
    ("vec", "Vector"),
    ("md", "Markdown"), ("markdown", "Markdown"),
    ("typ", "Typeset"), ("tex", "Typeset"), ("ts", "Typeset"),
    ("note", "Notes"), ("notes", "Notes"),
];

pub fn run() {
    if !cfg!(windows) {
        println!("File association registration is a Windows feature; on this platform DocCraft opens files from the command line or the launcher.");
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().replace('/', "\\");
    let mut registered = 0usize;
    for (ext, craft) in ASSOCIATIONS {
        let progid = format!("DocCraft.{ext}");
        let desc = format!("DocCraft {craft} Document");
        let cmd = format!("\"{exe}\" \"%1\"");
        let ok = reg(&["add", &format!("HKCU\\Software\\Classes\\{progid}"), "/ve", "/t", "REG_SZ", "/d", &desc, "/f"])
            && reg(&["add", &format!("HKCU\\Software\\Classes\\{progid}\\shell\\open\\command"), "/ve", "/t", "REG_SZ", "/d", &cmd, "/f"])
            && reg(&["add", &format!("HKCU\\Software\\Classes\\{progid}\\DefaultIcon"), "/ve", "/t", "REG_SZ", "/d", &format!("{exe},0"), "/f"])
            && reg(&["add", &format!("HKCU\\Software\\Classes\\.{ext}\\OpenWithProgids"), "/v", &progid, "/t", "REG_SZ", "/d", &desc, "/f"]);
        if ok {
            registered += 1;
        }
    }
    reg(&["add", "HKCU\\Software\\Classes\\Applications\\doccraft.exe\\shell\\open\\command", "/ve", "/t", "REG_SZ", "/d", &format!("\"{exe}\" \"%1\""), "/f"]);
    println!("DocCraft registered for {registered} file types. Windows fills in 'Open with' on next use.");
}

fn reg(args: &[&str]) -> bool {
    std::process::Command::new("reg").args(args).output().map(|o| o.status.success()).unwrap_or(false)
}
