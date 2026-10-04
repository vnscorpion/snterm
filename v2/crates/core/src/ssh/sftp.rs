//! Thao tác SFTP cho một tab (tương đương `SftpViewModel.cs` v1), chạy trên `russh_sftp::client::SftpSession`.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, StatusCode};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{CoreError, CoreResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpItem {
    pub name: String,
    pub full_name: String,
    pub is_directory: bool,
    pub is_parent_directory: bool,
    pub is_symbolic_link: bool,
    pub size: u64,
    /// Unix epoch giây
    pub last_modified: i64,
    pub permissions: String,
    pub mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    pub owner: String,
    pub group: String,
}

pub struct SftpClient {
    session: SftpSession,
    user_map: HashMap<u32, String>,
    group_map: HashMap<u32, String>,
    pub home: String,
}

pub fn map_sftp_err(e: russh_sftp::client::error::Error, path: &str) -> CoreError {
    use russh_sftp::client::error::Error as E;
    match e {
        E::Status(s) => match s.status_code {
            StatusCode::PermissionDenied => CoreError::PermissionDenied,
            StatusCode::NoSuchFile => CoreError::NotFound(path.to_string()),
            _ => CoreError::Other(format!("{}: {}", s.status_code, s.error_message)),
        },
        E::Timeout => CoreError::Timeout,
        E::IO(m) => CoreError::Io(m),
        other => CoreError::Other(other.to_string()),
    }
}

pub fn parent_of(path: &str) -> String {
    let norm = path.replace('\\', "/");
    let norm = norm.trim();
    let norm = if norm.len() > 1 { norm.trim_end_matches('/') } else { norm };
    if norm.is_empty() || norm == "/" {
        return "/".into();
    }
    match norm.rfind('/') {
        Some(0) => "/".into(),
        Some(i) => norm[..i].to_string(),
        None => "/".into(),
    }
}

pub fn join(dir: &str, name: &str) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), name)
}

pub fn permissions_string(mode: u32, ft: FileType, is_symlink: bool, is_dir: bool) -> String {
    let t = if is_dir {
        'd'
    } else if is_symlink {
        'l'
    } else {
        match ft {
            FileType::Dir => 'd',
            FileType::Symlink => 'l',
            _ => match mode & 0o170000 {
                0o140000 => 's',
                0o010000 => 'p',
                _ => '-',
            },
        }
    };
    let bit = |b: u32, c: char| if mode & b != 0 { c } else { '-' };
    let mut s = String::with_capacity(10);
    s.push(t);
    s.push(bit(0o400, 'r'));
    s.push(bit(0o200, 'w'));
    s.push(if mode & 0o4000 != 0 { 's' } else { bit(0o100, 'x') });
    s.push(bit(0o040, 'r'));
    s.push(bit(0o020, 'w'));
    s.push(if mode & 0o2000 != 0 { 's' } else { bit(0o010, 'x') });
    s.push(bit(0o004, 'r'));
    s.push(bit(0o002, 'w'));
    s.push(if mode & 0o1000 != 0 { 't' } else { bit(0o001, 'x') });
    s
}

impl SftpClient {
    pub async fn new(session: SftpSession) -> CoreResult<Self> {
        let home = session.canonicalize(".").await.map_err(|e| map_sftp_err(e, "."))?;
        let mut c = SftpClient { session, user_map: HashMap::new(), group_map: HashMap::new(), home };
        c.user_map.insert(0, "root".into());
        c.group_map.insert(0, "root".into());
        c.load_user_group_maps().await;
        Ok(c)
    }

    async fn load_user_group_maps(&mut self) {
        if let Ok(bytes) = self.session.read("/etc/passwd").await {
            for line in String::from_utf8_lossy(&bytes).lines() {
                let p: Vec<&str> = line.split(':').collect();
                if p.len() >= 3 {
                    if let Ok(uid) = p[2].parse::<u32>() {
                        if !p[0].is_empty() {
                            self.user_map.insert(uid, p[0].to_string());
                        }
                    }
                }
            }
        }
        if let Ok(bytes) = self.session.read("/etc/group").await {
            for line in String::from_utf8_lossy(&bytes).lines() {
                let p: Vec<&str> = line.split(':').collect();
                if p.len() >= 3 {
                    if let Ok(gid) = p[2].parse::<u32>() {
                        if !p[0].is_empty() {
                            self.group_map.insert(gid, p[0].to_string());
                        }
                    }
                }
            }
        }
    }

    pub async fn is_directory(&self, path: &str) -> bool {
        self.session.metadata(path).await.map(|m| m.is_dir()).unwrap_or(false)
    }

    /// Liệt kê thư mục: thư mục trước, rồi file, theo tên không phân biệt hoa thường; dòng ".." đầu tiên.
    pub async fn list_dir(&self, path: &str, show_hidden: bool) -> CoreResult<Vec<SftpItem>> {
        let path = if path.trim().is_empty() { "/".to_string() } else { path.replace('\\', "/") };
        let rd = self.session.read_dir(path.clone()).await.map_err(|e| map_sftp_err(e, &path))?;
        let mut items = Vec::new();
        for entry in rd {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            let meta = entry.metadata();
            let ft = meta.file_type();
            let is_symlink = matches!(ft, FileType::Symlink);
            let mut is_dir = matches!(ft, FileType::Dir);
            let full = join(&path, &name);
            if is_symlink {
                if let Ok(target) = self.session.metadata(full.clone()).await {
                    if target.is_dir() {
                        is_dir = true;
                    }
                }
            }
            let mode = meta.permissions.unwrap_or(0);
            let uid = meta.uid.unwrap_or(0);
            let gid = meta.gid.unwrap_or(0);
            let owner = self
                .user_map
                .get(&uid)
                .cloned()
                .or_else(|| meta.user.clone())
                .unwrap_or_else(|| if uid == 0 { "root".into() } else { uid.to_string() });
            let group = self
                .group_map
                .get(&gid)
                .cloned()
                .or_else(|| meta.group.clone())
                .unwrap_or_else(|| if gid == 0 { "root".into() } else { gid.to_string() });
            items.push(SftpItem {
                name: name.clone(),
                full_name: full,
                is_directory: is_dir,
                is_parent_directory: false,
                is_symbolic_link: is_symlink,
                size: meta.size.unwrap_or(0),
                last_modified: meta.mtime.unwrap_or(0) as i64,
                permissions: permissions_string(mode, ft, is_symlink, is_dir),
                mode: mode & 0o7777,
                user_id: uid,
                group_id: gid,
                owner,
                group,
            });
        }
        items.sort_by(|a, b| b.is_directory.cmp(&a.is_directory).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        items.insert(
            0,
            SftpItem {
                name: "..".into(),
                full_name: parent_of(&path),
                is_directory: true,
                is_parent_directory: true,
                is_symbolic_link: false,
                size: 0,
                last_modified: 0,
                permissions: String::new(),
                mode: 0,
                user_id: 0,
                group_id: 0,
                owner: String::new(),
                group: String::new(),
            },
        );
        Ok(items)
    }

    pub async fn create_dir(&self, path: &str) -> CoreResult<()> {
        self.session.create_dir(path).await.map_err(|e| map_sftp_err(e, path))
    }

    pub async fn rename(&self, from: &str, to: &str) -> CoreResult<()> {
        self.session.rename(from, to).await.map_err(|e| map_sftp_err(e, from))
    }

    pub async fn remove(&self, path: &str, is_dir: bool) -> CoreResult<()> {
        if is_dir {
            self.remove_dir_recursive(path).await
        } else {
            self.session.remove_file(path).await.map_err(|e| map_sftp_err(e, path))
        }
    }

    async fn remove_dir_recursive(&self, path: &str) -> CoreResult<()> {
        let mut stack = vec![path.to_string()];
        let mut dirs_post = Vec::new();
        while let Some(dir) = stack.pop() {
            let rd = self.session.read_dir(dir.clone()).await.map_err(|e| map_sftp_err(e, &dir))?;
            for e in rd {
                let n = e.file_name();
                if n == "." || n == ".." {
                    continue;
                }
                let full = join(&dir, &n);
                if matches!(e.file_type(), FileType::Dir) {
                    stack.push(full);
                } else {
                    self.session.remove_file(full.clone()).await.map_err(|e| map_sftp_err(e, &full))?;
                }
            }
            dirs_post.push(dir);
        }
        for d in dirs_post.into_iter().rev() {
            self.session.remove_dir(d.clone()).await.map_err(|e| map_sftp_err(e, &d))?;
        }
        Ok(())
    }

    pub async fn chmod(&self, path: &str, is_dir: bool, mode: u32, recursive: bool) -> CoreResult<()> {
        self.set_mode(path, mode).await?;
        if recursive && is_dir {
            let mut stack = vec![path.to_string()];
            while let Some(dir) = stack.pop() {
                let Ok(rd) = self.session.read_dir(dir.clone()).await else { continue };
                for e in rd {
                    let n = e.file_name();
                    if n == "." || n == ".." {
                        continue;
                    }
                    let full = join(&dir, &n);
                    let _ = self.set_mode(&full, mode).await;
                    if matches!(e.file_type(), FileType::Dir) {
                        stack.push(full);
                    }
                }
            }
        }
        Ok(())
    }

    async fn set_mode(&self, path: &str, mode: u32) -> CoreResult<()> {
        let mut attrs = self.session.metadata(path).await.map_err(|e| map_sftp_err(e, path))?;
        let keep_type = attrs.permissions.unwrap_or(0) & 0o170000;
        attrs.permissions = Some(keep_type | (mode & 0o7777));
        // Chỉ gửi quyền, tránh đụng mtime/size.
        let mut only = russh_sftp::protocol::FileAttributes::empty();
        only.permissions = attrs.permissions;
        self.session.set_metadata(path, only).await.map_err(|e| map_sftp_err(e, path))
    }

    pub async fn read_file(&self, path: &str) -> CoreResult<Vec<u8>> {
        self.session.read(path).await.map_err(|e| map_sftp_err(e, path))
    }

    pub async fn write_file(&self, path: &str, data: &[u8]) -> CoreResult<()> {
        let mut f = self.session.create(path).await.map_err(|e| map_sftp_err(e, path))?;
        f.write_all(data).await.map_err(|e| CoreError::Io(e.to_string()))?;
        f.shutdown().await.map_err(|e| CoreError::Io(e.to_string()))?;
        Ok(())
    }

    pub async fn exists(&self, path: &str) -> bool {
        self.session.try_exists(path).await.unwrap_or(false)
    }

    /// Upload một file có tiến trình; hủy → xóa file dở.
    pub async fn upload_file(
        &self,
        local: &std::path::Path,
        remote: &str,
        cancel: &Arc<AtomicBool>,
        mut progress: impl FnMut(u64, u64),
    ) -> CoreResult<()> {
        let mut src = tokio::fs::File::open(local).await?;
        let total = src.metadata().await.map(|m| m.len()).unwrap_or(0);
        let mut dst = self.session.create(remote).await.map_err(|e| map_sftp_err(e, remote))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(dst);
                let _ = self.session.remove_file(remote).await;
                return Err(CoreError::Other("canceled".into()));
            }
            let n = src.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n]).await.map_err(|e| CoreError::Io(e.to_string()))?;
            done += n as u64;
            progress(done, total);
        }
        dst.shutdown().await.map_err(|e| CoreError::Io(e.to_string()))?;
        Ok(())
    }

    pub async fn download_file(
        &self,
        remote: &str,
        local: &std::path::Path,
        expected_size: u64,
        cancel: &Arc<AtomicBool>,
        mut progress: impl FnMut(u64, u64),
    ) -> CoreResult<()> {
        let mut src = self.session.open(remote).await.map_err(|e| map_sftp_err(e, remote))?;
        let mut dst = tokio::fs::File::create(local).await?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(dst);
                let _ = tokio::fs::remove_file(local).await;
                return Err(CoreError::Other("canceled".into()));
            }
            let n = src.read(&mut buf).await.map_err(|e| CoreError::Io(e.to_string()))?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n]).await?;
            done += n as u64;
            progress(done, expected_size);
        }
        dst.flush().await?;
        Ok(())
    }

    /// Liệt kê đệ quy (đường dẫn tương đối, kích thước) để tải cả thư mục.
    pub async fn walk(&self, dir: &str) -> CoreResult<Vec<(String, bool, u64)>> {
        let mut out = Vec::new();
        let mut stack = vec![(dir.to_string(), String::new())];
        while let Some((abs, rel)) = stack.pop() {
            let rd = self.session.read_dir(abs.clone()).await.map_err(|e| map_sftp_err(e, &abs))?;
            for e in rd {
                let n = e.file_name();
                if n == "." || n == ".." {
                    continue;
                }
                let r = if rel.is_empty() { n.clone() } else { format!("{rel}/{n}") };
                let a = join(&abs, &n);
                if matches!(e.file_type(), FileType::Dir) {
                    out.push((r.clone(), true, 0));
                    stack.push((a, r));
                } else {
                    out.push((r, false, e.metadata().size.unwrap_or(0)));
                }
            }
        }
        Ok(out)
    }

    pub async fn close(&self) {
        let _ = self.session.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_and_perms() {
        assert_eq!(parent_of("/home/ubuntu/project"), "/home/ubuntu");
        assert_eq!(parent_of("/home"), "/");
        assert_eq!(parent_of("/"), "/");
        assert_eq!(parent_of("/home/ubuntu/"), "/home");
        assert_eq!(permissions_string(0o100644, FileType::File, false, false), "-rw-r--r--");
        assert_eq!(permissions_string(0o040755, FileType::Dir, false, true), "drwxr-xr-x");
        assert_eq!(permissions_string(0o104755, FileType::File, false, false), "-rwsr-xr-x");
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("/x/", "a"), "/x/a");
    }
}
