//! `top -b -n 1` parser with zombie + hog detection.
//!
//! We invoke `top -b -n 1 -q -o PID,USER,S,%CPU,RSS,ARGS` and classify each
//! process by its state column:
//!   - `Z` (zombie) — terminated but not reaped by parent. Always reportable.
//!   - `R` (running) with %CPU > threshold — a "hog" if it sustains it.
//!   - `D` (uninterruptible sleep) — typically I/O wait; flagged for visibility.
//!   - others (S, T, I) — normal background.
//!
//! Hog detection is a snapshot heuristic. True hogs need continuous sampling
//! (`CpuSampler` in heuristics/) — this parser only flags candidates.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
    Running,         // R
    Sleeping,        // S
    UninterruptibleSleep, // D
    Zombie,          // Z
    Stopped,         // T
    Idle,            // I
    Unknown,
}

impl ProcessState {
    fn from_char(c: char) -> Self {
        match c {
            'R' => Self::Running,
            'S' => Self::Sleeping,
            'D' => Self::UninterruptibleSleep,
            'Z' => Self::Zombie,
            'T' | 't' => Self::Stopped,
            'I' => Self::Idle,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "R",
            Self::Sleeping => "S",
            Self::UninterruptibleSleep => "D",
            Self::Zombie => "Z",
            Self::Stopped => "T",
            Self::Idle => "I",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRow {
    pub pid: u32,
    pub user: String,
    pub state: ProcessState,
    pub cpu_percent: f32,
    pub rss_kb: u64,
    pub args: String,
    pub package: Option<String>,
    /// Snapshot flag: high CPU usage in this single sample.
    pub is_hog_candidate: bool,
    /// Smart Hog Detection
    pub is_smart_hog: bool,
    /// Definitive: process is in Z state.
    pub is_zombie: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub rows: Vec<ProcessRow>,
    pub zombie_count: u32,
    pub hog_candidate_count: u32,
    pub total_cpu_percent: f32,
    pub total_rss_kb: u64,
    pub cpu_user: f32,
    pub cpu_sys: f32,
    pub cpu_iowait: f32,
    pub mem_available_mb: u64,
    pub swap_free_mb: u64,
    pub swap_total_mb: u64,
    #[serde(skip)]
    pub raw_stat: Option<crate::ipc::streams::SystemStatsRaw>,
}

/// Threshold above which a single-sample %CPU is flagged as hog candidate.
pub const HOG_CPU_THRESHOLD: f32 = 15.0;

// Removed static regex in favor of dynamic header parsing

pub struct ProcessStatusParser;

impl ProcessStatusParser {
    pub fn command() -> &'static str {
        // Always use batch mode (-b). The old `|| top -n 1` fallback ran top in
        // interactive mode, whose ANSI/redraw output parsed into garbage rows
        // (stray `'`, `-s`, zeroed CPU/RSS). Every fallback now stays batch.
        "cat /proc/stat; echo '---'; cat /proc/meminfo; echo '---'; top -b -n 1 -o PID,USER,S,%CPU,RSS,ARGS 2>/dev/null || top -b -n 1 2>/dev/null"
    }

    pub fn parse(input: &str) -> Result<ProcessSnapshot> {
        let parts: Vec<&str> = input.split("---").collect();
        let stat_str = parts.get(0).copied().unwrap_or("");
        let mem_str = parts.get(1).copied().unwrap_or("");
        let top_str = parts.get(2).copied().unwrap_or(input);

        let mut rows = Vec::new();
        let mut zombie_count = 0u32;
        let mut hog_count = 0u32;
        let mut total_cpu = 0.0f32;
        let mut total_rss = 0u64;

        let mut idx_pid = None;
        let mut idx_user = None;
        let mut idx_s = None;
        let mut idx_cpu = None;
        let mut idx_rss = None;
        let mut idx_args = None;
        let mut header_found = false;

        for line in top_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if !header_found && line.contains("PID") && (line.contains("USER") || line.contains("UID") || line.contains("PR")) {
                header_found = true;
                let tokens: Vec<&str> = line.split_whitespace().collect();
                let mut data_col_idx = 0;
                for &t in tokens.iter() {
                    match t {
                        "PID" => idx_pid = Some(data_col_idx),
                        "USER" | "UID" => idx_user = Some(data_col_idx),
                        "S" | "STAT" => idx_s = Some(data_col_idx),
                        "%CPU" | "CPU%" => idx_cpu = Some(data_col_idx),
                        "RSS" | "RES" | "RPRVT" => idx_rss = Some(data_col_idx),
                        "ARGS" | "Name" | "CMD" | "COMMAND" | "PROG" => idx_args = Some(data_col_idx),
                        "S[%CPU]" => {
                            idx_s = Some(data_col_idx);
                            idx_cpu = Some(data_col_idx + 1);
                            data_col_idx += 1;
                        }
                        _ => {}
                    }
                    data_col_idx += 1;
                }
                continue;
            }

            if !header_found { continue; }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            let pid_idx = idx_pid.unwrap_or(0);
            if tokens.len() <= pid_idx { continue; }

            let pid_str = tokens[pid_idx];
            let Ok(pid) = pid_str.parse::<u32>() else { continue };
            if pid == 0 { continue; }

            let user = idx_user.and_then(|i| tokens.get(i)).unwrap_or(&"?").to_string();
            let state_str = idx_s.and_then(|i| tokens.get(i)).unwrap_or(&"?");
            let state_char = state_str.chars().next().unwrap_or('?');
            let state = ProcessState::from_char(state_char);

            let cpu_str = idx_cpu.and_then(|i| tokens.get(i)).unwrap_or(&"0").trim_end_matches('%');
            let cpu_percent: f32 = cpu_str.parse().unwrap_or(0.0);

            let rss_str = idx_rss.and_then(|i| tokens.get(i)).unwrap_or(&"0");
            let rss_kb: u64 = if rss_str.ends_with('M') || rss_str.ends_with('m') {
                rss_str.trim_end_matches(|c| c == 'M' || c == 'm').parse::<f64>().unwrap_or(0.0) as u64 * 1024
            } else if rss_str.ends_with('K') || rss_str.ends_with('k') {
                rss_str.trim_end_matches(|c| c == 'K' || c == 'k').parse().unwrap_or(0)
            } else if rss_str.ends_with('G') || rss_str.ends_with('g') {
                rss_str.trim_end_matches(|c| c == 'G' || c == 'g').parse::<f64>().unwrap_or(0.0) as u64 * 1024 * 1024
            } else {
                rss_str.parse().unwrap_or(0)
            };

            let args_idx = idx_args.unwrap_or(tokens.len().saturating_sub(1));
            let args = if args_idx < tokens.len() {
                tokens[args_idx..].join(" ")
            } else {
                String::new()
            };

            let package = extract_package(&args);
            let is_zombie = state == ProcessState::Zombie;
            let is_hog_candidate = state == ProcessState::Running && cpu_percent >= HOG_CPU_THRESHOLD;

            if is_zombie {
                zombie_count += 1;
            }
            if is_hog_candidate {
                hog_count += 1;
            }
            total_cpu += cpu_percent;
            total_rss += rss_kb;

            rows.push(ProcessRow {
                pid,
                user,
                state,
                cpu_percent,
                rss_kb,
                args,
                package,
                is_hog_candidate,
                is_smart_hog: false,
                is_zombie,
            });
        }

        rows.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));

        let mut mem_available_mb = 0;
        let mut swap_free_mb = 0;
        let mut swap_total_mb = 0;
        let mut mem_free = 0;
        let mut buffers = 0;
        let mut cached = 0;

        for line in mem_str.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("MemAvailable:") {
                if let Some(kb) = parse_kb(v) { mem_available_mb = kb / 1024; }
            } else if let Some(v) = line.strip_prefix("MemFree:") {
                if let Some(kb) = parse_kb(v) { mem_free = kb / 1024; }
            } else if let Some(v) = line.strip_prefix("Buffers:") {
                if let Some(kb) = parse_kb(v) { buffers = kb / 1024; }
            } else if let Some(v) = line.strip_prefix("Cached:") {
                if let Some(kb) = parse_kb(v) { cached = kb / 1024; }
            } else if let Some(v) = line.strip_prefix("SwapFree:") {
                if let Some(kb) = parse_kb(v) { swap_free_mb = kb / 1024; }
            } else if let Some(v) = line.strip_prefix("SwapTotal:") {
                if let Some(kb) = parse_kb(v) { swap_total_mb = kb / 1024; }
            }
        }

        if mem_available_mb == 0 {
            mem_available_mb = mem_free + buffers + cached;
        }

        let mut raw_stat = None;
        if let Some(line) = stat_str.lines().find(|l| l.starts_with("cpu ")) {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 8 {
                let user: u64 = tokens[1].parse().unwrap_or(0) + tokens[2].parse::<u64>().unwrap_or(0);
                let sys: u64 = tokens[3].parse().unwrap_or(0) + tokens[6].parse::<u64>().unwrap_or(0) + tokens[7].parse::<u64>().unwrap_or(0);
                let idle: u64 = tokens[4].parse().unwrap_or(0);
                let io: u64 = tokens[5].parse().unwrap_or(0);
                let steal: u64 = tokens.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);
                let guest: u64 = tokens.get(9).and_then(|s| s.parse().ok()).unwrap_or(0);
                let guest_nice: u64 = tokens.get(10).and_then(|s| s.parse().ok()).unwrap_or(0);
                let total = user + sys + idle + io + steal + guest + guest_nice;
                
                raw_stat = Some(crate::ipc::streams::SystemStatsRaw {
                    user, sys, io, idle, total,
                });
            }
        }

        Ok(ProcessSnapshot {
            rows,
            zombie_count,
            hog_candidate_count: hog_count,
            total_cpu_percent: total_cpu,
            total_rss_kb: total_rss,
            cpu_user: 0.0,
            cpu_sys: 0.0,
            cpu_iowait: 0.0,
            mem_available_mb,
            swap_free_mb,
            swap_total_mb,
            raw_stat,
        })
    }
}

fn parse_kb(raw: &str) -> Option<u64> {
    raw.trim().trim_end_matches("kB").trim().parse().ok()
}

/// Extract a package name (java-like dotted identifier) from a process args
/// column. Falls back to the basename of an executable path.
fn extract_package(args: &str) -> Option<String> {
    if args.is_empty() {
        return None;
    }
    // Common pattern: package name appears as-is for Android apps
    let first_token = args.split_whitespace().next().unwrap_or(args);
    if first_token.contains('.') && first_token.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) {
        // Strip ":subprocess" suffix Android adds (e.g. "com.app:remote")
        let pkg = first_token.split(':').next().unwrap_or(first_token);
        // Validate java-package-like form
        if pkg.split('.').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
            return Some(pkg.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zombie() {
        let input = "\
  PID USER     S %CPU   RSS ARGS
 1234 root     R 25.3 65432 com.example.app
 5678 system   S  1.2 12345 system_server
 9012 u0_a23   Z  0.0     0 com.zombie.app
";
        let snap = ProcessStatusParser::parse(input).unwrap();
        assert_eq!(snap.zombie_count, 1);
        assert_eq!(snap.hog_candidate_count, 1);
        assert_eq!(snap.rows.len(), 3);
        let zombie = snap.rows.iter().find(|r| r.is_zombie).unwrap();
        assert_eq!(zombie.pid, 9012);
        assert_eq!(zombie.package.as_deref(), Some("com.zombie.app"));
    }

    #[test]
    fn strips_android_subprocess_suffix() {
        let pkg = extract_package("com.example.app:remote_service --flag");
        assert_eq!(pkg.as_deref(), Some("com.example.app"));
    }
}
