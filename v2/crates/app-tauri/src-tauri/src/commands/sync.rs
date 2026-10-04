//! Đồng bộ danh sách VM (Phần 2): lệnh Tauri + lịch chạy + trạng thái.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use snterm_core::crypto::dpapi;
use snterm_core::error::CoreError;
use snterm_core::ssh::{ConnectParams, HostKeyInfo, HostKeyVerifier};
use snterm_core::store::{HostKeyStatus, KnownHostsStore, SyncSettings};
use snterm_core::sync::backend::folder::FolderBackend;
use snterm_core::sync::backend::sftp::SftpBackend;
use snterm_core::sync::backend::SyncBackend;
use snterm_core::sync::engine::{self, SyncContext, SyncOutcome};
use snterm_core::sync::merge::MergeReport;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::state::AppState;
use super::CmdResult;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncBackendConfig {
    pub backend_type: String,
    pub folder_path: String,
    pub sftp_session_id: Option<Uuid>,
    pub sftp_remote_path: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    /// "never" | "idle" | "running" | "offline" | "needPassword" | "error"
    pub state: String,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub backend_label: String,
    pub backend_type: String,
    pub folder_path: String,
    pub sftp_session_id: Option<Uuid>,
    pub sftp_remote_path: String,
    pub interval_minutes: i32,
    pub include_key_files: bool,
    pub last_revision: i64,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub remote_existed: bool,
    pub report: MergeReport,
    pub local_count: usize,
    pub messages: Vec<String>,
}

/// Verifier không hỏi người dùng: chỉ chấp nhận host key đã tin cậy (đồng bộ chạy ngầm).
pub struct TrustedOnlyVerifier {
    pub known_hosts: Arc<KnownHostsStore>,
}

#[async_trait]
impl HostKeyVerifier for TrustedOnlyVerifier {
    async fn verify(&self, info: HostKeyInfo) -> bool {
        self.known_hosts.check_host(&info.host, info.port, &info.fingerprint_sha256) == HostKeyStatus::Trusted
    }
}

/// Trạng thái chạy ngầm của đồng bộ.
pub struct SyncManager {
    pub run_lock: tokio::sync::Mutex<()>,
    pub running: AtomicBool,
    pub state: std::sync::Mutex<String>,
    pub pending_change: AtomicBool,
    pub debounce_armed: AtomicBool,
    pub started: AtomicBool,
}

impl SyncManager {
    pub fn new() -> Self {
        SyncManager {
            run_lock: tokio::sync::Mutex::new(()),
            running: AtomicBool::new(false),
            state: std::sync::Mutex::new("never".into()),
            pending_change: AtomicBool::new(false),
            debounce_armed: AtomicBool::new(false),
            started: AtomicBool::new(false),
        }
    }
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "PC".into())
}

fn ensure_device(s: &mut SyncSettings) {
    if s.device_id.is_none() {
        s.device_id = Some(Uuid::new_v4());
    }
    if s.device_name.trim().is_empty() {
        s.device_name = device_name();
    }
}

fn backend_label(state: &AppState, cfg: &SyncBackendConfig) -> String {
    if cfg.backend_type.eq_ignore_ascii_case("Sftp") {
        let name = cfg.sftp_session_id.and_then(|id| state.sessions.find(id)).map(|s| s.display_name()).unwrap_or_else(|| "?".into());
        format!("{name}:{}", cfg.sftp_remote_path)
    } else {
        cfg.folder_path.clone()
    }
}

fn config_from_settings(s: &SyncSettings) -> SyncBackendConfig {
    SyncBackendConfig { backend_type: s.backend_type.clone(), folder_path: s.folder_path.clone(), sftp_session_id: s.sftp_session_id, sftp_remote_path: s.sftp_remote_path.clone() }
}

/// Dựng backend từ cấu hình (SFTP dùng thông tin đăng nhập của VM đã lưu).
fn make_backend(state: &AppState, cfg: &SyncBackendConfig) -> Result<Box<dyn SyncBackend>, CoreError> {
    if cfg.backend_type.eq_ignore_ascii_case("Sftp") {
        let id = cfg.sftp_session_id.ok_or_else(|| CoreError::InvalidData("Chưa chọn VM làm kho đồng bộ.".into()))?;
        let s = state.sessions.find(id).ok_or_else(|| CoreError::InvalidData("VM kho đồng bộ không còn trong danh sách.".into()))?;
        let settings = state.settings();
        Ok(Box::new(SftpBackend {
            params: ConnectParams {
                host: s.host.clone(),
                port: s.port,
                username: s.username.clone(),
                password: dpapi::decrypt(s.encrypted_password.as_deref()).map(Zeroizing::new),
                key_file_path: s.key_file_path.clone(),
                passphrase: dpapi::decrypt(s.encrypted_passphrase.as_deref()).map(Zeroizing::new),
                keepalive_seconds: settings.keep_alive_seconds.max(0) as u32,
                connect_timeout: Duration::from_secs(15),
            },
            verifier: Arc::new(TrustedOnlyVerifier { known_hosts: state.known_hosts.clone() }),
            remote_path: if cfg.sftp_remote_path.trim().is_empty() { "~/.snterm/sync.vault".into() } else { cfg.sftp_remote_path.clone() },
            label: s.display_name(),
        }))
    } else {
        if cfg.folder_path.trim().is_empty() {
            return Err(CoreError::InvalidData("Chưa chọn thư mục đồng bộ.".into()));
        }
        Ok(Box::new(FolderBackend::new(cfg.folder_path.trim())))
    }
}

fn log_line(state: &AppState, line: &str) {
    let dir = state.paths.logs_dir();
    let file = dir.join(format!("sync-{}.log", chrono::Local::now().format("%Y%m%d")));
    let entry = format!("[{}] {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), line);
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = f.write_all(entry.as_bytes());
    }
}

pub fn current_status(state: &AppState) -> SyncStatus {
    let s = state.settings().sync;
    let mgr = &state.sync;
    let st = mgr.state.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let cfg = config_from_settings(&s);
    SyncStatus {
        enabled: s.enabled,
        state: if !s.enabled { "never".into() } else { st },
        last_sync_at: s.last_sync_at.map(|d| d.to_rfc3339()),
        last_error: s.last_error.clone(),
        backend_label: backend_label(state, &cfg),
        backend_type: s.backend_type.clone(),
        folder_path: s.folder_path.clone(),
        sftp_session_id: s.sftp_session_id,
        sftp_remote_path: s.sftp_remote_path.clone(),
        interval_minutes: s.interval_minutes,
        include_key_files: s.include_key_files,
        last_revision: s.last_revision,
        device_name: s.device_name.clone(),
    }
}

fn emit_status(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit("sync:status", current_status(&state));
}

fn set_state(app: &AppHandle, value: &str) {
    let state = app.state::<AppState>();
    *state.sync.state.lock().unwrap_or_else(|e| e.into_inner()) = value.to_string();
    emit_status(app);
}

fn update_settings<F: FnOnce(&mut SyncSettings)>(state: &AppState, f: F) -> SyncSettings {
    let mut s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut s.sync);
    let _ = state.settings_store.save(&s);
    s.sync.clone()
}

fn classify_error(e: &CoreError) -> &'static str {
    match e {
        CoreError::WrongPassword => "needPassword",
        CoreError::Connect(_) | CoreError::Timeout | CoreError::ConnectionLost | CoreError::Io(_) => "offline",
        _ => "error",
    }
}

/// Chạy một chu kỳ (nếu đã bật). Không bao giờ hiện hộp thoại; cập nhật trạng thái + log.
pub async fn run_cycle(app: &AppHandle, manual: bool) -> Result<SyncOutcome, String> {
    let state = app.state::<AppState>();
    let sync = state.settings().sync;
    if !sync.enabled {
        return Err("disabled".into());
    }
    let lang = state.language();
    let _guard = match state.sync.run_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            state.sync.pending_change.store(true, Ordering::SeqCst);
            return Err("busy".into());
        }
    };
    state.sync.running.store(true, Ordering::SeqCst);
    set_state(app, "running");
    let password = match dpapi::decrypt(sync.encrypted_sync_password.as_deref()) {
        Some(p) => Zeroizing::new(p),
        None => {
            state.sync.running.store(false, Ordering::SeqCst);
            set_state(app, "needPassword");
            return Err("needPassword".into());
        }
    };
    let cfg = config_from_settings(&sync);
    let result = async {
        let backend = make_backend(&state, &cfg)?;
        let ctx = SyncContext {
            store: &state.sessions,
            backend: backend.as_ref(),
            password: &password,
            device_id: sync.device_id.unwrap_or_else(Uuid::new_v4),
            device_name: sync.device_name.clone(),
            include_key_files: sync.include_key_files,
            last_revision: sync.last_revision,
            keys_dir: state.paths.keys_dir(),
            app_version: env!("CARGO_PKG_VERSION").into(),
        };
        engine::run_once(&ctx, sync.last_revision == 0).await
    }
    .await;
    state.sync.running.store(false, Ordering::SeqCst);
    state.sync.pending_change.store(false, Ordering::SeqCst);
    match result {
        Ok(o) => {
            update_settings(&state, |s| {
                s.last_revision = o.revision.max(s.last_revision);
                s.last_sync_at = Some(Utc::now());
                s.last_error = None;
            });
            log_line(&state, &format!(
                "OK rev={} kho={} thêm={} cập nhật={} xóa={} gửi={} {}",
                o.revision, cfg.backend_type, o.report.add_local.len(), o.report.update_local.len(), o.report.delete_local.len(), o.report.to_remote, o.messages.join(" | ")
            ));
            if o.local_changed {
                let _ = app.emit("sessions:changed", ());
            }
            set_state(app, "idle");
            let _ = manual;
            Ok(o)
        }
        Err(e) => {
            let msg = match &e {
                CoreError::HostKeyRejected => crate::commands::sync::host_key_msg(&lang),
                other => other.translate(&lang, None),
            };
            let kind = match &e {
                CoreError::HostKeyRejected => "error",
                other => classify_error(other),
            };
            update_settings(&state, |s| s.last_error = Some(msg.clone()));
            log_line(&state, &format!("LỖI ({kind}): {msg}"));
            set_state(app, kind);
            Err(msg)
        }
    }
}

fn host_key_msg(lang: &str) -> String {
    if lang.eq_ignore_ascii_case("vi") {
        "Chưa xác nhận host key của VM kho — hãy mở VM đó một lần.".into()
    } else {
        "Host key of the store VM is not trusted yet — open that VM once.".into()
    }
}

/// Gọi khi danh sách VM đổi: chờ 10 s rồi đồng bộ (gộp nhiều thay đổi liên tiếp).
pub fn notify_changed(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.settings().sync.enabled {
        return;
    }
    if state.sync.debounce_armed.swap(true, Ordering::SeqCst) {
        return;
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let st = app2.state::<AppState>();
        st.sync.debounce_armed.store(false, Ordering::SeqCst);
        let _ = run_cycle(&app2, false).await;
    });
}

/// Khởi động lịch: sau 3 s rồi mỗi `IntervalMinutes` phút (tối thiểu 5).
pub fn start_scheduler(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.sync.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            let st = app2.state::<AppState>();
            let s = st.settings().sync;
            if s.enabled {
                let _ = run_cycle(&app2, false).await;
            }
            let mins = st.settings().sync.interval_minutes.max(5) as u64;
            tokio::time::sleep(Duration::from_secs(mins * 60)).await;
        }
    });
}

// ---------------- Lệnh ----------------

#[tauri::command]
pub fn sync_status(state: State<'_, AppState>) -> CmdResult<SyncStatus> {
    Ok(current_status(&state))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTestResult {
    pub vault_exists: bool,
    pub label: String,
}

#[tauri::command]
pub async fn sync_test_backend(state: State<'_, AppState>, config: SyncBackendConfig) -> CmdResult<SyncTestResult> {
    let lang = state.language();
    let backend = make_backend(&state, &config).map_err(|e| e.translate(&lang, None))?;
    backend.test().await.map_err(|e| match e {
        CoreError::HostKeyRejected => host_key_msg(&lang),
        other => other.translate(&lang, None),
    })?;
    let exists = backend.read_meta().await.map_err(|e| e.translate(&lang, None))?.is_some();
    Ok(SyncTestResult { vault_exists: exists, label: backend_label(&state, &config) })
}

#[tauri::command]
pub async fn sync_preview(state: State<'_, AppState>, config: SyncBackendConfig, password: String, include_key_files: bool) -> CmdResult<SyncPreview> {
    let lang = state.language();
    let backend = make_backend(&state, &config).map_err(|e| e.translate(&lang, None))?;
    let mut sync = state.settings().sync;
    ensure_device(&mut sync);
    let ctx = SyncContext {
        store: &state.sessions,
        backend: backend.as_ref(),
        password: &password,
        device_id: sync.device_id.unwrap(),
        device_name: sync.device_name.clone(),
        include_key_files,
        last_revision: 0,
        keys_dir: state.paths.keys_dir(),
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let plan = engine::prepare(&ctx).await.map_err(|e| match e {
        CoreError::WrongPassword => "wrong_password".to_string(),
        CoreError::HostKeyRejected => host_key_msg(&lang),
        other => other.translate(&lang, None),
    })?;
    Ok(SyncPreview { remote_existed: plan.remote_existed, report: plan.report, local_count: state.sessions.load().sessions.len(), messages: plan.messages })
}

/// Bật đồng bộ: lưu cấu hình + mật khẩu (DPAPI) rồi chạy chu kỳ đầu.
#[tauri::command]
pub async fn sync_enable(app: AppHandle, state: State<'_, AppState>, config: SyncBackendConfig, password: String, include_key_files: bool) -> CmdResult<SyncOutcome> {
    if password.len() < 8 {
        return Err("short_password".into());
    }
    update_settings(&state, |s| {
        s.enabled = true;
        s.backend_type = if config.backend_type.eq_ignore_ascii_case("Sftp") { "Sftp".into() } else { "Folder".into() };
        s.folder_path = config.folder_path.trim().to_string();
        s.sftp_session_id = config.sftp_session_id;
        s.sftp_remote_path = if config.sftp_remote_path.trim().is_empty() { "~/.snterm/sync.vault".into() } else { config.sftp_remote_path.trim().to_string() };
        s.encrypted_sync_password = dpapi::encrypt(Some(&password));
        s.include_key_files = include_key_files;
        s.last_revision = 0;
        s.last_error = None;
        ensure_device(s);
    });
    log_line(&state, &format!("Bật đồng bộ: {}", backend_label(&state, &config)));
    let r = run_cycle(&app, true).await;
    start_scheduler(&app);
    r
}

#[tauri::command]
pub async fn sync_run_now(app: AppHandle) -> CmdResult<SyncOutcome> {
    run_cycle(&app, true).await
}

#[tauri::command]
pub async fn sync_disable(app: AppHandle, state: State<'_, AppState>, delete_vault: bool) -> CmdResult<()> {
    let sync = state.settings().sync;
    if delete_vault {
        let lang = state.language();
        let cfg = config_from_settings(&sync);
        if let Ok(b) = make_backend(&state, &cfg) {
            b.delete_vault().await.map_err(|e| e.translate(&lang, None))?;
        }
    }
    update_settings(&state, |s| {
        s.enabled = false;
        s.encrypted_sync_password = None;
        s.last_error = None;
        s.last_revision = 0;
    });
    log_line(&state, "Ngắt đồng bộ");
    set_state(&app, "never");
    Ok(())
}

/// Nhập (lại) mật khẩu đồng bộ khi kho bị đổi mật khẩu ở máy khác.
#[tauri::command]
pub async fn sync_set_password(app: AppHandle, state: State<'_, AppState>, password: String) -> CmdResult<SyncOutcome> {
    update_settings(&state, |s| {
        s.encrypted_sync_password = dpapi::encrypt(Some(&password));
        s.last_error = None;
    });
    run_cycle(&app, true).await
}

/// Đổi mật khẩu đồng bộ: mã hóa lại kho với mật khẩu mới.
#[tauri::command]
pub async fn sync_change_password(app: AppHandle, state: State<'_, AppState>, new_password: String) -> CmdResult<()> {
    if new_password.len() < 8 {
        return Err("short_password".into());
    }
    let lang = state.language();
    let sync = state.settings().sync;
    let old = dpapi::decrypt(sync.encrypted_sync_password.as_deref()).map(Zeroizing::new).ok_or("needPassword")?;
    let cfg = config_from_settings(&sync);
    let backend = make_backend(&state, &cfg).map_err(|e| e.translate(&lang, None))?;
    let _guard = state.sync.run_lock.lock().await;
    let ctx = SyncContext {
        store: &state.sessions,
        backend: backend.as_ref(),
        password: &old,
        device_id: sync.device_id.unwrap_or_else(Uuid::new_v4),
        device_name: sync.device_name.clone(),
        include_key_files: sync.include_key_files,
        last_revision: sync.last_revision,
        keys_dir: state.paths.keys_dir(),
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let rev = engine::rewrite_with_password(&ctx, &new_password).await.map_err(|e| e.translate(&lang, None))?;
    update_settings(&state, |s| {
        s.encrypted_sync_password = dpapi::encrypt(Some(&new_password));
        s.last_revision = rev;
        s.last_sync_at = Some(Utc::now());
        s.last_error = None;
    });
    log_line(&state, &format!("Đổi mật khẩu đồng bộ, rev={rev}"));
    set_state(&app, "idle");
    Ok(())
}

#[tauri::command]
pub fn sync_update_settings(app: AppHandle, state: State<'_, AppState>, interval_minutes: i32, include_key_files: bool) -> CmdResult<()> {
    update_settings(&state, |s| {
        s.interval_minutes = interval_minutes.max(5);
        s.include_key_files = include_key_files;
    });
    emit_status(&app);
    Ok(())
}

#[tauri::command]
pub fn sync_log(state: State<'_, AppState>) -> CmdResult<String> {
    let dir = state.paths.logs_dir();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("sync-")).unwrap_or(false)).collect())
        .unwrap_or_default();
    files.sort();
    let mut out = String::new();
    for f in files.iter().rev().take(2).rev() {
        if let Ok(t) = std::fs::read_to_string(f) {
            out.push_str(&t);
        }
    }
    let lines: Vec<&str> = out.lines().collect();
    let tail = lines.iter().rev().take(200).rev().cloned().collect::<Vec<_>>().join("\n");
    Ok(tail)
}

/// Trước khi đóng app: nếu có thay đổi chưa đẩy → chạy một lần (tối đa 5 s).
#[tauri::command]
pub async fn sync_flush(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if !state.settings().sync.enabled {
        return Ok(());
    }
    if !state.sync.pending_change.load(Ordering::SeqCst) && !state.sync.debounce_armed.load(Ordering::SeqCst) {
        return Ok(());
    }
    let _ = tokio::time::timeout(Duration::from_secs(5), run_cycle(&app, false)).await;
    Ok(())
}
