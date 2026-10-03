//! File Export `.snterm` — byte-compatible với v1 (`SessionExporter.cs` / `SessionImporter.cs`):
//! PBKDF2-SHA256 600.000 vòng, salt 16 byte, AES-256-GCM nonce 12 byte, tag 16 byte,
//! AAD của `secrets` = chuỗi id VM, AAD của `check` = "SNTERM-CHECK", plaintext "SNTERM-OK".
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::dpapi;
use crate::error::{CoreError, CoreResult};
use crate::paths::atomic_write;
use crate::store::datetime;
use crate::store::{Session, SessionStore};

pub const FORMAT: &str = "snterm-sessions";
pub const FORMAT_MOBA: &str = "mobaxterm-sessions";
pub const VERSION: i32 = 1;
pub const PBKDF2_ITERATIONS: u32 = 600_000;
pub const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024;
const CHECK_AAD: &[u8] = b"SNTERM-CHECK";
const CHECK_PLAIN: &[u8] = b"SNTERM-OK";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CipherBlock {
    #[serde(rename = "nonce", default)]
    pub nonce: String,
    #[serde(rename = "cipherText", default)]
    pub cipher_text: String,
    #[serde(rename = "tag", default)]
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Protection {
    #[serde(rename = "kdf", default = "default_kdf")]
    pub kdf: String,
    #[serde(rename = "iterations", default = "default_iterations")]
    pub iterations: u32,
    #[serde(rename = "salt", default)]
    pub salt: String,
    #[serde(rename = "check", default)]
    pub check: CipherBlock,
}

fn default_kdf() -> String { "PBKDF2-SHA256".into() }
fn default_iterations() -> u32 { PBKDF2_ITERATIONS }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExportSessionItem {
    #[serde(rename = "id")]
    pub id: Uuid,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "group")]
    pub group: String,
    #[serde(rename = "host")]
    pub host: String,
    #[serde(rename = "port")]
    pub port: i64,
    #[serde(rename = "username")]
    pub username: String,
    #[serde(rename = "keyFileName")]
    pub key_file_name: Option<String>,
    #[serde(rename = "secrets")]
    pub secrets: Option<CipherBlock>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for ExportSessionItem {
    fn default() -> Self {
        ExportSessionItem {
            id: Uuid::new_v4(),
            name: String::new(),
            group: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            key_file_name: None,
            secrets: None,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SecretPayload {
    #[serde(rename = "password")]
    pub password: Option<String>,
    #[serde(rename = "passphrase")]
    pub passphrase: Option<String>,
    /// Base64
    #[serde(rename = "keyFileContent")]
    pub key_file_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExportFile {
    #[serde(rename = "format")]
    pub format: String,
    #[serde(rename = "version")]
    pub version: i32,
    #[serde(rename = "exportedAt", with = "datetime::required")]
    pub exported_at: DateTime<Utc>,
    #[serde(rename = "appVersion")]
    pub app_version: String,
    #[serde(rename = "protection")]
    pub protection: Option<Protection>,
    #[serde(rename = "sessions")]
    pub sessions: Vec<ExportSessionItem>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for ExportFile {
    fn default() -> Self {
        ExportFile {
            format: FORMAT.into(),
            version: VERSION,
            exported_at: Utc::now(),
            app_version: "2.0.0".into(),
            protection: None,
            sessions: Vec::new(),
            extra: serde_json::Map::new(),
        }
    }
}

fn b64e(b: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(b) }
fn b64d(s: &str) -> CoreResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s.trim()).map_err(|e| CoreError::Crypto(e.to_string()))
}

pub fn derive_key(password: &str, salt: &[u8], iterations: u32) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, key.as_mut());
    key
}

pub fn encrypt_block(key: &[u8; 32], plain: &[u8], aad: &[u8]) -> CoreResult<CipherBlock> {
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let out = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad })
        .map_err(|_| CoreError::Crypto("encrypt failed".into()))?;
    // aes-gcm trả về cipher||tag; v1 lưu tách riêng.
    let (ct, tag) = out.split_at(out.len() - 16);
    Ok(CipherBlock { nonce: b64e(&nonce), cipher_text: b64e(ct), tag: b64e(tag) })
}

pub fn decrypt_block(key: &[u8; 32], block: &CipherBlock, aad: &[u8]) -> CoreResult<Zeroizing<Vec<u8>>> {
    let nonce = b64d(&block.nonce)?;
    let mut ct = b64d(&block.cipher_text)?;
    let tag = b64d(&block.tag)?;
    if nonce.len() != 12 || tag.len() != 16 {
        return Err(CoreError::Crypto("bad nonce/tag size".into()));
    }
    ct.extend_from_slice(&tag);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| CoreError::Crypto(e.to_string()))?;
    let plain = cipher
        .decrypt(Nonce::from_slice(&nonce), Payload { msg: &ct, aad })
        .map_err(|_| CoreError::Crypto("decrypt failed".into()))?;
    Ok(Zeroizing::new(plain))
}

/// Tạo khối `protection` + khóa dẫn xuất cho một lần export.
pub fn new_protection(password: &str) -> CoreResult<(Protection, Zeroizing<[u8; 32]>)> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(password, &salt, PBKDF2_ITERATIONS);
    let check = encrypt_block(&key, CHECK_PLAIN, CHECK_AAD)?;
    Ok((
        Protection { kdf: "PBKDF2-SHA256".into(), iterations: PBKDF2_ITERATIONS, salt: b64e(&salt), check },
        key,
    ))
}

/// Kiểm tra mật khẩu Export; trả về khóa nếu đúng.
pub fn verify_password(protection: &Protection, password: &str) -> Option<Zeroizing<[u8; 32]>> {
    let salt = b64d(&protection.salt).ok()?;
    let key = derive_key(password, &salt, protection.iterations);
    let plain = decrypt_block(&key, &protection.check, CHECK_AAD).ok()?;
    if plain.as_slice() == CHECK_PLAIN { Some(key) } else { None }
}

#[derive(Debug, Clone, Default)]
pub struct ExportReport {
    pub exported: usize,
    pub messages: Vec<String>,
}

/// Tạo `ExportFile` từ danh sách VM (giống `SessionExporter.Export`).
pub fn build_export(
    sessions: &[Session],
    export_password: Option<&str>,
    include_key_content: bool,
) -> CoreResult<(ExportFile, ExportReport)> {
    let mut file = ExportFile::default();
    let mut report = ExportReport::default();
    let mut key: Option<Zeroizing<[u8; 32]>> = None;
    if let Some(pw) = export_password.filter(|p| !p.is_empty()) {
        let (prot, k) = new_protection(pw)?;
        file.protection = Some(prot);
        key = Some(k);
    }
    for s in sessions {
        let key_file_name = s
            .key_file_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .and_then(|p| std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()));
        let mut item = ExportSessionItem {
            id: s.id,
            name: s.name.clone(),
            group: s.group.clone(),
            host: s.host.clone(),
            port: s.port as i64,
            username: s.username.clone(),
            key_file_name,
            secrets: None,
            extra: serde_json::Map::new(),
        };
        if let Some(k) = &key {
            let password = dpapi::decrypt(s.encrypted_password.as_deref());
            if s.encrypted_password.is_some() && password.is_none() {
                report.messages.push(format!("VM '{}': không giải mã được mật khẩu đã lưu (DPAPI).", s.display_name()));
            }
            let passphrase = dpapi::decrypt(s.encrypted_passphrase.as_deref());
            let mut key_content = None;
            if include_key_content {
                if let Some(p) = s.key_file_path.as_deref().filter(|p| !p.trim().is_empty()) {
                    match std::fs::read(p) {
                        Ok(bytes) => key_content = Some(b64e(&bytes)),
                        Err(_) => report.messages.push(format!("VM '{}': file key không tồn tại, bỏ qua nội dung key.", s.display_name())),
                    }
                }
            }
            let payload = SecretPayload { password, passphrase, key_file_content: key_content };
            let json = Zeroizing::new(serde_json::to_vec(&payload)?);
            item.secrets = Some(encrypt_block(k, &json, s.id.to_string().as_bytes())?);
        }
        file.sessions.push(item);
        report.exported += 1;
    }
    Ok((file, report))
}

pub fn write_export(file: &ExportFile, path: &std::path::Path) -> CoreResult<()> {
    let json = serde_json::to_string_pretty(file)?;
    atomic_write(path, json.as_bytes())?;
    Ok(())
}

/// Đọc và kiểm tra file (`.snterm` hoặc MobaXterm) — giống `ReadAndValidateFile` v1.
pub fn read_and_validate(path: &std::path::Path) -> CoreResult<ExportFile> {
    let meta = std::fs::metadata(path).map_err(|_| CoreError::NotFound(path.display().to_string()))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err(CoreError::InvalidData("Kích thước file vượt quá giới hạn 20MB.".into()));
    }
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes).to_string();
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content).to_string();
    parse_export_content(&content, path.extension().and_then(|e| e.to_str()))
}

pub fn parse_export_content(content: &str, extension: Option<&str>) -> CoreResult<ExportFile> {
    let is_moba_ext = extension.map(|e| e.eq_ignore_ascii_case("mxtsessions")).unwrap_or(false);
    if is_moba_ext || crate::import::mobaxterm::is_mobaxterm_file(content) {
        let sessions = crate::import::mobaxterm::parse(content);
        if sessions.is_empty() {
            return Err(CoreError::InvalidData("Không tìm thấy cấu hình VM (SSH/SFTP) nào trong file MobaXterm.".into()));
        }
        return Ok(ExportFile { format: FORMAT_MOBA.into(), version: 1, sessions, protection: None, ..Default::default() });
    }
    let file: ExportFile = serde_json::from_str(content)
        .map_err(|_| CoreError::InvalidData("File không đúng định dạng SN Term hoặc MobaXterm.".into()))?;
    if file.format != FORMAT {
        return Err(CoreError::InvalidData("File không đúng định dạng SN Term hoặc MobaXterm.".into()));
    }
    if file.version > VERSION {
        return Err(CoreError::NewerVersion);
    }
    Ok(file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    Skip,
    Overwrite,
    AddCopy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub total_in_file: usize,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub overwritten_count: usize,
    pub added_copy_count: usize,
    pub corrupt_secrets_count: usize,
    pub messages: Vec<String>,
}

/// Trạng thái xem trước cho từng VM trong file (dùng ở hộp thoại Import).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewItem {
    pub id: Uuid,
    pub name: String,
    pub subtitle: String,
    pub group: String,
    /// "new" | "duplicate" | "invalid"
    pub status: String,
    pub existing_name: Option<String>,
    pub has_secrets: bool,
    pub key_file_name: Option<String>,
}

fn is_valid_item(item: &ExportSessionItem) -> bool {
    !item.host.trim().is_empty() && item.port >= 1 && item.port <= 65535 && !item.username.trim().is_empty()
}

fn find_conflict<'a>(existing: &'a [Session], item: &ExportSessionItem) -> Option<&'a Session> {
    existing.iter().find(|s| {
        s.id == item.id
            || (s.host.eq_ignore_ascii_case(&item.host)
                && s.port as i64 == item.port
                && s.username.eq_ignore_ascii_case(&item.username)
                && s.name.eq_ignore_ascii_case(&item.name))
    })
}

pub fn preview(file: &ExportFile, existing: &[Session]) -> Vec<ImportPreviewItem> {
    file.sessions
        .iter()
        .map(|item| {
            let (status, existing_name) = if !is_valid_item(item) {
                ("invalid".to_string(), None)
            } else if let Some(e) = find_conflict(existing, item) {
                ("duplicate".to_string(), Some(e.display_name()))
            } else {
                ("new".to_string(), None)
            };
            ImportPreviewItem {
                id: item.id,
                name: if item.name.trim().is_empty() { format!("{}@{}", item.username, item.host) } else { item.name.clone() },
                subtitle: format!("{}@{}:{}", item.username, item.host, item.port),
                group: item.group.clone(),
                status,
                existing_name,
                has_secrets: item.secrets.is_some(),
                key_file_name: item.key_file_name.clone(),
            }
        })
        .collect()
}

/// Nhập (gộp) vào `sessions.json` — giống `SessionImporter.Import` v1.
pub fn import(
    store: &SessionStore,
    file: &ExportFile,
    export_password: Option<&str>,
    resolution: ConflictResolution,
) -> CoreResult<ImportResult> {
    let mut result = ImportResult { total_in_file: file.sessions.len(), ..Default::default() };
    let key: Option<Zeroizing<[u8; 32]>> = match &file.protection {
        Some(p) => {
            let pw = export_password.unwrap_or("");
            Some(verify_password(p, pw).ok_or(CoreError::WrongPassword)?)
        }
        None => None,
    };

    store.backup_before_import();
    let mut current = store.load().sessions;
    let keys_dir = store.paths().keys_dir();

    for item in &file.sessions {
        if !is_valid_item(item) {
            result.skipped_count += 1;
            result.messages.push(format!("Bỏ qua VM '{}' do thông tin không hợp lệ.", item.name));
            continue;
        }

        let mut plain_password: Option<String> = None;
        let mut plain_passphrase: Option<String> = None;
        let mut resolved_key_path: Option<String> = None;

        if let (Some(k), Some(secrets)) = (&key, &item.secrets) {
            match decrypt_block(k, secrets, item.id.to_string().as_bytes())
                .and_then(|plain| serde_json::from_slice::<SecretPayload>(&plain).map_err(CoreError::from))
            {
                Ok(payload) => {
                    plain_password = payload.password;
                    plain_passphrase = payload.passphrase;
                    if let (Some(content), Some(name)) = (&payload.key_file_content, &item.key_file_name) {
                        if !content.is_empty() && !name.is_empty() {
                            if let Ok(bytes) = b64d(content) {
                                let safe_name = std::path::Path::new(name)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| name.clone());
                                let target = keys_dir.join(format!("{}_{}", item.id.simple(), safe_name));
                                if std::fs::write(&target, bytes).is_ok() {
                                    restrict_key_file(&target);
                                    resolved_key_path = Some(target.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    result.corrupt_secrets_count += 1;
                    result.messages.push(format!(
                        "VM '{}': Không thể giải mã mật khẩu (dữ liệu bí mật bị hỏng hoặc đã bị sửa).",
                        item.name
                    ));
                }
            }
        }

        if resolved_key_path.is_none() {
            if let Some(n) = item.key_file_name.as_deref().filter(|n| !n.is_empty()) {
                resolved_key_path = Some(n.to_string());
            }
        }

        let conflict_idx = find_conflict(&current, item).map(|e| e.id);
        if let Some(cid) = conflict_idx {
            match resolution {
                ConflictResolution::Skip => {
                    result.skipped_count += 1;
                    continue;
                }
                ConflictResolution::Overwrite => {
                    if let Some(existing) = current.iter_mut().find(|s| s.id == cid) {
                        existing.name = item.name.clone();
                        existing.group = item.group.clone();
                        existing.host = item.host.trim().to_string();
                        existing.port = item.port as u16;
                        existing.username = item.username.trim().to_string();
                        if let Some(p) = &plain_password {
                            existing.save_password = true;
                            existing.encrypted_password = dpapi::encrypt(Some(p));
                        }
                        if let Some(p) = &plain_passphrase {
                            existing.encrypted_passphrase = dpapi::encrypt(Some(p));
                        }
                        if let Some(k) = &resolved_key_path {
                            existing.key_file_path = Some(k.clone());
                        }
                    }
                    result.overwritten_count += 1;
                    continue;
                }
                ConflictResolution::AddCopy => {
                    current.push(Session {
                        id: Uuid::new_v4(),
                        name: format!("{} (nhập)", item.name),
                        group: item.group.clone(),
                        host: item.host.trim().to_string(),
                        port: item.port as u16,
                        username: item.username.trim().to_string(),
                        save_password: plain_password.is_some(),
                        encrypted_password: dpapi::encrypt(plain_password.as_deref()),
                        encrypted_passphrase: dpapi::encrypt(plain_passphrase.as_deref()),
                        key_file_path: resolved_key_path.clone(),
                        created_at: Utc::now(),
                        last_connected_at: None,
                        extra: serde_json::Map::new(),
                    });
                    result.added_copy_count += 1;
                    result.imported_count += 1;
                    continue;
                }
            }
        }

        current.push(Session {
            id: item.id,
            name: item.name.clone(),
            group: item.group.clone(),
            host: item.host.trim().to_string(),
            port: item.port as u16,
            username: item.username.trim().to_string(),
            save_password: plain_password.is_some(),
            encrypted_password: dpapi::encrypt(plain_password.as_deref()),
            encrypted_passphrase: dpapi::encrypt(plain_passphrase.as_deref()),
            key_file_path: resolved_key_path,
            created_at: Utc::now(),
            last_connected_at: None,
            extra: serde_json::Map::new(),
        });
        result.imported_count += 1;
    }

    store.save(&current)?;
    Ok(result)
}

/// Hạn chế quyền file key: Windows dùng icacls (chỉ user hiện tại), Unix chmod 600.
fn restrict_key_file(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        if let Ok(user) = std::env::var("USERNAME") {
            let p = path.to_string_lossy().to_string();
            let _ = std::process::Command::new("icacls")
                .args([&p, "/inheritance:r", "/grant:r", &format!("{user}:(R,W)")])
                .output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    fn store() -> (tempfile::TempDir, SessionStore) {
        let d = tempfile::tempdir().unwrap();
        let p = AppPaths::new(d.path().into(), d.path().into());
        (d, SessionStore::new(p))
    }

    fn session(name: &str, host: &str, user: &str, pass: Option<&str>) -> Session {
        Session {
            name: name.into(),
            host: host.into(),
            username: user.into(),
            encrypted_password: dpapi::encrypt(pass),
            ..Default::default()
        }
    }

    #[test]
    fn unprotected_roundtrip_has_no_password_in_file() {
        let (_d, st) = store();
        let s = session("Máy chủ tiếng Việt: Ắ Ằ Ợ", "10.0.0.1", "ubuntu", Some("SuperSecretPassword123"));
        let (file, _) = build_export(&[s], None, false).unwrap();
        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(!json.contains("SuperSecretPassword123"));
        assert!(json.contains("Máy chủ tiếng Việt"));
        assert!(json.contains("\"protection\": null"));
        let parsed = parse_export_content(&json, Some("snterm")).unwrap();
        assert!(parsed.protection.is_none());
        let r = import(&st, &parsed, None, ConflictResolution::Overwrite).unwrap();
        assert_eq!(r.imported_count, 1);
        let loaded = st.load().sessions;
        assert_eq!(loaded[0].name, "Máy chủ tiếng Việt: Ắ Ằ Ợ");
        assert!(loaded[0].encrypted_password.is_none());
    }

    #[test]
    fn protected_roundtrip_with_key_and_wrong_password() {
        let (d, st) = store();
        let key_file = d.path().join("id_ed25519");
        std::fs::write(&key_file, "-----BEGIN OPENSSH PRIVATE KEY-----\ntest\n-----END OPENSSH PRIVATE KEY-----").unwrap();
        let mut s = session("Protected-VM", "10.0.0.5", "root", Some("VMPassword_456!"));
        s.key_file_path = Some(key_file.to_string_lossy().to_string());
        let (file, _) = build_export(&[s], Some("StrongPassword@123"), true).unwrap();
        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(!json.contains("VMPassword_456!"));
        let parsed = parse_export_content(&json, None).unwrap();
        assert!(parsed.protection.is_some());
        assert_eq!(parsed.sessions[0].key_file_name.as_deref(), Some("id_ed25519"));
        assert!(matches!(import(&st, &parsed, Some("WrongPassword"), ConflictResolution::Overwrite), Err(CoreError::WrongPassword)));
        let r = import(&st, &parsed, Some("StrongPassword@123"), ConflictResolution::Overwrite).unwrap();
        assert_eq!(r.imported_count, 1);
        let loaded = st.load().sessions;
        assert_eq!(dpapi::decrypt(loaded[0].encrypted_password.as_deref()).as_deref(), Some("VMPassword_456!"));
        let kp = loaded[0].key_file_path.clone().unwrap();
        assert!(std::path::Path::new(&kp).exists());
        assert!(kp.contains("_id_ed25519"));
    }

    #[test]
    fn corrupt_byte_skips_only_that_secret() {
        let (_d, st) = store();
        let (mut file, _) = build_export(
            &[session("VM1", "10.0.0.1", "user1", Some("p1")), session("VM2", "10.0.0.2", "user2", Some("p2"))],
            Some("Pass123456"),
            false,
        )
        .unwrap();
        let vm1 = file.sessions.iter_mut().find(|s| s.name == "VM1").unwrap();
        let mut ct = b64d(&vm1.secrets.as_ref().unwrap().cipher_text).unwrap();
        ct[0] ^= 0xFF;
        vm1.secrets.as_mut().unwrap().cipher_text = b64e(&ct);
        let r = import(&st, &file, Some("Pass123456"), ConflictResolution::Overwrite).unwrap();
        assert_eq!(r.corrupt_secrets_count, 1);
        assert_eq!(r.imported_count, 2);
        let loaded = st.load().sessions;
        let vm1 = loaded.iter().find(|s| s.name == "VM1").unwrap();
        let vm2 = loaded.iter().find(|s| s.name == "VM2").unwrap();
        assert!(vm1.encrypted_password.is_none());
        assert_eq!(dpapi::decrypt(vm2.encrypted_password.as_deref()).as_deref(), Some("p2"));
    }

    #[test]
    fn swapped_secrets_fail_due_to_aad() {
        let (_d, st) = store();
        let (mut file, _) = build_export(
            &[session("VM1", "10.0.0.1", "user1", Some("p1")), session("VM2", "10.0.0.2", "user2", Some("p2"))],
            Some("Pass123456"),
            false,
        )
        .unwrap();
        let s0 = file.sessions[0].secrets.clone();
        file.sessions[0].secrets = file.sessions[1].secrets.clone();
        file.sessions[1].secrets = s0;
        let r = import(&st, &file, Some("Pass123456"), ConflictResolution::Overwrite).unwrap();
        assert_eq!(r.corrupt_secrets_count, 2);
        assert!(st.load().sessions.iter().all(|s| s.encrypted_password.is_none()));
    }

    #[test]
    fn conflict_resolutions() {
        let (_d, st) = store();
        let existing = session("Original VM", "10.0.0.1", "user", None);
        st.save(&[existing.clone()]).unwrap();
        let file = ExportFile {
            sessions: vec![ExportSessionItem {
                id: existing.id,
                name: "Renamed VM".into(),
                host: "10.0.0.1".into(),
                port: 22,
                username: "user".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let r = import(&st, &file, None, ConflictResolution::Skip).unwrap();
        assert_eq!(r.skipped_count, 1);
        assert_eq!(st.load().sessions[0].name, "Original VM");
        let r = import(&st, &file, None, ConflictResolution::Overwrite).unwrap();
        assert_eq!(r.overwritten_count, 1);
        assert_eq!(st.load().sessions[0].name, "Renamed VM");
        let r = import(&st, &file, None, ConflictResolution::AddCopy).unwrap();
        assert_eq!(r.added_copy_count, 1);
        let loaded = st.load().sessions;
        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().any(|s| s.name == "Renamed VM (nhập)"));
        // Có bản sao lưu trước Import
        let backups = std::fs::read_dir(st.paths().backups_dir()).unwrap().count();
        assert!(backups >= 1);
    }

    #[test]
    fn invalid_format_or_version() {
        assert!(matches!(parse_export_content(r#"{"format":"unknown","version":1}"#, None), Err(CoreError::InvalidData(_))));
        assert!(matches!(parse_export_content(r#"{"format":"snterm-sessions","version":999}"#, None), Err(CoreError::NewerVersion)));
        assert!(matches!(parse_export_content("not json", None), Err(CoreError::InvalidData(_))));
    }

    /// Fixture tạo độc lập bằng Python `cryptography` theo đúng thuật toán v1 (xem tests/fixtures/make_fixture.py).
    #[test]
    fn decrypts_fixture_made_independently() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v1-protected.snterm");
        if !path.exists() {
            eprintln!("fixture missing, skipping");
            return;
        }
        let file = read_and_validate(&path).unwrap();
        let prot = file.protection.as_ref().unwrap();
        assert!(verify_password(prot, "wrong").is_none());
        let key = verify_password(prot, "StrongPassword@123").unwrap();
        let item = &file.sessions[0];
        let plain = decrypt_block(&key, item.secrets.as_ref().unwrap(), item.id.to_string().as_bytes()).unwrap();
        let payload: SecretPayload = serde_json::from_slice(&plain).unwrap();
        assert_eq!(payload.password.as_deref(), Some("VMPassword_456!"));
    }
}
