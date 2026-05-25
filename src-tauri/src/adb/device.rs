//! Device-identity types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceSerial(pub String);

impl DeviceSerial {
    pub fn as_str(&self) -> &str { &self.0 }
}

impl std::fmt::Display for DeviceSerial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    Device,
    Unauthorized,
    Offline,
    Recovery,
    Sideload,
    Bootloader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub serial: DeviceSerial,
    pub state: DeviceState,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

/// Coarse build identity used to decide whether a stored snapshot can still be
/// safely restored. We bind to API level and (year, month) of the security
/// patch -- but not to the build fingerprint itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildIdentity {
    pub sdk_int: u32,
    pub security_patch_year: u32,
    pub security_patch_month: u32,
    pub fingerprint: String,
}

impl BuildIdentity {
    /// A snapshot is compatible if the SDK level matches exactly. The security
    /// patch is allowed to differ (monthly patches do not change APIs).
    pub fn is_compatible_with(&self, other: &BuildIdentity) -> bool {
        self.sdk_int == other.sdk_int
    }

    /// Parses `2026-04-05` into `(year, month)`. Returns `(0, 0)` on failure.
    pub fn parse_patch_date(raw: &str) -> (u32, u32) {
        let parts: Vec<&str> = raw.split('-').collect();
        let year = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let month = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        (year, month)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_date_parsing() {
        assert_eq!(BuildIdentity::parse_patch_date("2026-04-05"), (2026, 4));
        assert_eq!(BuildIdentity::parse_patch_date("garbage"), (0, 0));
    }

    #[test]
    fn compatibility_ignores_patch() {
        let a = BuildIdentity { sdk_int: 34, security_patch_year: 2026, security_patch_month: 3, fingerprint: "A".into() };
        let b = BuildIdentity { sdk_int: 34, security_patch_year: 2026, security_patch_month: 5, fingerprint: "B".into() };
        assert!(a.is_compatible_with(&b));
    }

    #[test]
    fn compatibility_rejects_sdk_change() {
        let a = BuildIdentity { sdk_int: 33, security_patch_year: 2026, security_patch_month: 3, fingerprint: "A".into() };
        let b = BuildIdentity { sdk_int: 34, security_patch_year: 2026, security_patch_month: 3, fingerprint: "B".into() };
        assert!(!a.is_compatible_with(&b));
    }
}
