//! Reads system-level tunable flags used by the Automation & Privacy modules.
//!
//! Three independent global settings, all owned by Settings.Global except
//! `max_phantom_processes` which lives in DeviceConfig under `activity_manager`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemTweaks {
    /// `settings get global settings_enable_monitor_phantom_procs`
    /// When `Some(false)`, the phantom-process tracker is fully disabled.
    /// `None` = setting unset (Android default = enabled).
    pub phantom_monitor_enabled: Option<bool>,
    /// `settings get global captive_portal_mode`
    /// 0 = off (no pings), 1 = enabled (default).
    pub captive_portal_mode: Option<u8>,
    /// `device_config get activity_manager max_phantom_processes`
    /// Android default: 32. Common preset: 1024. Maximum: 2^31-1 (effectively unlimited).
    pub max_phantom_processes: Option<u32>,
}

impl SystemTweaks {
    /// Parses three raw shell outputs (in order):
    ///   1. `settings get global settings_enable_monitor_phantom_procs`
    ///   2. `settings get global captive_portal_mode`
    ///   3. `device_config get activity_manager max_phantom_processes`
    pub fn from_parts(phantom: &str, captive: &str, max: &str) -> Self {
        Self {
            phantom_monitor_enabled: parse_bool_setting(phantom),
            captive_portal_mode: parse_u8(captive),
            max_phantom_processes: parse_u32(max),
        }
    }

    /// True iff captive portal pinging is fully suppressed.
    pub fn captive_portal_suppressed(&self) -> bool {
        matches!(self.captive_portal_mode, Some(0))
    }

    /// True iff phantom processes are effectively unrestricted
    /// (either monitor disabled or limit set high enough).
    pub fn phantom_unrestricted(&self) -> bool {
        matches!(self.phantom_monitor_enabled, Some(false))
            || matches!(self.max_phantom_processes, Some(n) if n >= 1024)
    }
}

fn parse_bool_setting(raw: &str) -> Option<bool> {
    let t = raw.trim().to_ascii_lowercase();
    match t.as_str() {
        "true" | "1"  => Some(true),
        "false" | "0" => Some(false),
        _ => None, // "null", empty, or anything unexpected → unset
    }
}

fn parse_u8(raw: &str) -> Option<u8> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return None; }
    t.parse().ok()
}

fn parse_u32(raw: &str) -> Option<u32> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return None; }
    t.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three() {
        let s = SystemTweaks::from_parts("false", "0", "2147483647");
        assert_eq!(s.phantom_monitor_enabled, Some(false));
        assert_eq!(s.captive_portal_mode, Some(0));
        assert_eq!(s.max_phantom_processes, Some(2_147_483_647));
        assert!(s.captive_portal_suppressed());
        assert!(s.phantom_unrestricted());
    }

    #[test]
    fn handles_unset_values() {
        let s = SystemTweaks::from_parts("null", "", "null");
        assert_eq!(s.phantom_monitor_enabled, None);
        assert_eq!(s.captive_portal_mode, None);
        assert_eq!(s.max_phantom_processes, None);
        assert!(!s.captive_portal_suppressed());
        assert!(!s.phantom_unrestricted());
    }

    #[test]
    fn detects_phantom_unrestricted_via_high_limit() {
        // Monitor still on (default) but limit is huge → effectively unrestricted
        let s = SystemTweaks::from_parts("true", "1", "2147483647");
        assert!(s.phantom_unrestricted());
    }
}