//! Filesystem-backed snapshot store.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::adb::{BuildIdentity, DeviceSerial};
use crate::error::{Error, Result};
use crate::parsers::{AppOpState, PackageName};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub device_serial: DeviceSerial,
    pub identity: BuildIdentity,
    pub appops: Vec<(PackageName, Vec<AppOpState>)>,
    pub standby: Vec<(PackageName, i32)>,
    pub label: Option<String>,
}

impl StoredSnapshot {
    pub fn new(
        device_serial: DeviceSerial,
        identity: BuildIdentity,
        appops: Vec<(PackageName, Vec<AppOpState>)>,
        standby: Vec<(PackageName, i32)>,
    ) -> Self {
        Self {
            schema_version: 1,
            created_at: Utc::now(),
            device_serial,
            identity,
            appops,
            standby,
            label: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub device_serial: DeviceSerial,
    pub sdk_int: u32,
    pub packages: usize,
    pub label: Option<String>,
}

pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path { &self.root }

    pub fn save(&self, snapshot: &StoredSnapshot) -> Result<String> {
        let bytes = serde_json::to_vec(snapshot)?;
        let id = hex::encode(Sha256::digest(&bytes));
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join(format!("{id}.json"));
        std::fs::write(&path, &bytes)?;
        Ok(id)
    }

    /// Validates that `id` is a 64-character lowercase hex string (SHA-256).
    /// Prevents path traversal via crafted snapshot ids (e.g. `../../etc/passwd`).
    fn check_id(id: &str) -> Result<()> {
        if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::SnapshotNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<StoredSnapshot> {
        Self::check_id(id)?;
        let path = self.root.join(format!("{id}.json"));
        if !path.exists() {
            return Err(Error::SnapshotNotFound(id.to_string()));
        }
        let raw = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn list(&self) -> Result<Vec<SnapshotMeta>> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let Ok(raw) = std::fs::read_to_string(&path) else { continue };
                let Ok(snap): std::result::Result<StoredSnapshot, _> = serde_json::from_str(&raw) else { continue };
                let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                out.push(SnapshotMeta {
                    id,
                    created_at: snap.created_at,
                    device_serial: snap.device_serial,
                    sdk_int: snap.identity.sdk_int,
                    packages: snap.appops.len(),
                    label: snap.label,
                });
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }
}
