//! Proxy-detector. Reattributes GMS wakelocks to the real third-party owner.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::parsers::jobscheduler::JobAttribution;
use crate::parsers::{AlarmAttribution, PackageName, WakelockEntry};

pub const PROXY_PACKAGES: &[&str] = &[
    "com.google.android.gms",
    "com.google.android.gsf",
    "com.huawei.hwid",
    "com.xiaomi.xmsf",
    "com.heytap.mcs",
    "com.vivo.pushservice",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CulpritRanking {
    pub package: PackageName,
    pub wakelock_ms: u64,
    pub wakeup_count: u32,
    pub job_count: u32,
    pub redirected_from_proxy: Option<PackageName>,
    pub score: f64,
}

pub fn rank(
    wakelocks: &[WakelockEntry],
    alarms: &[AlarmAttribution],
    jobs: &[JobAttribution],
) -> Vec<CulpritRanking> {
    let mut by_pkg: HashMap<PackageName, CulpritRanking> = HashMap::new();

    for wl in wakelocks {
        let (target, redirected) = if is_proxy(&wl.package) {
            match dominant_owner_for_proxy(&wl.package, alarms) {
                Some(real) => (real, Some(wl.package.clone())),
                None => (wl.package.clone(), None),
            }
        } else {
            (wl.package.clone(), None)
        };

        let entry = by_pkg.entry(target.clone()).or_insert(CulpritRanking {
            package: target, wakelock_ms: 0, wakeup_count: 0, job_count: 0,
            redirected_from_proxy: redirected, score: 0.0,
        });
        entry.wakelock_ms = entry.wakelock_ms.saturating_add(wl.total_ms);
    }

    for a in alarms {
        let pkg = a.triggering_package.clone();
        let entry = by_pkg.entry(pkg.clone()).or_insert(CulpritRanking {
            package: pkg, wakelock_ms: 0, wakeup_count: 0, job_count: 0,
            redirected_from_proxy: None, score: 0.0,
        });
        entry.wakeup_count = entry.wakeup_count.saturating_add(a.wake_count);
    }

    for j in jobs {
        let pkg = j.package.clone();
        let entry = by_pkg.entry(pkg.clone()).or_insert(CulpritRanking {
            package: pkg, wakelock_ms: 0, wakeup_count: 0, job_count: 0,
            redirected_from_proxy: None, score: 0.0,
        });
        entry.job_count = entry.job_count.saturating_add(j.job_count);
    }

    for entry in by_pkg.values_mut() {
        let wl_minutes = (entry.wakelock_ms as f64) / 60_000.0;
        entry.score = wl_minutes * 1.0
            + (entry.wakeup_count as f64) * 0.5
            + (entry.job_count as f64) * 0.2;
    }

    let mut out: Vec<CulpritRanking> = by_pkg.into_values().collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn is_proxy(pkg: &PackageName) -> bool {
    PROXY_PACKAGES.contains(&pkg.as_str())
}

fn dominant_owner_for_proxy(
    proxy: &PackageName,
    alarms: &[AlarmAttribution],
) -> Option<PackageName> {
    let mut counts: HashMap<PackageName, u32> = HashMap::new();
    for a in alarms {
        if &a.target_package == proxy {
            *counts.entry(a.triggering_package.clone()).or_insert(0) += a.wake_count;
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(p, _)| p)
}
