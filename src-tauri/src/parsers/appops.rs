//! `cmd appops get <package>` parser.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{AppOpMode, AppOpState, PackageName, Parser};

static OP_ROW: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?P<op>[A-Z_]+)\s*:\s*(?P<mode>\w+)").unwrap());

pub struct AppOpsParser {
    pub package: PackageName,
}

impl Parser for AppOpsParser {
    type Output = Vec<AppOpState>;

    fn parse(&self, input: &str) -> Result<Vec<AppOpState>> {
        let mut out = Vec::new();
        for line in input.lines() {
            if let Some(caps) = OP_ROW.captures(line) {
                if let Some(mode) = AppOpMode::from_raw(&caps["mode"]) {
                    out.push(AppOpState {
                        package: self.package.clone(),
                        op: caps["op"].to_string(),
                        mode,
                    });
                }
            }
        }
        Ok(out)
    }
}
