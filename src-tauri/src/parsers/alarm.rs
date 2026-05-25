//! `dumpsys alarm` parser. Credits wakeups to OWNER (sourcePackage), not target.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{AlarmAttribution, AlarmKind, PackageName, Parser};

static BATCH_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*Batch\{").unwrap());

static ALARM_HEAD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?P<kind>RTC_WAKEUP|ELAPSED_REALTIME_WAKEUP|RTC|ELAPSED_REALTIME)\s+#\d+:")
        .unwrap()
});

static PENDING_INTENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"PendingIntentRecord\{[^}]*?\s(?P<pkg>[\w.]+)\s*[}/ ]").unwrap());

static OPERATION_PKG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"operation=PendingIntent\{[^}]*?:\s*(?P<pkg>[\w.]+)").unwrap());

static STATS_HEADER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*Statistics over .* for (?P<pkg>[\w.]+):").unwrap());

static WAKEUPS_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?P<wakeups>\d+) wakeups?(?:\s|,)").unwrap());

pub struct AlarmParser;

impl Parser for AlarmParser {
    type Output = Vec<AlarmAttribution>;

    fn parse(&self, input: &str) -> Result<Vec<AlarmAttribution>> {
        let mut state = ParseState::default();
        let mut by_owner: HashMap<PackageName, AlarmAggregate> = HashMap::new();

        for line in input.lines() {
            if BATCH_LINE.is_match(line) {
                state = ParseState::default();
                continue;
            }

            if let Some(caps) = ALARM_HEAD.captures(line) {
                state.kind = AlarmKind::from_raw(&caps["kind"]);
                state.target = None;
                state.owner = None;
                continue;
            }

            if let Some(caps) = PENDING_INTENT.captures(line) {
                state.target = Some(PackageName::from(&caps["pkg"]));
            }
            if let Some(caps) = OPERATION_PKG.captures(line) {
                state.owner = Some(PackageName::from(&caps["pkg"]));
            }

            if line.trim().is_empty() {
                if let (Some(target), Some(owner), Some(kind)) =
                    (state.target.clone(), state.owner.clone(), state.kind)
                {
                    let entry = by_owner.entry(owner.clone()).or_insert(AlarmAggregate {
                        target, owner, kind, wake_count: 0,
                    });
                    entry.wake_count = entry.wake_count.saturating_add(1);
                }
                state = ParseState::default();
            }

            if let Some(caps) = STATS_HEADER.captures(line) {
                state.current_stats_pkg = Some(PackageName::from(&caps["pkg"]));
                continue;
            }
            if let Some(pkg) = state.current_stats_pkg.clone() {
                if let Some(caps) = WAKEUPS_LINE.captures(line) {
                    let n: u32 = caps["wakeups"].parse().unwrap_or(0);
                    let entry = by_owner.entry(pkg.clone()).or_insert(AlarmAggregate {
                        target: pkg.clone(), owner: pkg.clone(),
                        kind: AlarmKind::WakeupRtc, wake_count: 0,
                    });
                    entry.wake_count = entry.wake_count.max(n);
                    state.current_stats_pkg = None;
                }
            }
        }

        let mut out: Vec<AlarmAttribution> = by_owner
            .into_values()
            .map(|a| AlarmAttribution {
                target_package: a.target,
                triggering_package: a.owner,
                kind: a.kind,
                wake_count: a.wake_count,
            })
            .collect();
        out.sort_by(|a, b| b.wake_count.cmp(&a.wake_count));
        Ok(out)
    }
}

impl AlarmKind {
    fn from_raw(s: &str) -> Option<Self> {
        match s {
            "RTC_WAKEUP" => Some(Self::WakeupRtc),
            "ELAPSED_REALTIME_WAKEUP" => Some(Self::WakeupElapsed),
            "RTC" | "ELAPSED_REALTIME" => Some(Self::NonWakeup),
            _ => None,
        }
    }
}

#[derive(Default)]
struct ParseState {
    kind: Option<AlarmKind>,
    target: Option<PackageName>,
    owner: Option<PackageName>,
    current_stats_pkg: Option<PackageName>,
}

struct AlarmAggregate {
    target: PackageName,
    owner: PackageName,
    kind: AlarmKind,
    wake_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "
Batch{xyz num=1}
  RTC_WAKEUP #0: Alarm{aaa type 0 when 1 namespace:tag:com.google.android.gms}
    tag=*alarm*:com.google.android.gms/.AlarmReceiver
    operation=PendingIntent{bbb: PendingIntentRecord{ccc com.example.spammer}}

Statistics over last 24h for com.example.spammer:
  150 wakeups,  3 alarms total
";

    #[test]
    fn attributes_to_owner_not_target() {
        let parser = AlarmParser;
        let out = parser.parse(SAMPLE).expect("parse ok");
        assert!(!out.is_empty());
        let spam = out.iter().find(|a| a.triggering_package.as_str() == "com.example.spammer");
        assert!(spam.is_some());
        assert_eq!(spam.unwrap().wake_count, 150);
    }
}
