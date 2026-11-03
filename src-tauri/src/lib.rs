mod commands;
mod state;

use state::AppState;
use tauri::{Emitter, Manager};

/// Collects file paths from command-line arguments (skipping flags).
fn cli_files() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter(|a| std::path::Path::new(a).exists())
        .collect()
}

#[cfg(target_os = "macos")]
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{
        AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
    };
    let item = |id: &str, label: &str, accel: Option<&str>| {
        let mut b = MenuItemBuilder::with_id(id, label);
        if let Some(a) = accel {
            b = b.accelerator(a);
        }
        b.build(app)
    };
    let app_menu = SubmenuBuilder::new(app, "Hexly")
        .item(&PredefinedMenuItem::about(
            app,
            Some("About Hexly"),
            Some(AboutMetadata {
                website: Some("https://github.com/eliasthecactus/hexly".into()),
                license: Some("MIT OR Apache-2.0".into()),
                ..Default::default()
            }),
        )?)
        .item(&item("check-updates", "Check for Updates…", None)?)
        .separator()
        .item(&item("settings", "Settings…", Some("Cmd+,"))?)
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&item("quit", "Quit Hexly", Some("Cmd+Q"))?)
        .build()?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new", "New File", Some("Cmd+N"))?)
        .item(&item("open", "Open…", Some("Cmd+O"))?)
        .item(&item(
            "open-readonly",
            "Open Read-Only…",
            Some("Cmd+Shift+O"),
        )?)
        .separator()
        .item(&item("save", "Save", Some("Cmd+S"))?)
        .item(&item("save-as", "Save As…", Some("Cmd+Shift+S"))?)
        .item(&item(
            "export-selection",
            "Export Selection…",
            Some("Cmd+Alt+E"),
        )?)
        .item(&item("reload", "Revert to Saved", None)?)
        .separator()
        .item(&item("compare", "Compare With…", Some("Cmd+Alt+D"))?)
        .separator()
        .item(&item("close-tab", "Close Tab", Some("Cmd+W"))?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item("undo", "Undo", Some("Cmd+Z"))?)
        .item(&item("redo", "Redo", Some("Cmd+Shift+Z"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&item("copy-as", "Copy As…", Some("Cmd+Shift+C"))?)
        .item(&item("select-all", "Select All", Some("Cmd+A"))?)
        .separator()
        .item(&item(
            "toggle-insert",
            "Toggle Insert/Overwrite",
            Some("Cmd+I"),
        )?)
        .item(&item("fill", "Fill Selection…", Some("Cmd+Shift+F"))?)
        .item(&item("insert-bytes", "Insert Bytes…", Some("Cmd+Shift+I"))?)
        .separator()
        .item(&item("find", "Find…", Some("Cmd+F"))?)
        .item(&item("find-next", "Find Next", Some("Cmd+G"))?)
        .item(&item("find-prev", "Find Previous", Some("Cmd+Shift+G"))?)
        .build()?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&item("palette", "Command Palette…", Some("Cmd+K"))?)
        .item(&item("toggle-sidebar", "Toggle Sidebar", Some("Cmd+B"))?)
        .separator()
        .item(&item("zoom-in", "Zoom In", Some("Cmd+="))?)
        .item(&item("zoom-out", "Zoom Out", Some("Cmd+-"))?)
        .item(&item("zoom-reset", "Actual Size", Some("Cmd+0"))?)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;
    let go = SubmenuBuilder::new(app, "Go")
        .item(&item("goto", "Go to Offset…", Some("Cmd+L"))?)
        .item(&item("goto-entry", "Go to Entry Point", Some("Cmd+E"))?)
        .item(&item("next-diff", "Next Difference", Some("Cmd+Alt+Down"))?)
        .item(&item(
            "prev-diff",
            "Previous Difference",
            Some("Cmd+Alt+Up"),
        )?)
        .separator()
        .item(&item("toggle-bookmark", "Toggle Bookmark", Some("Cmd+D"))?)
        .item(&item("next-bookmark", "Next Bookmark", Some("F2"))?)
        .item(&item(
            "prev-bookmark",
            "Previous Bookmark",
            Some("Shift+F2"),
        )?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .separator()
        .item(&item("next-tab", "Next Tab", Some("Ctrl+Tab"))?)
        .item(&item("prev-tab", "Previous Tab", Some("Ctrl+Shift+Tab"))?)
        .build()?;
    let help = SubmenuBuilder::new(app, "Help")
        .item(&item("shortcuts", "Keyboard Shortcuts", Some("Cmd+/"))?)
        .item(&item("website", "Hexly on GitHub", None)?)
        .build()?;
    MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &view, &go, &window, &help])
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::default();
    state.pending_open.lock().extend(cli_files());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let menu = build_menu(app.handle())?;
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    let _ = app.emit("menu", event.id().0.as_str());
                });
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = app;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cancel_task,
            commands::open_file,
            commands::new_document,
            commands::close_document,
            commands::doc_info,
            commands::take_pending_opens,
            commands::read_range,
            commands::changed_on_disk,
            commands::save,
            commands::save_as,
            commands::export_range,
            commands::reload,
            commands::write_bytes,
            commands::replace_range,
            commands::delete_range,
            commands::fill_range,
            commands::undo,
            commands::redo,
            commands::copy_bytes,
            commands::format_bytes,
            commands::paste,
            commands::inspect,
            commands::write_value,
            commands::find,
            commands::find_all,
            commands::replace_one,
            commands::replace_all,
            commands::analyze_format,
            commands::disassemble,
            commands::architectures,
            commands::find_strings,
            commands::entropy,
            commands::hashes,
            commands::next_difference,
            commands::count_differences,
            commands::va_to_offset,
            commands::offset_to_va,
            commands::eval_offset,
            commands::load_bookmarks,
            commands::save_bookmarks,
        ]);

    builder
        .build(tauri::generate_context!())
        .expect("error while building Hexly")
        .run(|app, event| {
            // Files opened from Finder / "Open With" on macOS.
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                let paths: Vec<String> = urls
                    .iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                if !paths.is_empty() {
                    app.state::<AppState>().pending_open.lock().extend(paths);
                    let _ = app.emit("open-files", ());
                }
            }
            let _ = (app, event);
        });
}
