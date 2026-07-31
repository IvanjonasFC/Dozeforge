//! Bloatware knowledge base with two layers:
//!
//!   1. SEED  — DozeForge's own MIT-licensed curated seed, embedded at compile
//!              time from `resources/bloatware_seed.json`. Always available.
//!   2. OVERLAY (optional) — the full community UAD-NG database, which is
//!              GPL-3.0 and therefore NOT bundled in the binary. Users can
//!              download it on demand (scripts/sync-uad-list.mjs writes it to
//!              the app data directory as `community_bloat.json`); when present
//!              it is loaded at runtime and overlaid on top of the seed.
//!
//! This keeps the shipped binary free of GPL data while still letting users opt
//! into the richer community list. `lookup()` prefers the overlay, then the seed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Removal rating, safest to most dangerous. (Generic risk vocabulary; the seed
/// is our own data, the optional overlay reuses the same shape for interop.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum UadRemoval {
    /// Safe to remove for most users; no expected loss of core functionality.
    Recommended,
    /// Removal may disable a feature some users rely on. Review first.
    Advanced,
    /// Only remove if you know exactly what it does.
    Expert,
    /// Removing this can break the system or cause a bootloop.
    Unsafe,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UadEntry {
    #[serde(default)]
    pub list: String,
    pub removal: UadRemoval,
    #[serde(default)]
    pub description: String,
    /// True when this entry came from the optional community overlay rather than
    /// our bundled seed. Surfaced to the UI as "community-verified".
    #[serde(default)]
    pub community: bool,
}

const SEED_RAW: &str = include_str!("../../resources/bloatware_seed.json");

/// Parse a `{ package: {list, removal, description} }` map, skipping metadata
/// keys (those starting with `_`). Tolerant: a single bad entry is skipped.
fn parse_map(raw: &str, community: bool) -> HashMap<String, UadEntry> {
    let root: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(target: "dozeforge::bloat", "bloat list parse error: {e}");
            return HashMap::new();
        }
    };
    let Some(obj) = root.as_object() else { return HashMap::new() };

    let mut out = HashMap::with_capacity(obj.len());
    for (pkg, val) in obj {
        if pkg.starts_with('_') {
            continue; // metadata keys
        }
        match serde_json::from_value::<UadEntry>(val.clone()) {
            Ok(mut entry) => {
                entry.community = community;
                out.insert(pkg.clone(), entry);
            }
            Err(e) => {
                tracing::debug!(target: "dozeforge::bloat", "skipping malformed entry {pkg}: {e}");
            }
        }
    }
    out
}

/// Bundled, MIT-licensed seed. Always present.
static SEED: Lazy<HashMap<String, UadEntry>> = Lazy::new(|| {
    let m = parse_map(SEED_RAW, false);
    tracing::info!(target: "dozeforge::bloat", entries = m.len(), "bloatware seed loaded");
    m
});

/// Optional community overlay, populated at runtime from the app data dir.
static OVERLAY: Lazy<RwLock<HashMap<String, UadEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Load the optional community list (GPL-3.0, user-downloaded) from `path` if it
/// exists. Safe to call at startup; a missing or malformed file is a no-op.
pub fn load_community_overlay(path: &Path) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return; // not downloaded — seed-only mode
    };
    let map = parse_map(&raw, true);
    if map.is_empty() {
        return;
    }
    if let Ok(mut o) = OVERLAY.write() {
        let n = map.len();
        *o = map;
        tracing::info!(target: "dozeforge::bloat", entries = n, "community overlay loaded");
    }
}

/// Look up a package. Prefers the community overlay, then the bundled seed.
/// Returns an owned entry (the overlay lives behind a lock).
pub fn lookup(package: &str) -> Option<UadEntry> {
    if let Ok(o) = OVERLAY.read() {
        if let Some(e) = o.get(package) {
            return Some(e.clone());
        }
    }
    SEED.get(package).cloned()
}

/// Count of known entries (overlay if loaded, else seed). For diagnostics.
pub fn entry_count() -> usize {
    let overlay = OVERLAY.read().map(|o| o.len()).unwrap_or(0);
    if overlay > 0 {
        overlay
    } else {
        SEED.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_loads_and_skips_meta() {
        assert!(entry_count() >= 50, "expected a populated seed, got {}", entry_count());
        assert!(lookup("_meta").is_none());
    }

    #[test]
    fn known_ad_package_is_recommended() {
        let e = lookup("com.miui.msa.global").expect("MSA should be in the seed");
        assert_eq!(e.removal, UadRemoval::Recommended);
        assert!(!e.community, "seed entries are not community-flagged");
    }

    #[test]
    fn core_package_is_unsafe() {
        let e = lookup("com.google.android.gms").expect("GMS should be in the seed");
        assert_eq!(e.removal, UadRemoval::Unsafe);
        let sysui = lookup("com.android.systemui").expect("SystemUI should be in the seed");
        assert_eq!(sysui.removal, UadRemoval::Unsafe);
    }

    #[test]
    fn unknown_package_returns_none() {
        assert!(lookup("com.definitely.not.a.real.package.xyz").is_none());
    }
}
