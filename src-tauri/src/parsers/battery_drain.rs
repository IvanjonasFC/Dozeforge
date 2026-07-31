//! Per-app battery drain parser — extracts the `Estimated power use (mAh)`
//! block from `dumpsys batterystats` and attributes drain to packages.
//!
//! Format (line-based, indented under "Estimated power use (mAh):"):
//!
//!   Estimated power use (mAh):
//!     Capacity: 5000, Computed drain: 234.5, actual drain: 200-250
//!     Screen: 45.6
//!     Idle: 12.3
//!     Cell standby: 8.9
//!     Wifi: 3.2
//!     Bluetooth: 0.5
//!     Uid u0a123: 23.4 ( cpu=12.3 wake=2.1 wifi=8.9 ... )
//!     Uid u0a87: 18.7 ( cpu=10.0 wake=5.0 sensor=3.7 )
//!     Uid 1000: 45.0 ( cpu=45.0 )
//!
//! The trailing parenthesised breakdown is optional and varies by API level.
//! We capture only the per-UID total and the subsystem breakdown.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::parsers::{PackageName, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDrainEntry {
    pub package: PackageName,
    pub uid: i32,
    pub drain_mah: f64,
    /// Share of computed drain (0.0..=1.0). Useful for UI bars.
    pub drain_share: f32,
    /// Sub-component breakdown in mAh. Common keys: cpu, wake, wifi, sensor,
    /// audio, video, gps, bluetooth, cell, screen. Empty if not present.
    pub breakdown: HashMap<String, f64>,
    /// True if this app holds a wakelock currently (joined with live wakelocks).
    pub has_live_wakelock: bool,
    /// True if a process for this app was in zombie state during sampling.
    pub is_zombie: bool,
    /// Plain-language verdict for the UI.
    pub verdict: AppDrainVerdict,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppDrainVerdict {
    /// Negligible drain (< 0.5 mAh or < 0.2% share). Safe to ignore.
    Negligible,
    /// Drain explained by foreground use (CPU dominated by screen-on samples).
    LegitimateForeground,
    /// Drain explained by background media playback. Likely legitimate.
    LegitimateMedia,
    /// Background CPU drain. Worth restricting unless user uses the app often.
    BackgroundHog,
    /// Live wakelock + no recent foreground use. Almost certainly a leak.
    Zombie,
    /// Wakelock-only drain (sensor, GPS, wifi). High-impact targets.
    RadioHog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryDrain {
    pub capacity_mah: Option<u32>,
    pub computed_drain_mah: f64,
    pub actual_drain_min_mah: Option<f64>,
    pub actual_drain_max_mah: Option<f64>,
    pub entries: Vec<AppDrainEntry>,
}

static SECTION_START: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*Estimated power use \(mAh\):").unwrap());

// The next top-level section header after the power-use block. `dumpsys
// batterystats` follows "Estimated power use" with other sections that ALSO
// contain `Uid NNNN:` lines with a totally different meaning — notably
// "Per-app mobile ms per packet:" (`Uid 1051: 113 (382 packets ...)`). Without
// bounding the scope, those get mis-parsed as drain and collide with real
// entries. A section header is a line whose only trailing token is a colon.
static SECTION_END: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^[ \t]*[A-Za-z][^\n:]*:[ \t]*$").unwrap());

static CAPACITY_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)^\s*Capacity:\s*(?P<cap>\d+),\s*Computed drain:\s*(?P<comp>[\d.]+)(?:,\s*actual drain:\s*(?P<min>[\d.]+)(?:-(?P<max>[\d.]+))?)?",
    )
    .unwrap()
});

// Examples we must match, across Android versions:
//   Old:  Uid u0a123: 23.4 ( cpu=12.3 wake=2.1 )
//   Old:  Uid 1000: 45.0 ( cpu=45.0 )
//   New (A14/15):  UID u0a379: 153 fg: 101 (…) bg: 22.4 (…) cached: 0.363 (…)
//                    screen=29.0 cpu=70.2 cpu:fg=64.8 cpu:bg=5.08 …   ← breakdown line
// `inline` captures the old parenthesised breakdown; `cont` captures the new
// continuation-line breakdown (key=val tokens) on the following indented line.
static UID_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?m)^\s*[Uu][Ii][Dd]\s+(?P<raw>u?\d+a?\d*)\s*:\s*(?P<mah>[\d.]+)(?:\s*\((?P<inline>[^)]*)\))?[^\n]*(?:\n[ \t]+(?P<cont>[a-z][^\n]*))?",
    )
    .unwrap()
});

static BREAKDOWN_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?P<key>[a-zA-Z_]+)\s*=\s*(?P<val>[\d.]+)").unwrap());

pub struct BatteryDrainParser {
    /// Map from UID to package name, sourced from another parser pass.
    pub uid_to_pkg: HashMap<i32, PackageName>,
    /// Set of packages with live wakelocks, for cross-flagging.
    pub live_wakelock_pkgs: std::collections::HashSet<String>,
    /// Set of packages with zombie processes, for cross-flagging.
    pub zombie_pkgs: std::collections::HashSet<String>,
}

impl Parser for BatteryDrainParser {
    type Output = BatteryDrain;

    fn parse(&self, input: &str) -> Result<BatteryDrain> {
        // Constrain to the section. If absent, return empty (not an error;
        // some older API levels emit drain only with `--reset` first).
        let Some(start) = SECTION_START.find(input) else {
            return Ok(BatteryDrain::default());
        };
        // Bound the scope to just this section: from its header to the next
        // top-level section header. Prevents `Uid NNNN:` lines in later,
        // unrelated sections (e.g. per-packet network stats) from being parsed
        // as drain and producing duplicate UIDs.
        let scope_end = SECTION_END
            .find(&input[start.end()..])
            .map(|m| start.end() + m.start())
            .unwrap_or(input.len());
        let scope = &input[start.start()..scope_end];

        let mut out = BatteryDrain::default();
        if let Some(c) = CAPACITY_LINE.captures(scope) {
            out.capacity_mah = c["cap"].parse().ok();
            out.computed_drain_mah = c["comp"].parse().unwrap_or(0.0);
            out.actual_drain_min_mah = c.name("min").and_then(|m| m.as_str().parse().ok());
            out.actual_drain_max_mah = c.name("max").and_then(|m| m.as_str().parse().ok());
        }

        for caps in UID_LINE.captures_iter(scope) {
            let raw = &caps["raw"];
            let Some(uid) = parse_uid(raw) else { continue };
            let drain_mah: f64 = caps["mah"].parse().unwrap_or(0.0);
            if drain_mah < 0.5 {
                continue;
            }
            let mut breakdown: HashMap<String, f64> = HashMap::new();
            let bd_src = caps
                .name("inline")
                .or_else(|| caps.name("cont"))
                .map(|m| m.as_str())
                .unwrap_or("");
            for tok in BREAKDOWN_TOKEN.captures_iter(bd_src) {
                let key = tok["key"].to_ascii_lowercase();
                let val: f64 = tok["val"].parse().unwrap_or(0.0);
                breakdown.insert(key, val);
            }

            let package = self.uid_to_pkg.get(&uid).cloned().unwrap_or_else(|| {
                PackageName(if uid < 10_000 {
                    format!("system:uid={uid}")
                } else {
                    format!("uid={uid}")
                })
            });

            let pkg_str = package.0.clone();
            let has_live_wakelock = self.live_wakelock_pkgs.contains(&pkg_str);
            let is_zombie = self.zombie_pkgs.contains(&pkg_str);

            let drain_share = if out.computed_drain_mah > 0.0 {
                (drain_mah / out.computed_drain_mah) as f32
            } else {
                0.0
            };

            let verdict = classify(drain_mah, drain_share, &breakdown, has_live_wakelock, is_zombie);

            out.entries.push(AppDrainEntry {
                package,
                uid,
                drain_mah,
                drain_share,
                breakdown,
                has_live_wakelock,
                is_zombie,
                verdict,
            });
        }

        // Safety net: guarantee one entry per UID (keep the largest drain) so a
        // keyed UI list can never receive duplicate keys, even if a future ROM
        // format slips a repeated UID past the scope bound above.
        out.entries.sort_by(|a, b| {
            a.uid.cmp(&b.uid).then(
                b.drain_mah
                    .partial_cmp(&a.drain_mah)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        out.entries.dedup_by_key(|e| e.uid);

        out.entries.sort_by(|a, b| b.drain_mah.partial_cmp(&a.drain_mah).unwrap_or(std::cmp::Ordering::Equal));

        // When the device is charging / just fully charged, batterystats reports
        // a ~0 "Computed drain", which would flatten every share bar to 0%. Fall
        // back to a share relative to the summed per-app estimates so the bars
        // still convey each app's *relative* weight. Verdicts stay untouched —
        // with no real discharge session there is genuinely nothing to flag.
        if out.computed_drain_mah <= 0.0 {
            let sum: f64 = out.entries.iter().map(|e| e.drain_mah).sum();
            if sum > 0.0 {
                for e in out.entries.iter_mut() {
                    e.drain_share = (e.drain_mah / sum) as f32;
                }
            }
        }

        Ok(out)
    }
}

/// Parse Android UID notation. Examples: `u0a123` -> 10123, `1000` -> 1000.
/// `u<user>a<offset>` resolves to `user * 100_000 + 10_000 + offset` per AOSP.
fn parse_uid(s: &str) -> Option<i32> {
    if let Some(rest) = s.strip_prefix('u') {
        let (user_s, off_s) = rest.split_once('a')?;
        let user: i32 = user_s.parse().ok()?;
        let off: i32 = off_s.parse().ok()?;
        Some(user * 100_000 + 10_000 + off)
    } else {
        s.parse().ok()
    }
}

fn classify(
    drain_mah: f64,
    drain_share: f32,
    breakdown: &HashMap<String, f64>,
    has_live_wakelock: bool,
    is_zombie: bool,
) -> AppDrainVerdict {
    if drain_mah < 0.5 || drain_share < 0.002 {
        return AppDrainVerdict::Negligible;
    }
    if is_zombie || (has_live_wakelock && drain_share > 0.05) {
        return AppDrainVerdict::Zombie;
    }
    // Audio / video subsystems hot => likely media playback.
    let audio = breakdown.get("audio").copied().unwrap_or(0.0);
    let video = breakdown.get("video").copied().unwrap_or(0.0);
    if (audio + video) > 0.3 * drain_mah {
        return AppDrainVerdict::LegitimateMedia;
    }
    // Sensor / GPS / wifi-active dominate => radio hog.
    let radio = breakdown.get("sensor").copied().unwrap_or(0.0)
        + breakdown.get("gps").copied().unwrap_or(0.0)
        + breakdown.get("wifi").copied().unwrap_or(0.0)
        + breakdown.get("cell").copied().unwrap_or(0.0);
    if radio >= 0.4 * drain_mah && drain_share > 0.03 {
        return AppDrainVerdict::RadioHog;
    }
    // Screen-attributable CPU is hard to derive without screen-on-time; we
    // treat large drain with wake>cpu as legitimate foreground proxy.
    let cpu = breakdown.get("cpu").copied().unwrap_or(0.0);
    let wake = breakdown.get("wake").copied().unwrap_or(0.0);
    if wake > 0.0 && wake > cpu && drain_share < 0.05 {
        return AppDrainVerdict::LegitimateForeground;
    }
    if drain_share > 0.03 {
        AppDrainVerdict::BackgroundHog
    } else {
        AppDrainVerdict::LegitimateForeground
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "
  Estimated power use (mAh):
    Capacity: 5000, Computed drain: 234.5, actual drain: 200-250
    Screen: 45.6
    Idle: 12.3
    Cell standby: 8.9
    Uid u0a123: 60.0 ( cpu=20.0 wake=5.0 wifi=30.0 cell=5.0 )
    Uid u0a87: 25.0 ( cpu=10.0 wake=5.0 sensor=10.0 )
    Uid u0a44: 18.0 ( cpu=2.0 audio=15.0 )
    Uid 1000: 5.0
    Uid u0a999: 0.1
";

    fn parser() -> BatteryDrainParser {
        let mut uid_to_pkg = HashMap::new();
        uid_to_pkg.insert(10_123, PackageName("com.example.hog".into()));
        uid_to_pkg.insert(10_087, PackageName("com.example.radiohog".into()));
        uid_to_pkg.insert(10_044, PackageName("com.spotify.music".into()));
        BatteryDrainParser {
            uid_to_pkg,
            live_wakelock_pkgs: Default::default(),
            zombie_pkgs: Default::default(),
        }
    }

    #[test]
    fn parses_capacity_and_drain() {
        let out = parser().parse(SAMPLE).unwrap();
        assert_eq!(out.capacity_mah, Some(5000));
        assert!((out.computed_drain_mah - 234.5).abs() < 0.001);
        assert_eq!(out.actual_drain_min_mah, Some(200.0));
        assert_eq!(out.actual_drain_max_mah, Some(250.0));
    }

    #[test]
    fn drops_negligible_entries() {
        let out = parser().parse(SAMPLE).unwrap();
        // u0a999 has 0.1 mAh -> below threshold.
        assert!(!out.entries.iter().any(|e| e.uid == 10_999));
    }

    #[test]
    fn classifies_media_app() {
        let out = parser().parse(SAMPLE).unwrap();
        let spotify = out.entries.iter().find(|e| e.uid == 10_044).unwrap();
        assert_eq!(spotify.verdict, AppDrainVerdict::LegitimateMedia);
    }

    #[test]
    fn classifies_radio_hog() {
        let out = parser().parse(SAMPLE).unwrap();
        let radio = out.entries.iter().find(|e| e.uid == 10_087).unwrap();
        assert_eq!(radio.verdict, AppDrainVerdict::RadioHog);
    }

    #[test]
    fn parse_uid_handles_both_forms() {
        assert_eq!(parse_uid("u0a123"), Some(10_123));
        assert_eq!(parse_uid("u1a44"), Some(110_044));
        assert_eq!(parse_uid("1000"), Some(1000));
        assert_eq!(parse_uid("bogus"), None);
    }

    #[test]
    fn live_wakelock_flag_propagates_to_verdict() {
        let mut p = parser();
        p.live_wakelock_pkgs.insert("com.example.hog".into());
        let out = p.parse(SAMPLE).unwrap();
        let hog = out.entries.iter().find(|e| e.uid == 10_123).unwrap();
        assert!(hog.has_live_wakelock);
        assert_eq!(hog.verdict, AppDrainVerdict::Zombie);
    }

    // ── Real Android 14/15 (Pixel 8 Pro) fixture ─────────────────────────────
    // Uppercase "UID", value followed by fg:/bg:/cached:, and the per-app
    // breakdown on the *next* indented line (key=val). Guards the A14/15 fix.
    const A15_FORMAT: &str = "
  Estimated power use (mAh):
    Capacity: 4716, Computed drain: 1356, actual drain: 1356
    Global
     screen: 89.9 apps: 89.9
     cpu: 289 apps: 352 duration: 5h 0m 1s 450ms
     wakelock: 103 apps: 103 duration: 3h 35m 35s 621ms
  UID u0a379: 153 fg: 101 (27m 7s 396ms) bg: 22.4 (15ms) cached: 0.363 (42m 17s 164ms)
      screen=29.0 cpu=70.2 cpu:fg=64.8 cpu:bg=5.08
  UID 1000: 48.6 fg: 20 bg: 8.6
      cpu=30.2 wifi=8.6
  UID u0a44: 18.0 fg: 15
      audio=15.0 cpu=2.0
";

    fn a15_parser() -> BatteryDrainParser {
        let mut uid_to_pkg = HashMap::new();
        uid_to_pkg.insert(10_379, PackageName("com.foreground.app".into()));
        uid_to_pkg.insert(10_044, PackageName("com.spotify.music".into()));
        BatteryDrainParser {
            uid_to_pkg,
            live_wakelock_pkgs: Default::default(),
            zombie_pkgs: Default::default(),
        }
    }

    #[test]
    fn a15_parses_capacity_and_uppercase_uid_entries() {
        let out = a15_parser().parse(A15_FORMAT).unwrap();
        assert_eq!(out.capacity_mah, Some(4716));
        assert!((out.computed_drain_mah - 1356.0).abs() < 0.001);
        let fg = out.entries.iter().find(|e| e.uid == 10_379).expect("uppercase UID u0a379 parsed");
        assert!((fg.drain_mah - 153.0).abs() < 0.001);
        assert_eq!(out.entries.iter().find(|e| e.uid == 1000).map(|e| e.drain_mah), Some(48.6));
    }

    #[test]
    fn a15_captures_continuation_line_breakdown() {
        let out = a15_parser().parse(A15_FORMAT).unwrap();
        let fg = out.entries.iter().find(|e| e.uid == 10_379).unwrap();
        assert!(fg.breakdown.get("cpu").copied().unwrap_or(0.0) > 0.0);
        assert!(fg.breakdown.get("screen").copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn a15_classifies_media_from_continuation() {
        let out = a15_parser().parse(A15_FORMAT).unwrap();
        let media = out.entries.iter().find(|e| e.uid == 10_044).unwrap();
        assert_eq!(media.verdict, AppDrainVerdict::LegitimateMedia);
    }

    // ── Verbatim Pixel 8 Pro / Android 15 slice (real `--charged` dump) ───────
    // Captures two format quirks the synthetic fixtures didn't:
    //   1. System UID with only `bg:` and no fg/cached  (`UID 1051: 301 bg: 301`)
    //   2. Breakdown values followed by a parenthesised duration
    //      (`audio=18.8 (11m 57s 829ms)`) — the token regex must read 18.8, not
    //      choke on the trailing `(...)`.
    const PIXEL8_REAL: &str = "\
  Estimated power use (mAh):
    Capacity: 4716, Computed drain: 3050, actual drain: 3050
    Global
    screen: 191 apps: 191
    cpu: 551 apps: 761 duration: 12h 59m 7s 168ms
  UID 1051: 301 bg: 301
      mobile_radio=301 mobile_radio:bg=301 wifi=0.173 wifi:bg=0.173
  UID u0a301: 260 fg: 36.6 (13m 15s 307ms) bg: 79.9 (27m 26s 698ms) cached: 128 (17h 13m 7s 570ms)
      screen=15.7 cpu=18.7 cpu:fg=12.7 cpu:bg=6.02 audio=18.8 (11m 57s 829ms) video=3.35 (8m 3s 80ms) wifi=0.871 wakelock=202 (17h 56m 52s 710ms) GPU=1.05
  UID u0a307: 215 fg: 92.0 (55m 22s 725ms) bg: 20.3 (3h 9m 14s 523ms) cached: 31.2 (17h 55m 9s 989ms)
      screen=71.0 cpu=98.8 audio=13.0 (8m 17s 525ms) video=14.3 (34m 14s 229ms) wifi=1.26 GPU=16.2
";

    // Regression: a later section ("Per-app mobile ms per packet:") repeats
    // `Uid 1051:` with a different meaning. The parser must NOT leak it into the
    // drain list — that caused duplicate UIDs and crashed the keyed UI list.
    const TRAILING_SECTION_LEAK: &str = "\
  Estimated power use (mAh):
    Capacity: 4716, Computed drain: 3050, actual drain: 3050
  UID 1051: 301 bg: 301
      mobile_radio=301 wifi=0.173
  UID u0a301: 260 fg: 36.6
      cpu=18.7 audio=18.8 (11m 57s 829ms)
  Per-app mobile ms per packet:
    Uid 1051: 113 (382 packets over 43s 275ms) 6x
    Uid u0a301: 88 (200 packets over 20s) 4x
";

    #[test]
    fn does_not_leak_uids_from_trailing_sections() {
        let p = BatteryDrainParser {
            uid_to_pkg: HashMap::new(),
            live_wakelock_pkgs: Default::default(),
            zombie_pkgs: Default::default(),
        };
        let out = p.parse(TRAILING_SECTION_LEAK).unwrap();
        // Exactly two real entries; no duplicate UIDs.
        assert_eq!(out.entries.len(), 2);
        let count_1051 = out.entries.iter().filter(|e| e.uid == 1051).count();
        assert_eq!(count_1051, 1, "UID 1051 must appear once, not leak from the packet section");
        // The value must be the drain (301), not the packet-section number (113).
        let e = out.entries.iter().find(|e| e.uid == 1051).unwrap();
        assert!((e.drain_mah - 301.0).abs() < 0.001);
    }

    #[test]
    fn pixel8_real_parses_all_entries_and_durations() {
        let mut uid_to_pkg = HashMap::new();
        uid_to_pkg.insert(10_301, PackageName("com.media.app".into()));
        let p = BatteryDrainParser {
            uid_to_pkg,
            live_wakelock_pkgs: Default::default(),
            zombie_pkgs: Default::default(),
        };
        let out = p.parse(PIXEL8_REAL).unwrap();

        assert_eq!(out.capacity_mah, Some(4716));
        assert!((out.computed_drain_mah - 3050.0).abs() < 0.001);
        // All three real UIDs (system 1051 + two apps) survive parsing.
        assert!(out.entries.iter().any(|e| e.uid == 1051));
        assert!(out.entries.iter().any(|e| e.uid == 10_301));
        assert!(out.entries.iter().any(|e| e.uid == 10_307));

        // Breakdown value with trailing (duration) must read the number.
        let media = out.entries.iter().find(|e| e.uid == 10_301).unwrap();
        assert!((media.breakdown.get("audio").copied().unwrap_or(0.0) - 18.8).abs() < 0.01);
        assert!((media.drain_mah - 260.0).abs() < 0.001);
    }
}
