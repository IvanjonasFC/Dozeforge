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
            // We index up to column 12, so require at least 13 columns. The old
            // guard (>= 11) let `parts[11]`/`parts[12]` panic on shorter rows —
            // the exact kind of per-device format variance that must never crash.
            if parts.len() < 13 {
                continue;
            }
            let uid = parts[0].to_string();
            if uid == "uid" { continue; } // header

            // Safe accessors: a malformed row degrades to 0, never panics.
            let get = |i: usize| -> u64 { parts.get(i).and_then(|v| v.parse().ok()).unwrap_or(0) };
            stats.push(UidIoStat {
                uid,
                fg_read_bytes: get(5),
                fg_write_bytes: get(6),
                bg_read_bytes: get(11),
                bg_write_bytes: get(12),
            });
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_row() {
        // uid + 14 columns.
        let input = "10123 100 200 3 4 5000 6000 70 80 9 10 11000 12000 1 2\n";
        let s = IoStatsParser::parse(input).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].uid, "10123");
        assert_eq!(s[0].fg_read_bytes, 5000);
        assert_eq!(s[0].fg_write_bytes, 6000);
        assert_eq!(s[0].bg_read_bytes, 11000);
        assert_eq!(s[0].bg_write_bytes, 12000);
    }

    #[test]
    fn short_rows_do_not_panic() {
        // Rows with 11-12 columns previously indexed out of bounds and crashed.
        let input = "10001 1 2 3 4 5 6 7 8 9 10\n10002 1 2 3 4 5 6 7 8 9 10 11\nuid a b c d e f g h i j k l m\n";
        let s = IoStatsParser::parse(input).unwrap();
        // None have >=13 columns (or are the header) → skipped, no panic.
        assert!(s.is_empty());
    }
}
