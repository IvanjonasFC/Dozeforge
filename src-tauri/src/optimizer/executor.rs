//! Action executor with safety gates + automatic snapshotting.

use std::collections::HashSet;
use std::time::Duration;

use tracing::{info, warn};

use crate::adb::capabilities::DeviceCapabilities;
use crate::adb::{AdbClient, DeviceSerial};
use crate::error::{Error, Result};
use crate::heuristics::manifest::HybridManifest;
use crate::heuristics::risk::{classify, PackageVerdict, RiskTier};
use crate::parsers::{AppOpsParser, InstalledPackage, PackageName, Parser};
use crate::snapshot::store::{SnapshotStore, StoredSnapshot};

use super::actions::{OptimizationAction, OptimizationOutcome, OptimizationReport};

pub struct Executor<'a> {
    pub client: &'a AdbClient,
    pub serial: &'a DeviceSerial,
    pub capabilities: &'a DeviceCapabilities,
    pub manifest: &'a HybridManifest,
    pub snapshot_store: &'a SnapshotStore,
    pub installed_packages: &'a [InstalledPackage],
}

impl<'a> Executor<'a> {
    pub async fn apply_batch(&self, actions: Vec<OptimizationAction>) -> Result<OptimizationReport> {
        // 1. Safety gate
        for action in &actions {
            self.guard_action(action)?;
        }

        // 2. Snapshot affected packages
        let affected: HashSet<PackageName> = actions
            .iter()
            .filter_map(|a| a.target_package().cloned())
            .collect();
        let snapshot = self.take_snapshot(&affected).await?;
        let snapshot_id = self.snapshot_store.save(&snapshot)?;
        info!(target: "dozeforge::optimizer", snapshot_id = %snapshot_id, "snapshot saved");

        // 3. Execute
        let mut outcomes = Vec::with_capacity(actions.len());
        for action in actions {
            let outcome = self.execute_one(&action).await;
            let stop = !outcome.success;
            outcomes.push(outcome);
            if stop {
                warn!(target: "dozeforge::optimizer", "stopping batch after hard failure");
                break;
            }
        }

        Ok(OptimizationReport { snapshot_id, outcomes })
    }

    fn guard_action(&self, action: &OptimizationAction) -> Result<()> {
        if let Some(pkg) = action.target_package() {
            let installed = self
                .installed_packages
                .iter()
                .find(|p| &p.name == pkg)
                .ok_or_else(|| Error::other(format!("package {} not installed", pkg)))?;
            let verdict: PackageVerdict = classify(installed, self.manifest);
            if verdict.tier == RiskTier::Critical {
                return Err(Error::SystemPackageRefused(pkg.0.clone()));
            }
        }

        let cap_ok = match action {
            OptimizationAction::SetStandbyBucket { .. } => self.capabilities.am_set_standby_bucket,
            OptimizationAction::SetAppOp { .. } => self.capabilities.appops_set,
            OptimizationAction::DisablePackage { .. } | OptimizationAction::EnablePackage { .. } => {
                self.capabilities.pm_disable_user
            }
            OptimizationAction::SetPhantomProcessLimit { .. } => self.capabilities.device_config_put,
            _ => true,
        };
        if !cap_ok {
            return Err(Error::MissingCapability(format!("{:?}", action)));
        }
        Ok(())
    }

    async fn execute_one(&self, action: &OptimizationAction) -> OptimizationOutcome {
        let cmd = action.to_shell();
        let timeout = action.shell_timeout();
        let result = self.client.invoker.shell(self.serial, &cmd, timeout).await;
        match result {
            Ok(stdout) => OptimizationOutcome {
                action: action.clone(),
                success: true,
                message: stdout.trim().to_string(),
            },
            Err(e) => OptimizationOutcome {
                action: action.clone(),
                success: false,
                message: e.to_string(),
            },
        }
    }

    async fn take_snapshot(&self, packages: &HashSet<PackageName>) -> Result<StoredSnapshot> {
        use crate::parsers::AppOpState;

        let identity = self.client.build_identity(self.serial).await?;
        let mut appops_by_package: Vec<(PackageName, Vec<AppOpState>)> = Vec::with_capacity(packages.len());
        let mut standby_by_package: Vec<(PackageName, i32)> = Vec::with_capacity(packages.len());

        for pkg in packages {
            if self.capabilities.appops_get {
                if let Ok(raw) = self.client.invoker.shell(
                    self.serial,
                    &format!("cmd appops get {}", pkg),
                    Duration::from_secs(8),
                ).await {
                    let parser = AppOpsParser { package: pkg.clone() };
                    if let Ok(ops) = parser.parse(&raw) {
                        appops_by_package.push((pkg.clone(), ops));
                    }
                }
            }
            if let Ok(raw) = self.client.invoker.shell(
                self.serial,
                &format!("am get-standby-bucket {}", pkg),
                Duration::from_secs(5),
            ).await {
                if let Ok(n) = raw.trim().parse::<i32>() {
                    standby_by_package.push((pkg.clone(), n));
                }
            }
        }

        Ok(StoredSnapshot::new(self.serial.clone(), identity, appops_by_package, standby_by_package))
    }
}
