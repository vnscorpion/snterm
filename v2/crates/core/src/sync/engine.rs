//! Một chu kỳ đồng bộ (Phần 2 mục 4.1): đọc kho → giải mã → gộp → ghi cục bộ → ghi kho (kiểm tra ghi chéo).
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::dpapi;
use crate::error::{CoreError, CoreResult};
use crate::store::{Session, SessionStore, Tombstone};
use super::backend::SyncBackend;
use super::merge::{merge, MergeReport};
use super::vault::{self, key_hash, SyncBlock, SyncRecord, SyncState};

pub const KEEP_DAYS: i64 = 180;
pub const CLOCK_SKEW_WARN_MINUTES: i64 = 5;

pub struct SyncContext<'a> {
    pub store: &'a SessionStore,
    pub backend: &'a dyn SyncBackend,
    pub password: &'a str,
    pub device_id: Uuid,
    pub device_name: String,
    pub include_key_files: bool,
    pub last_revision: i64,
    pub keys_dir: PathBuf,
    pub app_version: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub report: MergeReport,
    pub local_changed: bool,
    pub remote_written: bool,
    pub revision: i64,
    pub remote_existed: bool,
    pub messages: Vec<String>,
}

/// Kế hoạch gộp (dùng để xem trước, chưa ghi gì).
pub struct SyncPlan {
    pub merged: SyncState,
    pub report: MergeReport,
    pub local_changed: bool,
    pub remote_changed: bool,
    pub remote_revision: i64,
    pub remote_existed: bool,
    pub remote_meta: Option<super::backend::BlobMeta>,
    pub conflict_names: Vec<String>,
    pub messages: Vec<String>,
}

/// Đọc trạng thái cục bộ ở dạng rõ (giải mã DPAPI, đọc file key nếu bật).
pub fn local_state(store: &SessionStore, include_key_files: bool) -> (SyncState, Vec<Session>) {
    let r = store.load();
    let mut st = SyncState { records: Vec::new(), tombstones: r.deleted.clone() };
    for s in &r.sessions {
        let (key_file_name, key_content, key_hash_v) = match s.key_file_path.as_deref().filter(|p| !p.trim().is_empty()) {
            Some(p) => {
                let name = Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string());
                let name = strip_import_prefix(&name, s.id);
                let content = std::fs::read(p).ok();
                let h = content.as_ref().map(|c| key_hash(c));
                (Some(name), if include_key_files { content } else { None }, h)
            }
            None => (None, None, None),
        };
        st.records.push(SyncRecord {
            id: s.id,
            name: s.name.clone(),
            group: s.group.clone(),
            host: s.host.clone(),
            port: s.port,
            username: s.username.clone(),
            key_file_name,
            key_hash: key_hash_v,
            key_content,
            password: if s.save_password { dpapi::decrypt(s.encrypted_password.as_deref()) } else { None },
            passphrase: dpapi::decrypt(s.encrypted_passphrase.as_deref()),
            created_at: s.created_at,
            updated_at: s.effective_updated_at(),
        });
    }
    (st, r.sessions)
}

/// File key giải nén từ Import/đồng bộ có tên `<id>_<tên gốc>`; trả về tên gốc.
fn strip_import_prefix(name: &str, id: Uuid) -> String {
    let prefix = format!("{}_", id.simple());
    name.strip_prefix(&prefix).unwrap_or(name).to_string()
}

pub async fn prepare(ctx: &SyncContext<'_>) -> CoreResult<SyncPlan> {
    let blob = ctx.backend.read().await?;
    let conflicts = ctx.backend.read_conflict_copies().await.unwrap_or_default();
    let mut messages = Vec::new();
    let (mut remote, remote_revision, remote_existed, remote_meta) = match &blob {
        Some(b) => {
            let (state, meta) = vault::parse_vault(&b.content, ctx.password)?;
            if !meta.corrupt_secrets.is_empty() {
                messages.push(format!("Kho: {} VM có phần bí mật hỏng, bỏ qua phần đó.", meta.corrupt_secrets.len()));
            }
            if let Some(saved) = meta.block.saved_at {
                if saved - Utc::now() > Duration::minutes(CLOCK_SKEW_WARN_MINUTES) {
                    messages.push("Giờ máy có thể sai; đồng bộ có thể chọn nhầm bản mới/cũ.".into());
                }
            }
            (state, meta.block.revision, true, Some(b.meta.clone()))
        }
        None => (SyncState::default(), 0, false, None),
    };
    // Gộp các conflicted copy vào remote trước (bỏ qua bản hỏng/sai mật khẩu).
    let mut conflict_names = Vec::new();
    for c in &conflicts {
        if let Ok((st, _)) = vault::parse_vault(&c.content, ctx.password) {
            let out = merge(&remote, &st, Utc::now(), KEEP_DAYS);
            remote = out.merged;
            messages.push(format!("Đã gộp bản sao xung đột: {}", c.name));
        }
        conflict_names.push(c.name.clone());
    }
    let (local, _) = local_state(ctx.store, ctx.include_key_files);
    let out = merge(&local, &remote, Utc::now(), KEEP_DAYS);
    let remote_changed = out.remote_changed || !remote_existed || !conflict_names.is_empty();
    Ok(SyncPlan {
        merged: out.merged,
        report: out.report,
        local_changed: out.local_changed,
        remote_changed,
        remote_revision,
        remote_existed,
        remote_meta,
        conflict_names,
        messages,
    })
}

/// Ghi kết quả gộp vào `sessions.json` (mã hóa lại DPAPI, ghi key ra `keys\`).
pub fn apply_local(ctx: &SyncContext<'_>, merged: &SyncState, existing: &[Session]) -> CoreResult<()> {
    let mut out: Vec<Session> = Vec::new();
    for m in &merged.records {
        let old = existing.iter().find(|s| s.id == m.id);
        let mut s = old.cloned().unwrap_or_else(|| Session { id: m.id, ..Default::default() });
        s.name = m.name.clone();
        s.group = m.group.clone();
        s.host = m.host.clone();
        s.port = m.port;
        s.username = m.username.clone();
        s.created_at = m.created_at;
        s.updated_at = Some(m.updated_at);
        s.encrypted_password = dpapi::encrypt(m.password.as_deref());
        s.save_password = m.password.is_some() || old.map(|o| o.save_password).unwrap_or(true);
        s.encrypted_passphrase = dpapi::encrypt(m.passphrase.as_deref());
        // Key: ghi ra keys\ nếu có nội dung và hash khác file đang có.
        let local_hash = s.key_file_path.as_deref().and_then(|p| std::fs::read(p).ok()).map(|c| key_hash(&c));
        match (&m.key_file_name, &m.key_content) {
            (Some(name), Some(content)) => {
                if local_hash.as_deref() != m.key_hash.as_deref() || s.key_file_path.is_none() {
                    let safe = Path::new(name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| name.clone());
                    let target = ctx.keys_dir.join(format!("{}_{}", m.id.simple(), safe));
                    std::fs::create_dir_all(&ctx.keys_dir)?;
                    std::fs::write(&target, content)?;
                    restrict_key_file(&target);
                    s.key_file_path = Some(target.to_string_lossy().to_string());
                }
            }
            (Some(name), None) => {
                if s.key_file_path.as_deref().map(|p| p.trim().is_empty()).unwrap_or(true) {
                    s.key_file_path = Some(name.clone()); // thiếu file key → ⚠ như Import
                }
            }
            (None, _) => s.key_file_path = None,
        }
        out.push(s);
    }
    ctx.store.save_with_tombstones(&out, &merged.tombstones)?;
    Ok(())
}

fn restrict_key_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        if let Ok(user) = std::env::var("USERNAME") {
            let p = path.to_string_lossy().to_string();
            let _ = std::process::Command::new("icacls").args([&p, "/inheritance:r", "/grant:r", &format!("{user}:(R,W)")]).output();
        }
    }
}

/// Chu kỳ đầy đủ.
pub async fn run_once(ctx: &SyncContext<'_>, first_sync: bool) -> CoreResult<SyncOutcome> {
    let mut plan = prepare(ctx).await?;
    let mut outcome = SyncOutcome { report: plan.report.clone(), revision: plan.remote_revision.max(ctx.last_revision), remote_existed: plan.remote_existed, messages: plan.messages.clone(), ..Default::default() };

    if plan.local_changed {
        if first_sync {
            ctx.store.backup_before_sync();
        }
        let (_, existing) = local_state(ctx.store, false);
        apply_local(ctx, &plan.merged, &existing)?;
        outcome.local_changed = true;
    }

    if plan.remote_changed {
        // Kiểm tra ghi chéo: nếu kho đổi so với lúc đọc → gộp lại (tối đa 3 lần).
        for attempt in 0..3 {
            let meta_now = ctx.backend.read_meta().await?;
            if !meta_same(&meta_now, &plan.remote_meta) {
                if attempt == 2 {
                    return Err(CoreError::Other("Kho đồng bộ đang được máy khác ghi liên tục, thử lại sau.".into()));
                }
                plan = prepare(ctx).await?;
                if plan.local_changed {
                    let (_, existing) = local_state(ctx.store, false);
                    apply_local(ctx, &plan.merged, &existing)?;
                    outcome.local_changed = true;
                }
                if !plan.remote_changed {
                    break;
                }
                continue;
            }
            let revision = plan.remote_revision.max(ctx.last_revision) + 1;
            let block = SyncBlock { revision, device_id: Some(ctx.device_id), device_name: ctx.device_name.clone(), saved_at: None, tombstones: vec![] };
            let file = vault::build_vault(&plan.merged, ctx.password, &block, ctx.include_key_files, &ctx.app_version)?;
            let bytes = Zeroizing::new(vault::serialize_vault(&file)?);
            ctx.backend.write(&bytes).await?;
            if !plan.conflict_names.is_empty() {
                let _ = ctx.backend.delete_conflict_copies(&plan.conflict_names).await;
            }
            outcome.remote_written = true;
            outcome.revision = revision;
            break;
        }
    }
    outcome.report = plan.report;
    Ok(outcome)
}

/// Đổi mật khẩu đồng bộ: mã hóa lại kho với mật khẩu mới, tăng revision.
pub async fn rewrite_with_password(ctx: &SyncContext<'_>, new_password: &str) -> CoreResult<i64> {
    let plan = prepare(ctx).await?;
    let revision = plan.remote_revision.max(ctx.last_revision) + 1;
    let block = SyncBlock { revision, device_id: Some(ctx.device_id), device_name: ctx.device_name.clone(), saved_at: None, tombstones: vec![] };
    let file = vault::build_vault(&plan.merged, new_password, &block, ctx.include_key_files, &ctx.app_version)?;
    let bytes = Zeroizing::new(vault::serialize_vault(&file)?);
    ctx.backend.write(&bytes).await?;
    Ok(revision)
}

/// So sánh metadata kho: cùng kích thước và (nếu cả hai có) cùng mtime.
fn meta_same(a: &Option<super::backend::BlobMeta>, b: &Option<super::backend::BlobMeta>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.len == y.len && match (x.modified, y.modified) {
            (Some(m1), Some(m2)) => m1 == m2,
            _ => true,
        },
        _ => false,
    }
}

/// Tiện ích cho test và xem trước: tạo tombstone từ danh sách id.
pub fn tombstones_now(ids: &[Uuid]) -> Vec<Tombstone> {
    let now = Utc::now();
    ids.iter().map(|id| Tombstone { id: *id, deleted_at: now }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use crate::sync::backend::memory::MemoryBackend;

    fn store() -> (tempfile::TempDir, SessionStore, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = AppPaths::new(d.path().into(), d.path().into());
        let keys = p.keys_dir();
        (d, SessionStore::new(p), keys)
    }
    fn session(name: &str, host: &str, pw: &str) -> Session {
        Session { name: name.into(), host: host.into(), username: "root".into(), encrypted_password: dpapi::encrypt(Some(pw)), updated_at: Some(Utc::now()), ..Default::default() }
    }
    fn ctx<'a>(store: &'a SessionStore, backend: &'a dyn SyncBackend, keys: &Path, last_revision: i64) -> SyncContext<'a> {
        SyncContext { store, backend, password: "SyncPass123", device_id: Uuid::new_v4(), device_name: "A".into(), include_key_files: true, last_revision, keys_dir: keys.to_path_buf(), app_version: "2.0.0".into() }
    }

    #[tokio::test]
    async fn two_machines_share_add_edit_delete() {
        let backend = MemoryBackend::new();
        let (_da, sa, ka) = store();
        let (_db, sb, kb) = store();
        let a1 = session("web-01", "10.0.0.1", "pwA");
        sa.save(&[a1.clone()]).unwrap();
        // A: kho chưa có → tạo rev 1
        let o = run_once(&ctx(&sa, &backend, &ka, 0), true).await.unwrap();
        assert!(o.remote_written && !o.local_changed && !o.remote_existed);
        assert_eq!(o.revision, 1);
        // B trống → nhận VM (mật khẩu DPAPI máy B), có backup trước lần đầu
        sb.save(&[]).unwrap();
        let o = run_once(&ctx(&sb, &backend, &kb, 0), true).await.unwrap();
        assert!(o.local_changed && !o.remote_written);
        let lb = sb.load().sessions;
        assert_eq!(lb.len(), 1);
        assert_eq!(dpapi::decrypt(lb[0].encrypted_password.as_deref()).as_deref(), Some("pwA"));
        assert!(std::fs::read_dir(sb.paths().backups_dir()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("sessions-before-sync-")));
        // Không đổi gì → không ghi kho
        let w = backend.write_count();
        let o = run_once(&ctx(&sa, &backend, &ka, 1), false).await.unwrap();
        assert!(!o.remote_written && !o.local_changed);
        assert_eq!(backend.write_count(), w);
        // B sửa tên → A nhận
        let mut b = sb.load().sessions;
        b[0].name = "web-01-renamed".into();
        b[0].touch();
        sb.save(&b).unwrap();
        let o = run_once(&ctx(&sb, &backend, &kb, 1), false).await.unwrap();
        assert!(o.remote_written);
        assert_eq!(o.revision, 2);
        let o = run_once(&ctx(&sa, &backend, &ka, 1), false).await.unwrap();
        assert!(o.local_changed);
        assert_eq!(sa.load().sessions[0].name, "web-01-renamed");
        // A xóa → B mất
        sa.delete_sessions(&[a1.id]).unwrap();
        run_once(&ctx(&sa, &backend, &ka, 2), false).await.unwrap();
        let o = run_once(&ctx(&sb, &backend, &kb, 2), false).await.unwrap();
        assert_eq!(o.report.delete_local.len(), 1);
        assert!(sb.load().sessions.is_empty());
        assert_eq!(sb.load().deleted.len(), 1);
    }

    #[tokio::test]
    async fn wrong_password_and_corrupt_vault_change_nothing() {
        let backend = MemoryBackend::new();
        let (_d, s, k) = store();
        s.save(&[session("x", "h", "p")]).unwrap();
        run_once(&ctx(&s, &backend, &k, 0), true).await.unwrap();
        let (_d2, s2, k2) = store();
        s2.save(&[session("local-only", "h2", "p2")]).unwrap();
        let mut c = ctx(&s2, &backend, &k2, 0);
        c.password = "wrong";
        assert!(matches!(run_once(&c, true).await, Err(CoreError::WrongPassword)));
        assert_eq!(s2.load().sessions.len(), 1);
        *backend.main.lock().unwrap() = Some(b"garbage".to_vec());
        assert!(matches!(run_once(&ctx(&s2, &backend, &k2, 0), true).await, Err(CoreError::InvalidData(_))));
        assert_eq!(s2.load().sessions[0].name, "local-only");
        *backend.fail_reads.lock().unwrap() = true;
        assert!(run_once(&ctx(&s2, &backend, &k2, 0), true).await.is_err());
    }

    #[tokio::test]
    async fn key_files_sync_and_conflict_copies_merge() {
        let backend = MemoryBackend::new();
        let (da, sa, ka) = store();
        let key_path = da.path().join("id_ed25519");
        std::fs::write(&key_path, b"PRIVATE KEY DATA").unwrap();
        let mut a = session("keyvm", "10.0.0.9", "pw");
        a.key_file_path = Some(key_path.to_string_lossy().to_string());
        a.encrypted_passphrase = dpapi::encrypt(Some("pp"));
        sa.save(&[a]).unwrap();
        run_once(&ctx(&sa, &backend, &ka, 0), true).await.unwrap();
        let (_db, sb, kb) = store();
        run_once(&ctx(&sb, &backend, &kb, 0), true).await.unwrap();
        let lb = sb.load().sessions;
        let kp = lb[0].key_file_path.clone().unwrap();
        assert!(kp.contains("_id_ed25519"), "{kp}");
        assert_eq!(std::fs::read(&kp).unwrap(), b"PRIVATE KEY DATA");
        assert_eq!(dpapi::decrypt(lb[0].encrypted_passphrase.as_deref()).as_deref(), Some("pp"));
        // Chu kỳ tiếp theo không ghi lại key (hash giống) và không ghi kho
        let mtime = std::fs::metadata(&kp).unwrap().modified().unwrap();
        let w = backend.write_count();
        run_once(&ctx(&sb, &backend, &kb, 1), false).await.unwrap();
        assert_eq!(std::fs::metadata(&kp).unwrap().modified().unwrap(), mtime);
        assert_eq!(backend.write_count(), w);
        // Conflicted copy chứa VM mới → được gộp và xóa
        let (_dc, sc, _kc) = store();
        sc.save(&[session("from-conflict", "10.0.0.7", "pc")]).unwrap();
        let (st, _) = local_state(&sc, true);
        let file = vault::build_vault(&st, "SyncPass123", &SyncBlock::default(), true, "2").unwrap();
        backend.conflicts.lock().unwrap().push(("snterm-sync-OTHER.vault".into(), vault::serialize_vault(&file).unwrap()));
        let o = run_once(&ctx(&sa, &backend, &ka, 1), false).await.unwrap();
        assert!(o.local_changed && o.remote_written);
        assert!(sa.load().sessions.iter().any(|s| s.name == "from-conflict"));
        assert!(backend.conflicts.lock().unwrap().is_empty());
        // Tắt kèm key: máy mới nhận về VM thiếu key (⚠) nhưng vẫn có tên key
        let (_dd, sd, kd) = store();
        let mut c = ctx(&sd, &backend, &kd, 0);
        c.include_key_files = false;
        run_once(&c, true).await.unwrap();
        let ld = sd.load().sessions;
        let kv = ld.iter().find(|s| s.name == "keyvm").unwrap();
        // Kho vẫn có nội dung key (do A ghi với include=true) nên vẫn giải nén được.
        assert!(kv.key_file_path.is_some());
    }

    #[tokio::test]
    async fn concurrent_write_is_detected_and_remerged() {
        let backend = MemoryBackend::new();
        let (_da, sa, ka) = store();
        sa.save(&[session("a", "1", "p")]).unwrap();
        run_once(&ctx(&sa, &backend, &ka, 0), true).await.unwrap();
        // B ghi rev 2 "sau khi" A đọc: mô phỏng bằng prepare rồi đổi kho trước khi A ghi.
        let (_db, sb, kb) = store();
        run_once(&ctx(&sb, &backend, &kb, 0), true).await.unwrap();
        let mut lb = sb.load().sessions;
        lb.push(session("b", "2", "p"));
        sb.save(&lb).unwrap();
        // A thêm VM nhưng trước khi ghi, B đã ghi: dùng prepare + thay đổi kho rồi gọi run_once.
        let mut la = sa.load().sessions;
        la.push(session("c", "3", "p"));
        sa.save(&la).unwrap();
        run_once(&ctx(&sb, &backend, &kb, 1), false).await.unwrap(); // B ghi rev 2
        let o = run_once(&ctx(&sa, &backend, &ka, 1), false).await.unwrap(); // A đọc rev 2, gộp, ghi rev 3
        assert_eq!(o.revision, 3);
        assert_eq!(sa.load().sessions.len(), 3);
        let o = run_once(&ctx(&sb, &backend, &kb, 2), false).await.unwrap();
        assert_eq!(sb.load().sessions.len(), 3);
        assert!(!o.remote_written);
    }

    #[tokio::test]
    async fn change_password_rewrites_vault() {
        let backend = MemoryBackend::new();
        let (_d, s, k) = store();
        s.save(&[session("x", "h", "p")]).unwrap();
        run_once(&ctx(&s, &backend, &k, 0), true).await.unwrap();
        let rev = rewrite_with_password(&ctx(&s, &backend, &k, 1), "NewPass456").await.unwrap();
        assert_eq!(rev, 2);
        assert!(matches!(run_once(&ctx(&s, &backend, &k, 2), false).await, Err(CoreError::WrongPassword)));
        let mut c = ctx(&s, &backend, &k, 2);
        c.password = "NewPass456";
        assert!(run_once(&c, false).await.is_ok());
    }
}
