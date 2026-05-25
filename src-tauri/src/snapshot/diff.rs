//! Snapshot differencing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::parsers::{AppOpMode, PackageName};

use super::store::StoredSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppOpChange {
    pub package: PackageName,
    pub op: String,
    pub from: Option<AppOpMode>,
    pub to: Option<AppOpMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandbyChange {
    pub package: PackageName,
    pub from: Option<i32>,
    pub to: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub appop_changes: Vec<AppOpChange>,
    pub standby_changes: Vec<StandbyChange>,
}

pub fn diff(old: &StoredSnapshot, new: &StoredSnapshot) -> SnapshotDiff {
    let mut appop_changes = Vec::new();

    let old_appops = flatten_appops(old);
    let new_appops = flatten_appops(new);

    for ((pkg, op), &new_mode) in &new_appops {
        let old_mode = old_appops.get(&(pkg.clone(), op.clone())).copied();
        if old_mode != Some(new_mode) {
            appop_changes.push(AppOpChange {
                package: pkg.clone(),
                op: op.clone(),
                from: old_mode,
                to: Some(new_mode),
            });
        }
    }
    for ((pkg, op), &old_mode) in &old_appops {
        if !new_appops.contains_key(&(pkg.clone(), op.clone())) {
            appop_changes.push(AppOpChange {
                package: pkg.clone(),
                op: op.clone(),
                from: Some(old_mode),
                to: None,
            });
        }
    }

    let mut standby_changes = Vec::new();
    let old_standby: HashMap<PackageName, i32> = old.standby.iter().cloned().collect();
    let new_standby: HashMap<PackageName, i32> = new.standby.iter().cloned().collect();
    for (pkg, &new_b) in &new_standby {
        let old_b = old_standby.get(pkg).copied();
        if old_b != Some(new_b) {
            standby_changes.push(StandbyChange { package: pkg.clone(), from: old_b, to: Some(new_b) });
        }
    }
    for (pkg, &old_b) in &old_standby {
        if !new_standby.contains_key(pkg) {
            standby_changes.push(StandbyChange { package: pkg.clone(), from: Some(old_b), to: None });
        }
    }

    SnapshotDiff { appop_changes, standby_changes }
}

fn flatten_appops(snap: &StoredSnapshot) -> HashMap<(PackageName, String), AppOpMode> {
    let mut out = HashMap::new();
    for (pkg, ops) in &snap.appops {
        for op in ops {
            out.insert((pkg.clone(), op.op.clone()), op.mode);
        }
    }
    out
}
