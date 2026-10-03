//! Đọc/ghi `sessions.json` (tương đương `SessionStore.cs` v1): ghi nguyên tử, sao lưu hằng ngày,
//! khôi phục khi file hỏng, giữ 10 bản sao lưu.
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Local;
use uuid::Uuid;

use crate::error::CoreResult;
use crate::paths::{atomic_write, AppPaths};
use super::model::{Session, SessionFileEnvelope};

pub struct SessionStore {
    paths: AppPaths,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    last_daily_backup: Option<String>,
    /// Trường lạ ở envelope (giữ nguyên khi ghi lại).
    envelope_extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct LoadResult {
    pub sessions: Vec<Session>,
    pub recovered_from_corruption: bool,
}

impl SessionStore {
    pub fn new(paths: AppPaths) -> Self {
        SessionStore { paths, state: Mutex::new(State::default()) }
    }

    pub fn paths(&self) -> &AppPaths { &self.paths }

    pub fn load(&self) -> LoadResult {
        let file = self.paths.sessions_file();
        if !file.exists() {
            return LoadResult::default();
        }
        match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|json| {
            serde_json::from_str::<SessionFileEnvelope>(strip_bom(&json)).map_err(|e| e.to_string())
        }) {
            Ok(env) => {
                if let Ok(mut st) = self.state.lock() {
                    st.envelope_extra = env.extra.clone();
                }
                LoadResult { sessions: env.sessions, recovered_from_corruption: false }
            }
            Err(err) => {
                tracing::warn!("sessions.json hỏng: {err}");
                let ts = Local::now().format("%Y%m%d-%H%M%S");
                let corrupt = self.paths.data_dir.join(format!("sessions.corrupt-{ts}.json"));
                let _ = std::fs::rename(&file, &corrupt);
                // Thử khôi phục từ bản sao lưu mới nhất.
                if let Some(latest) = self.latest_backup() {
                    if let Ok(json) = std::fs::read_to_string(&latest) {
                        if let Ok(env) = serde_json::from_str::<SessionFileEnvelope>(strip_bom(&json)) {
                            let _ = self.save(&env.sessions);
                            return LoadResult { sessions: env.sessions, recovered_from_corruption: true };
                        }
                    }
                }
                LoadResult { sessions: Vec::new(), recovered_from_corruption: true }
            }
        }
    }

    fn latest_backup(&self) -> Option<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(self.paths.backups_dir())
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("sessions-") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort();
        files.pop()
    }

    pub fn save(&self, sessions: &[Session]) -> CoreResult<()> {
        let extra = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            self.perform_daily_backup_if_needed(&mut st);
            st.envelope_extra.clone()
        };
        let env = SessionFileEnvelope { version: 1, sessions: sessions.to_vec(), extra };
        let json = serde_json::to_string_pretty(&env)?;
        atomic_write(&self.paths.sessions_file(), json.as_bytes())?;
        Ok(())
    }

    /// Cập nhật (hoặc thêm) một VM rồi lưu ngay.
    pub fn update_session(&self, updated: &Session) -> CoreResult<Vec<Session>> {
        let mut sessions = self.load().sessions;
        match sessions.iter_mut().find(|s| s.id == updated.id) {
            Some(s) => *s = updated.clone(),
            None => sessions.push(updated.clone()),
        }
        self.save(&sessions)?;
        Ok(sessions)
    }

    pub fn find(&self, id: Uuid) -> Option<Session> {
        self.load().sessions.into_iter().find(|s| s.id == id)
    }

    pub fn backup_before_import(&self) -> Option<PathBuf> {
        let file = self.paths.sessions_file();
        if !file.exists() {
            return None;
        }
        let ts = Local::now().format("%Y%m%d-%H%M%S");
        let backup = self.paths.backups_dir().join(format!("sessions-before-import-{ts}.json"));
        std::fs::copy(&file, &backup).ok()?;
        Some(backup)
    }

    fn perform_daily_backup_if_needed(&self, st: &mut State) {
        let today = Local::now().format("%Y%m%d").to_string();
        if st.last_daily_backup.as_deref() == Some(&today) {
            return;
        }
        let file = self.paths.sessions_file();
        if file.exists() {
            let backup = self.paths.backups_dir().join(format!("sessions-{today}.json"));
            if !backup.exists() {
                let _ = std::fs::copy(&file, &backup);
            }
            st.last_daily_backup = Some(today);
            self.clean_old_backups();
        }
    }

    fn clean_old_backups(&self) {
        let Ok(rd) = std::fs::read_dir(self.paths.backups_dir()) else { return };
        let mut daily: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("sessions-20") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .collect();
        daily.sort();
        daily.reverse();
        for old in daily.into_iter().skip(10) {
            let _ = std::fs::remove_file(old);
        }
    }
}

pub(crate) fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    fn temp_paths() -> (tempfile::TempDir, AppPaths) {
        let d = tempfile::tempdir().unwrap();
        let p = AppPaths::new(d.path().to_path_buf(), d.path().to_path_buf());
        (d, p)
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (_d, p) = temp_paths();
        let store = SessionStore::new(p.clone());
        let mut s = Session::default();
        s.name = "web-01 tiếng Việt".into();
        s.host = "10.0.0.1".into();
        s.username = "ubuntu".into();
        store.save(&[s.clone()]).unwrap();
        let loaded = store.load();
        assert!(!loaded.recovered_from_corruption);
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].name, "web-01 tiếng Việt");
        assert_eq!(loaded.sessions[0].id, s.id);
        let raw = std::fs::read_to_string(p.sessions_file()).unwrap();
        assert!(raw.starts_with("{\n  \"version\": 1"));
        assert!(!p.sessions_file().with_extension("json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_renamed_and_recovered_from_backup() {
        let (_d, p) = temp_paths();
        let store = SessionStore::new(p.clone());
        let mut s = Session::default();
        s.host = "h".into();
        s.username = "u".into();
        store.save(&[s.clone()]).unwrap();
        // Lần lưu thứ hai tạo bản sao lưu hằng ngày từ file hiện có.
        store.save(&[s.clone()]).unwrap();
        // Ép tạo backup: state đã có ngày hôm nay nên tạo store mới.
        let store2 = SessionStore::new(p.clone());
        store2.save(&[s.clone()]).unwrap();
        assert!(std::fs::read_dir(p.backups_dir()).unwrap().count() >= 1);

        std::fs::write(p.sessions_file(), "{ this is not json").unwrap();
        let store3 = SessionStore::new(p.clone());
        let loaded = store3.load();
        assert!(loaded.recovered_from_corruption);
        assert_eq!(loaded.sessions.len(), 1);
        let corrupt_exists = std::fs::read_dir(&p.data_dir).unwrap().any(|e| {
            e.unwrap().file_name().to_string_lossy().starts_with("sessions.corrupt-")
        });
        assert!(corrupt_exists);
    }

    #[test]
    fn update_session_adds_or_replaces() {
        let (_d, p) = temp_paths();
        let store = SessionStore::new(p);
        let mut s = Session::default();
        s.host = "h".into();
        s.username = "u".into();
        store.update_session(&s).unwrap();
        s.name = "renamed".into();
        let all = store.update_session(&s).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "renamed");
    }

    #[test]
    fn keeps_unknown_envelope_fields() {
        let (_d, p) = temp_paths();
        std::fs::write(p.sessions_file(), r#"{"version":1,"sessions":[],"Deleted":[{"Id":"x"}]}"#).unwrap();
        let store = SessionStore::new(p.clone());
        let _ = store.load();
        store.save(&[]).unwrap();
        let raw = std::fs::read_to_string(p.sessions_file()).unwrap();
        assert!(raw.contains("Deleted"));
    }
}
