//! Lỗi dùng chung cho core + bản dịch tiếng Việt/Anh (tương đương `ErrorTranslator.cs` v1).
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// Sai user/mật khẩu/key.
    Auth,
    /// Sai passphrase hoặc key cần passphrase.
    Passphrase,
    /// Không kết nối được (DNS, refused, unreachable).
    Connect(String),
    /// Hết thời gian chờ.
    Timeout,
    /// Mất kết nối giữa chừng.
    ConnectionLost,
    /// Không tìm thấy file key.
    KeyFileNotFound(String),
    /// Không đọc được file key.
    KeyFileInvalid(String),
    /// Host key bị người dùng từ chối.
    HostKeyRejected,
    /// Không có quyền (SFTP).
    PermissionDenied,
    /// Không tìm thấy file/thư mục (SFTP).
    NotFound(String),
    /// File .snterm sai định dạng.
    InvalidData(String),
    /// File do phiên bản mới hơn tạo.
    NewerVersion,
    /// Sai mật khẩu file Export.
    WrongPassword,
    /// Lỗi I/O.
    Io(String),
    /// Lỗi JSON.
    Json(String),
    /// Lỗi mã hóa.
    Crypto(String),
    /// Khác.
    Other(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.translate("en", None))
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => CoreError::PermissionDenied,
            std::io::ErrorKind::NotFound => CoreError::NotFound(e.to_string()),
            std::io::ErrorKind::TimedOut => CoreError::Timeout,
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe => CoreError::ConnectionLost,
            _ => CoreError::Io(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Json(e.to_string())
    }
}

impl CoreError {
    /// Thông báo cho người dùng. `lang` = "vi" | "en". Không kèm port để tránh lộ (giống v1).
    pub fn translate(&self, lang: &str, host: Option<&str>) -> String {
        let vi = lang.eq_ignore_ascii_case("vi");
        let host_label = match host {
            Some(h) if !h.trim().is_empty() => h.to_string(),
            _ => {
                if vi {
                    "máy chủ".to_string()
                } else {
                    "server".to_string()
                }
            }
        };
        match self {
            CoreError::Auth => {
                if vi {
                    "Sai thông tin đăng nhập (user, mật khẩu hoặc SSH key).".into()
                } else {
                    "Invalid credentials (username, password or SSH key).".into()
                }
            }
            CoreError::Passphrase => {
                if vi {
                    "Sai passphrase của key, hoặc key này bắt buộc phải có passphrase.".into()
                } else {
                    "Invalid key passphrase or passphrase is required.".into()
                }
            }
            CoreError::Connect(_) => {
                if vi {
                    format!("Không kết nối được tới {host_label}. Vui lòng kiểm tra địa chỉ IP/port và kết nối mạng.")
                } else {
                    format!("Cannot connect to {host_label}. Please check IP address/port and network.")
                }
            }
            CoreError::Timeout => {
                if vi {
                    format!("Hết thời gian chờ kết nối tới {host_label}. Máy chủ không phản hồi.")
                } else {
                    format!("Connection timeout to {host_label}. Server did not respond.")
                }
            }
            CoreError::ConnectionLost => {
                if vi {
                    format!("Mất kết nối tới {host_label}.")
                } else {
                    format!("Connection lost to {host_label}.")
                }
            }
            CoreError::KeyFileNotFound(p) => {
                if vi {
                    format!("Không tìm thấy file key: {p}")
                } else {
                    format!("Key file not found: {p}")
                }
            }
            CoreError::KeyFileInvalid(p) => {
                if vi {
                    format!("Không đọc được file key: {p}")
                } else {
                    format!("Cannot read key file: {p}")
                }
            }
            CoreError::HostKeyRejected => {
                if vi {
                    "Đã hủy kết nối (không tin cậy host key).".into()
                } else {
                    "Connection canceled (host key not trusted).".into()
                }
            }
            CoreError::PermissionDenied => {
                if vi {
                    "Không có quyền thực hiện thao tác này.".into()
                } else {
                    "Permission denied for this operation.".into()
                }
            }
            CoreError::NotFound(p) => {
                if vi {
                    format!("Không tìm thấy: {p}")
                } else {
                    format!("Not found: {p}")
                }
            }
            CoreError::InvalidData(m) => m.clone(),
            CoreError::NewerVersion => {
                if vi {
                    "File được tạo bởi phiên bản SN Term mới hơn, hãy cập nhật ứng dụng.".into()
                } else {
                    "File was created by a newer SN Term version, please update the app.".into()
                }
            }
            CoreError::WrongPassword => {
                if vi {
                    "Sai mật khẩu file.".into()
                } else {
                    "Wrong file password.".into()
                }
            }
            CoreError::Io(m) | CoreError::Json(m) | CoreError::Crypto(m) | CoreError::Other(m) => {
                if vi {
                    format!("Lỗi kết nối: {m}")
                } else {
                    format!("Connection error: {m}")
                }
            }
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
