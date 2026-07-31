//! Standby-bucket parser.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{Parser, PackageName, StandbyAssignment, StandbyBucket};

// Tolerant across ROMs / API levels. The bucket key varies: `standby_bucket=`,
// `standbyBucket=`, `appStandbyBucket=`, `bucket=` (all end in `bucket=`), or the
// legacy `group=`. Case-insensitive; matches any `…bucket=NN` or `group=NN` that
// appears on the same line as the `package=` token.
static ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)package=(?P<pkg>[\w.]+)[^\n]*?(?:bucket|group)=(?P<bucket>\d+)")
        .unwrap()
});

pub struct StandbyParser;

impl Parser for StandbyParser {
    type Output = Vec<StandbyAssignment>;

    fn parse(&self, input: &str) -> Result<Vec<StandbyAssignment>> {
        let mut latest: HashMap<PackageName, StandbyBucket> = HashMap::new();
        for line in input.lines() {
            if let Some(caps) = ROW.captures(line) {
                let pkg = PackageName::from(&caps["pkg"]);
                let raw: i32 = caps["bucket"].parse().unwrap_or(50);
                if let Some(bucket) = StandbyBucket::from_raw(raw) {
                    latest.insert(pkg, bucket);
                }
            }
        }
        Ok(latest
            .into_iter()
            .map(|(package, bucket)| StandbyAssignment { package, bucket })
            .collect())
    }
}
