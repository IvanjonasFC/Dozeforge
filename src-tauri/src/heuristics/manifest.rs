//! Hybrid manifest of known-good / known-bad packages.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::parsers::PackageName;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRule {
    #[serde(default)]
    pub never_touch: bool,
    #[serde(default)]
    pub known_offender: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestFile {
    pub version: u32,
    pub updated: String,
    pub rules: HashMap<String, ManifestRule>,
}

#[derive(Debug, Clone, Default)]
pub struct HybridManifest {
    rules: HashMap<String, ManifestRule>,
    pub source: Option<PathBuf>,
    pub version: u32,
}

impl HybridManifest {
    pub fn empty() -> Self { Self::default() }

    pub fn load_or_default() -> Result<Self> {
        let candidates = candidate_paths();
        for path in candidates {
            if path.exists() {
                if let Ok(s) = Self::load(&path) { return Ok(s); }
            }
        }
        Ok(Self::empty())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let file: ManifestFile = serde_json::from_str(&raw)?;
        Ok(Self {
            rules: file.rules,
            source: Some(path.to_path_buf()),
            version: file.version,
        })
    }

    pub fn lookup(&self, pkg: &PackageName) -> Option<&ManifestRule> {
        self.rules.get(&pkg.0)
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(env_path) = std::env::var("DOZEFORGE_MANIFEST") {
        out.push(PathBuf::from(env_path));
    }
    if let Some(data) = dirs::data_dir() {
        out.push(data.join("DozeForge").join("manifests").join("packages.json"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("manifests").join("packages.json"));
        }
    }
    out.push(PathBuf::from("src-tauri/manifests/packages.json"));
    out
}
