//! Standby-bucket parser.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{Parser, PackageName, StandbyAssignment, StandbyBucket};

static ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"package=(?P<pkg>[\w.]+)\s+(?:type=\S+\s+)?(?:.*?\s)?(?:group|standby_bucket)=(?P<bucket>\d+)")
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
