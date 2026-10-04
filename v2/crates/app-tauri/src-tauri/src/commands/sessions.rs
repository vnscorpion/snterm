use snterm_core::crypto::dpapi;
use snterm_core::store::Session;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::dto::{SessionDraft, SessionView, SessionsLoad};
use crate::state::AppState;
use super::CmdResult;

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> CmdResult<SessionsLoad> {
    let r = state.sessions.load();
    Ok(SessionsLoad {
        sessions: r.sessions.iter().map(SessionView::from).collect(),
        recovered_from_corruption: r.recovered_from_corruption,
    })
}

/// Thêm hoặc sửa VM từ form. Giống `SessionEditorViewModel.SaveAndClose` v1.
#[tauri::command]
pub fn save_session(app: AppHandle, state: State<'_, AppState>, draft: SessionDraft) -> CmdResult<SessionView> {
    if draft.host.trim().is_empty() || draft.port == 0 || draft.username.trim().is_empty() {
        return Err("invalid".into());
    }
    let existing = draft.id.and_then(|id| state.sessions.find(id));
    let mut s = existing.clone().unwrap_or_default();
    s.name = draft.name.trim().to_string();
    s.group = draft.group.trim().to_string();
    s.host = draft.host.trim().to_string();
    s.port = draft.port;
    s.username = draft.username.trim().to_string();
    s.save_password = draft.save_password;
    s.key_file_path = draft.key_file_path.as_deref().map(str::trim).filter(|p| !p.is_empty()).map(String::from);

    if draft.save_password {
        if let Some(p) = draft.password.as_deref().filter(|p| !p.is_empty()) {
            s.encrypted_password = dpapi::encrypt(Some(p));
        } else if draft.clear_password {
            s.encrypted_password = None;
        } else {
            s.encrypted_password = existing.as_ref().and_then(|e| e.encrypted_password.clone());
        }
        if let Some(p) = draft.passphrase.as_deref().filter(|p| !p.is_empty()) {
            s.encrypted_passphrase = dpapi::encrypt(Some(p));
        } else {
            s.encrypted_passphrase = existing.as_ref().and_then(|e| e.encrypted_passphrase.clone());
        }
    } else {
        s.encrypted_password = None;
        s.encrypted_passphrase = None;
    }
    s.touch();
    state.sessions.update_session(&s).map_err(|e| e.to_string())?;
    super::sync::notify_changed(&app);
    Ok(SessionView::from(&s))
}

#[tauri::command]
pub fn delete_sessions(app: AppHandle, state: State<'_, AppState>, ids: Vec<Uuid>) -> CmdResult<usize> {
    let n = state.sessions.delete_sessions(&ids).map_err(|e| e.to_string())?;
    super::sync::notify_changed(&app);
    Ok(n)
}

#[tauri::command]
pub fn duplicate_session(app: AppHandle, state: State<'_, AppState>, id: Uuid) -> CmdResult<SessionView> {
    let s = state.sessions.find(id).ok_or("not found")?;
    let clone = s.duplicate();
    state.sessions.update_session(&clone).map_err(|e| e.to_string())?;
    super::sync::notify_changed(&app);
    Ok(SessionView::from(&clone))
}

#[tauri::command]
pub fn move_sessions_to_group(app: AppHandle, state: State<'_, AppState>, ids: Vec<Uuid>, group: String) -> CmdResult<()> {
    let mut all = state.sessions.load().sessions;
    for s in all.iter_mut().filter(|s| ids.contains(&s.id)) {
        s.group = group.trim().to_string();
        s.touch();
    }
    state.sessions.save(&all).map_err(|e| e.to_string())?;
    super::sync::notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn rename_group(app: AppHandle, state: State<'_, AppState>, old_name: String, new_name: String) -> CmdResult<()> {
    if new_name.trim().is_empty() {
        return Ok(());
    }
    let mut all = state.sessions.load().sessions;
    for s in all.iter_mut().filter(|s| s.effective_group().eq_ignore_ascii_case(old_name.trim())) {
        s.group = new_name.trim().to_string();
        s.touch();
    }
    state.sessions.save(&all).map_err(|e| e.to_string())?;
    super::sync::notify_changed(&app);
    Ok(())
}

/// Đọc file key để báo sớm "cần passphrase" / "không đọc được" trong form (như v1).
#[tauri::command]
pub fn inspect_key_file(path: String, passphrase: Option<String>) -> CmdResult<String> {
    match snterm_core::ssh::auth::load_private_key(&path, passphrase.as_deref()) {
        Ok(k) => Ok(k.algorithm().to_string()),
        Err(snterm_core::error::CoreError::Passphrase) => Err("passphrase".into()),
        Err(snterm_core::error::CoreError::KeyFileNotFound(_)) => Err("notfound".into()),
        Err(_) => Err("invalid".into()),
    }
}

pub(crate) fn session_from_draft_for_test(draft: &SessionDraft, existing: Option<&Session>) -> (Session, Option<String>, Option<String>) {
    let mut s = existing.cloned().unwrap_or_default();
    s.host = draft.host.trim().to_string();
    s.port = draft.port;
    s.username = draft.username.trim().to_string();
    s.key_file_path = draft.key_file_path.clone().filter(|p| !p.trim().is_empty());
    let password = draft
        .password
        .clone()
        .filter(|p| !p.is_empty())
        .or_else(|| if draft.clear_password { None } else { existing.and_then(|e| dpapi::decrypt(e.encrypted_password.as_deref())) });
    let passphrase = draft
        .passphrase
        .clone()
        .filter(|p| !p.is_empty())
        .or_else(|| existing.and_then(|e| dpapi::decrypt(e.encrypted_passphrase.as_deref())));
    (s, password, passphrase)
}
