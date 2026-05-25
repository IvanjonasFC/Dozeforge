//! `dumpsys sensorservice` parser.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::{PackageName, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorClient {
    pub package: PackageName,
    pub sensors: Vec<String>,
}

static PACKAGE_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)package\s*=\s*(?P<pkg>[\w.]+)").unwrap());

static SENSOR_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)sensor\s*name\s*=\s*(?P<name>.+)$").unwrap());

pub struct SensorServiceParser;

impl Parser for SensorServiceParser {
    type Output = Vec<SensorClient>;

    fn parse(&self, input: &str) -> Result<Vec<SensorClient>> {
        let mut by_pkg: HashMap<PackageName, Vec<String>> = HashMap::new();
        let mut current: Option<PackageName> = None;

        for line in input.lines() {
            if let Some(caps) = PACKAGE_LINE.captures(line) {
                current = Some(PackageName::from(&caps["pkg"]));
                by_pkg.entry(current.clone().unwrap()).or_default();
                continue;
            }
            if let Some(caps) = SENSOR_LINE.captures(line) {
                if let Some(pkg) = current.clone() {
                    by_pkg.entry(pkg).or_default().push(caps["name"].trim().to_string());
                }
            }
        }

        Ok(by_pkg.into_iter().map(|(package, sensors)| SensorClient { package, sensors }).collect())
    }
}
