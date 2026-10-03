//! Mô hình dữ liệu, tên trường JSON **y hệt** v1 (`Models/*.cs`, System.Text.Json PascalCase).
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::datetime;

/// Tương đương `SessionInfo` v1. Trường lạ được giữ nguyên trong `extra` để không làm mất
/// dữ liệu của phiên bản khác (VD: khối đồng bộ của Phần 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub group: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub save_password: bool,
    pub encrypted_password: Option<String>,
    pub key_file_path: Option<String>,
    pub encrypted_passphrase: Option<String>,
    #[serde(with = "datetime::required")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "datetime::optional")]
    pub last_connected_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            id: Uuid::new_v4(),
            name: String::new(),
            group: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            save_password: true,
            encrypted_password: None,
            key_file_path: None,
            encrypted_passphrase: None,
            created_at: Utc::now(),
            last_connected_at: None,
            extra: serde_json::Map::new(),
        }
    }
}

pub const UNGROUPED_VI: &str = "Chưa phân nhóm";

impl Session {
    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            format!("{}@{}", self.username, self.host)
        } else {
            self.name.clone()
        }
    }

    /// Không kèm port (giống v1, tránh lộ port).
    pub fn subtitle(&self) -> String {
        format!("{}@{}", self.username, self.host)
    }

    pub fn effective_group(&self) -> String {
        if self.group.trim().is_empty() {
            UNGROUPED_VI.to_string()
        } else {
            self.group.trim().to_string()
        }
    }

    pub fn is_key_file_missing(&self) -> bool {
        match &self.key_file_path {
            Some(p) if !p.is_empty() => !std::path::Path::new(p).exists(),
            _ => false,
        }
    }

    /// Nhân bản (giống `SessionInfo.Clone()` v1): id mới, tên thêm " (bản sao)".
    pub fn duplicate(&self) -> Session {
        Session {
            id: Uuid::new_v4(),
            name: if self.name.trim().is_empty() { String::new() } else { format!("{} (bản sao)", self.name) },
            group: self.group.clone(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            save_password: self.save_password,
            encrypted_password: self.encrypted_password.clone(),
            key_file_path: self.key_file_path.clone(),
            encrypted_passphrase: self.encrypted_passphrase.clone(),
            created_at: Utc::now(),
            last_connected_at: None,
            extra: serde_json::Map::new(),
        }
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errs = Vec::new();
        if self.host.trim().is_empty() {
            errs.push("host".into());
        }
        if self.port == 0 {
            errs.push("port".into());
        }
        if self.username.trim().is_empty() {
            errs.push("username".into());
        }
        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }
}

/// Envelope của `sessions.json`: `{ "version": 1, "sessions": [...] }` (camelCase như v1 ghi).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionFileEnvelope {
    #[serde(rename = "version")]
    pub version: i32,
    #[serde(rename = "sessions")]
    pub sessions: Vec<Session>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Tương đương `AppSettings` v1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    pub font_family: String,
    pub font_size: i32,
    pub language: String,
    pub theme: String,
    pub copy_on_select: bool,
    pub right_click_action: String,
    pub confirm_multiline_paste: bool,
    pub scrollback: i32,
    pub cursor_blink: bool,
    pub keep_alive_seconds: i32,
    pub show_hidden_files: bool,
    pub custom_editor_path: String,
    pub collapsed_groups: Vec<String>,
    pub last_export_folder: String,
    pub last_import_folder: String,
    pub max_parallel_connects: i32,
    pub window_width: f64,
    pub window_height: f64,
    pub left_column_width: f64,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            font_family: "JetBrains Mono".into(),
            font_size: 14,
            language: "en".into(),
            theme: "Dark".into(),
            copy_on_select: true,
            right_click_action: "Paste".into(),
            confirm_multiline_paste: true,
            scrollback: 10000,
            cursor_blink: true,
            keep_alive_seconds: 5,
            show_hidden_files: true,
            custom_editor_path: String::new(),
            collapsed_groups: Vec::new(),
            last_export_folder: String::new(),
            last_import_folder: String::new(),
            max_parallel_connects: 4,
            window_width: 1100.0,
            window_height: 700.0,
            left_column_width: 400.0,
            extra: serde_json::Map::new(),
        }
    }
}

/// Tương đương `KnownHost` v1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct KnownHost {
    /// `host:port`
    pub host_key: String,
    pub algorithm: String,
    pub fingerprint_sha256: String,
    #[serde(with = "datetime::required")]
    pub added_at: DateTime<Utc>,
}

impl Default for KnownHost {
    fn default() -> Self {
        KnownHost { host_key: String::new(), algorithm: String::new(), fingerprint_sha256: String::new(), added_at: Utc::now() }
    }
}

/// Trạng thái kết nối của một tab (tương đương `ConnectionStatus` v1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1_SESSIONS: &str = r#"{
  "version": 1,
  "sessions": [
    {
      "Id": "3f2504e0-4f89-11d3-9a0c-0305e82c3301",
      "Name": "Máy chủ tiếng Việt: Ắ Ằ Ợ",
      "Group": "Dev",
      "Host": "10.0.0.5",
      "Port": 2222,
      "Username": "ubuntu",
      "SavePassword": true,
      "EncryptedPassword": "AQAAANCMnd8BFdERjHoAwE/Cl+sBAAAA",
      "KeyFilePath": null,
      "EncryptedPassphrase": null,
      "CreatedAt": "2026-09-28T07:30:00.1234567Z",
      "LastConnectedAt": null
    }
  ]
}"#;

    #[test]
    fn reads_v1_sessions_json_and_writes_same_field_names() {
        let env: SessionFileEnvelope = serde_json::from_str(V1_SESSIONS).unwrap();
        assert_eq!(env.version, 1);
        assert_eq!(env.sessions.len(), 1);
        let s = &env.sessions[0];
        assert_eq!(s.name, "Máy chủ tiếng Việt: Ắ Ằ Ợ");
        assert_eq!(s.port, 2222);
        assert_eq!(s.id.to_string(), "3f2504e0-4f89-11d3-9a0c-0305e82c3301");
        assert!(s.last_connected_at.is_none());
        let out = serde_json::to_string_pretty(&env).unwrap();
        for key in ["\"Id\"", "\"Name\"", "\"Group\"", "\"Host\"", "\"Port\"", "\"Username\"", "\"SavePassword\"",
            "\"EncryptedPassword\"", "\"KeyFilePath\"", "\"EncryptedPassphrase\"", "\"CreatedAt\"", "\"LastConnectedAt\"",
            "\"version\"", "\"sessions\""] {
            assert!(out.contains(key), "missing {key} in {out}");
        }
        assert!(out.contains("2026-09-28T07:30:00.1234567Z"));
        assert!(out.contains("Ắ Ằ Ợ"), "must not escape unicode");
    }

    #[test]
    fn missing_fields_use_defaults() {
        let s: Session = serde_json::from_str(r#"{"Host":"h","Username":"u"}"#).unwrap();
        assert_eq!(s.port, 22);
        assert!(s.save_password);
        assert_eq!(s.display_name(), "u@h");
        assert_eq!(s.effective_group(), UNGROUPED_VI);
    }

    #[test]
    fn unknown_fields_are_preserved() {
        let s: Session = serde_json::from_str(r#"{"Host":"h","Username":"u","UpdatedAt":"2026-01-01T00:00:00Z"}"#).unwrap();
        let out = serde_json::to_string(&s).unwrap();
        assert!(out.contains("UpdatedAt"));
    }

    #[test]
    fn settings_defaults_match_v1() {
        let s = AppSettings::default();
        assert_eq!(s.font_size, 14);
        assert_eq!(s.keep_alive_seconds, 5);
        assert_eq!(s.left_column_width, 400.0);
        let parsed: AppSettings = serde_json::from_str(r#"{"Theme":"Light","Language":"vi"}"#).unwrap();
        assert_eq!(parsed.theme, "Light");
        assert_eq!(parsed.max_parallel_connects, 4);
    }
}
