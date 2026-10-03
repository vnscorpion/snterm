use tauri::State;

use crate::dto::AppInfo;
use crate::state::AppState;
use super::CmdResult;

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.paths.data_dir.display().to_string(),
        local_dir: state.paths.local_dir.display().to_string(),
        logs_dir: state.paths.logs_dir().display().to_string(),
        backups_dir: state.paths.backups_dir().display().to_string(),
        platform: std::env::consts::OS.to_string(),
    })
}

/// Ghi log lỗi từ giao diện (lỗi JS chưa bắt) vào file log; không bao giờ ghi mật khẩu.
#[tauri::command]
pub fn log_frontend_error(message: String) {
    log::error!("[frontend] {message}");
}

/// Mở thư mục/file bằng ứng dụng mặc định của hệ điều hành.
#[tauri::command]
pub fn open_path(path: String) -> CmdResult<()> {
    open_with_os(&path).map_err(|e| e.to_string())
}

pub fn open_with_os(path: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd").args(["/C", "start", "", path]).spawn().map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn().map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn().map(|_| ())
    }
}
