mod commands;
mod metadata;
pub mod pipeline;
pub mod watermark;

use commands::{CancelFlag, PreviewCache};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first. Launching the app again — a second click on the taskbar
        // icon, or opening it while it is already running — focuses the existing window
        // instead of starting a second copy with its own queue.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(CancelFlag::default())
        .manage(PreviewCache::default())
        .invoke_handler(tauri::generate_handler![
            commands::cpu_count,
            commands::list_images_in_folder,
            commands::collect_dropped_paths,
            commands::preview_file,
            commands::process_batch,
            commands::cancel_batch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
