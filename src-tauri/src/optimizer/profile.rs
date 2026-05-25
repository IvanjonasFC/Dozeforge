//! Profile-based one-click optimisation.
//!
//! Each `Profile` is a recipe that scans installed packages, the manifest, and
//! device capabilities, then returns a deterministic list of actions plus a
//! preview of what will (and will not) be touched.
//!
//! Profiles never:
//! - touch Critical packages (`uid < 10000` or `/system|/vendor|/apex`)
//! - touch packages on the `Exclusions` list (comms, IMEs, banking, a11y)
//! - apply destructive (`pm disable-user`) to non-system apps
//!
//! Profiles always:
//! - take a snapshot first (handled by Executor)
//! - emit reversible actions where possible

use serde::{Deserialize, Serialize};

use crate::adb::capabilities::DeviceCapabilities;
use crate::heuristics::manifest::HybridManifest;
use crate::heuristics::risk::{classify, RiskTier};
use crate::parsers::{AppOpMode, InstalledPackage, StandbyBucket};

use super::actions::OptimizationAction;
use super::exclusions::Exclusions;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Touch only manifest known-offenders. Safe, fully reversible. ~10-30 actions.
    Conservative,
    /// Known offenders + lightly used user apps. Disable obvious OEM bloat. Default. ~50-100 actions.
    Balanced,
    /// Restrict all user-installed apps not in exclusions. Aggressive bloatware removal. ~200+ actions.
    Aggressive,
    /// Like Aggressive plus removes Doze whitelist exemptions. Maximum battery, breaks more workflows. ~300+ actions.
    Nuclear,
}

impl Profile {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Conservative => "Conservative",
            Self::Balanced => "Balanced",
            Self::Aggressive => "Aggressive",
            Self::Nuclear => "Nuclear",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Conservative => "Restrict only the well-known battery offenders from the curated manifest. Nothing else is touched. Safest option.",
            Self::Balanced => "Conservative plus: disable common OEM bloatware (Bixby, MIUI Weather, Samsung Daily). Recommended default.",
            Self::Aggressive => "Balanced plus: move every user-installed app (except communication, banking and keyboards) to the Restricted bucket and revoke their wakelock permission.",
            Self::Nuclear => "Aggressive plus: remove all third-party Doze whitelist exemptions and disable additional OEM apps. Maximum power savings, expect minor inconveniences.",
        }
    }

    pub fn phantom_limit(&self) -> Option<u32> {
        match self {
            Self::Conservative => None,
            Self::Balanced => Some(128),
            Self::Aggressive => Some(256),
            Self::Nuclear => Some(256),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePreview {
    pub profile: Profile,
    pub actions: Vec<OptimizationAction>,
    /// (package_name, reason) for every package that was skipped.
    pub excluded_packages: Vec<(String, String)>,
    pub summary: ProfileSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub apps_restricted: u32,
    pub bloatware_disabled: u32,
    pub wakelocks_revoked: u32,
    pub doze_whitelist_cleaned: u32,
    pub total_actions: u32,
    pub packages_excluded: u32,
}

pub struct ProfileBuilder<'a> {
    pub manifest: &'a HybridManifest,
    pub capabilities: &'a DeviceCapabilities,
    pub installed: &'a [InstalledPackage],
    pub exclusions: &'a Exclusions,
    /// Doze whitelist (user-whitelisted apps only; system ones are never touched).
    pub doze_user_whitelist: &'a [String],
}

impl<'a> ProfileBuilder<'a> {
    pub fn build(&self, profile: Profile) -> ProfilePreview {
        let mut actions: Vec<OptimizationAction> = Vec::new();
        let mut excluded: Vec<(String, String)> = Vec::new();
        let mut summary = ProfileSummary::default();

        for pkg in self.installed {
            // Skip exclusions list (communication apps, IMEs, banking, a11y...)
            if let Some(reason) = self.exclusions.reason_for(&pkg.name) {
                excluded.push((pkg.name.0.clone(), reason.to_string()));
                summary.packages_excluded += 1;
                continue;
            }

            let verdict = classify(pkg, self.manifest);

            // NEVER touch Critical packages, regardless of profile.
            if verdict.tier == RiskTier::Critical {
                continue;
            }

            let is_offender = self
                .manifest
                .lookup(&pkg.name)
                .map(|r| r.known_offender)
                .unwrap_or(false);

            match profile {
                Profile::Conservative => {
                    if is_offender {
                        actions.push(OptimizationAction::SetStandbyBucket {
                            package: pkg.name.clone(),
                            bucket: StandbyBucket::Restricted,
                        });
                        summary.apps_restricted += 1;
                        if self.capabilities.appops_set {
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "WAKE_LOCK".into(),
                                mode: AppOpMode::Ignore,
                            });
                            summary.wakelocks_revoked += 1;
                        }
                    }
                }
                Profile::Balanced => {
                    if is_offender && pkg.is_system && self.capabilities.pm_disable_user {
                        // OEM bloat known to drain → disable (reversible via pm enable)
                        actions.push(OptimizationAction::DisablePackage {
                            package: pkg.name.clone(),
                        });
                        summary.bloatware_disabled += 1;
                    } else if is_offender {
                        // Third-party offender → restrict + revoke wakelock
                        actions.push(OptimizationAction::SetStandbyBucket {
                            package: pkg.name.clone(),
                            bucket: StandbyBucket::Restricted,
                        });
                        summary.apps_restricted += 1;
                        if self.capabilities.appops_set {
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "WAKE_LOCK".into(),
                                mode: AppOpMode::Ignore,
                            });
                            summary.wakelocks_revoked += 1;
                        }
                    } else if !pkg.is_system && verdict.tier == RiskTier::Moderate {
                        // Generic user app → push to Rare (still receives messages, just slower bg)
                        actions.push(OptimizationAction::SetStandbyBucket {
                            package: pkg.name.clone(),
                            bucket: StandbyBucket::Rare,
                        });
                        summary.apps_restricted += 1;
                    }
                }
                Profile::Aggressive => {
                    if is_offender && pkg.is_system && self.capabilities.pm_disable_user {
                        actions.push(OptimizationAction::DisablePackage {
                            package: pkg.name.clone(),
                        });
                        summary.bloatware_disabled += 1;
                    } else if !pkg.is_system || (pkg.is_system && verdict.tier == RiskTier::Elevated) {
                        // EVERY user app and every Elevated system app → Restricted + revoke
                        actions.push(OptimizationAction::SetStandbyBucket {
                            package: pkg.name.clone(),
                            bucket: StandbyBucket::Restricted,
                        });
                        summary.apps_restricted += 1;
                        if self.capabilities.appops_set {
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "WAKE_LOCK".into(),
                                mode: AppOpMode::Ignore,
                            });
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "RUN_IN_BACKGROUND".into(),
                                mode: AppOpMode::Ignore,
                            });
                            summary.wakelocks_revoked += 1;
                        }
                    }
                }
                Profile::Nuclear => {
                    if pkg.is_system
                        && self.capabilities.pm_disable_user
                        && verdict.tier != RiskTier::Critical
                        && (is_offender || verdict.tier == RiskTier::Elevated)
                    {
                        actions.push(OptimizationAction::DisablePackage {
                            package: pkg.name.clone(),
                        });
                        summary.bloatware_disabled += 1;
                    } else if !pkg.is_system {
                        actions.push(OptimizationAction::SetStandbyBucket {
                            package: pkg.name.clone(),
                            bucket: StandbyBucket::Restricted,
                        });
                        summary.apps_restricted += 1;
                        if self.capabilities.appops_set {
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "WAKE_LOCK".into(),
                                mode: AppOpMode::Ignore,
                            });
                            actions.push(OptimizationAction::SetAppOp {
                                package: pkg.name.clone(),
                                op: "RUN_IN_BACKGROUND".into(),
                                mode: AppOpMode::Ignore,
                            });
                            summary.wakelocks_revoked += 1;
                        }
                    }
                }
            }
        }

        // Nuclear additionally cleans the Doze whitelist of user entries.
        if profile == Profile::Nuclear {
            for entry in self.doze_user_whitelist {
                let pkg = crate::parsers::PackageName(entry.clone());
                if self.exclusions.reason_for(&pkg).is_some() {
                    continue;
                }
                actions.push(OptimizationAction::RemoveDozeWhitelist { package: pkg });
                summary.doze_whitelist_cleaned += 1;
            }
        }

        // Phantom limit (Balanced and above)
        if let Some(value) = profile.phantom_limit() {
            actions.push(OptimizationAction::SetPhantomProcessLimit { value });
        }

        summary.total_actions = actions.len() as u32;

        ProfilePreview {
            profile,
            actions,
            excluded_packages: excluded,
            summary,
        }
    }
}
