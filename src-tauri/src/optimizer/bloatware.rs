//! Bloatware manager.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::adb::{AdbClient, DeviceSerial};
use crate::error::{Error, Result};
use crate::heuristics::manifest::HybridManifest;
use crate::heuristics::risk::{classify, RiskTier};
use crate::parsers::{InstalledPackage, PackageName};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloatwareReport {
    pub disabled: Vec<PackageName>,
    pub failed: Vec<(PackageName, String)>,
}

pub struct BloatwareManager<'a> {
    pub client: &'a AdbClient,
    pub serial: &'a DeviceSerial,
    pub manifest: &'a HybridManifest,
    pub installed_packages: &'a [InstalledPackage],
}

impl<'a> BloatwareManager<'a> {
    pub async fn disable_batch(&self, targets: &[PackageName]) -> Result<BloatwareReport> {
        let mut disabled = Vec::new();
        let mut failed = Vec::new();

        for target in targets {
            if let Err(e) = self.guard(target) {
                failed.push((target.clone(), e.to_string()));
                continue;
            }
            let cmd = format!("pm disable-user --user 0 {}", target);
            match self.client.invoker.shell(self.serial, &cmd, Duration::from_secs(8)).await {
                Ok(stdout) => {
                    info!(target: "dozeforge::bloatware", package = %target, "disabled");
                    if stdout.contains("disabled") || stdout.contains("new state: disabled-user") {
                        disabled.push(target.clone());
                    } else {
                        failed.push((target.clone(), stdout.trim().to_string()));
                    }
                }
                Err(e) => failed.push((target.clone(), e.to_string())),
            }
        }

        Ok(BloatwareReport { disabled, failed })
    }

    pub async fn enable_batch(&self, targets: &[PackageName]) -> Result<BloatwareReport> {
        let mut enabled = Vec::new();
        let mut failed = Vec::new();
        for target in targets {
            let cmd = format!("pm enable {}", target);
            match self.client.invoker.shell(self.serial, &cmd, Duration::from_secs(8)).await {
                Ok(stdout) if stdout.contains("enabled") => enabled.push(target.clone()),
                Ok(stdout) => failed.push((target.clone(), stdout.trim().to_string())),
                Err(e) => failed.push((target.clone(), e.to_string())),
            }
        }
        Ok(BloatwareReport { disabled: enabled, failed })
    }

    fn guard(&self, pkg: &PackageName) -> Result<()> {
        let installed = self
            .installed_packages
            .iter()
            .find(|p| &p.name == pkg)
            .ok_or_else(|| Error::other(format!("package {pkg} not installed")))?;
        let verdict = classify(installed, self.manifest);
        if verdict.tier == RiskTier::Critical {
            return Err(Error::SystemPackageRefused(pkg.0.clone()));
        }
        Ok(())
    }
}
