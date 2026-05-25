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
}

/// Threshold above which a single-sample %CPU is flagged as hog candidate.
pub const HOG_CPU_THRESHOLD: f32 = 15.0;

// Match: PID USER S %CPU RSS ARGS (whitespace separated)
// USER can be "root", "u0_a123", "shell", "system", etc.
static ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\s*(?P<pid>\d+)\s+(?P<user>\S+)\s+(?P<state>[A-Z])\s+(?P<cpu>\d+(?:\.\d+)?)\s+(?P<rss>\d+)(?:\s+(?P<args>.+))?$",
    )
    .unwrap()
});

pub struct ProcessStatusParser;

impl ProcessStatusParser {
    pub fn command() -> &'static str {
        // -b: batch (no interactive), -n 1: one iteration, -q: quiet
        // -o: explicit columns we care about
        "top -b -n 1 -q -o PID,USER,S,%CPU,RSS,ARGS"
    }

    pub fn parse(input: &str) -> Result<ProcessSnapshot> {
        let mut rows = Vec::new();
        let mut zombie_count = 0u32;
        let mut hog_count = 0u32;
        let mut total_cpu = 0.0f32;
        let mut total_rss = 0u64;

        for line in input.lines() {
            // Skip header and empty/noise lines
            if line.trim().is_empty() || line.contains("PID") && line.contains("USER") {
                continue;
            }

            let Some(caps) = ROW.captures(line) else { continue };
            let pid: u32 = caps["pid"].parse().unwrap_or(0);
            let user = caps["user"].to_string();
            let state_char = caps["state"].chars().next().unwrap_or('?');
            let state = ProcessState::from_char(state_char);
            let cpu_percent: f32 = caps["cpu"].parse().unwrap_or(0.0);
            let rss_kb: u64 = caps["rss"].parse().unwrap_or(0);
            let args = caps.name("args").map(|m| m.as_str().to_string()).unwrap_or_default();

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
                is_zombie,
            });
        }

        // Sort by CPU descending for UI convenience
        rows.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));

        Ok(ProcessSnapshot {
            rows,
            zombie_count,
            hog_candidate_count: hog_count,
            total_cpu_percent: total_cpu,
            total_rss_kb: total_rss,
        })
    }
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
