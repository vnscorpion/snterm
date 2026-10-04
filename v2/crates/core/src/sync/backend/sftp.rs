//! Kho trên một VM qua SFTP (mặc định `~/.snterm/sync.vault`), quyền 700/600. Kết nối mỗi lần thao tác rồi ngắt.
use std::sync::Arc;

use async_trait::async_trait;

use super::{Blob, BlobMeta, SyncBackend};
use crate::error::{CoreError, CoreResult};
use crate::ssh::sftp::{join, parent_of, SftpClient};
use crate::ssh::{ConnectParams, HostKeyVerifier, SshSession};

pub struct SftpBackend {
    pub params: ConnectParams,
    pub verifier: Arc<dyn HostKeyVerifier>,
    pub remote_path: String,
    pub label: String,
}

struct Conn {
    ssh: SshSession,
    sftp: SftpClient,
    path: String,
}

impl SftpBackend {
    async fn connect(&self) -> CoreResult<Conn> {
        let ssh = SshSession::connect(self.params.clone(), self.verifier.clone()).await?;
        let sftp = match ssh.open_sftp().await {
            Ok(s) => SftpClient::new(s).await?,
            Err(e) => {
                ssh.disconnect().await;
                return Err(e);
            }
        };
        let mut path = self.remote_path.trim().to_string();
        if path.is_empty() {
            path = "~/.snterm/sync.vault".into();
        }
        if let Some(rest) = path.strip_prefix("~/") {
            path = join(&sftp.home, rest);
        } else if path == "~" {
            path = join(&sftp.home, "sync.vault");
        } else if !path.starts_with('/') {
            path = join(&sftp.home, &path);
        }
        Ok(Conn { ssh, sftp, path })
    }
    async fn ensure_dir(c: &Conn) -> CoreResult<()> {
        let dir = parent_of(&c.path);
        if !c.sftp.exists(&dir).await {
            // Tạo từng cấp
            let mut cur = String::new();
            for part in dir.split('/').filter(|p| !p.is_empty()) {
                cur = format!("{cur}/{part}");
                if !c.sftp.exists(&cur).await {
                    c.sftp.create_dir(&cur).await?;
                    let _ = c.sftp.chmod(&cur, true, 0o700, false).await;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl SyncBackend for SftpBackend {
    fn display_name(&self) -> String {
        format!("{}:{}", self.label, self.remote_path)
    }
    async fn read(&self) -> CoreResult<Option<Blob>> {
        let c = self.connect().await?;
        let r = async {
            if !c.sftp.exists(&c.path).await {
                return Ok(None);
            }
            let content = c.sftp.read_file(&c.path).await?;
            Ok(Some(Blob { name: c.path.clone(), meta: BlobMeta { len: content.len() as u64, modified: None }, content }))
        }
        .await;
        c.sftp.close().await;
        c.ssh.disconnect().await;
        r
    }
    async fn read_meta(&self) -> CoreResult<Option<BlobMeta>> {
        // Đọc nội dung luôn (file nhỏ); đủ để so sánh kích thước.
        Ok(self.read().await?.map(|b| b.meta))
    }
    async fn write(&self, content: &[u8]) -> CoreResult<()> {
        let c = self.connect().await?;
        let r = async {
            Self::ensure_dir(&c).await?;
            let tmp = format!("{}.tmp", c.path);
            c.sftp.write_file(&tmp, content).await?;
            let _ = c.sftp.chmod(&tmp, false, 0o600, false).await;
            if c.sftp.exists(&c.path).await {
                // SFTP v3: rename không đè được file đích trên nhiều server.
                c.sftp.remove(&c.path, false).await?;
            }
            c.sftp.rename(&tmp, &c.path).await?;
            Ok(())
        }
        .await;
        c.sftp.close().await;
        c.ssh.disconnect().await;
        r
    }
    async fn test(&self) -> CoreResult<()> {
        let c = self.connect().await?;
        let r = Self::ensure_dir(&c).await;
        c.sftp.close().await;
        c.ssh.disconnect().await;
        r.map_err(|e| match e {
            CoreError::PermissionDenied => CoreError::PermissionDenied,
            other => other,
        })
    }
    async fn delete_vault(&self) -> CoreResult<()> {
        let c = self.connect().await?;
        let r = if c.sftp.exists(&c.path).await { c.sftp.remove(&c.path, false).await } else { Ok(()) };
        c.sftp.close().await;
        c.ssh.disconnect().await;
        r
    }
}
