//! Mã hóa mật khẩu cục bộ bằng Windows DPAPI (CurrentUser), **cùng entropy với v1**
//! (`SecretProtector.cs`: "SNTerm.DPAPI.Entropy.v1") để đọc được `sessions.json` của v1.
//! Trên hệ điều hành khác (chỉ để phát triển/kiểm thử) dùng mã hóa thay thế, KHÔNG an toàn.

const ENTROPY: &[u8] = b"SNTerm.DPAPI.Entropy.v1";

pub fn encrypt(plain: Option<&str>) -> Option<String> {
    let plain = plain?;
    if plain.is_empty() {
        return None;
    }
    protect(plain.as_bytes()).ok().map(|b| base64_encode(&b))
}

pub fn decrypt(cipher_b64: Option<&str>) -> Option<String> {
    let c = cipher_b64?;
    if c.is_empty() {
        return None;
    }
    let bytes = base64_decode(c).ok()?;
    let plain = unprotect(&bytes).ok()?;
    String::from_utf8(plain).ok()
}

fn base64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s.trim())
}

#[cfg(windows)]
mod imp {
    use super::ENTROPY;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
    };
    use windows::core::PCWSTR;

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        v
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(data);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(&input, PCWSTR::null(), Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
                .map_err(|e| e.to_string())?;
            Ok(take(out))
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(data);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
                .map_err(|e| e.to_string())?;
            Ok(take(out))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    //! Thay thế cho môi trường không phải Windows: XOR với khóa dẫn xuất từ entropy + tên người dùng.
    //! Chỉ để chạy test/dev. Không dùng cho dữ liệu thật.
    use super::ENTROPY;
    use sha2::{Digest, Sha256};

    const MAGIC: &[u8] = b"SNTERM-DEVDPAPI1";

    fn key() -> Vec<u8> {
        let user = std::env::var("USER").unwrap_or_default();
        let mut h = Sha256::new();
        h.update(ENTROPY);
        h.update(user.as_bytes());
        h.finalize().to_vec()
    }

    fn xor(data: &[u8]) -> Vec<u8> {
        let k = key();
        data.iter().enumerate().map(|(i, b)| b ^ k[i % k.len()]).collect()
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = MAGIC.to_vec();
        out.extend(xor(data));
        Ok(out)
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        if !data.starts_with(MAGIC) {
            return Err("not a dev-dpapi blob".into());
        }
        Ok(xor(&data[MAGIC.len()..]))
    }
}

use imp::{protect, unprotect};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let c = encrypt(Some("mật khẩu bí mật 123")).unwrap();
        assert_ne!(c, "mật khẩu bí mật 123");
        assert_eq!(decrypt(Some(&c)).as_deref(), Some("mật khẩu bí mật 123"));
        assert!(encrypt(None).is_none());
        assert!(encrypt(Some("")).is_none());
        assert!(decrypt(Some("not base64!!")).is_none());
        assert!(decrypt(Some("AAAA")).is_none());
    }
}
