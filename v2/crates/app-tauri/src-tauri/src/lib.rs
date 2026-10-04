//! SN Term v2 — Tauri app (một WebView duy nhất). Lõi ở crate `snterm-core`.
mod commands;
mod dto;
mod state;

use tauri::{Emitter, Manager};

use state::AppState;

pub fn run() {
    let state = AppState::new();
    let logs_dir = state.paths.logs_dir();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Lần chạy thứ hai (VD nhấp đúp file .snterm) → chuyển file cho cửa sổ đang mở.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            if let Some(file) = args.iter().skip(1).find(|a| a.to_lowercase().ends_with(".snterm")) {
                let _ = app.emit("app:open-file", file.clone());
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder { path: logs_dir, file_name: Some("snterm-v2".into()) }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        )
        .manage(state)
        .setup(|app| {
            std::panic::set_hook(Box::new(|info| {
                log::error!("panic: {info}");
            }));
            commands::sync::start_scheduler(app.handle());
            // Mở file .snterm truyền qua dòng lệnh lúc khởi động.
            let args: Vec<String> = std::env::args().collect();
            if let Some(file) = args.iter().skip(1).find(|a| a.to_lowercase().ends_with(".snterm")) {
                let handle = app.handle().clone();
                let f = file.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                    let _ = handle.emit("app:open-file", f);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::app::log_frontend_error,
            commands::app::open_path,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::save_window_state,
            commands::settings::save_collapsed_groups,
            commands::sessions::list_sessions,
            commands::sessions::save_session,
            commands::sessions::delete_sessions,
            commands::sessions::duplicate_session,
            commands::sessions::move_sessions_to_group,
            commands::sessions::rename_group,
            commands::sessions::inspect_key_file,
            commands::clipboard::clipboard_write,
            commands::clipboard::clipboard_read,
            commands::terminal::terminal_open,
            commands::terminal::terminal_input,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_reconnect,
            commands::terminal::terminal_close,
            commands::terminal::terminal_set_visible,
            commands::terminal::connected_tab_count,
            commands::terminal::test_connection,
            commands::terminal::dialog_answer,
            commands::sftp::sftp_list,
            commands::sftp::sftp_is_dir,
            commands::sftp::sftp_mkdir,
            commands::sftp::sftp_rename,
            commands::sftp::sftp_remove,
            commands::sftp::sftp_chmod,
            commands::sftp::sftp_upload,
            commands::sftp::sftp_download,
            commands::sftp::sftp_cancel_transfer,
            commands::sftp::sftp_open_in_editor,
            commands::export_import::export_sessions,
            commands::export_import::import_inspect,
            commands::export_import::import_sessions,
            commands::export_import::find_mobaxterm_candidate,
            commands::sync::sync_status,
            commands::sync::sync_test_backend,
            commands::sync::sync_preview,
            commands::sync::sync_enable,
            commands::sync::sync_run_now,
            commands::sync::sync_disable,
            commands::sync::sync_set_password,
            commands::sync::sync_change_password,
            commands::sync::sync_update_settings,
            commands::sync::sync_log,
            commands::sync::sync_flush,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SN Term");
}
