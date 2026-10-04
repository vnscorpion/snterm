//! Kho đồng bộ: chỉ cần đọc / ghi nguyên tử / đọc metadata (xem Phần 2 mục 2).
pub mod folder;
pub mod sftp;
pub mod memory;

use async_trait::async_trait;
use std::time::SystemTime;

use crate::error::CoreResult;

#[derive(Debug, Clone, PartialEq)]
pub struct BlobMeta {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct Blob {
    pub name: String,
    pub content: Vec<u8>,
    pub meta: BlobMeta,
}

pub const VAULT_NAME: &str = "snterm-sync.vault";

#[async_trait]
pub trait SyncBackend: Send + Sync {
    fn display_name(&self) -> String;
    /// `None` nếu kho chưa có.
    async fn read(&self) -> CoreResult<Option<Blob>>;
    async fn read_meta(&self) -> CoreResult<Option<BlobMeta>>;
    /// Ghi nguyên tử (tmp rồi đổi tên).
    async fn write(&self, content: &[u8]) -> CoreResult<()>;
    /// Bản "conflicted copy" do ổ đám mây tạo (chỉ backend thư mục).
    async fn read_conflict_copies(&self) -> CoreResult<Vec<Blob>> {
        Ok(Vec::new())
    }
    async fn delete_conflict_copies(&self, _names: &[String]) -> CoreResult<()> {
        Ok(())
    }
    /// Kiểm tra truy cập được (tạo thư mục nếu cần).
    async fn test(&self) -> CoreResult<()>;
    /// Xóa kho (khi "Ngắt đồng bộ" và người dùng chọn xóa).
    async fn delete_vault(&self) -> CoreResult<()>;
}
