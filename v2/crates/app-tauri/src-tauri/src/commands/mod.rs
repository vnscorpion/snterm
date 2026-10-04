pub mod app;
pub mod sessions;
pub mod settings;
pub mod terminal;
pub mod sftp;
pub mod export_import;
pub mod clipboard;
pub mod sync;

/// Lỗi trả về giao diện: chuỗi đã dịch theo ngôn ngữ hiện tại.
pub type CmdResult<T> = Result<T, String>;
