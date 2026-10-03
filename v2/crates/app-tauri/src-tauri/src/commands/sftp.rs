//! Lệnh SFTP cho giao diện (tương đương `SftpViewModel` + `TransferQueueViewModel` v1).
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use snterm_core::error::CoreError;
use snterm_core::ssh::sftp::{join, SftpClient, SftpItem};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::commands::terminal::{ensure_sftp, tab_inner, TabInner};
use crate::state::AppState;
use super::CmdResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpListing {
    pub path: String,
    pub home: String,
    pub items: Vec<SftpItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub tab_id: String,
    pub transfer_id: String,
    pub name: String,
    pub done: u64,
    pub total: u64,
    pub speed_bps: f64,
    pub finished: bool,
    pub error: Option<String>,
}

fn tr(state: &AppState, e: CoreError, host: &str) -> String {
    e.translate(&state.language(), Some(host))
}

async fn client(state: &AppState, tab_id: &str) -> Result<(Arc<TabInner>, Arc<SftpClient>, String), String> {
    let inner = tab_inner(state, tab_id).ok_or("tab not found")?;
    let host = inner.session.lock().await.host.clone();
    let c = ensure_sftp(&inner).await.map_err(|e| tr(state, e, &host))?;
    Ok((inner, c, host))
}

#[tauri::command]
pub async fn sftp_list(state: State<'_, AppState>, tab_id: String, path: Option<String>, show_hidden: bool) -> CmdResult<SftpListing> {
    let (_inner, c, host) = client(&state, &tab_id).await?;
    let path = path.filter(|p| !p.trim().is_empty()).unwrap_or_else(|| c.home.clone());
    let items = c.list_dir(&path, show_hidden).await.map_err(|e| tr(&state, e, &host))?;
    Ok(SftpListing { path, home: c.home.clone(), items })
}

#[tauri::command]
pub async fn sftp_is_dir(state: State<'_, AppState>, tab_id: String, path: String) -> CmdResult<bool> {
    let (_i, c, _h) = client(&state, &tab_id).await?;
    Ok(c.is_directory(&path).await)
}

#[tauri::command]
pub async fn sftp_mkdir(state: State<'_, AppState>, tab_id: String, path: String) -> CmdResult<()> {
    let (_i, c, host) = client(&state, &tab_id).await?;
    c.create_dir(&path).await.map_err(|e| tr(&state, e, &host))
}

#[tauri::command]
pub async fn sftp_rename(state: State<'_, AppState>, tab_id: String, from: String, to: String) -> CmdResult<()> {
    let (_i, c, host) = client(&state, &tab_id).await?;
    c.rename(&from, &to).await.map_err(|e| tr(&state, e, &host))
}

#[tauri::command]
pub async fn sftp_remove(state: State<'_, AppState>, tab_id: String, paths: Vec<(String, bool)>) -> CmdResult<()> {
    let (_i, c, host) = client(&state, &tab_id).await?;
    for (p, is_dir) in paths {
        c.remove(&p, is_dir).await.map_err(|e| tr(&state, e, &host))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_chmod(state: State<'_, AppState>, tab_id: String, paths: Vec<(String, bool)>, mode: u32, recursive: bool) -> CmdResult<()> {
    let (_i, c, host) = client(&state, &tab_id).await?;
    for (p, is_dir) in paths {
        c.chmod(&p, is_dir, mode, recursive).await.map_err(|e| tr(&state, e, &host))?;
    }
    Ok(())
}

fn emit_progress(app: &AppHandle, p: &TransferProgress) {
    let _ = app.emit("transfer:progress", p);
}

/// Upload nhiều file/thư mục (đệ quy) vào thư mục đang xem. Chạy lần lượt, có tiến trình và hủy.
#[tauri::command]
pub async fn sftp_upload(app: AppHandle, state: State<'_, AppState>, tab_id: String, local_paths: Vec<String>, remote_dir: String) -> CmdResult<Vec<String>> {
    let (inner, c, host) = client(&state, &tab_id).await?;
    inner.transfer_cancel.store(false, Ordering::SeqCst);
    let mut errors = Vec::new();
    for lp in local_paths {
        let local = PathBuf::from(&lp);
        if local.is_file() {
            if let Err(e) = upload_one(&app, &inner, &c, &local, &remote_dir).await {
                errors.push(format!("{}: {}", local.display(), tr(&state, e, &host)));
            }
        } else if local.is_dir() {
            let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let remote_root = join(&remote_dir, &name);
            if let Err(e) = upload_dir(&app, &inner, &c, &local, &remote_root).await {
                errors.push(format!("{}: {}", local.display(), tr(&state, e, &host)));
            }
        }
        if inner.transfer_cancel.load(Ordering::SeqCst) {
            break;
        }
    }
    Ok(errors)
}

async fn upload_dir(app: &AppHandle, inner: &Arc<TabInner>, c: &SftpClient, local: &Path, remote: &str) -> Result<(), CoreError> {
    if !c.exists(remote).await {
        c.create_dir(remote).await?;
    }
    let mut entries: Vec<_> = std::fs::read_dir(local)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if inner.transfer_cancel.load(Ordering::SeqCst) {
            return Err(CoreError::Other("canceled".into()));
        }
        let p = e.path();
        if p.is_file() {
            upload_one(app, inner, c, &p, remote).await?;
        } else if p.is_dir() {
            let name = e.file_name().to_string_lossy().to_string();
            Box::pin(upload_dir(app, inner, c, &p, &join(remote, &name))).await?;
        }
    }
    Ok(())
}

async fn upload_one(app: &AppHandle, inner: &Arc<TabInner>, c: &SftpClient, local: &Path, remote_dir: &str) -> Result<(), CoreError> {
    let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let remote = join(remote_dir, &name);
    let id = Uuid::new_v4().to_string();
    let mut last = Instant::now();
    let mut last_done = 0u64;
    let mut speed = 0.0;
    let tab = inner.tab_id.clone();
    emit_progress(app, &TransferProgress { tab_id: tab.clone(), transfer_id: id.clone(), name: format!("Upload: {name}"), done: 0, total: 0, speed_bps: 0.0, finished: false, error: None });
    let r = c
        .upload_file(local, &remote, &inner.transfer_cancel, |done, total| {
            let now = Instant::now();
            let el = now.duration_since(last).as_secs_f64();
            if el >= 0.5 || done == total {
                if el > 0.0 {
                    speed = (done - last_done) as f64 / el;
                }
                last = now;
                last_done = done;
                emit_progress(app, &TransferProgress { tab_id: tab.clone(), transfer_id: id.clone(), name: format!("Upload: {name}"), done, total, speed_bps: speed, finished: false, error: None });
            }
        })
        .await;
    emit_progress(app, &TransferProgress { tab_id: tab, transfer_id: id, name: format!("Upload: {name}"), done: 0, total: 0, speed_bps: 0.0, finished: true, error: r.as_ref().err().map(|e| e.to_string()) });
    r
}

/// Download nhiều mục (file hoặc thư mục đệ quy) về thư mục đích.
#[tauri::command]
pub async fn sftp_download(app: AppHandle, state: State<'_, AppState>, tab_id: String, items: Vec<(String, bool, u64)>, dest_dir: String) -> CmdResult<Vec<String>> {
    let (inner, c, host) = client(&state, &tab_id).await?;
    inner.transfer_cancel.store(false, Ordering::SeqCst);
    let dest = PathBuf::from(dest_dir);
    let mut errors = Vec::new();
    for (remote, is_dir, size) in items {
        let name = remote.rsplit('/').next().unwrap_or("file").to_string();
        let r = if is_dir {
            download_dir(&app, &inner, &c, &remote, &dest.join(&name)).await
        } else {
            download_one(&app, &inner, &c, &remote, size, &dest).await
        };
        if let Err(e) = r {
            errors.push(format!("{name}: {}", tr(&state, e, &host)));
        }
        if inner.transfer_cancel.load(Ordering::SeqCst) {
            break;
        }
    }
    Ok(errors)
}

async fn download_dir(app: &AppHandle, inner: &Arc<TabInner>, c: &SftpClient, remote: &str, local: &Path) -> Result<(), CoreError> {
    std::fs::create_dir_all(local)?;
    for (rel, is_dir, size) in c.walk(remote).await? {
        if inner.transfer_cancel.load(Ordering::SeqCst) {
            return Err(CoreError::Other("canceled".into()));
        }
        let lp = local.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if is_dir {
            std::fs::create_dir_all(&lp)?;
        } else {
            let parent = lp.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| local.to_path_buf());
            std::fs::create_dir_all(&parent)?;
            download_one(app, inner, c, &join(remote, &rel), size, &parent).await?;
        }
    }
    Ok(())
}

async fn download_one(app: &AppHandle, inner: &Arc<TabInner>, c: &SftpClient, remote: &str, size: u64, local_dir: &Path) -> Result<(), CoreError> {
    let name = remote.rsplit('/').next().unwrap_or("file").to_string();
    let local = local_dir.join(&name);
    let id = Uuid::new_v4().to_string();
    let mut last = Instant::now();
    let mut last_done = 0u64;
    let mut speed = 0.0;
    let tab = inner.tab_id.clone();
    emit_progress(app, &TransferProgress { tab_id: tab.clone(), transfer_id: id.clone(), name: format!("Download: {name}"), done: 0, total: size, speed_bps: 0.0, finished: false, error: None });
    let r = c
        .download_file(remote, &local, size, &inner.transfer_cancel, |done, total| {
            let now = Instant::now();
            let el = now.duration_since(last).as_secs_f64();
            if el >= 0.5 || done == total {
                if el > 0.0 {
                    speed = (done - last_done) as f64 / el;
                }
                last = now;
                last_done = done;
                emit_progress(app, &TransferProgress { tab_id: tab.clone(), transfer_id: id.clone(), name: format!("Download: {name}"), done, total, speed_bps: speed, finished: false, error: None });
            }
        })
        .await;
    emit_progress(app, &TransferProgress { tab_id: tab, transfer_id: id, name: format!("Download: {name}"), done: 0, total: 0, speed_bps: 0.0, finished: true, error: r.as_ref().err().map(|e| e.to_string()) });
    r
}

#[tauri::command]
pub fn sftp_cancel_transfer(state: State<'_, AppState>, tab_id: String) -> CmdResult<()> {
    if let Some(inner) = tab_inner(&state, &tab_id) {
        inner.transfer_cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Tải file về thư mục tạm, mở bằng trình soạn thảo, theo dõi thay đổi → tự upload lại (như v1).
#[tauri::command]
pub async fn sftp_open_in_editor(app: AppHandle, state: State<'_, AppState>, tab_id: String, remote_path: String, editor: Option<String>) -> CmdResult<String> {
    let (inner, c, host) = client(&state, &tab_id).await?;
    let name = remote_path.rsplit('/').next().unwrap_or("file").to_string();
    let dir = state.paths.edit_dir().join(Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let local = dir.join(&name);
    let bytes = c.read_file(&remote_path).await.map_err(|e| tr(&state, e, &host))?;
    std::fs::write(&local, bytes).map_err(|e| e.to_string())?;

    // Theo dõi file: khi lưu → upload lại (debounce 800 ms).
    let watch_local = local.clone();
    let remote2 = remote_path.clone();
    let inner2 = inner.clone();
    let app2 = app.clone();
    let tab = tab_id.clone();
    let name2 = name.clone();
    std::thread::spawn(move || {
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if ev.kind.is_modify() || ev.kind.is_create() {
                    let _ = tx.send(());
                }
            }
        }) else { return };
        if watcher.watch(watch_local.parent().unwrap_or(&watch_local), RecursiveMode::NonRecursive).is_err() {
            return;
        }
        let mut last_upload = Instant::now() - std::time::Duration::from_secs(10);
        while rx.recv().is_ok() {
            if inner2.closed.load(Ordering::SeqCst) {
                break;
            }
            if last_upload.elapsed().as_millis() < 800 {
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            while rx.try_recv().is_ok() {}
            last_upload = Instant::now();
            let Ok(data) = std::fs::read(&watch_local) else { continue };
            let inner3 = inner2.clone();
            let remote3 = remote2.clone();
            let app3 = app2.clone();
            let tab3 = tab.clone();
            let n3 = name2.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(c) = ensure_sftp(&inner3).await {
                    let r = c.write_file(&remote3, &data).await;
                    let _ = app3.emit("sftp:autosaved", serde_json::json!({ "tabId": tab3, "name": n3, "ok": r.is_ok() }));
                }
            });
        }
    });

    let p = local.to_string_lossy().to_string();
    match editor.filter(|e| !e.trim().is_empty() && Path::new(e).exists()) {
        Some(exe) => {
            std::process::Command::new(exe).arg(&p).spawn().map_err(|e| e.to_string())?;
        }
        None => crate::commands::app::open_with_os(&p).map_err(|e| e.to_string())?,
    }
    let _ = app.state::<AppState>();
    Ok(p)
}
