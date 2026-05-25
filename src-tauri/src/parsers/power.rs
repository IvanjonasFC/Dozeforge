//! `dumpsys power` - currently held wakelocks.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{LiveWakelock, PackageName, Parser};

static WL_ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?P<flags>\w+_WAKE_LOCK)\s+'(?P<tag>[^']*)'\s*(?:\(uid=(?P<uid>\d+)\s+pid=\d+\))?",
    )
    .unwrap()
});

pub struct PowerParser;

impl Parser for PowerParser {
    type Output = Vec<LiveWakelock>;

    fn parse(&self, input: &str) -> Result<Vec<LiveWakelock>> {
        let mut inside = false;
        let mut out = Vec::new();

        for line in input.lines() {
            if line.contains("Wake Locks:") || line.contains("mWakeLocks") {
                inside = true;
                continue;
            }
            if inside && line.trim_start().starts_with("Suspend Blockers:") {
                break;
            }
            if !inside { continue; }
            if let Some(caps) = WL_ROW.captures(line) {
                out.push(LiveWakelock {
                    tag: caps["tag"].to_string(),
                    flags: caps["flags"].to_string(),
                    package: caps
                        .name("uid")
                        .and_then(|m| m.as_str().parse::<u32>().ok())
                        .map(|uid| PackageName(format!("uid:{uid}"))),
                });
            }
        }

        Ok(out)
    }
}
