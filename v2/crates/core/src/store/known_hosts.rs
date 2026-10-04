//! `known_hosts.json` (tương đương `KnownHostsStore.cs` v1). Khóa `host:port`, so sánh không phân biệt hoa thường.
use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Utc;

use crate::paths::{atomic_write, AppPaths};
use super::model::KnownHost;
use super::session_store::strip_bom;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyStatus {
    Trusted,
    NewHost,
    Changed,
}

pub struct KnownHostsStore {
    paths: AppPaths,
    hosts: Mutex<HashMap<String, KnownHost>>,
}

fn norm_key(host: &str, port: u16) -> String {
    format!("{}:{}", host.trim(), port).to_lowercase()
}

/// Fingerprint SHA256 chuẩn hóa: bỏ tiền tố "SHA256:", bỏ '=' đệm, so sánh không phân biệt hoa thường.
pub fn normalize_fingerprint(fp: &str) -> String {
    let fp = fp.trim();
    let fp = fp.strip_prefix("SHA256:").unwrap_or(fp);
    fp.trim_end_matches('=').to_string()
}

impl KnownHostsStore {
    pub fn new(paths: AppPaths) -> Self {
        let s = KnownHostsStore { paths, hosts: Mutex::new(HashMap::new()) };
        s.load();
        s
    }

    pub fn load(&self) {
        let mut map = self.hosts.lock().unwrap_or_else(|e| e.into_inner());
        map.clear();
        let file = self.paths.known_hosts_file();
        if !file.exists() {
            return;
        }
        if let Ok(json) = std::fs::read_to_string(&file) {
            if let Ok(list) = serde_json::from_str::<Vec<KnownHost>>(strip_bom(&json)) {
                for h in list {
                    map.insert(h.host_key.to_lowercase(), h);
                }
            }
        }
    }

    pub fn save(&self) {
        let list: Vec<KnownHost> = {
            let map = self.hosts.lock().unwrap_or_else(|e| e.into_inner());
            let mut v: Vec<KnownHost> = map.values().cloned().collect();
            v.sort_by(|a, b| a.host_key.cmp(&b.host_key));
            v
        };
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = atomic_write(&self.paths.known_hosts_file(), json.as_bytes());
        }
    }

    pub fn check_host(&self, host: &str, port: u16, fingerprint_sha256: &str) -> HostKeyStatus {
        let map = self.hosts.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(&norm_key(host, port)) {
            Some(existing) => {
                if normalize_fingerprint(&existing.fingerprint_sha256)
                    .eq_ignore_ascii_case(&normalize_fingerprint(fingerprint_sha256))
                {
                    HostKeyStatus::Trusted
                } else {
                    HostKeyStatus::Changed
                }
            }
            None => HostKeyStatus::NewHost,
        }
    }

    pub fn add_or_update(&self, host: &str, port: u16, algorithm: &str, fingerprint_sha256: &str) {
        {
            let mut map = self.hosts.lock().unwrap_or_else(|e| e.into_inner());
            let key_display = format!("{}:{}", host.trim(), port);
            map.insert(
                norm_key(host, port),
                KnownHost {
                    host_key: key_display,
                    algorithm: algorithm.to_string(),
                    fingerprint_sha256: normalize_fingerprint(fingerprint_sha256),
                    added_at: Utc::now(),
                },
            );
        }
        self.save();
    }

    pub fn all(&self) -> Vec<KnownHost> {
        self.hosts.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trust_flow() {
        let d = tempfile::tempdir().unwrap();
        let p = AppPaths::new(d.path().into(), d.path().into());
        let kh = KnownHostsStore::new(p.clone());
        assert_eq!(kh.check_host("10.0.0.5", 22, "abc"), HostKeyStatus::NewHost);
        kh.add_or_update("10.0.0.5", 22, "ssh-ed25519", "abc=");
        assert_eq!(kh.check_host("10.0.0.5", 22, "ABC"), HostKeyStatus::Trusted);
        assert_eq!(kh.check_host("10.0.0.5", 22, "SHA256:abc"), HostKeyStatus::Trusted);
        assert_eq!(kh.check_host("10.0.0.5", 22, "xyz"), HostKeyStatus::Changed);
        // Đọc lại từ file (định dạng v1: mảng KnownHost PascalCase)
        let kh2 = KnownHostsStore::new(p.clone());
        assert_eq!(kh2.check_host("10.0.0.5", 22, "abc"), HostKeyStatus::Trusted);
        let raw = std::fs::read_to_string(p.known_hosts_file()).unwrap();
        assert!(raw.contains("\"HostKey\": \"10.0.0.5:22\""));
        assert!(raw.contains("\"FingerprintSha256\""));
    }
}
