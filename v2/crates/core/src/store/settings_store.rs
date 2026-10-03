//! Đọc/ghi `settings.json` (tương đương `SettingsStore.cs` v1).
use crate::error::CoreResult;
use crate::paths::{atomic_write, AppPaths};
use super::model::AppSettings;
use super::session_store::strip_bom;

pub struct SettingsStore {
    paths: AppPaths,
}

impl SettingsStore {
    pub fn new(paths: AppPaths) -> Self { SettingsStore { paths } }

    pub fn load(&self) -> AppSettings {
        let file = self.paths.settings_file();
        if !file.exists() {
            return AppSettings::default();
        }
        std::fs::read_to_string(&file)
            .ok()
            .and_then(|j| serde_json::from_str::<AppSettings>(strip_bom(&j)).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &AppSettings) -> CoreResult<()> {
        let json = serde_json::to_string_pretty(settings)?;
        atomic_write(&self.paths.settings_file(), json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_defaults_on_missing() {
        let d = tempfile::tempdir().unwrap();
        let p = AppPaths::new(d.path().into(), d.path().into());
        let st = SettingsStore::new(p.clone());
        assert_eq!(st.load(), AppSettings::default());
        let mut s = AppSettings::default();
        s.theme = "Light".into();
        s.collapsed_groups = vec!["Dev".into()];
        st.save(&s).unwrap();
        let raw = std::fs::read_to_string(p.settings_file()).unwrap();
        assert!(raw.contains("\"CollapsedGroups\""));
        assert_eq!(st.load(), s);
        std::fs::write(p.settings_file(), "garbage").unwrap();
        assert_eq!(st.load(), AppSettings::default());
    }
}
