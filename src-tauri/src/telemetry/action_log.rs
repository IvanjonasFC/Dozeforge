//! Append-only JSONL log of every action DozeForge applies.
//!
//! Each line is a self-contained JSON object that captures *what* was applied,
//! *when*, against *which device*, and the outcome. This lets us:
//!   - Compute "before vs after" Sleep Score trends.
//!   - Audit what changed if a device starts misbehaving.
//!   - Allow the user to re-export their full optimization history as a
//!     standalone shell script.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::optimizer::OptimizationAction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLogEntry {
    pub ts: DateTime<Utc>,
    pub device_serial: String,
    pub action: OptimizationAction,
    pub success: bool,
    pub message: String,
    /// Optional snapshot id captured before the action.
    pub snapshot_id: Option<String>,
}

pub struct ActionLog {
    path: PathBuf,
}

impl ActionLog {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }

    pub fn append(&self, entry: &ActionLogEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    pub fn read_all(&self) -> Result<Vec<ActionLogEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let mut out = Vec::with_capacity(raw.lines().count());
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let entry: ActionLogEntry = serde_json::from_str(line).map_err(|e| {
                Error::other(format!("malformed action log line: {e}"))
            })?;
            out.push(entry);
        }
        Ok(out)
    }

    /// Returns the most recent `n` entries (sorted newest first).
    pub fn tail(&self, n: usize) -> Result<Vec<ActionLogEntry>> {
        let mut all = self.read_all()?;
        all.sort_by(|a, b| b.ts.cmp(&a.ts));
        all.truncate(n);
        Ok(all)
    }
}
