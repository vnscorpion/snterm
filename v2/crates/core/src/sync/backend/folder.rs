//! Kho là một thư mục (OneDrive / Google Drive / Dropbox / ổ mạng / USB).
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use super::{Blob, BlobMeta, SyncBackend, VAULT_NAME};
use crate::error::{CoreError, CoreResult};
use crate::paths::atomic_write;

pub struct FolderBackend {
    pub dir: PathBuf,
}

impl FolderBackend {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FolderBackend { dir: dir.into() }
    }
    fn main_path(&self) -> PathBuf {
        self.dir.join(VAULT_NAME)
    }
    /// Đọc có thử lại (ổ đám mây có thể khóa file trong chốc lát).
    async fn read_retry(&self, p: &PathBuf) -> CoreResult<Vec<u8>> {
        let mut last = None;
        for _ in 0..5 {
            match tokio::fs::read(p).await {
                Ok(b) => return Ok(b),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(CoreError::NotFound(p.display().to_string())),
                Err(e) => {
                    last = Some(e);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
        Err(CoreError::Io(last.map(|e| e.to_string()).unwrap_or_default()))
    }
    fn meta_of(m: &std::fs::Metadata) -> BlobMeta {
        BlobMeta { len: m.len(), modified: m.modified().ok() }
    }
    fn is_conflict_copy(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.starts_with("snterm-sync") && lower.ends_with(".vault") && lower != VAULT_NAME
    }
}

#[async_trait]
impl SyncBackend for FolderBackend {
    fn display_name(&self) -> String {
        self.dir.display().to_string()
    }
    async fn read(&self) -> CoreResult<Option<Blob>> {
        let p = self.main_path();
        let Ok(m) = tokio::fs::metadata(&p).await else { return Ok(None) };
        let content = self.read_retry(&p).await?;
        Ok(Some(Blob { name: VAULT_NAME.into(), content, meta: Self::meta_of(&m) }))
    }
    async fn read_meta(&self) -> CoreResult<Option<BlobMeta>> {
        Ok(tokio::fs::metadata(self.main_path()).await.ok().map(|m| Self::meta_of(&m)))
    }
    async fn write(&self, content: &[u8]) -> CoreResult<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let p = self.main_path();
        let data = content.to_vec();
        tokio::task::spawn_blocking(move || atomic_write(&p, &data))
            .await
            .map_err(|e| CoreError::Other(e.to_string()))??;
        Ok(())
    }
    async fn read_conflict_copies(&self) -> CoreResult<Vec<Blob>> {
        let mut out = Vec::new();
        let Ok(mut rd) = tokio::fs::read_dir(&self.dir).await else { return Ok(out) };
        while let Ok(Some(e)) = rd.next_entry().await {
            let name = e.file_name().to_string_lossy().to_string();
            if !Self::is_conflict_copy(&name) {
                continue;
            }
            if let Ok(content) = self.read_retry(&e.path()).await {
                let meta = e.metadata().await.map(|m| Self::meta_of(&m)).unwrap_or(BlobMeta { len: content.len() as u64, modified: None });
                out.push(Blob { name, content, meta });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
    async fn delete_conflict_copies(&self, names: &[String]) -> CoreResult<()> {
        for n in names {
            if Self::is_conflict_copy(n) {
                let _ = tokio::fs::remove_file(self.dir.join(n)).await;
            }
        }
        Ok(())
    }
    async fn test(&self) -> CoreResult<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let probe = self.dir.join(".snterm-sync-test");
        tokio::fs::write(&probe, b"ok").await?;
        let _ = tokio::fs::remove_file(&probe).await;
        Ok(())
    }
    async fn delete_vault(&self) -> CoreResult<()> {
        let _ = tokio::fs::remove_file(self.main_path()).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn folder_roundtrip_and_conflict_copies() {
        let d = tempfile::tempdir().unwrap();
        let b = FolderBackend::new(d.path().join("sync"));
        assert!(b.read().await.unwrap().is_none());
        b.test().await.unwrap();
        b.write(b"v1").await.unwrap();
        assert_eq!(b.read().await.unwrap().unwrap().content, b"v1");
        assert_eq!(b.read_meta().await.unwrap().unwrap().len, 2);
        std::fs::write(d.path().join("sync/snterm-sync-LAPTOP.vault"), b"c1").unwrap();
        std::fs::write(d.path().join("sync/snterm-sync (conflicted copy).vault"), b"c2").unwrap();
        std::fs::write(d.path().join("sync/other.txt"), b"x").unwrap();
        let cc = b.read_conflict_copies().await.unwrap();
        assert_eq!(cc.len(), 2);
        b.delete_conflict_copies(&cc.iter().map(|c| c.name.clone()).collect::<Vec<_>>()).await.unwrap();
        assert!(b.read_conflict_copies().await.unwrap().is_empty());
        assert!(!d.path().join("sync/snterm-sync.vault.tmp").exists());
        b.delete_vault().await.unwrap();
        assert!(b.read().await.unwrap().is_none());
    }
}
