//! Reads performance and background optimization settings.
//!
//! Includes animation scales, background process limits, background scanning (Wi-Fi/BLE),
//! global data saver (restrict-background), and custom doze constants.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub window_animation_scale: Option<f32>,
    pub transition_animation_scale: Option<f32>,
    pub animator_duration_scale: Option<f32>,
    pub background_process_limit: Option<u32>,
    pub wifi_scan_always_enabled: Option<bool>,
    pub ble_scan_always_enabled: Option<bool>,
    pub restrict_background_data: bool,
    pub aggressive_doze_enabled: bool,
}

impl PerformanceSettings {
    /// Parses 8 shell outputs in order:
    ///   1. window_animation_scale
    ///   2. transition_animation_scale
    ///   3. animator_duration_scale
    ///   4. background_process_limit
    ///   5. wifi_scan_always_enabled
    ///   6. ble_scan_always_enabled
    ///   7. device_idle_constants
    ///   8. cmd netpolicy get restrict-background
    pub fn from_parts(
        win_anim: &str,
        trans_anim: &str,
        dur_anim: &str,
        bg_limit: &str,
        wifi_scan: &str,
        ble_scan: &str,
        doze_consts: &str,
        netpolicy: &str,
    ) -> Self {
        Self {
            window_animation_scale: parse_f32(win_anim),
            transition_animation_scale: parse_f32(trans_anim),
            animator_duration_scale: parse_f32(dur_anim),
            background_process_limit: parse_u32(bg_limit),
            wifi_scan_always_enabled: parse_bool(wifi_scan),
            ble_scan_always_enabled: parse_bool(ble_scan),
            aggressive_doze_enabled: doze_consts.contains("light_idle_factor=2") && doze_consts.contains("light_max_idle_to=900000"),
            restrict_background_data: netpolicy.to_lowercase().contains("enabled"),
        }
    }
}

fn parse_f32(raw: &str) -> Option<f32> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return None; }
    t.parse().ok()
}

fn parse_u32(raw: &str) -> Option<u32> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return None; }
    t.parse().ok()
}

fn parse_bool(raw: &str) -> Option<bool> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return None; }
    Some(t == "1" || t.eq_ignore_ascii_case("true"))
}
