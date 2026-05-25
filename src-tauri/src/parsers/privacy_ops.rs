//! Privacy ops scanner.
//!
//! Walks `dumpsys appops` output to extract only the ops relevant to the
//! Privacy module:
//!   - RUN_ANY_IN_BACKGROUND   → "background firewall" (blocks background
//!                                network/CPU work the app would otherwise do)
//!   - RUN_IN_BACKGROUND       → softer version of the above
//!   - READ_CLIPBOARD          → clipboard surveillance (telemetry / ads SDK)
//!
//! Single ADB roundtrip, deterministic, robust to interleaving with other ops.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::parsers::{AppOpMode, PackageName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyOp {
    RunAnyInBackground,
    RunInBackground,
    ReadClipboard,
}

impl PrivacyOp {
    pub fn as_op_name(&self) -> &'static str {
        match self {
            Self::RunAnyInBackground => "RUN_ANY_IN_BACKGROUND",
            Self::RunInBackground    => "RUN_IN_BACKGROUND",
            Self::ReadClipboard      => "READ_CLIPBOARD",
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        match s {
            "RUN_ANY_IN_BACKGROUND" => Some(Self::RunAnyInBackground),
            "RUN_IN_BACKGROUND"     => Some(Self::RunInBackground),
            "READ_CLIPBOARD"        => Some(Self::ReadClipboard),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyAppEntry {
    pub package: PackageName,
    /// Map: op -> current mode.
    pub ops: HashMap<String, AppOpMode>,
    /// Convenience flag: any of the firewall ops is set to ignore/deny.
    pub firewall_active: bool,
    /// Convenience flag: READ_CLIPBOARD is set to ignore/deny.
    pub clipboard_blocked: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyScan {
    /// Apps with at least one non-default privacy op.
    pub apps: Vec<PrivacyAppEntry>,
}

// "Package com.example.app:"  capturing the package
static PACKAGE_HEADER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*Package\s+(?P<pkg>[\w.]+):\s*$").unwrap()
});
// "  RUN_ANY_IN_BACKGROUND: mode=ignore"  capturing op + mode
static OP_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?P<op>[A-Z_]+):\s*mode=(?P<mode>\w+)").unwrap()
});

pub struct PrivacyOpsScanner;

impl PrivacyOpsScanner {
    pub fn command() -> &'static str {
        "dumpsys appops"
    }

    pub fn parse(input: &str) -> Result<PrivacyScan> {
        let mut apps_map: HashMap<String, PrivacyAppEntry> = HashMap::new();
        let mut current_pkg: Option<String> = None;

        for line in input.lines() {
            if let Some(caps) = PACKAGE_HEADER.captures(line) {
                current_pkg = Some(caps["pkg"].to_string());
                continue;
            }
            let Some(pkg) = current_pkg.as_ref() else { continue };
            let Some(caps) = OP_LINE.captures(line) else { continue };

            let op_name = &caps["op"];
            // Skip ops we don't care about — this is the hot loop, keep it cheap.
            if PrivacyOp::from_name(op_name).is_none() {
                continue;
            }
            let mode_raw = &caps["mode"];
            let Some(mode) = AppOpMode::from_raw(mode_raw) else { continue };

            // We only flag *changed* ops. Default modes are background noise.
            if matches!(mode, AppOpMode::Default | AppOpMode::Allow) {
                continue;
            }

            let entry = apps_map.entry(pkg.clone()).or_insert_with(|| PrivacyAppEntry {
                package: PackageName(pkg.clone()),
                ops: HashMap::new(),
                firewall_active: false,
                clipboard_blocked: false,
            });

            entry.ops.insert(op_name.to_string(), mode);

            match op_name {
                "RUN_ANY_IN_BACKGROUND" | "RUN_IN_BACKGROUND" => {
                    if matches!(mode, AppOpMode::Ignore | AppOpMode::Deny) {
                        entry.firewall_active = true;
                    }
                }
                "READ_CLIPBOARD" => {
                    if matches!(mode, AppOpMode::Ignore | AppOpMode::Deny) {
                        entry.clipboard_blocked = true;
                    }
                }
                _ => {}
            }
        }

        let mut apps: Vec<_> = apps_map.into_values().collect();
        apps.sort_by(|a, b| a.package.as_str().cmp(b.package.as_str()));

        Ok(PrivacyScan { apps })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DangerousPermissionEntry {
    pub package: PackageName,
    pub permissions: HashMap<String, String>,
}

pub struct DangerousPermissionsScanner;

impl DangerousPermissionsScanner {
    pub fn parse(input: &str) -> Result<Vec<DangerousPermissionEntry>> {
        let mut apps_map: HashMap<String, DangerousPermissionEntry> = HashMap::new();
        let mut current_pkg: Option<String> = None;

        for line in input.lines() {
            if let Some(caps) = PACKAGE_HEADER.captures(line) {
                current_pkg = Some(caps["pkg"].to_string());
                continue;
            }
            let Some(pkg) = current_pkg.as_ref() else { continue };
            let Some(caps) = OP_LINE.captures(line) else { continue };

            let op_name = &caps["op"];
            let mode = &caps["mode"];

            if matches!(op_name, "CAMERA" | "RECORD_AUDIO" | "COARSE_LOCATION" | "FINE_LOCATION" | "READ_CONTACTS") {
                if mode == "allow" || mode == "foreground" {
                    let entry = apps_map.entry(pkg.clone()).or_insert_with(|| DangerousPermissionEntry {
                        package: PackageName(pkg.clone()),
                        permissions: HashMap::new(),
                    });
                    entry.permissions.insert(op_name.to_string(), mode.to_string());
                }
            }
        }

        let mut apps: Vec<_> = apps_map.into_values().collect();
        apps.sort_by(|a, b| a.package.as_str().cmp(b.package.as_str()));
        Ok(apps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_firewall_and_clipboard_blocks() {
        let input = "
  Uid u0_a123:
    Package com.tracker.evil:
      RUN_ANY_IN_BACKGROUND: mode=ignore
      READ_CLIPBOARD: mode=deny
      WAKE_LOCK: mode=allow
  Uid u0_a999:
    Package com.benign.app:
      RUN_ANY_IN_BACKGROUND: mode=default
";
        let scan = PrivacyOpsScanner::parse(input).unwrap();
        assert_eq!(scan.apps.len(), 1, "only the tracker should be reported");
        let tracker = &scan.apps[0];
        assert_eq!(tracker.package.as_str(), "com.tracker.evil");
        assert!(tracker.firewall_active);
        assert!(tracker.clipboard_blocked);
    }

    #[test]
    fn ignores_unrelated_ops() {
        let input = "
    Package com.example.app:
      VIBRATE: mode=ignore
      POST_NOTIFICATION: mode=ignore
";
        let scan = PrivacyOpsScanner::parse(input).unwrap();
        assert_eq!(scan.apps.len(), 0);
    }
}
