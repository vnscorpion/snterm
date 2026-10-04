//! Backend trong bộ nhớ cho kiểm thử: mô phỏng cả "conflicted copy" và đếm số lần ghi.
use std::sync::Mutex;

use async_trait::async_trait;

use super::{Blob, BlobMeta, SyncBackend};
use crate::error::{CoreError, CoreResult};

#[derive(Default)]
pub struct MemoryBackend {
    pub main: Mutex<Option<Vec<u8>>>,
    pub conflicts: Mutex<Vec<(String, Vec<u8>)>>,
    pub writes: Mutex<u32>,
    pub fail_reads: Mutex<bool>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn content(&self) -> Option<Vec<u8>> {
        self.main.lock().unwrap().clone()
    }
    pub fn write_count(&self) -> u32 {
        *self.writes.lock().unwrap()
    }
}

#[async_trait]
impl SyncBackend for MemoryBackend {
    fn display_name(&self) -> String {
        "memory".into()
    }
    async fn read(&self) -> CoreResult<Option<Blob>> {
        if *self.fail_reads.lock().unwrap() {
            return Err(CoreError::Io("simulated offline".into()));
        }
        Ok(self.main.lock().unwrap().clone().map(|c| Blob {
            name: super::VAULT_NAME.into(),
            meta: BlobMeta { len: c.len() as u64, modified: None },
            content: c,
        }))
    }
    async fn read_meta(&self) -> CoreResult<Option<BlobMeta>> {
        Ok(self.main.lock().unwrap().as_ref().map(|c| BlobMeta { len: c.len() as u64, modified: None }))
    }
    async fn write(&self, content: &[u8]) -> CoreResult<()> {
        *self.main.lock().unwrap() = Some(content.to_vec());
        *self.writes.lock().unwrap() += 1;
        Ok(())
    }
    async fn read_conflict_copies(&self) -> CoreResult<Vec<Blob>> {
        Ok(self
            .conflicts
            .lock()
            .unwrap()
            .iter()
            .map(|(n, c)| Blob { name: n.clone(), meta: BlobMeta { len: c.len() as u64, modified: None }, content: c.clone() })
            .collect())
    }
    async fn delete_conflict_copies(&self, names: &[String]) -> CoreResult<()> {
        self.conflicts.lock().unwrap().retain(|(n, _)| !names.contains(n));
        Ok(())
    }
    async fn test(&self) -> CoreResult<()> {
        Ok(())
    }
    async fn delete_vault(&self) -> CoreResult<()> {
        *self.main.lock().unwrap() = None;
        Ok(())
    }
}
