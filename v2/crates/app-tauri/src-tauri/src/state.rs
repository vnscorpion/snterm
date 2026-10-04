//! Trạng thái toàn ứng dụng chia sẻ giữa các lệnh Tauri.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use snterm_core::paths::AppPaths;
use snterm_core::store::{AppSettings, KnownHostsStore, SessionStore, SettingsStore};
use tokio::sync::oneshot;

use crate::commands::sync::SyncManager;
use crate::commands::terminal::TabHandle;

/// Câu trả lời của người dùng cho hộp thoại do lõi yêu cầu (host key / mật khẩu).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DialogAnswer {
    HostKey { decision: String },
    Password { password: Option<String>, save: bool },
}

pub struct AppState {
    pub paths: AppPaths,
    pub sessions: SessionStore,
    pub settings_store: SettingsStore,
    pub settings: Mutex<AppSettings>,
    pub known_hosts: Arc<KnownHostsStore>,
    pub tabs: Mutex<HashMap<String, TabHandle>>,
    pub pending_dialogs: Mutex<HashMap<String, oneshot::Sender<DialogAnswer>>>,
    /// Hàng đợi hộp thoại: chỉ một hộp thoại cần người dùng trả lời tại một thời điểm (như v1).
    pub dialog_gate: Arc<tokio::sync::Mutex<()>>,
    pub connect_gate: Arc<tokio::sync::Semaphore>,
    pub sync: SyncManager,
}

impl AppState {
    pub fn new() -> Self {
        let paths = AppPaths::default_paths();
        let settings_store = SettingsStore::new(paths.clone());
        let settings = settings_store.load();
        let max_parallel = settings.max_parallel_connects.clamp(1, 32) as usize;
        AppState {
            sessions: SessionStore::new(paths.clone()),
            known_hosts: Arc::new(KnownHostsStore::new(paths.clone())),
            settings_store,
            settings: Mutex::new(settings),
            paths,
            tabs: Mutex::new(HashMap::new()),
            pending_dialogs: Mutex::new(HashMap::new()),
            dialog_gate: Arc::new(tokio::sync::Mutex::new(())),
            connect_gate: Arc::new(tokio::sync::Semaphore::new(max_parallel)),
            sync: SyncManager::new(),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn language(&self) -> String {
        self.settings().language
    }
}
