//! Reads current display + audio tuning settings from the device.
//!
//! All values are best-effort: vendors may strip some of these properties or
//! return "null". Missing fields stay `None` / `false`. Every setting probed
//! here works on stock AOSP and is preserved across major OEM skins (One UI,
//! MIUI, OxygenOS).
//!
//! ## Reads vs writes
//!
//! - **Display + master mono**: `settings get/put system`. Read = unprivileged.
//! - **Spatial audio**: `settings get/put secure`. Requires
//!   `WRITE_SECURE_SETTINGS` (granted via `adb shell pm grant ...` or via
//!   the `WRITE_SECURE_SETTINGS` AppOp). Most modern adb shells already have it.
//! - **BT Absolute Volume / AVRCP version**: `getprop` / `setprop` on
//!   `persist.*`. Survives reboot. Effective after re-pairing the headset.
//! - **Max FB buffers**: `getprop ro.*`. Read-only diagnostic.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplaySettings {
    // ─── Display ───
    /// `settings get system min_refresh_rate` — floor for adaptive refresh.
    pub min_refresh_rate: Option<f32>,
    /// `settings get system peak_refresh_rate` — ceiling for adaptive refresh.
    pub peak_refresh_rate: Option<f32>,
    /// `getprop ro.surface_flinger.max_frame_buffer_acquired_buffers` —
    /// read-only diagnostic, useful for Pixel devices.
    pub max_frame_buffer_buffers: Option<u32>,

    // ─── Audio ───
    /// `getprop persist.bluetooth.disableabsolutevolume` — "1" = disabled.
    pub bt_absolute_volume_disabled: bool,
    /// `settings get system master_mono` — "1" = forced mono output for
    /// every audio stream (useful for users with hearing loss in one ear
    /// or for single-earbud listening).
    pub master_mono: bool,
    /// `settings get secure spatial_audio_enabled` — "1" = head-tracked
    /// virtual surround on supported headsets (Pixel Buds Pro, AirPods over
    /// Bluetooth on Android 13+). On unsupported devices the key is missing.
    pub spatial_audio_enabled: Option<bool>,
    /// `getprop persist.bluetooth.avrcpversion` — codec-level remote-control
    /// profile. "avrcp16" enables higher fidelity metadata + better
    /// compatibility with some DACs. Empty/absent = vendor default.
    pub avrcp_version: Option<String>,
}

impl DisplaySettings {
    /// Parses 7 trimmed string outputs in order:
    ///   1. `settings get system min_refresh_rate`
    ///   2. `settings get system peak_refresh_rate`
    ///   3. `getprop persist.bluetooth.disableabsolutevolume`
    ///   4. `getprop ro.surface_flinger.max_frame_buffer_acquired_buffers`
    ///   5. `settings get system master_mono`
    ///   6. `settings get secure spatial_audio_enabled`
    ///   7. `getprop persist.bluetooth.avrcpversion`
    pub fn from_parts(
        min: &str,
        peak: &str,
        bt_disabled: &str,
        fb: &str,
        mono: &str,
        spatial: &str,
        avrcp: &str,
    ) -> Self {
        Self {
            min_refresh_rate: parse_f32_or_null(min),
            peak_refresh_rate: parse_f32_or_null(peak),
            bt_absolute_volume_disabled: bt_disabled.trim() == "1",
            max_frame_buffer_buffers: fb.trim().parse().ok(),
            master_mono: mono.trim() == "1",
            spatial_audio_enabled: parse_bool_or_null(spatial),
            avrcp_version: parse_opt_string(avrcp),
        }
    }
}

fn parse_f32_or_null(raw: &str) -> Option<f32> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") {
        None
    } else {
        t.parse::<f32>().ok()
    }
}

fn parse_bool_or_null(raw: &str) -> Option<bool> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") {
        None
    } else {
        Some(t == "1" || t.eq_ignore_ascii_case("true"))
    }
}

fn parse_opt_string(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pixel_8_pro_typical() {
        let s = DisplaySettings::from_parts(
            "60.0", "120.0", "0", "3",
            "0", "1", "avrcp16",
        );
        assert_eq!(s.min_refresh_rate, Some(60.0));
        assert_eq!(s.peak_refresh_rate, Some(120.0));
        assert!(!s.bt_absolute_volume_disabled);
        assert_eq!(s.max_frame_buffer_buffers, Some(3));
        assert!(!s.master_mono);
        assert_eq!(s.spatial_audio_enabled, Some(true));
        assert_eq!(s.avrcp_version.as_deref(), Some("avrcp16"));
    }

    #[test]
    fn handles_null_outputs() {
        let s = DisplaySettings::from_parts(
            "null", "", "", "null",
            "null", "null", "",
        );
        assert_eq!(s.min_refresh_rate, None);
        assert_eq!(s.peak_refresh_rate, None);
        assert!(!s.bt_absolute_volume_disabled);
        assert_eq!(s.max_frame_buffer_buffers, None);
        assert!(!s.master_mono);
        assert_eq!(s.spatial_audio_enabled, None);
        assert_eq!(s.avrcp_version, None);
    }

    #[test]
    fn detects_bt_abs_volume_disabled() {
        let s = DisplaySettings::from_parts("60.0", "120.0", "1", "3", "0", "0", "");
        assert!(s.bt_absolute_volume_disabled);
    }

    #[test]
    fn detects_master_mono_on() {
        let s = DisplaySettings::from_parts("60.0", "120.0", "0", "3", "1", "0", "");
        assert!(s.master_mono);
    }

    #[test]
    fn parses_spatial_audio_true_string() {
        let s = DisplaySettings::from_parts("60.0", "120.0", "0", "3", "0", "true", "");
        assert_eq!(s.spatial_audio_enabled, Some(true));
    }
}
