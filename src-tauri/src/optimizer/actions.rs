//! Concrete optimisation actions.

use serde::{Deserialize, Serialize};

use crate::parsers::private_dns::PrivateDnsMode;
use crate::parsers::{AppOpMode, PackageName, StandbyBucket};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OptimizationAction {
    SetStandbyBucket { package: PackageName, bucket: StandbyBucket },
    SetAppOp { package: PackageName, op: String, mode: AppOpMode },
    KillPackage { package: PackageName },
    DisablePackage { package: PackageName },
    EnablePackage { package: PackageName },
    RemoveDozeWhitelist { package: PackageName },
    AddDozeWhitelist { package: PackageName },
    SetPhantomProcessLimit { value: u32 },
    /// Sets the system-wide Private DNS (DNS-over-TLS).
    SetPrivateDns { mode: PrivateDnsMode, hostname: Option<String> },
    /// Clears the cache directory of a single app. Data is preserved.
    ClearAppCache { package: PackageName },
    /// Asks PackageManager to trim system caches until at least `target_free_bytes`
    /// of free space is available on /data.
    TrimSystemCaches { target_free_bytes: u64 },
    /// Triggers the background dexopt job. WARNING: 30-45min of high CPU load,
    /// significant thermal pressure. Must only be invoked with a confirmed user gesture.
    RunBgDexopt,
    /// Sets the minimum refresh rate for the display (`settings put system min_refresh_rate`).
    SetMinRefreshRate { rate: f32 },
    /// Sets the peak refresh rate (`settings put system peak_refresh_rate`).
    SetPeakRefreshRate { rate: f32 },
    /// Toggles Bluetooth Absolute Volume (`setprop persist.bluetooth.disableabsolutevolume`).
    /// `disabled=true` decouples Android volume from headset firmware - required for high-fidelity DACs.
    SetBluetoothAbsoluteVolume { disabled: bool },
    /// Forces mono audio output (`settings put system master_mono`).
    /// Accessibility feature; reversible.
    SetMasterMono { enabled: bool },
    /// Toggles spatial / head-tracked audio on supported headsets
    /// (`settings put secure spatial_audio_enabled`).
    SetSpatialAudio { enabled: bool },
    /// Pins the AVRCP profile version used over Bluetooth
    /// (`setprop persist.bluetooth.avrcpversion`). Accepted: avrcp13/14/15/16.
    /// Effective after re-pairing the headset.
    SetAvrcpVersion { version: String },
    /// Disables/enables the phantom-process tracker entirely (Android 12+).
    SetPhantomMonitor { enabled: bool },
    /// Captive portal pinging (`connectivitycheck.gstatic.com`).
    SetCaptivePortalMode { disabled: bool },
    /// Compiles a package AOT with a given mode.
    CompilePackage { package: PackageName, mode: String },
    /// Resets the AOT compilation cache for a package.
    ResetCompilation { package: PackageName },
    /// Sets animation scales (0.0 to 1.0).
    SetAnimationScales { scale: f32 },
    /// Injects strict Doze constants to `device_idle_constants` if enabled, or deletes them if false.
    SetAggressiveDoze { enabled: bool },
    /// Controls background scanning for Wi-Fi and Bluetooth.
    SetBackgroundScan { wifi: bool, ble: bool },
    /// Enables or disables global background data restriction (Data Saver).
    SetDataSaver { enabled: bool },
    /// Hibernates (freezes) an app natively on Android 12+.
    HibernatePackage { package: PackageName, hibernate: bool },
    /// Sets the game mode for a package (1=Standard, 2=Performance, 3=Battery).
    SetGameMode { package: PackageName, mode: u8 },
    /// Sets the maximum number of background processes. None = standard.
    SetBackgroundProcessLimit { limit: Option<u32> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOutcome {
    pub action: OptimizationAction,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub snapshot_id: String,
    pub outcomes: Vec<OptimizationOutcome>,
}

impl OptimizationAction {
    /// Returns the shell command to apply this action.
    pub fn to_shell(&self) -> String {
        match self {
            Self::SetStandbyBucket { package, bucket } => {
                format!("am set-standby-bucket {} {}", package, *bucket as u8)
            }
            Self::SetAppOp { package, op, mode } => {
                format!("cmd appops set {} {} {}", package, op, mode.as_cmd_value())
            }
            Self::KillPackage { package } => format!("am kill {}", package),
            Self::DisablePackage { package } => format!("pm disable-user --user 0 {}", package),
            Self::EnablePackage { package } => format!("pm enable {}", package),
            Self::RemoveDozeWhitelist { package } => format!("cmd deviceidle whitelist -{}", package),
            Self::AddDozeWhitelist { package } => format!("cmd deviceidle whitelist +{}", package),
            Self::SetPhantomProcessLimit { value } => format!(
                "device_config put activity_manager max_phantom_processes {}",
                value
            ),
            Self::SetPrivateDns { mode, hostname } => {
                let mode_str = mode.as_setting();
                match (mode, hostname) {
                    (PrivateDnsMode::Hostname, Some(host)) => format!(
                        "settings put global private_dns_mode hostname; \
                         settings put global private_dns_specifier {}",
                        host
                    ),
                    _ => format!(
                        "settings put global private_dns_mode {}; \
                         settings delete global private_dns_specifier",
                        mode_str
                    ),
                }
            }
            Self::ClearAppCache { package } => format!("pm clear --cache-only {}", package),
            Self::TrimSystemCaches { target_free_bytes } => {
                format!("pm trim-caches {}", target_free_bytes)
            }
            Self::RunBgDexopt => "cmd package bg-dexopt-job".to_string(),
            Self::SetMinRefreshRate { rate } => format!("settings put system min_refresh_rate {}", rate),
            Self::SetPeakRefreshRate { rate } => format!("settings put system peak_refresh_rate {}", rate),
            Self::SetBluetoothAbsoluteVolume { disabled } => format!(
                "setprop persist.bluetooth.disableabsolutevolume {}",
                if *disabled { "1" } else { "0" }
            ),
            Self::SetMasterMono { enabled } => format!(
                "settings put system master_mono {}",
                if *enabled { "1" } else { "0" }
            ),
            Self::SetSpatialAudio { enabled } => format!(
                "settings put secure spatial_audio_enabled {}",
                if *enabled { "1" } else { "0" }
            ),
            Self::SetAvrcpVersion { version } => format!(
                "setprop persist.bluetooth.avrcpversion {}", version
            ),
            Self::SetPhantomMonitor { enabled } => format!(
                "settings put global settings_enable_monitor_phantom_procs {}",
                if *enabled { "true" } else { "false" }
            ),
            Self::SetCaptivePortalMode { disabled } => format!(
                "settings put global captive_portal_mode {}",
                if *disabled { "0" } else { "1" }
            ),
            Self::CompilePackage { package, mode } => format!(
                "cmd package compile -m {} -f {}", mode, package
            ),
            Self::ResetCompilation { package } => format!(
                "cmd package compile --reset {}", package
            ),
            Self::SetAnimationScales { scale } => format!(
                "settings put global window_animation_scale {0}; \
                 settings put global transition_animation_scale {0}; \
                 settings put global animator_duration_scale {0}",
                scale
            ),
            Self::SetAggressiveDoze { enabled } => {
                if *enabled {
                    "settings put global device_idle_constants light_after_inactive_to=60000,light_idle_to=180000,light_idle_factor=2,light_max_idle_to=900000".to_string()
                } else {
                    "settings delete global device_idle_constants".to_string()
                }
            }
            Self::SetBackgroundScan { wifi, ble } => format!(
                "settings put global wifi_scan_always_enabled {0}; \
                 settings put global ble_scan_always_enabled {1}",
                if *wifi { "1" } else { "0" },
                if *ble { "1" } else { "0" }
            ),
            Self::SetDataSaver { enabled } => format!(
                "cmd netpolicy set restrict-background {}",
                if *enabled { "true" } else { "false" }
            ),
            Self::HibernatePackage { package, hibernate } => format!(
                "cmd app_hibernation set-state {} --user 0 {}",
                package,
                if *hibernate { "true" } else { "false" }
            ),
            Self::SetGameMode { package, mode } => format!(
                "cmd game mode --mode {} {}",
                mode, package
            ),
            Self::SetBackgroundProcessLimit { limit } => {
                if let Some(l) = limit {
                    format!("settings put global background_process_limit {}", l)
                } else {
                    "settings delete global background_process_limit".to_string()
                }
            }
        }
    }

    pub fn target_package(&self) -> Option<&PackageName> {
        match self {
            Self::SetStandbyBucket { package, .. }
            | Self::SetAppOp { package, .. }
            | Self::KillPackage { package }
            | Self::DisablePackage { package }
            | Self::EnablePackage { package }
            | Self::RemoveDozeWhitelist { package }
            | Self::AddDozeWhitelist { package }
            | Self::ClearAppCache { package }
            | Self::CompilePackage { package, .. }
            | Self::ResetCompilation { package }
            | Self::HibernatePackage { package, .. }
            | Self::SetGameMode { package, .. } => Some(package),
            Self::SetPhantomProcessLimit { .. }
            | Self::SetPrivateDns { .. }
            | Self::TrimSystemCaches { .. }
            | Self::RunBgDexopt
            | Self::SetMinRefreshRate { .. }
            | Self::SetPeakRefreshRate { .. }
            | Self::SetBluetoothAbsoluteVolume { .. }
            | Self::SetMasterMono { .. }
            | Self::SetSpatialAudio { .. }
            | Self::SetAvrcpVersion { .. }
            | Self::SetPhantomMonitor { .. }
            | Self::SetCaptivePortalMode { .. }
            | Self::SetAnimationScales { .. }
            | Self::SetAggressiveDoze { .. }
            | Self::SetBackgroundScan { .. }
            | Self::SetDataSaver { .. }
            | Self::SetBackgroundProcessLimit { .. } => None,
        }
    }

    /// How long to wait for the shell command before giving up.
    pub fn shell_timeout(&self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            Self::RunBgDexopt              => Duration::from_secs(90 * 60),
            Self::TrimSystemCaches { .. }  => Duration::from_secs(120),
            Self::ClearAppCache { .. }     => Duration::from_secs(30),
            Self::CompilePackage { .. }    => Duration::from_secs(300),
            Self::ResetCompilation { .. }  => Duration::from_secs(30),
            _ => Duration::from_secs(15),
        }
    }

    /// Performs strict input validation to prevent ADB shell command injection.
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(pkg) = self.target_package() {
            if !pkg.is_valid() {
                return Err(crate::error::Error::other(format!("Invalid package name format: {}", pkg)));
            }
        }
        match self {
            Self::SetPrivateDns { hostname: Some(host), .. } => {
                if host.is_empty() || host.len() > 255 || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
                    return Err(crate::error::Error::other(format!("Invalid DNS hostname: {}", host)));
                }
            }
            Self::CompilePackage { mode, .. } => {
                let valid_modes = ["speed", "speed-profile", "everything", "verify", "quicken", "extract", "space"];
                if !valid_modes.contains(&mode.as_str()) {
                    return Err(crate::error::Error::other(format!("Invalid compilation mode: {}", mode)));
                }
            }
            Self::SetAvrcpVersion { version } => {
                let valid_versions = ["1.3", "1.4", "1.5", "1.6"];
                if !valid_versions.contains(&version.as_str()) {
                    return Err(crate::error::Error::other(format!("Invalid AVRCP version: {}", version)));
                }
            }
            Self::SetAppOp { op, .. } => {
                if op.is_empty() || !op.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Err(crate::error::Error::other(format!("Invalid AppOp code: {}", op)));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
