//! Crate-wide error and result types.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("ADB executable not found in PATH or ANDROID_HOME")]
    AdbNotFound,

    #[error("no device with serial `{0}` is attached")]
    DeviceNotFound(String),

    #[error("device `{0}` is unauthorized; accept the RSA prompt on the phone")]
    DeviceUnauthorized(String),

    #[error("no devices attached")]
    NoDevices,

    #[error("ADB command failed (exit {exit_code}): {stderr}")]
    AdbCommand { exit_code: i32, stderr: String },

    #[error("ADB command timed out after {0:?}")]
    AdbTimeout(std::time::Duration),

    #[error("unsupported Android API level {0}; DozeForge requires API 31+")]
    UnsupportedApiLevel(u32),

    #[error("missing required ADB capability: {0}")]
    MissingCapability(String),

    #[error("could not parse `{parser_name}` output: {reason}")]
    Parse { parser_name: &'static str, reason: String },

    #[error("snapshot `{0}` not found")]
    SnapshotNotFound(String),

    #[error("snapshot was taken under SDK {snapshot_sdk} but device is now SDK {device_sdk}; rollback aborted to avoid bootloop risk")]
    SnapshotIncompatible {
        snapshot_sdk: u32,
        device_sdk: u32,
    },

    #[error("integrity check failed for exported script `{path}`")]
    IntegrityViolation { path: String },

    #[error("operation refused: package `{0}` is system-critical (uid < 10000)")]
    SystemPackageRefused(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("internal: {0}")]
    Other(String),
}

impl Error {
    pub fn other<S: Into<String>>(msg: S) -> Self {
        Self::Other(msg.into())
    }
}

/// Wire-friendly representation sent to the SvelteKit frontend.
#[derive(Debug, serde::Serialize)]
pub struct IpcError {
    pub kind: String,
    pub message: String,
}

impl From<Error> for IpcError {
    fn from(value: Error) -> Self {
        let kind = match &value {
            Error::AdbNotFound => "adb_not_found",
            Error::DeviceNotFound(_) => "device_not_found",
            Error::DeviceUnauthorized(_) => "device_unauthorized",
            Error::NoDevices => "no_devices",
            Error::AdbCommand { .. } => "adb_command_failed",
            Error::AdbTimeout(_) => "adb_timeout",
            Error::UnsupportedApiLevel(_) => "unsupported_api_level",
            Error::MissingCapability(_) => "missing_capability",
            Error::Parse { .. } => "parse_error",
            Error::SnapshotNotFound(_) => "snapshot_not_found",
            Error::SnapshotIncompatible { .. } => "snapshot_incompatible",
            Error::IntegrityViolation { .. } => "integrity_violation",
            Error::SystemPackageRefused(_) => "system_package_refused",
            Error::Io(_) => "io_error",
            Error::Json(_) => "json_error",
            Error::Regex(_) => "regex_error",
            Error::Tauri(_) => "tauri_error",
            Error::Other(_) => "internal_error",
        };
        Self {
            kind: kind.to_string(),
            message: value.to_string(),
        }
    }
}

/// Allow internal functions that return `crate::error::Result<_>` to use `?`
/// against validators that return `IpcError`. The conversion is lossy (we
/// fold everything into `Error::Other`) which is fine because the original
/// validation message is preserved verbatim.
impl From<IpcError> for Error {
    fn from(value: IpcError) -> Self {
        Error::Other(value.message)
    }
}
