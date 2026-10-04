//! Export / Import `.snterm` và MobaXterm (tương đương `ExportDialog`/`ImportDialog` v1).
use std::path::PathBuf;

use serde::Serialize;
use snterm_core::crypto::snterm_file::{self, ConflictResolution, ExportFile, ImportPreviewItem, ImportResult};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::state::AppState;
use super::CmdResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    pub exported: usize,
    pub path: String,
    pub messages: Vec<String>,
}

#[tauri::command]
pub fn export_sessions(
    state: State<'_, AppState>,
    ids: Vec<Uuid>,
    path: String,
    password: Option<String>,
    include_key_content: bool,
) -> CmdResult<ExportOutcome> {
    let all = state.sessions.load().sessions;
    let selected: Vec<_> = all.into_iter().filter(|s| ids.contains(&s.id)).collect();
    if selected.is_empty() {
        return Err("empty".into());
    }
    let pw = password.filter(|p| !p.is_empty());
    let (file, report) = snterm_file::build_export(&selected, pw.as_deref(), include_key_content).map_err(|e| e.translate(&state.language(), None))?;
    snterm_file::write_export(&file, &PathBuf::from(&path)).map_err(|e| e.translate(&state.language(), None))?;
    if let Some(dir) = PathBuf::from(&path).parent() {
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        s.last_export_folder = dir.display().to_string();
        let _ = state.settings_store.save(&s);
    }
    Ok(ExportOutcome { exported: report.exported, path, messages: report.messages })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileInfo {
    pub path: String,
    pub file_name: String,
    pub format: String,
    pub count: usize,
    pub protected: bool,
    pub items: Vec<ImportPreviewItem>,
}

/// Đọc + kiểm tra file, trả về xem trước (chưa nhập gì).
#[tauri::command]
pub fn import_inspect(state: State<'_, AppState>, path: String) -> CmdResult<ImportFileInfo> {
    let file = snterm_file::read_and_validate(&PathBuf::from(&path)).map_err(|e| e.translate(&state.language(), None))?;
    let existing = state.sessions.load().sessions;
    let items = snterm_file::preview(&file, &existing);
    Ok(ImportFileInfo {
        file_name: PathBuf::from(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        path,
        format: file.format.clone(),
        count: file.sessions.len(),
        protected: file.protection.is_some(),
        items,
    })
}

#[tauri::command]
pub fn import_sessions(app: AppHandle, state: State<'_, AppState>, path: String, password: Option<String>, resolution: ConflictResolution) -> CmdResult<ImportResult> {
    let file: ExportFile = snterm_file::read_and_validate(&PathBuf::from(&path)).map_err(|e| e.translate(&state.language(), None))?;
    let r = snterm_file::import(&state.sessions, &file, password.as_deref(), resolution).map_err(|e| match e {
        snterm_core::error::CoreError::WrongPassword => "wrong_password".to_string(),
        other => other.translate(&state.language(), None),
    })?;
    if let Some(dir) = PathBuf::from(&path).parent() {
        let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        s.last_import_folder = dir.display().to_string();
        let _ = state.settings_store.save(&s);
    }
    super::sync::notify_changed(&app);
    Ok(r)
}

/// Tìm file MobaXterm trên máy (thư mục hiện tại, thư mục app, Desktop, %APPDATA%\MobaXterm).
#[tauri::command]
pub fn find_mobaxterm_candidate() -> CmdResult<Option<String>> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        dirs.push(PathBuf::from(home).join("Desktop"));
    }
    for d in &dirs {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x.eq_ignore_ascii_case("mxtsessions")).unwrap_or(false) {
                    return Ok(Some(p.display().to_string()));
                }
            }
        }
        let ini = d.join("MobaXterm.ini");
        if ini.exists() {
            return Ok(Some(ini.display().to_string()));
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let ini = PathBuf::from(appdata).join("MobaXterm").join("MobaXterm.ini");
        if ini.exists() {
            return Ok(Some(ini.display().to_string()));
        }
    }
    Ok(None)
}
