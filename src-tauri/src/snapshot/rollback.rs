//! Snapshot rollback.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::adb::{AdbClient, DeviceSerial};
use crate::error::{Error, Result};
use crate::parsers::{AppOpMode, AppOpState, PackageName, StandbyBucket};

use super::store::StoredSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackReport {
    pub commands: Vec<String>,
    pub applied: u32,
    pub failed: Vec<(String, String)>,
}

pub struct Rollback<'a> {
    pub client: &'a AdbClient,
    pub serial: &'a DeviceSerial,
}

impl<'a> Rollback<'a> {
    pub fn plan(snapshot: &StoredSnapshot, only: Option<&[PackageName]>) -> Vec<String> {
        let mut cmds = Vec::new();
        let allowed = only.map(|s| s.iter().collect::<std::collections::HashSet<_>>());
        let pkg_allowed = |p: &PackageName| -> bool {
            allowed.as_ref().map(|s| s.contains(p)).unwrap_or(true)
        };

        for (pkg, ops) in &snapshot.appops {
            if !pkg_allowed(pkg) { continue; }
            for AppOpState { op, mode, .. } in ops {
                cmds.push(format!("cmd appops set {} {} {}", pkg, op, AppOpMode::as_cmd_value(*mode)));
            }
        }
        for (pkg, raw_bucket) in &snapshot.standby {
            if !pkg_allowed(pkg) { continue; }
            let bucket = StandbyBucket::from_raw(*raw_bucket).unwrap_or(StandbyBucket::Active);
            cmds.push(format!("am set-standby-bucket {} {}", pkg, bucket as u8));
        }
        cmds
    }

    pub async fn execute(
        &self,
        snapshot: &StoredSnapshot,
        only: Option<&[PackageName]>,
    ) -> Result<RollbackReport> {
        let live = self.client.build_identity(self.serial).await?;
        if !snapshot.identity.is_compatible_with(&live) {
            return Err(Error::SnapshotIncompatible {
                snapshot_sdk: snapshot.identity.sdk_int,
                device_sdk: live.sdk_int,
            });
        }
        if snapshot.identity.security_patch_month != live.security_patch_month
            || snapshot.identity.security_patch_year != live.security_patch_year
        {
            warn!(
                target: "dozeforge::rollback",
                snapshot_patch = %format!("{}-{:02}", snapshot.identity.security_patch_year, snapshot.identity.security_patch_month),
                device_patch = %format!("{}-{:02}", live.security_patch_year, live.security_patch_month),
                "patch level differs from snapshot -- continuing"
            );
        }

        let commands = Self::plan(snapshot, only);
        let mut applied = 0u32;
        let mut failed = Vec::new();
        for cmd in &commands {
            match self.client.invoker.shell(self.serial, cmd, Duration::from_secs(8)).await {
                Ok(_) => applied += 1,
                Err(e) => failed.push((cmd.clone(), e.to_string())),
            }
        }

        Ok(RollbackReport { commands, applied, failed })
    }
}
