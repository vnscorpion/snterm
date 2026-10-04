//! Clipboard với retry (tương đương `ClipboardService.cs` v1: 5 lần × 50 ms).
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::CmdResult;

#[tauri::command]
pub async fn clipboard_write(app: AppHandle, text: String) -> CmdResult<()> {
    let mut last = String::new();
    for _ in 0..5 {
        match app.clipboard().write_text(text.clone()) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e.to_string();
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
    Err(last)
}

#[tauri::command]
pub async fn clipboard_read(app: AppHandle) -> CmdResult<String> {
    let mut last = String::new();
    for _ in 0..5 {
        match app.clipboard().read_text() {
            Ok(t) => return Ok(t),
            Err(e) => {
                last = e.to_string();
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
    Err(last)
}
