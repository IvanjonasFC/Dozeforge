//! High-level ADB client.

use std::path::PathBuf;
use std::time::Duration;

use tracing::info;

use crate::error::{Error, Result};

use super::command::{AdbInvoker, DEFAULT_TIMEOUT};
use super::device::{BuildIdentity, Device, DeviceSerial, DeviceState};

pub struct AdbClient {
    pub invoker: AdbInvoker,
}

impl AdbClient {
    pub fn discover() -> Result<Self> {
        if let Ok(path) = which_adb() {
            info!(target: "dozeforge::adb", path = %path.display(), "adb resolved");
            return Ok(Self { invoker: AdbInvoker::new(path) });
        }

        if let Ok(home) = std::env::var("ANDROID_HOME") {
            let candidate = PathBuf::from(home).join("platform-tools").join(adb_binary_name());
            if candidate.exists() {
                info!(target: "dozeforge::adb", path = %candidate.display(), "adb resolved from ANDROID_HOME");
                return Ok(Self { invoker: AdbInvoker::new(candidate) });
            }
        }

        Err(Error::AdbNotFound)
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        let raw = self.invoker.exec(&["devices", "-l"], DEFAULT_TIMEOUT).await.unwrap_or_default();
        let mut devices = Vec::new();
        for line in raw.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('*') { continue; }
            let mut parts = line.split_whitespace();
            let Some(serial_raw) = parts.next() else { continue };
            let state_raw = parts.next().unwrap_or("offline");

            let state = match state_raw {
                "device" => DeviceState::Device,
                "unauthorized" => DeviceState::Unauthorized,
                "recovery" => DeviceState::Recovery,
                "sideload" => DeviceState::Sideload,
                "bootloader" | "fastboot" => DeviceState::Bootloader,
                _ => DeviceState::Offline,
            };

            let serial = DeviceSerial(serial_raw.to_string());

            let (manufacturer, model, product) = if state == DeviceState::Device {
                let m = self.getprop(&serial, "ro.product.manufacturer").await.ok();
                let mo = self.getprop(&serial, "ro.product.model").await.ok();
                let p = self.getprop(&serial, "ro.product.name").await.ok();
                (m, mo, p)
            } else {
                (None, None, None)
            };

            devices.push(Device { serial, state, manufacturer, model, product });
        }

        // Check fastboot devices
        if let Ok(Ok(fb_out)) = tokio::time::timeout(DEFAULT_TIMEOUT, tokio::process::Command::new("fastboot").arg("devices").output()).await {
            let fb_raw = String::from_utf8_lossy(&fb_out.stdout);
            for line in fb_raw.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }
                let mut parts = line.split_whitespace();
                let Some(serial_raw) = parts.next() else { continue };
                let state_raw = parts.next().unwrap_or("");
                if state_raw == "fastboot" || state_raw == "bootloader" {
                    if !devices.iter().any(|d| d.serial.0 == serial_raw) {
                        let serial = DeviceSerial(serial_raw.to_string());
                        devices.push(Device { serial, state: DeviceState::Bootloader, manufacturer: None, model: Some("Fastboot Device".to_string()), product: None });
                    }
                }
            }
        }

        Ok(devices)
    }

    pub async fn getprop(&self, serial: &DeviceSerial, key: &str) -> Result<String> {
        let cmd = format!("getprop {key}");
        Ok(self.invoker.shell(serial, &cmd, Duration::from_secs(5)).await?.trim().to_string())
    }

    pub async fn build_identity(&self, serial: &DeviceSerial) -> Result<BuildIdentity> {
        let sdk_str = self.getprop(serial, "ro.build.version.sdk").await?;
        let sdk_int: u32 = sdk_str.parse()
            .map_err(|e| Error::other(format!("invalid sdk int `{sdk_str}`: {e}")))?;

        if sdk_int < 31 {
            return Err(Error::UnsupportedApiLevel(sdk_int));
        }

        let patch = self.getprop(serial, "ro.build.version.security_patch").await?;
        let (year, month) = BuildIdentity::parse_patch_date(&patch);
        let fingerprint = self.getprop(serial, "ro.build.fingerprint").await?;

        Ok(BuildIdentity {
            sdk_int,
            security_patch_year: year,
            security_patch_month: month,
            fingerprint,
        })
    }
}

#[cfg(windows)]
fn adb_binary_name() -> &'static str { "adb.exe" }

#[cfg(not(windows))]
fn adb_binary_name() -> &'static str { "adb" }

fn which_adb() -> Result<PathBuf> {
    let name = adb_binary_name();
    let path_var = std::env::var_os("PATH").ok_or(Error::AdbNotFound)?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(Error::AdbNotFound)
}
