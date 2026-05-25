//! Sleep timeline parser — extracts screen-off / deep-sleep / awake-with-screen-off
//! from `dumpsys batterystats` (textual form, NOT --checkin).
//!
//! These three numbers answer the question "is my device actually sleeping
//! when locked?". Their relationship:
//!
//!   screen_off_realtime - screen_off_uptime  ≈  deep_sleep_time
//!   screen_off_uptime                        ≈  awake_with_screen_off  (the leak)
//!
//! `realtime` is wall-clock (whether CPU active or not); `uptime` excludes
//! suspend. A perfectly-sleeping device locked for 8h has
//! screen_off_realtime ≈ 8h, screen_off_uptime ≈ a few minutes.
//! A leaky device might show screen_off_uptime = 3h, meaning the CPU was
//! awake for 3h with the screen off.
//!
//! Source lines we parse (formatted by `BatteryStatsImpl.dumpLocked`):
//!
//!   Time on battery: 2d 3h 27m 51s 270ms (90.5%) realtime, 9h 12m 4s 391ms (16.0%) uptime
//!   Time on battery screen off: 2d 1h 13m 56s 891ms (88.0%) realtime, 8h 5m 9s 12ms (14.0%) uptime
//!   Total uptime: 12h 13m 5s 720ms
//!   Total realtime: 2d 11h 53m 30s 612ms

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::parsers::Parser;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SleepTimeline {
    /// Wall-clock time on battery (no screen-off filter), in ms.
    pub on_battery_realtime_ms: u64,
    /// CPU active time on battery (no screen-off filter), in ms.
    pub on_battery_uptime_ms: u64,
    /// Wall-clock time on battery with screen off, in ms.
    pub screen_off_realtime_ms: u64,
    /// CPU-awake time during screen off (the leak metric), in ms.
    pub screen_off_uptime_ms: u64,
    /// Derived: time the CPU was actually suspended with screen off.
    pub deep_sleep_ms: u64,
    /// Efficiency ratio in 0.0..=1.0 (deep_sleep / screen_off_realtime).
    /// Higher is better. 0.85+ is healthy, < 0.60 indicates a leak.
    pub efficiency_ratio: f32,
    /// Human-readable tier derived from `efficiency_ratio`.
    pub tier: SleepEfficiencyTier,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SleepEfficiencyTier {
    /// >= 90% efficient. Device sleeps cleanly.
    Excellent,
    /// 75-90%. Normal modern Android with light background activity.
    #[default]
    Good,
    /// 60-75%. Noticeable drain at night.
    Mediocre,
    /// < 60%. The device is barely sleeping; something is holding it awake.
    Bad,
}

static ON_BATTERY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)^\s*Time on battery:\s+(?P<rt>[^(]+?)\s*\([\d.]+%\)\s+realtime,\s+(?P<ut>[^(]+?)\s*\([\d.]+%\)\s+uptime",
    )
    .expect("ON_BATTERY regex compiles")
});

static SCREEN_OFF: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)^\s*Time on battery screen off:\s+(?P<rt>[^(]+?)\s*\([\d.]+%\)\s+realtime,\s+(?P<ut>[^(]+?)\s*\([\d.]+%\)\s+uptime",
    )
    .expect("SCREEN_OFF regex compiles")
});

pub struct SleepTimelineParser;

impl Parser for SleepTimelineParser {
    type Output = SleepTimeline;

    fn parse(&self, input: &str) -> Result<SleepTimeline> {
        let on_battery = ON_BATTERY.captures(input);
        let screen_off = SCREEN_OFF.captures(input);

        if on_battery.is_none() && screen_off.is_none() {
            return Err(Error::Parse {
                parser_name: "sleep_timeline",
                reason: "no `Time on battery` or `Time on battery screen off` lines found".into(),
            });
        }

        let (on_battery_realtime_ms, on_battery_uptime_ms) = on_battery
            .map(|c| (duration_to_ms(&c["rt"]), duration_to_ms(&c["ut"])))
            .unwrap_or((0, 0));

        let (screen_off_realtime_ms, screen_off_uptime_ms) = screen_off
            .map(|c| (duration_to_ms(&c["rt"]), duration_to_ms(&c["ut"])))
            .unwrap_or((0, 0));

        let deep_sleep_ms = screen_off_realtime_ms.saturating_sub(screen_off_uptime_ms);

        let efficiency_ratio = if screen_off_realtime_ms == 0 {
            0.0
        } else {
            (deep_sleep_ms as f64 / screen_off_realtime_ms as f64) as f32
        };

        let tier = match efficiency_ratio {
            r if r >= 0.90 => SleepEfficiencyTier::Excellent,
            r if r >= 0.75 => SleepEfficiencyTier::Good,
            r if r >= 0.60 => SleepEfficiencyTier::Mediocre,
            _ => SleepEfficiencyTier::Bad,
        };

        Ok(SleepTimeline {
            on_battery_realtime_ms,
            on_battery_uptime_ms,
            screen_off_realtime_ms,
            screen_off_uptime_ms,
            deep_sleep_ms,
            efficiency_ratio,
            tier,
        })
    }
}

/// Parse Android-formatted durations like `2d 3h 27m 51s 270ms` to milliseconds.
///
/// Components are space-separated and optional. Each component ends with one
/// of `d`, `h`, `m`, `s`, `ms`. We support fractional values defensively, but
/// AOSP always emits integers here.
pub(crate) fn duration_to_ms(s: &str) -> u64 {
    let mut total: u64 = 0;
    for token in s.split_whitespace() {
        let Some(stripped) = strip_unit(token) else {
            continue;
        };
        let (num_str, mult_ms) = stripped;
        let Ok(num) = num_str.parse::<f64>() else {
            continue;
        };
        total = total.saturating_add((num * mult_ms).round() as u64);
    }
    total
}

/// Returns (numeric_part, milliseconds_per_unit) for a token like `2d` or `270ms`.
fn strip_unit(tok: &str) -> Option<(&str, f64)> {
    // Check longer suffixes first (`ms` before `s`).
    for (suffix, mult) in &[
        ("ms", 1.0_f64),
        ("s", 1_000.0),
        ("m", 60_000.0),
        ("h", 3_600_000.0),
        ("d", 86_400_000.0),
    ] {
        if let Some(num) = tok.strip_suffix(*suffix) {
            return Some((num, *mult));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL_SAMPLE: &str = "
Battery History (12% used, 18033 used of 153KB, 27 strings using 920):
                0 (9) RESET:TIME: 1700000000000

Statistics since last charge:
  System starts: 0, currently on battery: true
  Time on battery: 2d 3h 27m 51s 270ms (90.5%) realtime, 9h 12m 4s 391ms (16.0%) uptime
  Time on battery screen off: 2d 1h 13m 56s 891ms (88.0%) realtime, 8h 5m 9s 12ms (14.0%) uptime
  Total uptime: 12h 13m 5s 720ms
  Total realtime: 2d 11h 53m 30s 612ms
";

    #[test]
    fn parses_pixel_sample() {
        let t = SleepTimelineParser.parse(PIXEL_SAMPLE).expect("parse ok");
        // screen_off_realtime ≈ 2d 1h 13m 56s 891ms
        let expected_so_rt = (2 * 86_400 + 1 * 3600 + 13 * 60 + 56) * 1000 + 891;
        assert_eq!(t.screen_off_realtime_ms, expected_so_rt);
        // screen_off_uptime ≈ 8h 5m 9s 12ms
        let expected_so_ut = (8 * 3600 + 5 * 60 + 9) * 1000 + 12;
        assert_eq!(t.screen_off_uptime_ms, expected_so_ut);
        assert_eq!(t.deep_sleep_ms, expected_so_rt - expected_so_ut);
        // Efficiency ≈ 0.8367 → "good"
        assert!(t.efficiency_ratio > 0.83 && t.efficiency_ratio < 0.84);
        assert_eq!(t.tier, SleepEfficiencyTier::Good);
    }

    #[test]
    fn missing_block_errors() {
        let err = SleepTimelineParser.parse("nothing here").unwrap_err();
        match err {
            Error::Parse { parser_name, .. } => assert_eq!(parser_name, "sleep_timeline"),
            _ => panic!("expected Parse error"),
        }
    }

    #[test]
    fn duration_parser_handles_all_units() {
        assert_eq!(duration_to_ms("1d"), 86_400_000);
        assert_eq!(duration_to_ms("2h 30m"), 2 * 3_600_000 + 30 * 60_000);
        assert_eq!(duration_to_ms("45s 123ms"), 45_123);
        assert_eq!(duration_to_ms(""), 0);
    }

    #[test]
    fn perfect_sleep_yields_excellent() {
        let input = "
  Time on battery screen off: 8h 0m 0s 0ms (100.0%) realtime, 5m 0s 0ms (1.0%) uptime
";
        let t = SleepTimelineParser.parse(input).unwrap();
        assert_eq!(t.tier, SleepEfficiencyTier::Excellent);
        assert!(t.efficiency_ratio > 0.98);
    }

    #[test]
    fn leaky_sleep_yields_bad() {
        let input = "
  Time on battery screen off: 8h 0m 0s 0ms (100.0%) realtime, 4h 0m 0s 0ms (50.0%) uptime
";
        let t = SleepTimelineParser.parse(input).unwrap();
        assert_eq!(t.tier, SleepEfficiencyTier::Bad);
        assert!((t.efficiency_ratio - 0.5).abs() < 0.01);
    }
}
