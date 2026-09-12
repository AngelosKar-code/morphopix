mod commands;
mod metadata;
pub mod pipeline;
pub mod watermark;

use commands::{CancelFlag, PreviewCache};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
