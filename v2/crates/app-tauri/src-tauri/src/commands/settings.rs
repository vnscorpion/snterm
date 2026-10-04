use snterm_core::store::AppSettings;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use super::CmdResult;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    Ok(state.settings())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: AppSettings) -> CmdResult<AppSettings> {
    let mut s = settings;
    if s.keep_alive_seconds < 0 {
        s.keep_alive_seconds = 5;
    }
    if s.max_parallel_connects < 1 {
        s.max_parallel_connects = 4;
    }
    if s.font_size < 8 || s.font_size > 40 {
        s.font_size = 14;
    }
    if s.scrollback < 100 {
        s.scrollback = 100;
    }
    state.settings_store.save(&s).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap_or_else(|e| e.into_inner()) = s.clone();
    let _ = app.emit("settings:changed", &s);
    Ok(s)
}

/// Lưu kích thước cửa sổ và độ rộng cột trái (gọi khi đóng/thay đổi, không phát sự kiện).
#[tauri::command]
pub fn save_window_state(state: State<'_, AppState>, width: f64, height: f64, left_column_width: f64) -> CmdResult<()> {
    let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    if width >= 800.0 {
        s.window_width = width;
    }
    if height >= 500.0 {
        s.window_height = height;
    }
    s.left_column_width = if (140.0..=600.0).contains(&left_column_width) { left_column_width } else { 400.0 };
    state.settings_store.save(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_collapsed_groups(state: State<'_, AppState>, groups: Vec<String>) -> CmdResult<()> {
    let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    s.collapsed_groups = groups;
    state.settings_store.save(&s).map_err(|e| e.to_string())
}
