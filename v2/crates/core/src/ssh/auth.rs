//! Đọc private key (OpenSSH / PKCS#8 / PEM / PuTTY .ppk) — tương đương `PrivateKeyFile` của SSH.NET.
use std::path::Path;

use russh::keys::PrivateKey;

use crate::crypto::ppk;
use crate::error::{CoreError, CoreResult};

pub fn load_private_key(path: &str, passphrase: Option<&str>) -> CoreResult<PrivateKey> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(CoreError::KeyFileNotFound(path.to_string()));
    }
    let content = std::fs::read_to_string(p).map_err(|_| CoreError::KeyFileInvalid(path.to_string()))?;
    let pass = passphrase.filter(|s| !s.is_empty());
    if ppk::is_ppk(&content) {
        return ppk::parse(&content, pass);
    }
    match russh::keys::decode_secret_key(&content, pass) {
        Ok(k) => Ok(k),
        Err(russh::keys::Error::KeyIsEncrypted) => Err(CoreError::Passphrase),
        Err(russh::keys::Error::SshKey(ssh_key::Error::Crypto))
        | Err(russh::keys::Error::SshKey(ssh_key::Error::Encrypted))
        | Err(russh::keys::Error::SshKey(ssh_key::Error::Decrypted)) => Err(CoreError::Passphrase),
        Err(e) => {
            // Key có passphrase nhưng nhập sai thường ra lỗi giải mã/padding.
            let msg = e.to_string().to_lowercase();
            if pass.is_some() && (msg.contains("pad") || msg.contains("crypto") || msg.contains("decrypt")) {
                Err(CoreError::Passphrase)
            } else {
                tracing::debug!("key decode error: {e}");
                Err(CoreError::KeyFileInvalid(path.to_string()))
            }
        }
    }
}
