//! Kho đồng bộ = một file `.snterm` v1 hợp lệ (luôn có bảo vệ) + khối `sync` + `updatedAt`/`createdAt`/`keyHash`
//! trên từng VM. Bản v1 (hoặc Import của v2) vẫn đọc được như một bản sao lưu.
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::crypto::snterm_file::{self, CipherBlock, ExportFile, ExportSessionItem, SecretPayload};
use crate::error::{CoreError, CoreResult};
use crate::store::Tombstone;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncTombstone {
    pub id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncBlock {
    pub revision: i64,
    pub device_id: Option<Uuid>,
    pub device_name: String,
    pub saved_at: Option<DateTime<Utc>>,
    pub tombstones: Vec<SyncTombstone>,
}

/// Một VM ở dạng rõ (trong RAM) để gộp. Xóa khỏi bộ nhớ khi hủy.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SyncRecord {
    #[zeroize(skip)]
    pub id: Uuid,
    pub name: String,
    pub group: String,
    pub host: String,
    #[zeroize(skip)]
    pub port: u16,
    pub username: String,
    pub key_file_name: Option<String>,
    pub key_hash: Option<String>,
    pub key_content: Option<Vec<u8>>,
    pub password: Option<String>,
    pub passphrase: Option<String>,
    #[zeroize(skip)]
    pub created_at: DateTime<Utc>,
    #[zeroize(skip)]
    pub updated_at: DateTime<Utc>,
}

impl std::fmt::Debug for SyncRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncRecord").field("id", &self.id).field("name", &self.name).field("host", &self.host).field("updated_at", &self.updated_at).finish()
    }
}

impl SyncRecord {
    /// So sánh nội dung (không so sánh nội dung key, chỉ hash) để biết có cần ghi lại hay không.
    pub fn same_content(&self, o: &SyncRecord) -> bool {
        self.id == o.id
            && self.name == o.name
            && self.group == o.group
            && self.host == o.host
            && self.port == o.port
            && self.username == o.username
            && self.key_file_name == o.key_file_name
            && self.key_hash == o.key_hash
            && self.password == o.password
            && self.passphrase == o.passphrase
            && self.updated_at == o.updated_at
    }
    pub fn dedupe_key(&self) -> String {
        format!("{}|{}|{}|{}", self.host.trim().to_lowercase(), self.port, self.username.trim().to_lowercase(), self.name.trim().to_lowercase())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyncState {
    pub records: Vec<SyncRecord>,
    pub tombstones: Vec<Tombstone>,
}

pub fn key_hash(content: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(content))
}

pub struct VaultMeta {
    pub block: SyncBlock,
    pub exported_at: DateTime<Utc>,
    /// VM có `secrets` nhưng không giải mã được (file bị sửa).
    pub corrupt_secrets: Vec<String>,
}

/// Đóng gói trạng thái thành file `.snterm` có bảo vệ bằng mật khẩu đồng bộ.
pub fn build_vault(state: &SyncState, password: &str, block: &SyncBlock, include_key_files: bool, app_version: &str) -> CoreResult<ExportFile> {
    let (protection, key) = snterm_file::new_protection(password)?;
    let mut file = ExportFile { protection: Some(protection), app_version: app_version.to_string(), ..Default::default() };
    for r in &state.records {
        let payload = SecretPayload {
            password: r.password.clone(),
            passphrase: r.passphrase.clone(),
            key_file_content: if include_key_files { r.key_content.as_ref().map(|c| base64::engine::general_purpose::STANDARD.encode(c)) } else { None },
        };
        let json = Zeroizing::new(serde_json::to_vec(&payload)?);
        let secrets: CipherBlock = snterm_file::encrypt_block(&key, &json, r.id.to_string().as_bytes())?;
        let mut item = ExportSessionItem {
            id: r.id,
            name: r.name.clone(),
            group: r.group.clone(),
            host: r.host.clone(),
            port: r.port as i64,
            username: r.username.clone(),
            key_file_name: r.key_file_name.clone(),
            secrets: Some(secrets),
            extra: serde_json::Map::new(),
        };
        item.extra.insert("updatedAt".into(), serde_json::Value::String(r.updated_at.to_rfc3339()));
        item.extra.insert("createdAt".into(), serde_json::Value::String(r.created_at.to_rfc3339()));
        if let Some(h) = &r.key_hash {
            item.extra.insert("keyHash".into(), serde_json::Value::String(h.clone()));
        }
        file.sessions.push(item);
    }
    let mut blk = block.clone();
    blk.tombstones = state.tombstones.iter().map(|t| SyncTombstone { id: t.id, deleted_at: t.deleted_at }).collect();
    blk.saved_at = Some(Utc::now());
    file.extra.insert("sync".into(), serde_json::to_value(&blk)?);
    Ok(file)
}

pub fn serialize_vault(file: &ExportFile) -> CoreResult<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(file)?)
}

/// Đọc kho; `WrongPassword` nếu sai mật khẩu, `InvalidData` nếu hỏng.
pub fn parse_vault(bytes: &[u8], password: &str) -> CoreResult<(SyncState, VaultMeta)> {
    let text = String::from_utf8_lossy(bytes);
    let file = snterm_file::parse_export_content(&text, Some("snterm"))?;
    let prot = file.protection.as_ref().ok_or_else(|| CoreError::InvalidData("Kho đồng bộ không có bảo vệ.".into()))?;
    let key = snterm_file::verify_password(prot, password).ok_or(CoreError::WrongPassword)?;
    let block: SyncBlock = file.extra.get("sync").cloned().map(serde_json::from_value).transpose()?.unwrap_or_default();
    let mut state = SyncState::default();
    let mut corrupt = Vec::new();
    for item in &file.sessions {
        let parse_dt = |k: &str| {
            item.extra.get(k).and_then(|v| v.as_str()).and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
        };
        let created_at = parse_dt("createdAt").unwrap_or(file.exported_at);
        let updated_at = parse_dt("updatedAt").unwrap_or(created_at);
        let mut rec = SyncRecord {
            id: item.id,
            name: item.name.clone(),
            group: item.group.clone(),
            host: item.host.clone(),
            port: item.port.clamp(1, 65535) as u16,
            username: item.username.clone(),
            key_file_name: item.key_file_name.clone(),
            key_hash: item.extra.get("keyHash").and_then(|v| v.as_str()).map(String::from),
            key_content: None,
            password: None,
            passphrase: None,
            created_at,
            updated_at,
        };
        if let Some(secrets) = &item.secrets {
            match snterm_file::decrypt_block(&key, secrets, item.id.to_string().as_bytes())
                .and_then(|p| serde_json::from_slice::<SecretPayload>(&p).map_err(CoreError::from))
            {
                Ok(payload) => {
                    rec.password = payload.password;
                    rec.passphrase = payload.passphrase;
                    if let Some(b64) = payload.key_file_content {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                            if rec.key_hash.is_none() {
                                rec.key_hash = Some(key_hash(&bytes));
                            }
                            rec.key_content = Some(bytes);
                        }
                    }
                }
                Err(_) => corrupt.push(item.name.clone()),
            }
        }
        state.records.push(rec);
    }
    state.tombstones = block.tombstones.iter().map(|t| Tombstone { id: t.id, deleted_at: t.deleted_at }).collect();
    Ok((state, VaultMeta { block, exported_at: file.exported_at, corrupt_secrets: corrupt }))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn rec(name: &str, host: &str, pw: Option<&str>) -> SyncRecord {
        SyncRecord {
            id: Uuid::new_v4(),
            name: name.into(),
            group: "Dev".into(),
            host: host.into(),
            port: 22,
            username: "root".into(),
            key_file_name: None,
            key_hash: None,
            key_content: None,
            password: pw.map(String::from),
            passphrase: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn vault_roundtrip_and_is_valid_snterm_file() {
        let mut r = rec("web-01 tiếng Việt", "10.0.0.5", Some("Secret#1"));
        r.key_file_name = Some("id_ed25519".into());
        r.key_content = Some(b"KEYDATA".to_vec());
        r.key_hash = Some(key_hash(b"KEYDATA"));
        let state = SyncState { records: vec![r.clone()], tombstones: vec![Tombstone { id: Uuid::new_v4(), deleted_at: Utc::now() }] };
        let block = SyncBlock { revision: 7, device_id: Some(Uuid::new_v4()), device_name: "LAPTOP".into(), ..Default::default() };
        let file = build_vault(&state, "SyncPass123", &block, true, "2.0.0").unwrap();
        let bytes = serialize_vault(&file).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains("Secret#1"));
        assert!(!text.contains("KEYDATA"));
        assert!(text.contains("\"format\": \"snterm-sessions\""));
        assert!(text.contains("\"revision\": 7"));
        // Đọc lại bằng bộ Import thường (giống v1): vẫn là file hợp lệ
        let as_export = snterm_file::parse_export_content(&text, Some("snterm")).unwrap();
        assert_eq!(as_export.sessions.len(), 1);
        assert!(snterm_file::verify_password(as_export.protection.as_ref().unwrap(), "SyncPass123").is_some());

        assert!(matches!(parse_vault(&bytes, "wrong"), Err(CoreError::WrongPassword)));
        let (st, meta) = parse_vault(&bytes, "SyncPass123").unwrap();
        assert_eq!(meta.block.revision, 7);
        assert_eq!(meta.block.device_name, "LAPTOP");
        assert_eq!(st.records.len(), 1);
        assert_eq!(st.tombstones.len(), 1);
        let got = &st.records[0];
        assert_eq!(got.password.as_deref(), Some("Secret#1"));
        assert_eq!(got.key_content.as_deref(), Some(&b"KEYDATA"[..]));
        assert_eq!(got.key_hash, r.key_hash);
        assert_eq!(got.updated_at.timestamp(), r.updated_at.timestamp());
        assert!(meta.corrupt_secrets.is_empty());
        // Không kèm key
        let file2 = build_vault(&state, "SyncPass123", &block, false, "2.0.0").unwrap();
        let (st2, _) = parse_vault(&serialize_vault(&file2).unwrap(), "SyncPass123").unwrap();
        assert!(st2.records[0].key_content.is_none());
        assert_eq!(st2.records[0].key_hash, r.key_hash);
    }

    #[test]
    fn corrupt_vault_is_rejected() {
        assert!(matches!(parse_vault(b"{ not json", "x"), Err(CoreError::InvalidData(_))));
        let file = build_vault(&SyncState::default(), "pw", &SyncBlock::default(), true, "2").unwrap();
        let mut bytes = serialize_vault(&file).unwrap();
        // Hỏng 1 byte trong check block → sai mật khẩu
        let pos = String::from_utf8_lossy(&bytes).find("\"cipherText\": \"").unwrap() + 15;
        bytes[pos] = if bytes[pos] == b'A' { b'B' } else { b'A' };
        assert!(matches!(parse_vault(&bytes, "pw"), Err(CoreError::WrongPassword)));
    }
}
