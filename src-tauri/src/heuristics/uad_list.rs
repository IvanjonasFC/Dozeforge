//! Cross-reference against the Universal Android Debloater Next Generation
//! (UAD-NG) community package database.
//!
//! DozeForge's own `bloatware_recommendation` classifier is prefix-based and
//! works on any device offline. This module adds a second, authoritative
//! opinion sourced from the community-maintained UAD-NG list: for packages the
//! community has explicitly reviewed, we trust their `removal` rating over our
//! heuristic and surface their description.
//!
//! The dataset is embedded at compile time from `resources/uad_lists.json`.
//! Ship the curated subset or drop in the full upstream file — the code is
//! identical either way (see `scripts/sync-uad-list.mjs`).

use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// The community removal rating. Ordered from safest to most dangerous.
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
}

const RAW: &str = include_str!("../../resources/uad_lists.json");

static DB: Lazy<HashMap<String, UadEntry>> = Lazy::new(|| {
    // The file is a flat map of package -> entry, plus a leading `_meta` object
    // we skip. Parse into a generic value first so a single malformed/extra
    // entry (e.g. `_meta`) never poisons the whole table.
    let root: serde_json::Value = match serde_json::from_str(RAW) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(target: "dozeforge::uad", "uad_lists.json parse error: {e}");
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
            Ok(entry) => {
                out.insert(pkg.clone(), entry);
            }
            Err(e) => {
                tracing::debug!(target: "dozeforge::uad", "skipping malformed uad entry {pkg}: {e}");
            }
        }
    }
    tracing::info!(target: "dozeforge::uad", entries = out.len(), "UAD-NG list loaded");
    out
});

/// Look up a package in the community database.
pub fn lookup(package: &str) -> Option<&'static UadEntry> {
    DB.get(package)
}

/// Number of loaded community entries (for diagnostics / about screen).
pub fn entry_count() -> usize {
    DB.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_list_loads_and_skips_meta() {
        // The curated file must parse and contain real entries, not `_meta`.
        assert!(entry_count() >= 50, "expected a populated list, got {}", entry_count());
        assert!(lookup("_meta").is_none());
    }

    #[test]
    fn known_ad_package_is_recommended() {
        let e = lookup("com.miui.msa.global").expect("MSA should be in the list");
        assert_eq!(e.removal, UadRemoval::Recommended);
    }

    #[test]
    fn core_package_is_unsafe() {
        let e = lookup("com.google.android.gms").expect("GMS should be in the list");
        assert_eq!(e.removal, UadRemoval::Unsafe);
        let sysui = lookup("com.android.systemui").expect("SystemUI should be in the list");
        assert_eq!(sysui.removal, UadRemoval::Unsafe);
    }

    #[test]
    fn unknown_package_returns_none() {
        assert!(lookup("com.definitely.not.a.real.package.xyz").is_none());
    }
}
