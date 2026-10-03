//! Kiểu dữ liệu trao đổi với giao diện (camelCase). Không bao giờ gửi mật khẩu đã mã hóa ra UI.
use serde::{Deserialize, Serialize};
use snterm_core::store::Session;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: Uuid,
    pub name: String,
    pub group: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub save_password: bool,
    pub has_password: bool,
    pub has_passphrase: bool,
    pub key_file_path: Option<String>,
    pub key_file_missing: bool,
    pub created_at: String,
    pub last_connected_at: Option<String>,
    pub display_name: String,
    pub subtitle: String,
    pub effective_group: String,
}

impl From<&Session> for SessionView {
    fn from(s: &Session) -> Self {
        SessionView {
            id: s.id,
            name: s.name.clone(),
            group: s.group.clone(),
            host: s.host.clone(),
            port: s.port,
            username: s.username.clone(),
            save_password: s.save_password,
            has_password: s.encrypted_password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            has_passphrase: s.encrypted_passphrase.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            key_file_path: s.key_file_path.clone(),
            key_file_missing: s.is_key_file_missing(),
            created_at: s.created_at.to_rfc3339(),
            last_connected_at: s.last_connected_at.map(|d| d.to_rfc3339()),
            display_name: s.display_name(),
            subtitle: s.subtitle(),
            effective_group: s.effective_group(),
        }
    }
}

/// Dữ liệu từ form Thêm/Sửa VM.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDraft {
    pub id: Option<Uuid>,
    pub name: String,
    pub group: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub save_password: bool,
    /// Mật khẩu mới (None = giữ nguyên mật khẩu cũ).
    pub password: Option<String>,
    /// Xóa mật khẩu đã lưu.
    #[serde(default)]
    pub clear_password: bool,
    pub key_file_path: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsLoad {
    pub sessions: Vec<SessionView>,
    pub recovered_from_corruption: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub local_dir: String,
    pub logs_dir: String,
    pub backups_dir: String,
    pub platform: String,
}
