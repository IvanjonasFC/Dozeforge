//! `dumpsys batterystats --checkin` parser.
//!
//! Two responsibilities:
//!   1. Build a UID -> package name map from `apk` lines.
//!   2. Aggregate partial-wakelock totals per UID from `pwl` / `wl` lines.
//!
//! The checkin format is a compact CSV (one row per fact). See:
//!   frameworks/base/core/java/android/os/BatteryStats.java#dumpCheckinLocked
//!
//! Field layout for `pwl` (partial-wakelock) rows:
//!   [0]=ver, [1]=uid, [2]=l, [3]=pwl, [4]=name,
//!   [5]=full_total_ms, [6]=full_count,
//!   [7]=partial_total_ms, [8]=partial_count,
//!   [9]=window_total_ms, [10]=window_count, ...
//!
//! On API 33+ the same data appears under `wl` rows with the same layout,
//! so we accept both tags.

use std::collections::HashMap;

use crate::error::{Error, Result};

use super::{Parser, PackageName, WakelockEntry};

pub struct BatteryStatsParser {
    pub api_level: u32,
}

impl BatteryStatsParser {
    pub fn for_api(api_level: u32) -> Self {
        Self { api_level }
    }
}

impl Parser for BatteryStatsParser {
    type Output = Vec<WakelockEntry>;

    fn parse(&self, input: &str) -> Result<Vec<WakelockEntry>> {
        let uid_to_pkg = build_uid_to_package_map(input);

        // Aggregate partial wakelock totals per UID.
        let mut acc: HashMap<i32, (u64, u32)> = HashMap::new();
        for line in input.lines() {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 9 {
                continue;
            }
            let tag = match cols.get(3) {
                Some(t) => *t,
                None => continue,
            };
            if tag != "pwl" && tag != "wl" {
                continue;
            }
            let uid: i32 = match cols[1].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let partial_ms: u64 = cols.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
            let partial_count: u32 = cols.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);
            // Fallback: some vendor builds emit only fields 5-6 (full).
            // Only use them if partial slot is empty.
            let (ms, n) = if partial_ms == 0 && partial_count == 0 {
                let full_ms: u64 = cols.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                let full_count: u32 = cols.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                (full_ms, full_count)
            } else {
                (partial_ms, partial_count)
            };
            let entry = acc.entry(uid).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(ms);
            entry.1 = entry.1.saturating_add(n);
        }

        let mut out: Vec<WakelockEntry> = acc
            .into_iter()
            .filter_map(|(uid, (total_ms, count))| {
                let package = uid_to_pkg.get(&uid).cloned()?;
                Some(WakelockEntry { package, uid, total_ms, count })
            })
            .collect();

        out.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));

        if out.is_empty() && !input.contains("apk,") {
            return Err(Error::Parse {
                parser_name: "batterystats",
                reason: "no `apk` rows found; output may be truncated".into(),
            });
        }

        Ok(out)
    }
}

/// Public helper — exported so other parsers (BatteryDrainParser, etc.) can
/// reuse the same UID -> package resolution without re-walking the input.
pub fn build_uid_to_package_map(input: &str) -> HashMap<i32, PackageName> {
    let mut map: HashMap<i32, PackageName> = HashMap::new();
    for line in input.lines() {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 6 {
            continue;
        }
        if cols.get(3) != Some(&"apk") {
            continue;
        }
        let uid: i32 = match cols[1].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(pkg) = cols.get(5) {
            map.entry(uid).or_insert_with(|| PackageName::from(*pkg));
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
9,0,i,vers,28,189,FAKE,FAKE
9,10086,l,apk,1,com.google.android.gms,com.google.android.gms.gcm.GcmService,123,456
9,10211,l,apk,1,com.example.spammer,com.example.spammer.PushService,7,8
9,10211,l,pwl,SyncService,0,0,3600000,42,0,0,0,0,0,0,0
9,10086,l,pwl,GcmService,0,0,1800000,18,0,0,0,0,0,0,0
";

    #[test]
    fn aggregates_pwl_by_uid() {
        let parser = BatteryStatsParser::for_api(34);
        let out = parser.parse(SAMPLE).expect("parse ok");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].package.as_str(), "com.example.spammer");
        assert_eq!(out[0].total_ms, 3_600_000);
        assert_eq!(out[0].count, 42);
        assert_eq!(out[1].package.as_str(), "com.google.android.gms");
    }

    #[test]
    fn empty_input_errors() {
        let parser = BatteryStatsParser::for_api(34);
        assert!(parser.parse("").is_err());
    }

    #[test]
    fn builds_uid_package_map() {
        let map = build_uid_to_package_map(SAMPLE);
        assert_eq!(map.get(&10086).map(|p| p.as_str()), Some("com.google.android.gms"));
        assert_eq!(map.get(&10211).map(|p| p.as_str()), Some("com.example.spammer"));
    }

    #[test]
    fn accepts_wl_tag_as_alias() {
        let sample = "\
9,0,i,vers,28,189,FAKE,FAKE
9,10086,l,apk,1,com.example.foo,com.example.foo.Svc,0,0
9,10086,l,wl,Foo,0,0,500000,10,0,0,0,0,0,0,0
";
        let out = BatteryStatsParser::for_api(34).parse(sample).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].total_ms, 500_000);
    }
}
