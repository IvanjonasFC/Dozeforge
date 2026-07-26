//! Kernel wakelock parser - Ring 0 wakelocks.
//!
//! `dumpsys batterystats` includes a section listing wakelocks held by the
//! kernel itself (drivers, modem, Wi-Fi stack, display, NFC...). These are
//! the most overlooked source of overnight battery drain because they are
//! invisible to userland tools: an app whitelist cannot fix `wlan_rx_wake`
//! firing 12,000 times because of a noisy router pushing IPv6 RAs.
//!
//! ## Format variants
//!
//! Across Android versions we have seen three distinct shapes. The parser
//! accepts all of them and is keyed off the line containing the literal
//! token `Kernel Wakelock`:
//!
//! ### Form A - Legacy (Android 10-12)
//! ```text
//! All kernel wake locks:
//! Kernel Wakelock "wlan_rx_wake": 4h 21m 3s (12847 times)
//! Kernel Wakelock "qcom_rx_wakelock": 1h 47m 12s (3201 times)
//! ```
//!
//! ### Form B - n= notation (Android 12-13)
//! ```text
//! Kernel Wakelocks of type wakeup:
//! Kernel Wakelock "wlan_rx_wake": 30m 0s (n=8000)
//! ```
//!
//! ### Form C - Android 14+ compact, no `Wakelock` keyword on the line
//! ```text
//! Wakeup reason "wlan_rx_wake": 4h 21m 3s (12847 times) realtime
//! ```
//!
//! Form C is what's been spotted on Pixel 8 Pro and later: the literal
//! string `Kernel Wakelock` was renamed to `Wakeup reason` in the checkin
//! refactor that landed in API 34. Both names point at the same data and we
//! treat them as synonyms.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::parsers::Parser;
use crate::parsers::sleep_timeline::duration_to_ms;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelWakelock {
    pub name: String,
    pub total_ms: u64,
    pub count: u64,
    /// Plain-language explanation suitable for a non-technical user.
    pub explanation: &'static str,
    /// Severity tier based on time held.
    pub severity: KernelWakelockSeverity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KernelWakelockSeverity {
    Negligible,
    Low,
    Moderate,
    High,
    Critical,
}

static SECTION_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?im)^\s*(All kernel wake locks|Kernel Wakelocks of type wakeup|All wakeup reasons|Wakeup reasons):",
    )
    .unwrap()
});

// Accepts:
//   Kernel Wakelock "name": <duration> (<n> times)
//   Kernel Wakelock "name": <duration> (n=<n>)
//   Wakeup reason "name":   <duration> (<n> times)
//   Wakeup reason "name":   <duration> (n=<n>)
//
// And tolerates optional trailing tokens (`realtime`, `uptime`, etc.)
static KW_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m)^\s*(?:Kernel Wakelock|Wakeup reason)\s+"(?P<name>[^"]+)":\s+(?P<dur>[\d.dhmsy ]+?)(?:\s*\((?:n=)?(?P<cnt>\d+)(?:\s+times?)?\))"#,
    )
    .expect("KW_LINE regex compiles")
});

pub struct KernelWakelocksParser;

impl Parser for KernelWakelocksParser {
    type Output = Vec<KernelWakelock>;

    fn parse(&self, input: &str) -> Result<Vec<KernelWakelock>> {
        // Constrain scope to the kernel-wakelocks section if present, to
        // avoid catching the userland `Wake lock` lines emitted earlier.
        let scope = if let Some(m) = SECTION_START.find(input) {
            &input[m.start()..]
        } else {
            input
        };

        let mut out: Vec<KernelWakelock> = Vec::new();
        for caps in KW_LINE.captures_iter(scope) {
            let name = caps["name"].trim().to_string();
            let total_ms = duration_to_ms(&caps["dur"]);
            let count: u64 = caps.name("cnt").and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let explanation = classify(&name);
            let severity = severity_for(total_ms);
            out.push(KernelWakelock { name, total_ms, count, explanation, severity });
        }

        out.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
        Ok(out)
    }
}

impl KernelWakelocksParser {
    /// Parses `/proc/wakelocks` directly. Used as a final fallback when
    /// `dumpsys batterystats` returns zero kernel wakelocks - which happens
    /// on devices that haven't run on battery long enough, or whose
    /// batterystats blob has been freshly reset.
    ///
    /// Format (from `kernel/power/wakeup_reason.c`):
    ///
    /// ```text
    /// name                              active_count  event_count  wakeup_count  expire_count  active_since  total_time  max_time  last_change  prevent_suspend_time
    /// "wlan_rx_wake"                    1234          0            56            0             0             15692000    1200      5421000      14782000
    /// ```
    ///
    /// Times are in milliseconds. We only surface entries where
    /// `wakeup_count > 0` to filter out idle drivers.
    pub fn parse_proc_wakelocks(input: &str) -> Vec<KernelWakelock> {
        let mut out: Vec<KernelWakelock> = Vec::new();
        for (i, line) in input.lines().enumerate() {
            if i == 0 {
                // Header row.
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }
            let name = parts[0].trim_matches('"').to_string();
            let wakeup_count: u64 = parts[3].parse().unwrap_or(0);
            if wakeup_count == 0 {
                continue;
            }
            // total_time at index 6
            let total_ms: u64 = parts[6].parse().unwrap_or(0);
            if name.is_empty() || total_ms == 0 {
                continue;
            }
            let explanation = classify(&name);
            let severity = severity_for(total_ms);
            out.push(KernelWakelock { name, total_ms, count: wakeup_count, explanation, severity });
        }
        out.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
        out
    }
}

/// Plain-language description of common kernel wakelocks.
fn classify(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("wlan_rx") || n.contains("wlan_ctrl") || n.contains("wlan_wake") {
        "Wi-Fi radio kept awake by inbound traffic. Often a router pushing multicast/IPv6 RAs. Try disabling 'Always-on Wi-Fi during sleep' or upgrading router firmware."
    } else if n.contains("qcom_rx_wakelock") || n.contains("ipa_rx") || n.contains("ipa-wake") {
        "Cellular modem (Qualcomm) kept awake. Usually weak signal forcing constant re-registration, or a chatty app keeping the data path open."
    } else if n.contains("nfc") {
        "NFC controller kept awake. Disable NFC when not needed."
    } else if n.contains("bluetooth") || n.contains("bt_") || n.contains("btusb") {
        "Bluetooth radio kept awake. A paired peripheral (watch, headphones) is reconnecting frequently."
    } else if n.contains("powermanagerservice.display") {
        "Display subsystem kept awake. Usually proximity-sensor or digitizer noise; can also be 'Lift to wake' / 'Tap to wake' over-firing."
    } else if n.contains("powermanagerservice.wakelocks") {
        "Generic userland wakelock proxy. Look at app-level wakelocks for the real culprit."
    } else if n.contains("alarmtimer") || n.contains("rtc-alarm") {
        "Real-time clock alarms. Maps to app wakeups in the Wakeups column."
    } else if n.contains("sensorservice") || n.contains("sensors") {
        "Sensor service kept awake by an app subscribed to accelerometer or step counter."
    } else if n.contains("ipoll") || n.contains("audit") {
        "System audit/logging. Normally low impact."
    } else if n.contains("ipc") || n.contains("hidl") {
        "Inter-process communication wakelock. Usually proxies a higher-level holder."
    } else if n.contains("event") {
        "Generic event-driven wakelock. Diagnostic info only."
    } else {
        "Kernel-level wakelock. Diagnostic info only."
    }
}

fn severity_for(ms: u64) -> KernelWakelockSeverity {
    let hours = ms as f64 / 3_600_000.0;
    match hours {
        h if h >= 4.0 => KernelWakelockSeverity::Critical,
        h if h >= 1.5 => KernelWakelockSeverity::High,
        h if h >= 0.5 => KernelWakelockSeverity::Moderate,
        h if h >= 0.05 => KernelWakelockSeverity::Low,
        _ => KernelWakelockSeverity::Negligible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TIMES_FORM: &str = "
  All kernel wake locks:
  Kernel Wakelock \"wlan_rx_wake\": 4h 21m 3s (12847 times)
  Kernel Wakelock \"qcom_rx_wakelock\": 1h 47m 12s (3201 times)
  Kernel Wakelock \"PowerManagerService.Display\": 18m 4s (0 times)
  Kernel Wakelock \"unrelated\": 5s (1 times)
";

    const SAMPLE_N_FORM: &str = "
  Kernel Wakelocks of type wakeup:
  Kernel Wakelock \"wlan_rx_wake\": 30m 0s (n=8000)
  Kernel Wakelock \"bluetooth_timer\": 5m 30s (n=120)
";

    // Android 14+ "Wakeup reason" form, also seen on Pixel 8 Pro.
    const SAMPLE_WAKEUP_REASON_FORM: &str = "
  All wakeup reasons:
  Wakeup reason \"wlan_rx_wake\": 4h 21m 3s (12847 times)
  Wakeup reason \"qcom_rx_wakelock\": 1h 47m 12s (n=3201)
";

    const SAMPLE_PROC_WAKELOCKS: &str = "name                              active_count    event_count     wakeup_count    expire_count    active_since    total_time      max_time        last_change     prevent_suspend_time
\"wlan_rx_wake\"                    1234            0               56              0               0               15692000        1200            5421000         14782000
\"qcom_rx_wakelock\"                0               0               0               0               0               1000            100             100             0
\"bluetooth_timer\"                 5               0               10              0               0               330000          500             8000            300000
";

    #[test]
    fn parses_times_form() {
        let out = KernelWakelocksParser.parse(SAMPLE_TIMES_FORM).unwrap();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].name, "wlan_rx_wake");
        assert_eq!(out[0].count, 12847);
        assert!(out[0].explanation.contains("Wi-Fi"));
        assert_eq!(out[0].severity, KernelWakelockSeverity::Critical);
    }

    #[test]
    fn parses_n_form() {
        let out = KernelWakelocksParser.parse(SAMPLE_N_FORM).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "wlan_rx_wake");
        assert_eq!(out[0].count, 8000);
        assert_eq!(out[0].total_ms, 30 * 60_000);
    }

    #[test]
    fn parses_wakeup_reason_form_api_34() {
        let out = KernelWakelocksParser.parse(SAMPLE_WAKEUP_REASON_FORM).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "wlan_rx_wake");
        assert_eq!(out[0].count, 12847);
        assert_eq!(out[1].name, "qcom_rx_wakelock");
        assert_eq!(out[1].count, 3201);
    }

    #[test]
    fn parses_proc_wakelocks_fallback() {
        let out = KernelWakelocksParser::parse_proc_wakelocks(SAMPLE_PROC_WAKELOCKS);
        // qcom_rx_wakelock has wakeup_count=0 so it must be filtered out.
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "wlan_rx_wake");
        assert_eq!(out[0].count, 56);
        assert_eq!(out[0].total_ms, 15692000);
    }

    #[test]
    fn classifies_unknown_generically() {
        assert!(classify("some_proprietary_thing").contains("Diagnostic"));
    }

    #[test]
    fn sorted_descending() {
        let out = KernelWakelocksParser.parse(SAMPLE_TIMES_FORM).unwrap();
        assert!(out[0].total_ms >= out[1].total_ms);
        assert!(out[1].total_ms >= out[2].total_ms);
    }
}
