//! `dumpsys usagestats` parser.
//!
//! We care about `lastTimeUsed` per package. Crossing this with the standby
//! bucket tells us which apps are miscategorized (e.g. sitting in ACTIVE
//! despite not being opened in 5+ days).

use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::parsers::PackageName;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub package: PackageName,
    /// UTC timestamp of the last `lastTimeUsed`.
    pub last_time_used: Option<DateTime<Utc>>,
    pub total_time_used_ms: Option<u64>,
}

/// `package=com.example.app` ... `lastTimeUsed="2026-05-22 12:15:33"` ... `totalTimeUsed="00:23:45"`
static PACKAGE_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"package=(?P<pkg>[\w.]+)"#).unwrap()
});
static LAST_USED_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"lastTimeUsed="(?P<ts>[\d\-: ]+)""#).unwrap()
});
static TOTAL_USED_LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"totalTimeUsed="(?P<dur>[\d:]+)""#).unwrap()
});

pub struct UsageStatsParser;

impl UsageStatsParser {
    pub fn command() -> &'static str {
        "dumpsys usagestats"
    }

    pub fn parse(input: &str) -> Result<Vec<UsageEntry>> {
        // Pre-process: split into per-package logical blocks. usagestats output
        // can have one entry across multiple lines or compressed onto one line.
        let mut entries: HashMap<String, UsageEntry> = HashMap::new();

        for line in input.lines() {
            let Some(pkg_match) = PACKAGE_LINE.captures(line) else { continue };
            let pkg = pkg_match["pkg"].to_string();

            let last_time_used = LAST_USED_LINE
                .captures(line)
                .and_then(|c| parse_dt(&c["ts"]));
            let total_time_used_ms = TOTAL_USED_LINE
                .captures(line)
                .and_then(|c| parse_duration_to_ms(&c["dur"]));

            let pname = PackageName(pkg.clone());
            entries
                .entry(pkg)
                .and_modify(|e| {
                    // Keep the latest lastTimeUsed
                    if let Some(new_ts) = last_time_used {
                        if e.last_time_used.map(|old| new_ts > old).unwrap_or(true) {
                            e.last_time_used = Some(new_ts);
                        }
                    }
                    if let Some(d) = total_time_used_ms {
                        e.total_time_used_ms = Some(e.total_time_used_ms.unwrap_or(0).max(d));
                    }
                })
                .or_insert(UsageEntry {
                    package: pname,
                    last_time_used,
                    total_time_used_ms,
                });
        }

        Ok(entries.into_values().collect())
    }
}

fn parse_dt(raw: &str) -> Option<DateTime<Utc>> {
    // "2026-05-22 12:15:33" — assume device local time and treat as UTC.
    // This is "good enough" for last-used heuristics measured in days.
    NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|ndt| Utc.from_utc_datetime(&ndt))
}

fn parse_duration_to_ms(raw: &str) -> Option<u64> {
    // "00:23:45" -> ms
    let parts: Vec<&str> = raw.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let s: u64 = parts[2].parse().ok()?;
    Some(((h * 3600) + (m * 60) + s) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_block() {
        let input = r#"
In-memory daily stats
  time="2026-05-22-14-30-00" type=DAILY count=1
    package=com.example.spammer totalTimeUsed="00:45:30" lastTimeUsed="2026-05-22 12:15:33"
    package=com.example.dormant totalTimeUsed="00:00:00" lastTimeUsed="2026-05-15 08:00:00"
"#;
        let out = UsageStatsParser::parse(input).unwrap();
        assert_eq!(out.len(), 2);
        let spammer = out.iter().find(|e| e.package.as_str() == "com.example.spammer").unwrap();
        assert!(spammer.last_time_used.is_some());
        assert_eq!(spammer.total_time_used_ms, Some((45 * 60 + 30) * 1000));
    }

    #[test]
    fn parses_duration() {
        assert_eq!(parse_duration_to_ms("01:23:45"), Some(((3600 + 23 * 60 + 45) * 1000)));
        assert_eq!(parse_duration_to_ms("invalid"), None);
    }
}
