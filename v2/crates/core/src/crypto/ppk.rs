//! Key PuTTY `.ppk` (v2/v3) → `PrivateKey`, dùng bộ đọc PPK có sẵn của `ssh-key` (cùng phiên bản với russh).
use russh::keys::PrivateKey;
use ssh_key::Error as KeyError;

use crate::error::{CoreError, CoreResult};

pub fn is_ppk(content: &str) -> bool {
    content.trim_start().starts_with("PuTTY-User-Key-File-")
}

pub fn parse(content: &str, passphrase: Option<&str>) -> CoreResult<PrivateKey> {
    match PrivateKey::from_ppk(content, passphrase.map(|s| s.to_string())) {
        Ok(k) => Ok(k),
        Err(KeyError::Ppk(inner)) => {
            let m = inner.to_string().to_lowercase();
            if m.contains("mac") || m.contains("encrypt") || m.contains("passphrase") {
                Err(CoreError::Passphrase)
            } else {
                Err(CoreError::KeyFileInvalid(format!("PPK: {inner}")))
            }
        }
        Err(KeyError::Encrypted) | Err(KeyError::Crypto) => Err(CoreError::Passphrase),
        Err(e) => Err(CoreError::KeyFileInvalid(format!("PPK: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ppk() {
        assert!(is_ppk("PuTTY-User-Key-File-2: ssh-rsa\n"));
        assert!(!is_ppk("-----BEGIN OPENSSH PRIVATE KEY-----"));
    }

    /// Fixture do `puttygen` tạo (tests/fixtures/make_ppk.sh): v2/v3, có/không passphrase, rsa/ed25519/ecdsa.
    #[test]
    fn parses_puttygen_fixtures() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ppk");
        assert!(dir.exists(), "chạy tests/fixtures/make_ppk.sh để tạo fixture");
        let mut checked = 0;
        for (name, pass) in [
            ("ed25519-v3-plain.ppk", None),
            ("ed25519-v3-pass.ppk", Some("secret123")),
            ("rsa-v3-plain.ppk", None),
            ("rsa-v2-pass.ppk", Some("secret123")),
            ("ecdsa-v3-plain.ppk", None),
        ] {
            let p = dir.join(name);
            let content = std::fs::read_to_string(&p).unwrap();
            let key = parse(&content, pass).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let expected = std::fs::read_to_string(dir.join(format!("{}.pub", name.trim_end_matches(".ppk")))).unwrap();
            let expected = expected.split_whitespace().nth(1).unwrap().to_string();
            let got = key.public_key().to_openssh().unwrap();
            assert_eq!(got.split_whitespace().nth(1).unwrap(), expected, "{name}");
            if pass.is_some() {
                assert!(matches!(parse(&content, Some("wrong")), Err(CoreError::Passphrase)), "{name} wrong pass");
                assert!(matches!(parse(&content, None), Err(CoreError::Passphrase)), "{name} no pass");
            }
            checked += 1;
        }
        assert_eq!(checked, 5);
    }
}
