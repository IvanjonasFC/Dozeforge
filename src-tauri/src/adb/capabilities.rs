//! Capability detection.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::Result;

use super::client::AdbClient;
use super::device::DeviceSerial;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub appops_set: bool,
    pub appops_get: bool,
    pub am_set_standby_bucket: bool,
    pub pm_disable_user: bool,
    pub device_config_put: bool,
    pub dumpsys_jobscheduler: bool,
    pub dumpsys_deviceidle: bool,
    pub dumpsys_sensorservice: bool,
    pub write_secure_settings: bool,
}

impl DeviceCapabilities {
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.appops_set { out.push("appops_set"); }
        if !self.appops_get { out.push("appops_get"); }
        if !self.am_set_standby_bucket { out.push("am_set_standby_bucket"); }
        if !self.pm_disable_user { out.push("pm_disable_user"); }
        if !self.device_config_put { out.push("device_config_put"); }
        if !self.dumpsys_jobscheduler { out.push("dumpsys_jobscheduler"); }
        if !self.dumpsys_deviceidle { out.push("dumpsys_deviceidle"); }
        if !self.dumpsys_sensorservice { out.push("dumpsys_sensorservice"); }
        if !self.write_secure_settings { out.push("write_secure_settings"); }
        out
    }
}

pub struct CapabilityProbe;

impl CapabilityProbe {
    pub async fn probe(client: &AdbClient, serial: &DeviceSerial) -> Result<DeviceCapabilities> {
        let short = Duration::from_secs(5);

        let appops_get = probe_ok(client, serial, "cmd appops get android", short).await;
        // `appops set` is gated identically to `appops get` from the adb shell,
        // so derive it rather than run a state-mutating probe.
        let appops_set = appops_get;
        let am_set_standby_bucket = probe_ok(client, serial, "am get-standby-bucket android", short).await;
        let pm_disable_user = probe_ok(client, serial, "pm list packages -s", short).await;
        let device_config_put = probe_ok(client, serial, "device_config list activity_manager", short).await;
        let dumpsys_jobscheduler = probe_ok(client, serial, "dumpsys jobscheduler | head -n 1", short).await;
        let dumpsys_deviceidle = probe_ok(client, serial, "dumpsys deviceidle | head -n 1", short).await;
        let dumpsys_sensorservice = probe_ok(client, serial, "dumpsys sensorservice | head -n 1", short).await;
        let write_secure_settings = probe_ok(client, serial, "settings get global development_settings_enabled", short).await;

        let caps = DeviceCapabilities {
            appops_set, appops_get, am_set_standby_bucket, pm_disable_user,
            device_config_put, dumpsys_jobscheduler, dumpsys_deviceidle,
            dumpsys_sensorservice, write_secure_settings,
        };

        let missing = caps.missing();
        if !missing.is_empty() {
            warn!(target: "dozeforge::adb", serial = %serial, ?missing,
                  "device is missing some optimisation capabilities; UI will downgrade");
        }

        Ok(caps)
    }
}

/// Whether a primitive is usable. A non-zero exit code does NOT mean the
/// primitive is missing — many valid Android commands (`dumpsys`,
/// `device_config`, `am get-standby-bucket`…) exit non-zero while working fine.
/// We therefore treat a command as AVAILABLE unless the device explicitly
/// reports it as unknown or permission-denied. Transient errors (timeout / IO)
/// also stay "available" so a flaky probe never disables a working feature.
async fn probe_ok(client: &AdbClient, serial: &DeviceSerial, cmd: &str, deadline: Duration) -> bool {
    match client.invoker.shell(serial, cmd, deadline).await {
        Ok(_) => true,
        Err(crate::error::Error::AdbCommand { stderr, .. }) => {
            let s = stderr.to_lowercase();
            !(s.contains("unknown command")
                || s.contains("not found")
                || s.contains("no such")
                || s.contains("permission den")
                || s.contains("security exception")
                || s.contains("requires the following permission")
                || s.contains("op not implemented"))
        }
        Err(_) => true,
    }
}
