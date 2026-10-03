//! Đường dẫn dữ liệu, tương đương `AppPaths.cs` v1 — cùng thư mục `%APPDATA%\SNTerm`.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub local_dir: PathBuf,
}

impl AppPaths {
    /// Thư mục mặc định. Biến môi trường `SNTERM_DATA_DIR` (chỉ để kiểm thử) ghi đè cả hai.
    pub fn default_paths() -> AppPaths {
        if let Ok(dir) = std::env::var("SNTERM_DATA_DIR") {
            if !dir.trim().is_empty() {
                let p = PathBuf::from(dir);
                return AppPaths::new(p.clone(), p);
            }
        }
        let data = roaming_app_data().join("SNTerm");
        let local = local_app_data().join("SNTerm");
        AppPaths::new(data, local)
    }

    pub fn new(data_dir: PathBuf, local_dir: PathBuf) -> AppPaths {
        let p = AppPaths { data_dir, local_dir };
        p.ensure_dirs();
        p
    }

    pub fn ensure_dirs(&self) {
        for d in [&self.data_dir, &self.keys_dir(), &self.backups_dir(), &self.logs_dir()] {
            let _ = std::fs::create_dir_all(d);
        }
    }

    pub fn sessions_file(&self) -> PathBuf { self.data_dir.join("sessions.json") }
    pub fn settings_file(&self) -> PathBuf { self.data_dir.join("settings.json") }
    pub fn known_hosts_file(&self) -> PathBuf { self.data_dir.join("known_hosts.json") }
    pub fn keys_dir(&self) -> PathBuf { self.data_dir.join("keys") }
    pub fn backups_dir(&self) -> PathBuf { self.data_dir.join("backups") }
    pub fn logs_dir(&self) -> PathBuf { self.local_dir.join("logs") }
    pub fn webview2_dir(&self) -> PathBuf { self.local_dir.join("WebView2") }
    pub fn edit_dir(&self) -> PathBuf { self.local_dir.join("edit") }
}

fn roaming_app_data() -> PathBuf {
    if let Ok(v) = std::env::var("APPDATA") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    home().join(".config")
}

fn local_app_data() -> PathBuf {
    if let Ok(v) = std::env::var("LOCALAPPDATA") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    home().join(".local").join("share")
}

fn home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Ghi file an toàn: ghi `.tmp` rồi đổi tên đè (giống v1).
pub fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp = tmp_path(path);
    std::fs::write(&tmp, content)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows: rename không đè được khi file đích đang mở → xóa rồi đổi tên.
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path)
        }
    }
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}
