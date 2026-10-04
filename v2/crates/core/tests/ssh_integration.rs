//! Kiểm thử tích hợp với sshd thật (tests/sshd, cổng 2222, user sntest / Test@1234).
//! Chạy: `tests/sshd` phải đang chạy (xem README v2). Tự bỏ qua nếu cổng 2222 không mở.
use std::sync::Arc;
use std::time::Duration;

use snterm_core::error::CoreError;
use snterm_core::ssh::host_key::{HostKeyInfo, HostKeyVerifier};
use snterm_core::ssh::monitor::{parse_sample, MonitorTracker, MONITOR_COMMAND};
use snterm_core::ssh::sftp::SftpClient;
use snterm_core::ssh::{ConnectParams, SshEvent, SshSession};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 2222;
const USER: &str = "sntest";
const PASS: &str = "Test@1234";

fn sshd_available() -> bool {
    std::net::TcpStream::connect_timeout(&format!("{HOST}:{PORT}").parse().unwrap(), Duration::from_millis(500)).is_ok()
}

struct Recorder {
    seen: std::sync::Mutex<Vec<HostKeyInfo>>,
    accept: bool,
}

#[async_trait::async_trait]
impl HostKeyVerifier for Recorder {
    async fn verify(&self, info: HostKeyInfo) -> bool {
        self.seen.lock().unwrap().push(info);
        self.accept
    }
}

fn params(password: Option<&str>, key: Option<&str>, passphrase: Option<&str>) -> ConnectParams {
    ConnectParams {
        host: HOST.into(),
        port: PORT,
        username: USER.into(),
        password: password.map(|p| Zeroizing::new(p.to_string())),
        key_file_path: key.map(|k| format!("{}/../../tests/sshd/{k}", env!("CARGO_MANIFEST_DIR"))),
        passphrase: passphrase.map(|p| Zeroizing::new(p.to_string())),
        keepalive_seconds: 5,
        connect_timeout: Duration::from_secs(10),
    }
}

async fn read_until(rx: &mut mpsc::Receiver<SshEvent>, needle: &str, timeout: Duration) -> String {
    let mut acc = String::new();
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(SshEvent::Output(b))) => {
                acc.push_str(&String::from_utf8_lossy(&b));
                if acc.contains(needle) {
                    return acc;
                }
            }
            Ok(Some(SshEvent::Closed(r))) => panic!("closed: {r:?}; got so far: {acc}"),
            _ => break,
        }
    }
    acc
}

#[tokio::test]
async fn password_shell_exec_and_sftp() {
    if !sshd_available() {
        eprintln!("sshd 2222 not running, skipping");
        return;
    }
    let rec = Arc::new(Recorder { seen: Default::default(), accept: true });
    let ssh = SshSession::connect(params(Some(PASS), None, None), rec.clone()).await.expect("connect");
    {
        let seen = rec.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].algorithm, "ssh-ed25519");
        assert!(!seen[0].fingerprint_sha256.contains("SHA256:"));
        assert!(!seen[0].fingerprint_sha256.ends_with('='));
        assert!(seen[0].fingerprint_sha256.len() >= 40);
    }

    // Shell
    let (tx, mut rx) = mpsc::channel(64);
    ssh.open_shell(100, 30, tx).await.expect("shell");
    ssh.send_input(b"echo XIN_CHAO_tieng_Viet_co_dau_\xe1\xba\xaf\xe1\xbb\xa3; stty size\n").await.unwrap();
    let out = read_until(&mut rx, "100", Duration::from_secs(5)).await;
    assert!(out.contains("XIN_CHAO_tieng_Viet_co_dau_ắợ"), "output: {out}");
    assert!(out.contains("30 100"), "stty size should reflect pty: {out}");
    ssh.resize(120, 40).await.unwrap();
    ssh.send_input(b"stty size\n").await.unwrap();
    let out = read_until(&mut rx, "40 120", Duration::from_secs(5)).await;
    assert!(out.contains("40 120"), "resize: {out}");

    // Exec + monitor (kênh riêng, không ảnh hưởng shell)
    let raw = ssh.exec(MONITOR_COMMAND, Duration::from_secs(5)).await.expect("exec");
    let sample = parse_sample(&raw).expect("MON line");
    assert!(!sample.hostname.is_empty());
    assert_eq!(sample.username, USER);
    assert!(sample.total_mb > 0);
    let info = MonitorTracker::default().update(&sample, "h", "u");
    assert!(!info.df_output.is_empty(), "df should be base64 decoded: {raw}");
    assert!(!info.os_group.is_empty());

    // SFTP trên cùng phiên
    let sftp = SftpClient::new(ssh.open_sftp().await.expect("sftp channel")).await.expect("sftp");
    assert!(sftp.home.starts_with("/home/"));
    let dir = format!("{}/snterm_test_{}", sftp.home, std::process::id());
    sftp.create_dir(&dir).await.unwrap();
    let local = std::env::temp_dir().join("snterm_up.txt");
    std::fs::write(&local, "xin chào SFTP\n".repeat(1000)).unwrap();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut progress_calls = 0;
    let remote = format!("{dir}/tệp tiếng việt.txt");
    sftp.upload_file(&local, &remote, &cancel, |_, _| progress_calls += 1).await.unwrap();
    assert!(progress_calls >= 1);
    let items = sftp.list_dir(&dir, true).await.unwrap();
    assert!(items[0].is_parent_directory);
    let f = items.iter().find(|i| i.name == "tệp tiếng việt.txt").expect("uploaded file listed");
    assert_eq!(f.size, std::fs::metadata(&local).unwrap().len());
    assert_eq!(f.owner, USER);
    assert!(f.permissions.starts_with("-rw"));
    sftp.chmod(&remote, false, 0o600, false).await.unwrap();
    let items = sftp.list_dir(&dir, true).await.unwrap();
    assert_eq!(items.iter().find(|i| i.name == "tệp tiếng việt.txt").unwrap().permissions, "-rw-------");
    let renamed = format!("{dir}/renamed.txt");
    sftp.rename(&remote, &renamed).await.unwrap();
    let dl = std::env::temp_dir().join("snterm_down.txt");
    sftp.download_file(&renamed, &dl, 0, &cancel, |_, _| {}).await.unwrap();
    assert_eq!(std::fs::read(&dl).unwrap(), std::fs::read(&local).unwrap());
    // Thư mục con + xóa đệ quy + walk
    sftp.create_dir(&format!("{dir}/sub")).await.unwrap();
    sftp.write_file(&format!("{dir}/sub/a.txt"), b"a").await.unwrap();
    let walked = sftp.walk(&dir).await.unwrap();
    assert!(walked.iter().any(|(r, d, _)| r == "sub" && *d));
    assert!(walked.iter().any(|(r, d, s)| r == "sub/a.txt" && !*d && *s == 1));
    sftp.remove(&dir, true).await.unwrap();
    assert!(!sftp.exists(&dir).await);
    // Quyền: thư mục root không ghi được
    assert!(matches!(sftp.create_dir("/snterm_denied").await, Err(CoreError::PermissionDenied)));
    sftp.close().await;

    ssh.disconnect().await;
    // Sau disconnect, shell báo Closed
    let mut closed = false;
    for _ in 0..20 {
        match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Some(SshEvent::Closed(_))) | Ok(None) => { closed = true; break; }
            _ => {}
        }
    }
    assert!(closed, "shell should report Closed after disconnect");
}

#[tokio::test]
async fn key_auth_ed25519_and_rsa_with_passphrase() {
    if !sshd_available() { return; }
    let rec = Arc::new(Recorder { seen: Default::default(), accept: true });
    let ssh = SshSession::connect(params(None, Some("client_ed25519"), None), rec.clone()).await.expect("ed25519 key auth");
    ssh.disconnect().await;
    let ssh = SshSession::connect(params(None, Some("client_rsa_pass"), Some("keypass123")), rec.clone()).await.expect("rsa key with passphrase");
    ssh.disconnect().await;
    // Sai passphrase → Passphrase (trước khi kết nối)
    assert!(matches!(SshSession::connect(params(None, Some("client_rsa_pass"), Some("wrong")), rec.clone()).await, Err(CoreError::Passphrase)));
    assert!(matches!(SshSession::connect(params(None, Some("client_rsa_pass"), None), rec.clone()).await, Err(CoreError::Passphrase)));
    // Key không tồn tại
    assert!(matches!(SshSession::connect(params(None, Some("nope_key"), None), rec.clone()).await, Err(CoreError::KeyFileNotFound(_))));
}

#[tokio::test]
async fn wrong_password_host_key_rejection_and_refused_port() {
    if !sshd_available() { return; }
    let rec = Arc::new(Recorder { seen: Default::default(), accept: true });
    assert!(matches!(SshSession::connect(params(Some("wrong"), None, None), rec.clone()).await, Err(CoreError::Auth)));
    let reject = Arc::new(Recorder { seen: Default::default(), accept: false });
    assert!(matches!(SshSession::connect(params(Some(PASS), None, None), reject).await, Err(CoreError::HostKeyRejected)));
    let mut p = params(Some(PASS), None, None);
    p.port = 2;
    assert!(matches!(SshSession::connect(p, rec.clone()).await, Err(CoreError::Connect(_))));
    let mut p = params(Some(PASS), None, None);
    p.host = "10.255.255.1".into();
    p.connect_timeout = Duration::from_secs(2);
    assert!(matches!(SshSession::connect(p, rec).await, Err(CoreError::Timeout) | Err(CoreError::Connect(_))));
}

#[tokio::test]
async fn sftp_sync_backend_roundtrip_with_permissions() {
    if !sshd_available() { return; }
    use snterm_core::sync::backend::sftp::SftpBackend;
    use snterm_core::sync::backend::SyncBackend;
    let rec = Arc::new(Recorder { seen: Default::default(), accept: true });
    let remote_path = format!("~/.snterm-test-{}/sync.vault", std::process::id());
    let b = SftpBackend { params: params(Some(PASS), None, None), verifier: rec.clone(), remote_path: remote_path.clone(), label: "sntest".into() };
    b.test().await.expect("test backend");
    assert!(b.read().await.unwrap().is_none());
    b.write(b"v1 content").await.unwrap();
    assert_eq!(b.read().await.unwrap().unwrap().content, b"v1 content");
    b.write(b"v2").await.unwrap();
    assert_eq!(b.read().await.unwrap().unwrap().content, b"v2");
    assert_eq!(b.read_meta().await.unwrap().unwrap().len, 2);
    // Quyền 700 thư mục, 600 file
    let ssh = SshSession::connect(params(Some(PASS), None, None), rec.clone()).await.unwrap();
    let sftp = SftpClient::new(ssh.open_sftp().await.unwrap()).await.unwrap();
    let dir = format!("{}/.snterm-test-{}", sftp.home, std::process::id());
    let items = sftp.list_dir(&dir, true).await.unwrap();
    let f = items.iter().find(|i| i.name == "sync.vault").unwrap();
    assert_eq!(f.permissions, "-rw-------");
    assert!(!items.iter().any(|i| i.name.ends_with(".tmp")));
    let parent = sftp.list_dir(&sftp.home.clone(), true).await.unwrap();
    let d = parent.iter().find(|i| i.name == format!(".snterm-test-{}", std::process::id())).unwrap();
    assert_eq!(d.permissions, "drwx------");
    b.delete_vault().await.unwrap();
    assert!(b.read().await.unwrap().is_none());
    sftp.remove(&dir, true).await.unwrap();
    sftp.close().await;
    ssh.disconnect().await;
    // Host key chưa tin cậy → lỗi rõ ràng, không treo
    let reject = Arc::new(Recorder { seen: Default::default(), accept: false });
    let b2 = SftpBackend { params: params(Some(PASS), None, None), verifier: reject, remote_path, label: "x".into() };
    assert!(matches!(b2.test().await, Err(CoreError::HostKeyRejected)));
}
