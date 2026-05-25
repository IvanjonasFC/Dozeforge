//! Package metadata parser.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::{InstalledPackage, PackageName, Parser};

static FU_ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^package:(?P<path>\S+\.apk)=(?P<pkg>[\w.]+)(?:\s+uid:(?P<uid>\d+))?")
        .unwrap()
});

static U_ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^package:(?P<pkg>[\w.]+)\s+uid:(?P<uid>\d+)").unwrap()
});

pub struct PackageListParser;

impl Parser for PackageListParser {
    type Output = Vec<InstalledPackage>;

    fn parse(&self, input: &str) -> Result<Vec<InstalledPackage>> {
        let mut by_pkg: HashMap<PackageName, InstalledPackage> = HashMap::new();
        for line in input.lines() {
            if let Some(caps) = FU_ROW.captures(line) {
                let pkg = PackageName::from(&caps["pkg"]);
                let path = caps["path"].to_string();
                let uid: i32 = caps
                    .name("uid")
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(-1);
                let is_system = path.starts_with("/system")
                    || path.starts_with("/vendor")
                    || path.starts_with("/product")
                    || path.starts_with("/apex");
                by_pkg.insert(pkg.clone(), InstalledPackage {
                    name: pkg, uid, install_path: path, is_system, label: None,
                });
            } else if let Some(caps) = U_ROW.captures(line) {
                let pkg = PackageName::from(&caps["pkg"]);
                let uid: i32 = caps["uid"].parse().unwrap_or(-1);
                by_pkg
                    .entry(pkg.clone())
                    .and_modify(|p| { if p.uid < 0 { p.uid = uid; } })
                    .or_insert(InstalledPackage {
                        name: pkg, uid, install_path: String::new(),
                        is_system: false, label: None,
                    });
            }
        }

        Ok(by_pkg.into_values().collect())
    }
}
