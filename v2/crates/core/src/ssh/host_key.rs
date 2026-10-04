//! Xác minh host key: lõi chỉ hỏi "có tin không?"; app quyết định (so `known_hosts.json`, hỏi người dùng).
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    /// Base64 không đệm (giống OpenSSH/SSH.NET).
    pub fingerprint_sha256: String,
}

#[async_trait]
pub trait HostKeyVerifier: Send + Sync {
    async fn verify(&self, info: HostKeyInfo) -> bool;
}

/// Tin cậy mọi host (chỉ dùng trong test).
pub struct TrustAll;

#[async_trait]
impl HostKeyVerifier for TrustAll {
    async fn verify(&self, _info: HostKeyInfo) -> bool {
        true
    }
}
