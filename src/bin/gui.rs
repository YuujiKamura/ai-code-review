#[cfg(feature = "gui")]
fn main() {
    gui_shell::install_plugins(tauri::Builder::default())
        .setup(|app| {
            let _tray = gui_shell::setup_tray(&app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ai-code-review GUI");
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("Enable the gui feature: cargo run --features gui --bin gui");
}
