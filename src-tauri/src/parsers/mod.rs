//! Version-aware parsers for `dumpsys` and related ADB shell commands.

pub mod alarm;
pub mod app_labels;
pub mod appops;
pub mod batterystats;
pub mod battery_drain;
pub mod battery_sysfs;
pub mod cpuinfo;
pub mod deviceidle;
pub mod display_settings;
pub mod diskstats;
pub mod jobscheduler;
pub mod kernel_wakelocks;
pub mod package_sizes;
pub mod packages;
pub mod performance_settings;
pub mod power;
pub mod private_dns;
pub mod privacy_ops;
pub mod process_status;
pub mod sensorservice;
pub mod sleep_timeline;
pub mod standby;
pub mod system_settings;
pub mod usage_stats;
pub mod meminfo;
pub mod io_stats;

pub use appops::AppOpsParser;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub trait Parser {
    type Output;
    fn parse(&self, input: &str) -> Result<Self::Output>;
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageName(pub String);

impl PackageName {
    pub fn as_str(&self) -> &str { &self.0 }

    pub fn is_valid(&self) -> bool {
        if self.0.is_empty() || self.0.len() > 255 {
            return false;
        }
        self.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    }
}

impl std::fmt::Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for PackageName {
    fn from(value: &str) -> Self { Self(value.to_string()) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSample {
    pub pid: u32,
    pub package: Option<PackageName>,
    pub args: String,
    pub cpu_percent: f32,
    pub state: char,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakelockEntry {
    pub package: PackageName,
    pub uid: i32,
    pub total_ms: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmAttribution {
    pub target_package: PackageName,
    pub triggering_package: PackageName,
    pub kind: AlarmKind,
    pub wake_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlarmKind {
    WakeupRtc,
    WakeupElapsed,
    NonWakeup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandbyAssignment {
    pub package: PackageName,
    pub bucket: StandbyBucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandbyBucket {
    Exempted = 5,
    Active = 10,
    WorkingSet = 20,
    Frequent = 30,
    Rare = 40,
    Restricted = 45,
    Never = 50,
}

impl StandbyBucket {
    pub fn from_raw(value: i32) -> Option<Self> {
        match value {
            5 => Some(Self::Exempted),
            10 => Some(Self::Active),
            20 => Some(Self::WorkingSet),
            30 => Some(Self::Frequent),
            40 => Some(Self::Rare),
            45 => Some(Self::Restricted),
            50 => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: PackageName,
    pub uid: i32,
    pub install_path: String,
    pub is_system: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppOpState {
    pub package: PackageName,
    pub op: String,
    pub mode: AppOpMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppOpMode {
    Allow,
    Deny,
    Ignore,
    Default,
    Foreground,
}

impl AppOpMode {
    pub fn from_raw(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            "ignore" => Some(Self::Ignore),
            "default" => Some(Self::Default),
            "foreground" => Some(Self::Foreground),
            _ => None,
        }
    }

    pub fn as_cmd_value(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ignore => "ignore",
            Self::Default => "default",
            Self::Foreground => "foreground",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveWakelock {
    pub tag: String,
    pub package: Option<PackageName>,
    pub flags: String,
}