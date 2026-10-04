//! Một phiên SSH (`russh`) dùng cho một tab: shell + exec + SFTP trên **cùng một kết nối TCP**
//! (tương đương `SshConnection.cs` + `SshConnectionFactory.cs` v1, nhưng không cần kết nối thứ hai cho SFTP).
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle, KeyboardInteractiveAuthResponse, Msg};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, ChannelWriteHalf, Disconnect};
use tokio::sync::{mpsc, Mutex, Notify};
use zeroize::Zeroizing;

use crate::error::{CoreError, CoreResult};
use super::auth;
use super::host_key::{HostKeyInfo, HostKeyVerifier};

#[derive(Clone)]
pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<Zeroizing<String>>,
    pub key_file_path: Option<String>,
    pub passphrase: Option<Zeroizing<String>>,
    pub keepalive_seconds: u32,
    pub connect_timeout: Duration,
}

#[derive(Debug)]
pub enum SshEvent {
    Output(Vec<u8>),
    Closed(Option<CoreError>),
}

/// Trạng thái "đường truyền": đóng chưa, vì sao.
pub struct LinkState {
    closed: AtomicBool,
    reason: std::sync::Mutex<Option<CoreError>>,
    pub notify: Notify,
}

impl LinkState {
    fn new() -> Arc<Self> {
        Arc::new(LinkState { closed: AtomicBool::new(false), reason: std::sync::Mutex::new(None), notify: Notify::new() })
    }
    pub fn mark_closed(&self, reason: Option<CoreError>) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            if let Ok(mut r) = self.reason.lock() {
                *r = reason;
            }
        }
        self.notify.notify_waiters();
        self.notify.notify_one();
    }
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    pub fn reason(&self) -> Option<CoreError> {
        self.reason.lock().ok().and_then(|r| r.clone())
    }
}

struct ClientHandler {
    host: String,
    port: u16,
    verifier: Arc<dyn HostKeyVerifier>,
    link: Arc<LinkState>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let (algorithm, fingerprint) = match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => {
                (key.algorithm().to_string(), key.fingerprint(HashAlg::Sha256).to_string())
            }
            PublicKeyOrCertificate::Certificate(cert) => {
                (cert.algorithm().to_string(), cert.public_key().fingerprint(HashAlg::Sha256).to_string())
            }
        };
        let fingerprint = fingerprint.strip_prefix("SHA256:").unwrap_or(&fingerprint).to_string();
        let info = HostKeyInfo { host: self.host.clone(), port: self.port, algorithm, fingerprint_sha256: fingerprint };
        Ok(self.verifier.verify(info).await)
    }

    async fn disconnected(&mut self, reason: client::DisconnectReason<Self::Error>) -> Result<(), Self::Error> {
        match reason {
            client::DisconnectReason::ReceivedDisconnect(_) => self.link.mark_closed(None),
            client::DisconnectReason::Error(e) => self.link.mark_closed(Some(map_err(&e))),
        }
        Ok(())
    }
}

pub fn map_err(e: &russh::Error) -> CoreError {
    use russh::Error as E;
    match e {
        E::UnknownKey => CoreError::HostKeyRejected,
        E::NotAuthenticated | E::NoAuthMethod => CoreError::Auth,
        E::IO(io) => match io.kind() {
            std::io::ErrorKind::TimedOut => CoreError::Timeout,
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::AddrNotAvailable => CoreError::Connect(io.to_string()),
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::UnexpectedEof => CoreError::ConnectionLost,
            _ => {
                let m = io.to_string().to_lowercase();
                if m.contains("resolve") || m.contains("name") || m.contains("lookup") || m.contains("refused") || m.contains("unreachable") {
                    CoreError::Connect(io.to_string())
                } else {
                    CoreError::Other(io.to_string())
                }
            }
        },
        E::Disconnect | E::HUP | E::SendError | E::Inconsistent => CoreError::ConnectionLost,
        E::KeepaliveTimeout | E::InactivityTimeout => CoreError::ConnectionLost,
        E::ConnectionTimeout => CoreError::Timeout,
        other => CoreError::Other(other.to_string()),
    }
}

pub struct SshSession {
    handle: Arc<Handle<ClientHandler>>,
    shell: Mutex<Option<ChannelWriteHalf<Msg>>>,
    pub link: Arc<LinkState>,
    pub host: String,
    pub port: u16,
}

impl SshSession {
    /// Kết nối + xác thực theo thứ tự v1: key → password → keyboard-interactive.
    pub async fn connect(params: ConnectParams, verifier: Arc<dyn HostKeyVerifier>) -> CoreResult<SshSession> {
        let ka = if params.keepalive_seconds == 0 { 5 } else { params.keepalive_seconds.clamp(2, 60) };
        let config = Arc::new(client::Config {
            keepalive_interval: Some(Duration::from_secs(ka as u64)),
            keepalive_max: 3,
            nodelay: true,
            inactivity_timeout: None,
            ..Default::default()
        });
        let link = LinkState::new();
        let handler = ClientHandler { host: params.host.clone(), port: params.port, verifier, link: link.clone() };

        // Nạp key trước khi kết nối (sai passphrase báo ngay, như v1).
        let key = match params.key_file_path.as_deref().filter(|p| !p.trim().is_empty()) {
            Some(path) => Some(Arc::new(auth::load_private_key(path, params.passphrase.as_deref().map(|s| s.as_str()))?)),
            None => None,
        };

        let addr = (params.host.trim().to_string(), params.port);
        let mut handle = tokio::time::timeout(params.connect_timeout, client::connect(config, addr, handler))
            .await
            .map_err(|_| CoreError::Timeout)?
            .map_err(|e| map_err(&e))?;

        let user = params.username.trim().to_string();
        let mut authed = false;

        if let Some(key) = key {
            let hash = handle.best_supported_rsa_hash().await.ok().flatten().flatten();
            let r = handle
                .authenticate_publickey(user.clone(), PrivateKeyWithHashAlg::new(key, hash))
                .await
                .map_err(|e| map_err(&e))?;
            authed = r.success();
        }

        if !authed {
            if let Some(pw) = params.password.as_ref().filter(|p| !p.is_empty()) {
                let r = handle.authenticate_password(user.clone(), pw.as_str()).await.map_err(|e| map_err(&e))?;
                authed = r.success();
                if !authed {
                    let mut resp = handle
                        .authenticate_keyboard_interactive_start(user.clone(), None)
                        .await
                        .map_err(|e| map_err(&e))?;
                    for _ in 0..10 {
                        match resp {
                            KeyboardInteractiveAuthResponse::Success => {
                                authed = true;
                                break;
                            }
                            KeyboardInteractiveAuthResponse::Failure { .. } => break,
                            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                                let answers: Vec<String> = prompts.iter().map(|_| pw.as_str().to_string()).collect();
                                resp = handle
                                    .authenticate_keyboard_interactive_respond(answers)
                                    .await
                                    .map_err(|e| map_err(&e))?;
                            }
                        }
                    }
                }
            }
        }

        if !authed {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            return Err(CoreError::Auth);
        }

        Ok(SshSession { handle: Arc::new(handle), shell: Mutex::new(None), link, host: params.host, port: params.port })
    }

    pub fn is_alive(&self) -> bool {
        !self.link.is_closed() && !self.handle.is_closed()
    }

    /// Mở kênh shell với pty `xterm-256color`; output đi qua `tx`.
    pub async fn open_shell(&self, cols: u32, rows: u32, tx: mpsc::Sender<SshEvent>) -> CoreResult<()> {
        let channel = self.handle.channel_open_session().await.map_err(|e| map_err(&e))?;
        channel
            .request_pty(true, "xterm-256color", cols.max(10), rows.max(5), 0, 0, &[])
            .await
            .map_err(|e| map_err(&e))?;
        channel.request_shell(true).await.map_err(|e| map_err(&e))?;
        let (mut read, write) = channel.split();
        *self.shell.lock().await = Some(write);
        let link = self.link.clone();
        tokio::spawn(async move {
            loop {
                if link.is_closed() {
                    let _ = tx.send(SshEvent::Closed(link.reason())).await;
                    break;
                }
                tokio::select! {
                    msg = read.wait() => match msg {
                        Some(ChannelMsg::Data { data }) => {
                            if tx.send(SshEvent::Output(data.to_vec())).await.is_err() { break; }
                        }
                        Some(ChannelMsg::ExtendedData { data, .. }) => {
                            if tx.send(SshEvent::Output(data.to_vec())).await.is_err() { break; }
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                            let _ = tx.send(SshEvent::Closed(link.reason())).await;
                            break;
                        }
                        _ => {}
                    },
                    _ = link.notify.notified() => {
                        let _ = tx.send(SshEvent::Closed(link.reason())).await;
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn send_input(&self, bytes: &[u8]) -> CoreResult<()> {
        let guard = self.shell.lock().await;
        let Some(w) = guard.as_ref() else { return Err(CoreError::ConnectionLost) };
        w.data(bytes).await.map_err(|e| {
            let err = map_err(&e);
            self.link.mark_closed(Some(err.clone()));
            err
        })
    }

    pub async fn resize(&self, cols: u32, rows: u32) -> CoreResult<()> {
        let guard = self.shell.lock().await;
        let Some(w) = guard.as_ref() else { return Ok(()) };
        w.window_change(cols.max(10), rows.max(5), 0, 0).await.map_err(|e| map_err(&e))
    }

    /// Chạy một lệnh trên kênh riêng, trả về stdout (timeout → `Timeout`).
    pub async fn exec(&self, command: &str, timeout: Duration) -> CoreResult<String> {
        let handle = self.handle.clone();
        let fut = async move {
            let mut ch = handle.channel_open_session().await.map_err(|e| map_err(&e))?;
            ch.exec(true, command).await.map_err(|e| map_err(&e))?;
            let mut out = Vec::new();
            loop {
                match ch.wait().await {
                    Some(ChannelMsg::Data { data }) => out.extend_from_slice(&data),
                    Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            Ok::<String, CoreError>(String::from_utf8_lossy(&out).to_string())
        };
        tokio::time::timeout(timeout, fut).await.map_err(|_| CoreError::Timeout)?
    }

    /// Mở kênh SFTP trên cùng phiên.
    pub async fn open_sftp(&self) -> CoreResult<russh_sftp::client::SftpSession> {
        let ch = self.handle.channel_open_session().await.map_err(|e| map_err(&e))?;
        ch.request_subsystem(true, "sftp").await.map_err(|e| map_err(&e))?;
        russh_sftp::client::SftpSession::new(ch.into_stream())
            .await
            .map_err(|e| CoreError::Other(format!("SFTP: {e}")))
    }

    pub async fn disconnect(&self) {
        self.link.mark_closed(None);
        if let Some(w) = self.shell.lock().await.take() {
            let _ = w.close().await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), self.handle.disconnect(Disconnect::ByApplication, "", "en")).await;
    }
}
