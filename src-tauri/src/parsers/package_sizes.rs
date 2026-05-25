//! Per-package APK size scanner.
//!
//! Re-written to avoid spawning `stat` on Android 1000 times.
//! Now takes the output of `pm list packages -f` and `du -sk` 
//! from common app directories, joining them in Rust.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::{Error, Result};
use crate::parsers::PackageName;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSize {
    pub package: PackageName,
    pub apk_bytes: u64,
    pub split_count: u32,
}

pub struct PackageSizesScanner;

impl PackageSizesScanner {
    pub fn parse(pm_raw: &str, du_raw: &str) -> Result<Vec<PackageSize>> {
        let trimmed_pm = pm_raw.trim();
        if trimmed_pm.is_empty() {
            return Err(Error::Parse {
                parser_name: "package_sizes",
                reason: "empty output from `pm list packages -f`".into(),
            });
        }

        // 1. Parse du_raw into a map of directory -> bytes
        let mut dir_sizes: HashMap<String, u64> = HashMap::new();
        for line in du_raw.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(kb) = parts[0].parse::<u64>() {
                    let dir = parts[1..].join(" ");
                    dir_sizes.insert(dir, kb * 1024);
                }
            }
        }

        // 2. Parse pm_raw
        let mut seen: HashMap<String, PackageSize> = HashMap::new();
        
        for line in pm_raw.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            
            // package:/data/app/~~ABC==/com.foo-XYZ==/base.apk=com.foo
            let Some(no_prefix) = line.strip_prefix("package:") else { continue };
            let Some((path_str, pkg)) = no_prefix.rsplit_once('=') else { continue };
            
            // extract directory
            let Some((dir, _)) = path_str.rsplit_once('/') else { continue };
            
            let bytes = dir_sizes.get(dir).copied().unwrap_or(0);
            
            let entry = seen.entry(pkg.to_string()).or_insert(PackageSize {
                package: PackageName(pkg.to_string()),
                apk_bytes: bytes,
                split_count: 1, // We don't have exact split count without `ls`, default to 1.
            });
            if bytes > entry.apk_bytes {
                entry.apk_bytes = bytes;
            }
        }

        if seen.is_empty() {
            return Err(Error::Parse {
                parser_name: "package_sizes",
                reason: "no valid package entries found".into(),
            });
        }

        let mut out: Vec<PackageSize> = seen.into_values().collect();
        out.sort_by(|a, b| b.apk_bytes.cmp(&a.apk_bytes));
        Ok(out)
    }
}

