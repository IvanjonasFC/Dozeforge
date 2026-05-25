//! `dumpsys jobscheduler` parser.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::{PackageName, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAttribution {
    pub package: PackageName,
    pub job_count: u32,
    pub periodic_count: u32,
}

static JOB_HEADER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"JOB #u0a\d+/\d+:.*?\s(?P<pkg>[\w.]+)/").unwrap()
});

static PERIODIC_FLAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"Periodic: true").unwrap());

pub struct JobSchedulerParser;

impl Parser for JobSchedulerParser {
    type Output = Vec<JobAttribution>;

    fn parse(&self, input: &str) -> Result<Vec<JobAttribution>> {
        let mut by_pkg: HashMap<PackageName, JobAttribution> = HashMap::new();
        let mut current_pkg: Option<PackageName> = None;

        for line in input.lines() {
            if let Some(caps) = JOB_HEADER.captures(line) {
                let pkg = PackageName::from(&caps["pkg"]);
                current_pkg = Some(pkg.clone());
                let entry = by_pkg.entry(pkg.clone()).or_insert(JobAttribution {
                    package: pkg, job_count: 0, periodic_count: 0,
                });
                entry.job_count = entry.job_count.saturating_add(1);
                continue;
            }
            if PERIODIC_FLAG.is_match(line) {
                if let Some(pkg) = current_pkg.clone() {
                    if let Some(entry) = by_pkg.get_mut(&pkg) {
                        entry.periodic_count = entry.periodic_count.saturating_add(1);
                    }
                }
            }
        }

        let mut out: Vec<JobAttribution> = by_pkg.into_values().collect();
        out.sort_by(|a, b| b.job_count.cmp(&a.job_count));
        Ok(out)
    }
}
