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
    // ── Category breakdown (bytes), from the "X Size:" lines that modern
    // Android 12+ `dumpsys diskstats` emits. Lets the UI show what's actually
    // eating space without a per-app scan. `app_cache_size_bytes` is the total
    // reclaimable app cache across the device.
    pub app_size_bytes: Option<u64>,
    pub app_data_size_bytes: Option<u64>,
    pub app_cache_size_bytes: Option<u64>,
    pub photos_size_bytes: Option<u64>,
    pub videos_size_bytes: Option<u64>,
    pub audio_size_bytes: Option<u64>,
    pub downloads_size_bytes: Option<u64>,
    pub system_size_bytes: Option<u64>,
    pub other_size_bytes: Option<u64>,
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

            // ── Legacy format (older AOSP/Pixel): "Cache Free Bytes: N" (bytes)
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

            // ── Modern format (Android 12+, Nothing/Samsung/Pixel today):
            //    "Data-Free: 201885332K / 485805756K total = 41% free"  (KiB)
            } else if let Some(v) = line.strip_prefix("Data-Free:") {
                let (free, total) = parse_free_total_kib(v);
                s.data_free_bytes = free.or(s.data_free_bytes);
                s.data_total_bytes = total.or(s.data_total_bytes);
            } else if let Some(v) = line.strip_prefix("Cache-Free:") {
                let (free, total) = parse_free_total_kib(v);
                s.cache_free_bytes = free.or(s.cache_free_bytes);
                s.cache_total_bytes = total.or(s.cache_total_bytes);
            } else if let Some(v) = line.strip_prefix("System-Free:") {
                let (free, total) = parse_free_total_kib(v);
                s.system_free_bytes = free.or(s.system_free_bytes);
                s.system_total_bytes = total.or(s.system_total_bytes);

            // ── Category breakdown (bytes), modern format only.
            } else if let Some(v) = line.strip_prefix("App Cache Size:") {
                s.app_cache_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("App Data Size:") {
                s.app_data_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("App Size:") {
                s.app_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Photos Size:") {
                s.photos_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Videos Size:") {
                s.videos_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Audio Size:") {
                s.audio_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Downloads Size:") {
                s.downloads_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("System Size:") {
                s.system_size_bytes = parse_u64(v);
            } else if let Some(v) = line.strip_prefix("Other Size:") {
                s.other_size_bytes = parse_u64(v);

            // ── Common to both formats.
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

/// Parse the "X-Free: <free>K / <total>K total = NN% free" shape into
/// (free_bytes, total_bytes). Values are in KiB; we normalise to bytes.
fn parse_free_total_kib(raw: &str) -> (Option<u64>, Option<u64>) {
    // e.g. " 201885332K / 485805756K total = 41% free"
    let kib = |tok: &str| -> Option<u64> {
        tok.trim()
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse::<u64>()
            .ok()
            .map(|k| k * 1024)
    };
    let mut parts = raw.split('/');
    let free = parts.next().and_then(kib);
    let total = parts
        .next()
        .and_then(|t| t.split_whitespace().next())
        .and_then(kib);
    (free, total)
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

    // Real Nothing Phone (2a) / Android 15 `dumpsys diskstats` — the modern
    // "X-Free: NK / NK total" + "X Size:" breakdown format the legacy parser
    // couldn't read at all.
    const NOTHING_MODERN: &str = "\
Latency: 1ms [512B Data Write]
Recent Disk Write Speed (kB/s) = 37425
Data-Free: 201885332K / 485805756K total = 41% free
Cache-Free: 201885332K / 485805756K total = 41% free
System-Free: 0K / 1110828K total = 0% free
File-based Encryption: true
App Size: 26987196416
App Data Size: 60813024308
App Cache Size: 29726090240
Photos Size: 30384693248
Videos Size: 158214832128
Audio Size: 1269858304
Downloads Size: 0
System Size: 512000000000
Other Size: 13696401408
";

    #[test]
    fn parses_modern_nothing_format() {
        let s = DiskStatsParser::parse(NOTHING_MODERN).unwrap();
        // KiB → bytes normalisation.
        assert_eq!(s.data_total_bytes, Some(485_805_756 * 1024));
        assert_eq!(s.data_free_bytes, Some(201_885_332 * 1024));
        // The reclaimable cache the UI needs.
        assert_eq!(s.app_cache_size_bytes, Some(29_726_090_240));
        assert_eq!(s.app_size_bytes, Some(26_987_196_416));
        assert_eq!(s.videos_size_bytes, Some(158_214_832_128));
        assert_eq!(s.recent_write_speed_kb_s, Some(37425));
        assert_eq!(s.file_based_encryption, Some(true));
        // ~41% used derived correctly.
        let used_pct = s.data_used_percent().unwrap();
        assert!((used_pct - 58.4).abs() < 1.0, "got {used_pct}");
    }

    #[test]
    fn app_size_not_confused_with_app_data_or_cache() {
        let s = DiskStatsParser::parse(NOTHING_MODERN).unwrap();
        assert_eq!(s.app_size_bytes, Some(26_987_196_416));
        assert_eq!(s.app_data_size_bytes, Some(60_813_024_308));
        assert_eq!(s.app_cache_size_bytes, Some(29_726_090_240));
    }
}
