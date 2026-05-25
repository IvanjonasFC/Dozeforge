//! Private DNS state reader.
//!
//! Android stores Private DNS configuration in two global settings:
//!   - `private_dns_mode`: "off" | "opportunistic" | "hostname"
//!   - `private_dns_specifier`: the hostname when mode=hostname
//!
//! The strings returned by `settings get global ...` are sometimes
//! literal "null" when unset, which we treat as None.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivateDnsMode {
    Off,
    Opportunistic,
    Hostname,
}

impl PrivateDnsMode {
    pub fn from_raw(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "off"      => Self::Off,
            "hostname" => Self::Hostname,
            // "opportunistic" or "null" or empty → opportunistic (Android default)
            _ => Self::Opportunistic,
        }
    }

    pub fn as_setting(&self) -> &'static str {
        match self {
            Self::Off           => "off",
            Self::Opportunistic => "opportunistic",
            Self::Hostname      => "hostname",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateDnsState {
    pub mode: PrivateDnsMode,
    pub hostname: Option<String>,
}

impl PrivateDnsState {
    pub fn parse(mode_raw: &str, hostname_raw: &str) -> Self {
        let mode = PrivateDnsMode::from_raw(mode_raw);
        let hostname_trim = hostname_raw.trim();
        let hostname = if hostname_trim.is_empty() || hostname_trim.eq_ignore_ascii_case("null") {
            None
        } else {
            Some(hostname_trim.to_string())
        };
        Self { mode, hostname }
    }
}

/// Curated list of well-known privacy-respecting DNS hostnames.
/// These are the DNS-over-TLS endpoints required by Android's
/// "Private DNS" hostname mode (NOT DoH URLs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsPreset {
    pub label: &'static str,
    pub hostname: &'static str,
    pub blocks_ads: bool,
    pub blocks_trackers: bool,
}

pub const DNS_PRESETS: &[DnsPreset] = &[
    DnsPreset { label: "AdGuard DNS",           hostname: "dns.adguard-dns.com",  blocks_ads: true,  blocks_trackers: true },
    DnsPreset { label: "AdGuard Family",        hostname: "family.adguard-dns.com", blocks_ads: true, blocks_trackers: true },
    DnsPreset { label: "Cloudflare 1.1.1.1",    hostname: "one.one.one.one",      blocks_ads: false, blocks_trackers: false },
    DnsPreset { label: "Cloudflare Malware",    hostname: "security.cloudflare-dns.com", blocks_ads: false, blocks_trackers: true },
    DnsPreset { label: "Cloudflare Family",     hostname: "family.cloudflare-dns.com",   blocks_ads: true,  blocks_trackers: true },
    DnsPreset { label: "Quad9 Standard",        hostname: "dns.quad9.net",        blocks_ads: false, blocks_trackers: true },
    DnsPreset { label: "Mullvad Adblock",       hostname: "adblock.dns.mullvad.net", blocks_ads: true, blocks_trackers: true },
    DnsPreset { label: "NextDNS (config req.)", hostname: "dns.nextdns.io",       blocks_ads: true,  blocks_trackers: true },
    DnsPreset { label: "ControlD Free",         hostname: "p0.freedns.controld.com", blocks_ads: false, blocks_trackers: false },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hostname_mode() {
        let s = PrivateDnsState::parse("hostname\n", "dns.adguard.com\n");
        assert_eq!(s.mode, PrivateDnsMode::Hostname);
        assert_eq!(s.hostname.as_deref(), Some("dns.adguard.com"));
    }

    #[test]
    fn parses_off_mode_with_null_hostname() {
        let s = PrivateDnsState::parse("off", "null");
        assert_eq!(s.mode, PrivateDnsMode::Off);
        assert_eq!(s.hostname, None);
    }

    #[test]
    fn unknown_mode_defaults_to_opportunistic() {
        let s = PrivateDnsState::parse("", "");
        assert_eq!(s.mode, PrivateDnsMode::Opportunistic);
        assert_eq!(s.hostname, None);
    }
}
