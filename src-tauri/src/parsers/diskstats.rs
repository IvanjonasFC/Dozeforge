//! `dumpsys diskstats` parser.
//!
//! Returns the overall disk usage breakdown: Cache, System, Data partitions.
//! Per-app sizes are NOT exposed reliably in shell-readable form on Android
//! 12+ (the `AppSize:` field was deprecated for privacy); we compute those
//! separately in `package_sizes.rs`.

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskStats {
    pub cache_free_bytes: Option<u64>,
    pub cache_total_bytes: Option<u64>,
    pub system_free_bytes: Option<u64>,
    pub system_total_bytes: Option<u64>,
    pub data_free_bytes: Option<u64>,
    pub data_total_bytes: Option<u64>,
    /// Recent write speed in kB/s, reported by diskstats benchmark.
    pub recent_write_speed_kb_s: Option<u64>,
    /// Whether the device uses FBE (file-based encryption).
    pub file_based_encryption: Option<bool>,
}

impl DiskStats {
    pub fn data_used_bytes(&self) -> Option<u64> {
        match (self.data_total_bytes, self.data_free_bytes) {
            (Some(t), Some(f)) => Some(t.saturating_sub(f)),
            _ => None,
        }
    }
    pub fn data_used_percent(&self) -> Option<f32> {
        match (self.data_total_bytes, self.data_used_bytes()) {
            (Some(t), Some(u)) if t > 0 => Some((u as f32 / t as f32) * 100.0),
            _ => None,
        }
    }
}

pub struct DiskStatsParser;

impl DiskStatsParser {
    pub fn command() -> &'static str {
        "dumpsys diskstats"
    }

    pub fn parse(input: &str) -> Result<DiskStats> {
        let mut s = DiskStats::default();

        for line in input.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("Cache Free Bytes:") {
                s.cache_free_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Cache Total Bytes:") {
                s.cache_total_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("System Free Bytes:") {
                s.system_free_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("System Total Bytes:") {
                s.system_total_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Data Free Bytes:") {
                s.data_free_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Data Total Bytes:") {
                s.data_total_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Recent Disk Write Speed (kB/s) =") {
                s.recent_write_speed_kb_s = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("File-based Encryption:") {
                let val = v.trim().to_ascii_lowercase();
                s.file_based_encryption = Some(val == "true" || val == "yes");
            }
        }
        Ok(s)
    }
}

fn parse_u64(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_pixel_output() {
        let input = "Latency: 1ms [512B Data Write]
Recent Disk Write Speed (kB/s) = 8543
File-based Encryption: true
Cache Free Bytes: 12345678
Cache Total Bytes: 134217728
System Free Bytes: 5368709120
System Total Bytes: 9663676416
Data Free Bytes: 134217728000
Data Total Bytes: 256000000000
";
        let s = DiskStatsParser::parse(input).unwrap();
        assert_eq!(s.cache_free_bytes, Some(12_345_678));
        assert_eq!(s.data_total_bytes, Some(256_000_000_000));
        assert_eq!(s.recent_write_speed_kb_s, Some(8543));
        assert_eq!(s.file_based_encryption, Some(true));
        // 256GB total - 134GB free = 122GB used
        let used = s.data_used_bytes().unwrap();
        assert_eq!(used, 256_000_000_000 - 134_217_728_000);
    }

    #[test]
    fn handles_missing_fields() {
        let input = "Cache Total Bytes: 100\nSomething else: 42\n";
        let s = DiskStatsParser::parse(input).unwrap();
        assert_eq!(s.cache_total_bytes, Some(100));
        assert_eq!(s.cache_free_bytes, None);
    }
}
