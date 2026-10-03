//! Tab terminal: kết nối SSH, stream output về xterm.js qua `Channel`, resize, reconnect, monitor.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use snterm_core::crypto::dpapi;
use snterm_core::error::CoreError;
use snterm_core::ssh::monitor::{parse_sample, MonitorTracker, MONITOR_COMMAND};
use snterm_core::ssh::sftp::SftpClient;
use snterm_core::ssh::{ConnectParams, HostKeyInfo, HostKeyVerifier, SshEvent, SshSession};
use snterm_core::store::{ConnectionStatus, HostKeyStatus, KnownHostsStore, Session};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, oneshot, Mutex, Notify};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::state::{AppState, DialogAnswer};
use super::CmdResult;

/// Dữ liệu một tab đang mở (giữ trong `AppState.tabs`).
pub struct TabHandle {
    pub inner: Arc<TabInner>,
}

pub struct TabInner {
    pub tab_id: String,
    pub session: Mutex<Session>,
    pub ssh: Mutex<Option<Arc<SshSession>>>,
    pub sftp: Mutex<Option<Arc<SftpClient>>>,
    pub status: std::sync::Mutex<ConnectionStatus>,
    pub output: Channel<InvokeResponseBody>,
    pub cols: std::sync::Mutex<(u32, u32)>,
    pub cached_password: Mutex<Option<Zeroizing<String>>>,
    pub cached_passphrase: Mutex<Option<Zeroizing<String>>>,
    pub monitor_enabled: AtomicBool,
    pub monitor_wake: Notify,
    pub transfer_cancel: Arc<AtomicBool>,
    pub closed: AtomicBool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabStatusEvent {
    pub tab_id: String,
    pub session_id: Uuid,
    pub status: ConnectionStatus,
    /// Dòng in vào terminal (đã có mã màu ANSI) hoặc rỗng.
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogRequest {
    pub request_id: String,
    pub tab_id: String,
    pub kind: String, // "hostKey" | "password"
    pub title: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub changed: bool,
    pub was_rejected: bool,
}

/// Verifier dùng chung: so `known_hosts.json`; chưa tin cậy → hỏi giao diện, chờ trả lời (hàng đợi hộp thoại).
pub struct AppHostKeyVerifier {
    pub app: AppHandle,
    pub tab_id: String,
    pub title: String,
    pub username: String,
    pub known_hosts: Arc<KnownHostsStore>,
    pub dialog_gate: Arc<tokio::sync::Mutex<()>>,
}

#[async_trait]
impl HostKeyVerifier for AppHostKeyVerifier {
    async fn verify(&self, info: HostKeyInfo) -> bool {
        if self.known_hosts.check_host(&info.host, info.port, &info.fingerprint_sha256) == HostKeyStatus::Trusted {
            return true;
        }
        let _gate = self.dialog_gate.lock().await;
        let status = self.known_hosts.check_host(&info.host, info.port, &info.fingerprint_sha256);
        if status == HostKeyStatus::Trusted {
            return true;
        }
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let state = self.app.state::<AppState>();
            state.pending_dialogs.lock().unwrap_or_else(|e| e.into_inner()).insert(request_id.clone(), tx);
        }
        let _ = self.app.emit(
            "dialog:request",
            DialogRequest {
                request_id: request_id.clone(),
                tab_id: self.tab_id.clone(),
                kind: "hostKey".into(),
                title: self.title.clone(),
                host: info.host.clone(),
                port: info.port,
                username: self.username.clone(),
                algorithm: info.algorithm.clone(),
                fingerprint: info.fingerprint_sha256.clone(),
                changed: status == HostKeyStatus::Changed,
                was_rejected: false,
            },
        );
        let answer = tokio::time::timeout(Duration::from_secs(300), rx).await;
        {
            let state = self.app.state::<AppState>();
            state.pending_dialogs.lock().unwrap_or_else(|e| e.into_inner()).remove(&request_id);
        }
        match answer {
            Ok(Ok(DialogAnswer::HostKey { decision })) => match decision.as_str() {
                "trustSave" => {
                    self.known_hosts.add_or_update(&info.host, info.port, &info.algorithm, &info.fingerprint_sha256);
                    true
                }
                "trustOnce" => true,
                _ => false,
            },
            _ => false,
        }
    }
}

/// Hỏi mật khẩu qua giao diện (hàng đợi hộp thoại). Trả về (mật khẩu, lưu lại?).
pub async fn prompt_password(
    app: &AppHandle,
    tab_id: &str,
    session: &Session,
    was_rejected: bool,
) -> Option<(Zeroizing<String>, bool)> {
    let state = app.state::<AppState>();
    let gate = state.dialog_gate.clone();
    let _g = gate.lock().await;
    let request_id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    state.pending_dialogs.lock().unwrap_or_else(|e| e.into_inner()).insert(request_id.clone(), tx);
    let _ = app.emit(
        "dialog:request",
        DialogRequest {
            request_id: request_id.clone(),
            tab_id: tab_id.to_string(),
            kind: "password".into(),
            title: session.display_name(),
            host: session.host.clone(),
            port: session.port,
            username: session.username.clone(),
            algorithm: String::new(),
            fingerprint: String::new(),
            changed: false,
            was_rejected,
        },
    );
    let answer = tokio::time::timeout(Duration::from_secs(600), rx).await;
    state.pending_dialogs.lock().unwrap_or_else(|e| e.into_inner()).remove(&request_id);
    match answer {
        Ok(Ok(DialogAnswer::Password { password: Some(p), save })) if !p.is_empty() => Some((Zeroizing::new(p), save)),
        _ => None,
    }
}

#[tauri::command]
pub fn dialog_answer(state: State<'_, AppState>, request_id: String, answer: DialogAnswer) -> CmdResult<()> {
    let tx = state.pending_dialogs.lock().unwrap_or_else(|e| e.into_inner()).remove(&request_id);
    if let Some(tx) = tx {
        let _ = tx.send(answer);
    }
    Ok(())
}

fn set_status(app: &AppHandle, inner: &TabInner, session_id: Uuid, status: ConnectionStatus, message: &str) {
    *inner.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
    let _ = app.emit(
        "tab:status",
        TabStatusEvent { tab_id: inner.tab_id.clone(), session_id, status, message: message.to_string() },
    );
}

fn write_text(inner: &TabInner, text: &str) {
    let _ = inner.output.send(InvokeResponseBody::Raw(text.as_bytes().to_vec()));
}

fn disconnect_banner(lang: &str, detail: Option<&str>) -> String {
    let vi = lang.eq_ignore_ascii_case("vi");
    let prompt = if vi { "Bấm [R] để kết nối lại" } else { "Press [R] to reconnect" };
    let d = detail.map(|d| format!(": {d}")).unwrap_or_default();
    format!("\r\n\x1b[31m[Disconnect{d}]\x1b[0m\r\n\x1b[33m{prompt}\x1b[0m\r\n")
}

/// Mở tab mới cho một VM và bắt đầu kết nối. `on_output` nhận bytes thô của shell.
#[tauri::command]
pub async fn terminal_open(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Uuid,
    cols: u32,
    rows: u32,
    on_output: Channel<InvokeResponseBody>,
) -> CmdResult<String> {
    let session = state.sessions.find(session_id).ok_or("not found")?;
    let tab_id = Uuid::new_v4().to_string();
    let cached_passphrase = dpapi::decrypt(session.encrypted_passphrase.as_deref()).map(Zeroizing::new);
    let inner = Arc::new(TabInner {
        tab_id: tab_id.clone(),
        session: Mutex::new(session.clone()),
        ssh: Mutex::new(None),
        sftp: Mutex::new(None),
        status: std::sync::Mutex::new(ConnectionStatus::Disconnected),
        output: on_output,
        cols: std::sync::Mutex::new((cols.max(10), rows.max(5))),
        cached_password: Mutex::new(None),
        cached_passphrase: Mutex::new(cached_passphrase),
        monitor_enabled: AtomicBool::new(true),
        monitor_wake: Notify::new(),
        transfer_cancel: Arc::new(AtomicBool::new(false)),
        closed: AtomicBool::new(false),
    });
    state.tabs.lock().unwrap_or_else(|e| e.into_inner()).insert(
        tab_id.clone(),
        TabHandle { inner: inner.clone() },
    );
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        start_connection(app2, inner).await;
    });
    Ok(tab_id)
}

fn get_tab(state: &AppState, tab_id: &str) -> Option<Arc<TabInner>> {
    state.tabs.lock().unwrap_or_else(|e| e.into_inner()).get(tab_id).map(|t| t.inner.clone())
}

/// Luồng kết nối: giống `TerminalTabViewModel.StartConnectionAsync` v1.
pub async fn start_connection(app: AppHandle, inner: Arc<TabInner>) {
    let state = app.state::<AppState>();
    {
        let st = *inner.status.lock().unwrap_or_else(|e| e.into_inner());
        if st == ConnectionStatus::Connected || st == ConnectionStatus::Connecting {
            return;
        }
    }
    let settings = state.settings();
    let lang = settings.language.clone();
    let session = inner.session.lock().await.clone();
    set_status(&app, &inner, session.id, ConnectionStatus::Connecting, "");
    let vi = lang.eq_ignore_ascii_case("vi");
    write_text(
        &inner,
        &if vi {
            format!("Đang kết nối tới {} ({})...\r\n", session.display_name(), session.host)
        } else {
            format!("Connecting to {} ({})...\r\n", session.display_name(), session.host)
        },
    );

    let mut auth_retry = false;
    loop {
        // Mật khẩu: đã lưu → dùng; chưa có và không có key → hỏi.
        let has_cached = inner.cached_password.lock().await.is_some();
        if !has_cached {
            if session.save_password && session.encrypted_password.is_some() && !auth_retry {
                if let Some(p) = dpapi::decrypt(session.encrypted_password.as_deref()) {
                    *inner.cached_password.lock().await = Some(Zeroizing::new(p));
                }
            } else if session.key_file_path.as_deref().map(|p| p.trim().is_empty()).unwrap_or(true) || auth_retry {
                match prompt_password(&app, &inner.tab_id, &session, auth_retry).await {
                    Some((pw, save)) => {
                        if save {
                            let mut s = inner.session.lock().await;
                            s.save_password = true;
                            s.encrypted_password = dpapi::encrypt(Some(pw.as_str()));
                            let _ = state.sessions.update_session(&s);
                            let _ = app.emit("sessions:changed", ());
                        }
                        *inner.cached_password.lock().await = Some(pw);
                    }
                    None => {
                        let msg = if vi {
                            "\r\nĐã hủy kết nối.\r\nNhấn Enter để kết nối lại...\r\n"
                        } else {
                            "\r\nConnection canceled.\r\nPress Enter to reconnect...\r\n"
                        };
                        write_text(&inner, msg);
                        set_status(&app, &inner, session.id, ConnectionStatus::Disconnected, "");
                        return;
                    }
                }
            }
        }

        let params = ConnectParams {
            host: session.host.clone(),
            port: session.port,
            username: session.username.clone(),
            password: inner.cached_password.lock().await.clone(),
            key_file_path: session.key_file_path.clone(),
            passphrase: inner.cached_passphrase.lock().await.clone(),
            keepalive_seconds: settings.keep_alive_seconds.max(0) as u32,
            connect_timeout: Duration::from_secs(15),
        };
        let verifier = Arc::new(AppHostKeyVerifier {
            app: app.clone(),
            tab_id: inner.tab_id.clone(),
            title: session.display_name(),
            username: session.username.clone(),
            known_hosts: state.known_hosts.clone(),
            dialog_gate: state.dialog_gate.clone(),
        });

        // Giới hạn số kết nối song song (MaxParallelConnects).
        let permit = state.connect_gate.clone().acquire_owned().await.ok();
        let result = SshSession::connect(params, verifier).await;
        drop(permit);

        match result {
            Ok(ssh) => {
                let ssh = Arc::new(ssh);
                let (cols, rows) = *inner.cols.lock().unwrap_or_else(|e| e.into_inner());
                let (tx, rx) = mpsc::channel::<SshEvent>(256);
                if let Err(e) = ssh.open_shell(cols, rows, tx).await {
                    let msg = e.translate(&lang, Some(&session.host));
                    write_text(&inner, &disconnect_banner(&lang, Some(&msg)));
                    set_status(&app, &inner, session.id, ConnectionStatus::Disconnected, "");
                    return;
                }
                *inner.ssh.lock().await = Some(ssh.clone());
                set_status(&app, &inner, session.id, ConnectionStatus::Connected, "");
                {
                    let mut s = inner.session.lock().await;
                    s.last_connected_at = Some(chrono::Utc::now());
                    let _ = state.sessions.update_session(&s);
                }
                let _ = app.emit("sessions:changed", ());
                let _ = app.emit("tab:connected", &inner.tab_id);
                spawn_output_pump(app.clone(), inner.clone(), rx, lang.clone());
                spawn_monitor(app.clone(), inner.clone(), ssh.clone());
                return;
            }
            Err(CoreError::Auth) => {
                if !auth_retry && session.encrypted_password.is_some() {
                    auth_retry = true;
                    *inner.cached_password.lock().await = None;
                    let msg = if vi { "\r\n\x1b[31mMật khẩu đã lưu không chính xác.\x1b[0m\r\n" } else { "\r\n\x1b[31mSaved password is incorrect.\x1b[0m\r\n" };
                    write_text(&inner, msg);
                    continue;
                }
                *inner.cached_password.lock().await = None;
                let msg = if vi {
                    "\r\n\x1b[31mSai thông tin đăng nhập.\x1b[0m\r\nNhấn Enter để thử lại...\r\n"
                } else {
                    "\r\n\x1b[31mIncorrect credentials.\x1b[0m\r\nPress Enter to try again...\r\n"
                };
                write_text(&inner, msg);
                set_status(&app, &inner, session.id, ConnectionStatus::Disconnected, "");
                return;
            }
            Err(e) => {
                let msg = e.translate(&lang, Some(&session.host));
                write_text(&inner, &disconnect_banner(&lang, Some(&msg)));
                set_status(&app, &inner, session.id, ConnectionStatus::Disconnected, "");
                return;
            }
        }
    }
}

/// Gom output ~16 ms hoặc 64 KB rồi đẩy sang xterm.js (như v1 mục 8.3).
fn spawn_output_pump(app: AppHandle, inner: Arc<TabInner>, mut rx: mpsc::Receiver<SshEvent>, lang: String) {
    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<u8> = Vec::with_capacity(65536);
        loop {
            let first = rx.recv().await;
            match first {
                Some(SshEvent::Output(b)) => buf.extend_from_slice(&b),
                Some(SshEvent::Closed(reason)) => {
                    on_closed(&app, &inner, reason, &lang).await;
                    break;
                }
                None => {
                    on_closed(&app, &inner, None, &lang).await;
                    break;
                }
            }
            let deadline = tokio::time::sleep(Duration::from_millis(16));
            tokio::pin!(deadline);
            let mut closed: Option<Option<CoreError>> = None;
            loop {
                tokio::select! {
                    _ = &mut deadline => break,
                    msg = rx.recv() => match msg {
                        Some(SshEvent::Output(b)) => {
                            buf.extend_from_slice(&b);
                            if buf.len() >= 65536 { break; }
                        }
                        Some(SshEvent::Closed(r)) => { closed = Some(r); break; }
                        None => { closed = Some(None); break; }
                    }
                }
            }
            if !buf.is_empty() {
                let _ = inner.output.send(InvokeResponseBody::Raw(std::mem::take(&mut buf)));
                buf.reserve(65536);
            }
            if let Some(reason) = closed {
                on_closed(&app, &inner, reason, &lang).await;
                break;
            }
        }
    });
}

async fn on_closed(app: &AppHandle, inner: &Arc<TabInner>, reason: Option<CoreError>, lang: &str) {
    if inner.closed.load(Ordering::SeqCst) {
        return;
    }
    let session_id = inner.session.lock().await.id;
    let host = inner.session.lock().await.host.clone();
    inner.ssh.lock().await.take();
    inner.sftp.lock().await.take();
    let detail = reason.map(|e| e.translate(lang, Some(&host)));
    write_text(inner, &disconnect_banner(lang, detail.as_deref()));
    set_status(app, inner, session_id, ConnectionStatus::Disconnected, "");
    let _ = app.emit("tab:monitor", serde_json::json!({ "tabId": inner.tab_id, "info": serde_json::Value::Null }));
}

/// Thu thập thông tin máy chủ mỗi 3 giây khi tab đang được xem (tạm dừng khi ẩn để nhẹ máy).
fn spawn_monitor(app: AppHandle, inner: Arc<TabInner>, ssh: Arc<SshSession>) {
    tauri::async_runtime::spawn(async move {
        let mut tracker = MonitorTracker::default();
        let (host, user) = {
            let s = inner.session.lock().await;
            (s.host.clone(), s.username.clone())
        };
        loop {
            if inner.closed.load(Ordering::SeqCst) || !ssh.is_alive() {
                break;
            }
            if !inner.monitor_enabled.load(Ordering::SeqCst) {
                inner.monitor_wake.notified().await;
                continue;
            }
            match ssh.exec(MONITOR_COMMAND, Duration::from_secs(4)).await {
                Ok(raw) => {
                    if let Some(sample) = parse_sample(&raw) {
                        let info = tracker.update(&sample, &host, &user);
                        let _ = app.emit("tab:monitor", serde_json::json!({ "tabId": inner.tab_id, "info": info }));
                    }
                }
                Err(CoreError::Timeout) => {}
                Err(_) => {
                    if !ssh.is_alive() {
                        break;
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                _ = inner.monitor_wake.notified() => {}
            }
        }
    });
}

#[tauri::command]
pub async fn terminal_input(app: AppHandle, state: State<'_, AppState>, tab_id: String, data: String) -> CmdResult<()> {
    let Some(inner) = get_tab(&state, &tab_id) else { return Ok(()) };
    let status = *inner.status.lock().unwrap_or_else(|e| e.into_inner());
    if status == ConnectionStatus::Disconnected {
        // Phím R / Enter khi đã ngắt → kết nối lại (như v1).
        if data.contains('r') || data.contains('R') || data.contains('\r') || data.contains('\n') {
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move { start_connection(app2, inner).await });
        }
        return Ok(());
    }
    if status != ConnectionStatus::Connected {
        return Ok(());
    }
    let ssh = inner.ssh.lock().await.clone();
    if let Some(ssh) = ssh {
        let _ = ssh.send_input(data.as_bytes()).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn terminal_resize(state: State<'_, AppState>, tab_id: String, cols: u32, rows: u32) -> CmdResult<()> {
    let Some(inner) = get_tab(&state, &tab_id) else { return Ok(()) };
    *inner.cols.lock().unwrap_or_else(|e| e.into_inner()) = (cols.max(10), rows.max(5));
    let ssh = inner.ssh.lock().await.clone();
    if let Some(ssh) = ssh {
        let _ = ssh.resize(cols, rows).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn terminal_reconnect(app: AppHandle, state: State<'_, AppState>, tab_id: String) -> CmdResult<()> {
    let Some(inner) = get_tab(&state, &tab_id) else { return Ok(()) };
    if let Some(ssh) = inner.ssh.lock().await.take() {
        ssh.disconnect().await;
    }
    inner.sftp.lock().await.take();
    *inner.status.lock().unwrap_or_else(|e| e.into_inner()) = ConnectionStatus::Disconnected;
    start_connection(app, inner).await;
    Ok(())
}

#[tauri::command]
pub async fn terminal_close(state: State<'_, AppState>, tab_id: String) -> CmdResult<()> {
    let handle = state.tabs.lock().unwrap_or_else(|e| e.into_inner()).remove(&tab_id);
    if let Some(h) = handle {
        h.inner.closed.store(true, Ordering::SeqCst);
        h.inner.transfer_cancel.store(true, Ordering::SeqCst);
        h.inner.monitor_wake.notify_waiters();
        if let Some(ssh) = h.inner.ssh.lock().await.take() {
            ssh.disconnect().await;
        }
        h.inner.sftp.lock().await.take();
        *h.inner.cached_password.lock().await = None;
    }
    Ok(())
}

/// Bật/tắt thu thập thông tin máy chủ (tab đang xem mới chạy).
#[tauri::command]
pub fn terminal_set_visible(state: State<'_, AppState>, tab_id: String, visible: bool) -> CmdResult<()> {
    if let Some(inner) = get_tab(&state, &tab_id) {
        inner.monitor_enabled.store(visible, Ordering::SeqCst);
        if visible {
            inner.monitor_wake.notify_one();
        }
    }
    Ok(())
}

/// Số tab đang kết nối (để hỏi xác nhận khi đóng app).
#[tauri::command]
pub fn connected_tab_count(state: State<'_, AppState>) -> CmdResult<usize> {
    let tabs = state.tabs.lock().unwrap_or_else(|e| e.into_inner());
    Ok(tabs
        .values()
        .filter(|t| *t.inner.status.lock().unwrap_or_else(|e| e.into_inner()) == ConnectionStatus::Connected)
        .count())
}

/// "Kiểm tra kết nối" trong form Thêm/Sửa VM: kết nối + xác thực rồi ngắt.
#[tauri::command]
pub async fn test_connection(app: AppHandle, state: State<'_, AppState>, draft: crate::dto::SessionDraft) -> CmdResult<String> {
    let existing = draft.id.and_then(|id| state.sessions.find(id));
    let (session, password, passphrase) = crate::commands::sessions::session_from_draft_for_test(&draft, existing.as_ref());
    let lang = state.language();
    let params = ConnectParams {
        host: session.host.clone(),
        port: session.port,
        username: session.username.clone(),
        password: password.map(Zeroizing::new),
        key_file_path: session.key_file_path.clone(),
        passphrase: passphrase.map(Zeroizing::new),
        keepalive_seconds: 5,
        connect_timeout: Duration::from_secs(15),
    };
    let verifier = Arc::new(AppHostKeyVerifier {
        app: app.clone(),
        tab_id: String::new(),
        title: session.display_name(),
        username: session.username.clone(),
        known_hosts: state.known_hosts.clone(),
        dialog_gate: state.dialog_gate.clone(),
    });
    match SshSession::connect(params, verifier).await {
        Ok(ssh) => {
            ssh.disconnect().await;
            Ok("ok".into())
        }
        Err(e) => Err(e.translate(&lang, Some(&session.host))),
    }
}

/// Lấy (hoặc mở lười) SFTP của tab.
pub async fn ensure_sftp(inner: &Arc<TabInner>) -> Result<Arc<SftpClient>, CoreError> {
    if let Some(s) = inner.sftp.lock().await.clone() {
        return Ok(s);
    }
    let ssh = inner.ssh.lock().await.clone().ok_or(CoreError::ConnectionLost)?;
    let session = ssh.open_sftp().await?;
    let client = Arc::new(SftpClient::new(session).await?);
    *inner.sftp.lock().await = Some(client.clone());
    Ok(client)
}

pub fn tab_inner(state: &AppState, tab_id: &str) -> Option<Arc<TabInner>> {
    get_tab(state, tab_id)
}
