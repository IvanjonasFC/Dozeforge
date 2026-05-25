use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UidIoStat {
    pub uid: String,
    pub fg_read_bytes: u64,
    pub fg_write_bytes: u64,
    pub bg_read_bytes: u64,
    pub bg_write_bytes: u64,
}

pub struct IoStatsParser;

impl IoStatsParser {
    pub fn parse(input: &str) -> Result<Vec<UidIoStat>> {
        let mut stats = Vec::new();
        // Typically: uid fg_rchars fg_wchars fg_syscr fg_syscw fg_rbytes fg_wbytes bg_rchars bg_wchars bg_syscr bg_syscw bg_rbytes bg_wbytes fg_fsync bg_fsync
        for line in input.lines() {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 11 {
                let uid = parts[0].to_string();
                if uid == "uid" { continue; } // header
                
                let fg_read_bytes = parts[5].parse().unwrap_or(0);
                let fg_write_bytes = parts[6].parse().unwrap_or(0);
                let bg_read_bytes = parts[11].parse().unwrap_or(0);
                let bg_write_bytes = parts[12].parse().unwrap_or(0);

                stats.push(UidIoStat {
                    uid,
                    fg_read_bytes,
                    fg_write_bytes,
                    bg_read_bytes,
                    bg_write_bytes,
                });
            }
        }
        Ok(stats)
    }
}
