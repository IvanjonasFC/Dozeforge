//! Tauri command handlers. The only Rust surface exposed to the SvelteKit UI.

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::adb::capabilities::{CapabilityProbe, DeviceCapabilities};
use crate::adb::{Device, DeviceSerial};
use crate::error::{IpcError, Result};
use crate::heuristics::proxy_detector::{rank, CulpritRanking};
use crate::heuristics::risk::{classify, PackageVerdict};
use crate::heuristics::sampling::{CpuAggregate, CpuSampler, SamplingConfig};
use crate::optimizer::bloatware::{BloatwareManager, BloatwareReport};
use crate::optimizer::profile::{Profile, ProfileBuilder, ProfilePreview};
use crate::optimizer::Exclusions;
use crate::optimizer::{Executor, OptimizationAction, OptimizationReport};
use crate::parsers::alarm::AlarmParser;
use crate::parsers::batterystats::BatteryStatsParser;
use crate::parsers::battery_sysfs::{BatteryHealth, BatterySysfsParser};
use crate::parsers::deviceidle::{DeviceIdleParser, DozeWhitelist};
use crate::parsers::diskstats::{DiskStats, DiskStatsParser};
use crate::parsers::display_settings::DisplaySettings;
use crate::parsers::system_settings::SystemTweaks;
use crate::parsers::jobscheduler::{JobAttribution, JobSchedulerParser};
use crate::parsers::package_sizes::{PackageSize, PackageSizesScanner};
use crate::parsers::packages::PackageListParser;
use crate::parsers::app_labels::AppLabelsResolver;
use crate::parsers::power::PowerParser;
use crate::parsers::private_dns::{PrivateDnsMode, PrivateDnsState, DNS_PRESETS};
use crate::parsers::privacy_ops::{DangerousPermissionEntry, DangerousPermissionsScanner, PrivacyOpsScanner, PrivacyScan};
use crate::parsers::process_status::{ProcessSnapshot, ProcessStatusParser};
use crate::parsers::sensorservice::{SensorClient, SensorServiceParser};
use crate::parsers::standby::StandbyParser;
use crate::parsers::usage_stats::UsageStatsParser;
use crate::parsers::{
    AlarmAttribution, InstalledPackage, LiveWakelock, PackageName, Parser, WakelockEntry,
};
use crate::heuristics::miscategorized::{MiscategorizedApp, MiscategorizedDetector};
use crate::heuristics::bloatware_recommendation::{
    self, BloatPreset, BloatwareRecommendation,
};
use crate::heuristics::sleep_score::{SleepScore, SleepScoreCalc};
use crate::ipc::streams;
use crate::snapshot::rollback::{Rollback, RollbackReport};
use crate::snapshot::store::SnapshotMeta;
use crate::state::AppState;

async fn safe_pm_list_packages(invoker: &crate::adb::command::AdbInvoker, serial: &DeviceSerial) -> crate::error::Result<String> {
    if let Ok(out) = invoker.shell(serial, "pm list packages -f -U --user 0", Duration::from_secs(20)).await {
        if !out.trim().is_empty() && !out.contains("Exception") { return Ok(out); }
    }
    if let Ok(out) = invoker.shell(serial, "pm list packages -f -U", Duration::from_secs(20)).await {
        if !out.trim().is_empty() && !out.contains("Exception") { return Ok(out); }
    }
    invoker.shell(serial, "pm list packages -f", Duration::from_secs(20)).await.map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub device_serial: DeviceSerial,
    pub sdk_int: u32,
    pub wakelocks: Vec<WakelockEntry>,
    pub alarms: Vec<AlarmAttribution>,
    pub jobs: Vec<JobAttribution>,
    pub culprits: Vec<CulpritRanking>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeupSources {
    pub alarms: Vec<AlarmAttribution>,
    pub jobs: Vec<JobAttribution>,
    pub doze_whitelist: DozeWhitelist,
    pub live_wakelocks: Vec<LiveWakelock>,
    pub sensors: Vec<SensorClient>,
}

/// Validates a raw frontend-supplied serial and returns a `DeviceSerial`.
/// Use this in every Tauri command that accepts a `serial: String`.
fn checked_serial(raw: &str) -> std::result::Result<DeviceSerial, IpcError> {
    crate::security::validate_serial(raw)?;
    Ok(DeviceSerial(raw.to_string()))
}

#[tauri::command]
pub async fn list_devices(
    state: State<'_, Arc<AppState>>,
) -> std::result::Result<Vec<Device>, IpcError> {
    inner_list_devices(state.inner().clone()).await.map_err(Into::into)
}
async fn inner_list_devices(state: Arc<AppState>) -> Result<Vec<Device>> {
    state.adb.list_devices().await
}

#[tauri::command]
pub async fn probe_capabilities(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<DeviceCapabilities, IpcError> {
    let serial = checked_serial(&serial)?;
    CapabilityProbe::probe(&state.adb, &serial).await.map_err(Into::into)
}

#[tauri::command]
pub async fn audit_device(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<AuditReport, IpcError> {
    inner_audit(state.inner().clone(), serial).await.map_err(Into::into)
}
async fn inner_audit(state: Arc<AppState>, serial: String) -> Result<AuditReport> {
    let serial = checked_serial(&serial)?;
    let identity = state.adb.build_identity(&serial).await?;
    let api = identity.sdk_int;

    let bs_raw = state.adb.invoker
        .shell(&serial, "dumpsys batterystats --checkin", Duration::from_secs(45))
        .await?;
    let wakelocks = BatteryStatsParser::for_api(api).parse(&bs_raw).unwrap_or_default();

    let alarm_raw = state.adb.invoker
        .shell(&serial, "dumpsys alarm", Duration::from_secs(30))
        .await?;
    let alarms = AlarmParser.parse(&alarm_raw).unwrap_or_default();

    let jobs_raw = state.adb.invoker
        .shell(&serial, "dumpsys jobscheduler", Duration::from_secs(30))
        .await
        .unwrap_or_default();
    let jobs = JobSchedulerParser.parse(&jobs_raw).unwrap_or_default();

    let culprits = rank(&wakelocks, &alarms, &jobs);

    Ok(AuditReport {
        device_serial: serial,
        sdk_int: api,
        wakelocks, alarms, jobs, culprits,
    })
}

#[tauri::command]
pub async fn check_root(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<bool, IpcError> {
    let serial = checked_serial(&serial)?;
    // Execute `su -c id`. If it contains 'uid=0(root)', root access is granted.
    let output = state.adb.invoker.shell(&serial, "su -c id", Duration::from_secs(5)).await;
    match output {
        Ok(out) => Ok(out.contains("uid=0")),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn sample_cpu(
    state: State<'_, Arc<AppState>>,
    serial: String,
    duration_secs: Option<u32>,
) -> std::result::Result<Vec<CpuAggregate>, IpcError> {
    let serial = checked_serial(&serial)?;
    let interval = Duration::from_secs(2);
    let total_samples = duration_secs.map(|d| (d.max(2) / 2).max(1)).unwrap_or(15);
    let sampler = CpuSampler {
        client: &state.adb,
        serial: &serial,
        config: SamplingConfig { interval, total_samples },
    };
    sampler.run().await.map_err(Into::into)
}

#[tauri::command]
pub async fn get_live_ram(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<crate::parsers::meminfo::MemInfo, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = state.adb.invoker.shell(&serial, "dumpsys meminfo", Duration::from_secs(10)).await?;
    crate::parsers::meminfo::MemInfoParser::parse(&raw).map_err(IpcError::from)
}

#[tauri::command]
pub async fn get_io_stats(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<crate::parsers::io_stats::UidIoStat>, IpcError> {
    let serial = checked_serial(&serial)?;
    // Requires root mode, but let's just attempt it with su -c
    let raw = state.adb.invoker.shell(&serial, "su -c cat /proc/uid_io/stats", Duration::from_secs(10)).await?;
    crate::parsers::io_stats::IoStatsParser::parse(&raw).map_err(IpcError::from)
}

#[tauri::command]
pub async fn list_wakeup_sources(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<WakeupSources, IpcError> {
    inner_wakeup_sources(state.inner().clone(), serial).await.map_err(Into::into)
}
async fn inner_wakeup_sources(state: Arc<AppState>, serial: String) -> Result<WakeupSources> {
    let serial = checked_serial(&serial)?;

    let alarm_raw = state.adb.invoker
        .shell(&serial, "dumpsys alarm", Duration::from_secs(30)).await?;
    let alarms = AlarmParser.parse(&alarm_raw).unwrap_or_default();

    let jobs_raw = state.adb.invoker
        .shell(&serial, "dumpsys jobscheduler", Duration::from_secs(30)).await
        .unwrap_or_default();
    let jobs = JobSchedulerParser.parse(&jobs_raw).unwrap_or_default();

    let idle_raw = state.adb.invoker
        .shell(&serial, "dumpsys deviceidle", Duration::from_secs(15)).await
        .unwrap_or_default();
    let doze_whitelist = DeviceIdleParser.parse(&idle_raw).unwrap_or(DozeWhitelist {
        user_whitelisted: Vec::new(), system_whitelisted: Vec::new(),
    });

    let power_raw = state.adb.invoker
        .shell(&serial, "dumpsys power", Duration::from_secs(15)).await
        .unwrap_or_default();
    let live_wakelocks = PowerParser.parse(&power_raw).unwrap_or_default();

    let sensors_raw = state.adb.invoker
        .shell(&serial, "dumpsys sensorservice", Duration::from_secs(20)).await
        .unwrap_or_default();
    let sensors = SensorServiceParser.parse(&sensors_raw).unwrap_or_default();

    Ok(WakeupSources { alarms, jobs, doze_whitelist, live_wakelocks, sensors })
}

#[tauri::command]
pub async fn list_packages(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<InstalledPackage>, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial).await?;
    PackageListParser.parse(&raw).map_err(Into::into)
}

#[tauri::command]
pub async fn classify_packages(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<Vec<PackageVerdict>, IpcError> {
    inner_classify(state.inner().clone(), serial, packages).await.map_err(Into::into)
}
async fn inner_classify(
    state: Arc<AppState>,
    serial: String,
    packages: Vec<String>,
) -> Result<Vec<PackageVerdict>> {
    let serial = checked_serial(&serial)?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial).await?;
    let installed = PackageListParser.parse(&raw)?;

    let manifest = state.manifest.read().await;

    let filter: Option<std::collections::HashSet<String>> = if packages.is_empty() {
        None
    } else {
        Some(packages.into_iter().collect())
    };

    let out: Vec<PackageVerdict> = installed
        .iter()
        .filter(|p| filter.as_ref().map(|f| f.contains(&p.name.0)).unwrap_or(true))
        .map(|p| classify(p, &manifest))
        .collect();
    Ok(out)
}

#[tauri::command]
pub async fn apply_optimization(
    state: State<'_, Arc<AppState>>,
    serial: String,
    actions: Vec<OptimizationAction>,
) -> std::result::Result<OptimizationReport, IpcError> {
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}
async fn inner_apply(
    state: Arc<AppState>,
    serial: String,
    actions: Vec<OptimizationAction>,
) -> Result<OptimizationReport> {
    let serial = checked_serial(&serial)?;
    let capabilities = CapabilityProbe::probe(&state.adb, &serial).await?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial).await?;
    let installed = PackageListParser.parse(&raw)?;

    let manifest = state.manifest.read().await;
    let executor = Executor {
        client: &state.adb,
        serial: &serial,
        capabilities: &capabilities,
        manifest: &manifest,
        snapshot_store: &state.snapshot_store,
        installed_packages: &installed,
    };
    let report = executor.apply_batch(actions).await?;

    // Persist every outcome to the JSONL action log. Best-effort: a logging
    // failure must NOT mask the actual operation result.
    let serial_str = serial.0.clone();
    for outcome in &report.outcomes {
        let entry = crate::telemetry::ActionLogEntry {
            ts: chrono::Utc::now(),
            device_serial: serial_str.clone(),
            action: outcome.action.clone(),
            success: outcome.success,
            message: outcome.message.clone(),
            snapshot_id: Some(report.snapshot_id.clone()),
        };
        if let Err(e) = state.action_log.append(&entry) {
            tracing::warn!(target: "dozeforge::action_log", "log append failed: {e}");
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn take_snapshot(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
    label: Option<String>,
) -> std::result::Result<SnapshotMeta, IpcError> {
    inner_take_snapshot(state.inner().clone(), serial, packages, label).await.map_err(Into::into)
}
async fn inner_take_snapshot(
    state: Arc<AppState>,
    serial: String,
    packages: Vec<String>,
    label: Option<String>,
) -> Result<SnapshotMeta> {
    use crate::parsers::AppOpsParser;
    use crate::snapshot::store::StoredSnapshot;

    let serial = checked_serial(&serial)?;
    let identity = state.adb.build_identity(&serial).await?;

    let mut appops: Vec<(PackageName, Vec<crate::parsers::AppOpState>)> = Vec::new();
    let mut standby: Vec<(PackageName, i32)> = Vec::new();

    for pkg_raw in packages {
        let pkg = PackageName(pkg_raw);
        if !pkg.is_valid() { continue; }
        if let Ok(raw) = state.adb.invoker.shell(
            &serial, &format!("cmd appops get {}", pkg), Duration::from_secs(8),
        ).await {
            let parser = AppOpsParser { package: pkg.clone() };
            if let Ok(ops) = parser.parse(&raw) {
                appops.push((pkg.clone(), ops));
            }
        }
        if let Ok(raw) = state.adb.invoker.shell(
            &serial, &format!("am get-standby-bucket {}", pkg), Duration::from_secs(5),
        ).await {
            if let Ok(n) = raw.trim().parse::<i32>() {
                standby.push((pkg.clone(), n));
            }
        }
    }

    let mut snapshot = StoredSnapshot::new(serial.clone(), identity, appops, standby);
    snapshot.label = label;
    let id = state.snapshot_store.save(&snapshot)?;
    Ok(SnapshotMeta {
        id,
        created_at: snapshot.created_at,
        device_serial: serial,
        sdk_int: snapshot.identity.sdk_int,
        packages: snapshot.appops.len(),
        label: snapshot.label,
    })
}

#[tauri::command]
pub async fn list_snapshots(
    state: State<'_, Arc<AppState>>,
) -> std::result::Result<Vec<SnapshotMeta>, IpcError> {
    state.snapshot_store.list().map_err(Into::into)
}

#[tauri::command]
pub async fn rollback_snapshot(
    state: State<'_, Arc<AppState>>,
    serial: String,
    snapshot_id: String,
    only: Option<Vec<String>>,
) -> std::result::Result<RollbackReport, IpcError> {
    let serial = checked_serial(&serial)?;
    let snapshot = state.snapshot_store.load(&snapshot_id).map_err(IpcError::from)?;
    let only_pkgs: Option<Vec<PackageName>> = only.map(|v| v.into_iter().map(PackageName).collect());
    let rb = Rollback { client: &state.adb, serial: &serial };
    rb.execute(&snapshot, only_pkgs.as_deref()).await.map_err(Into::into)
}

#[tauri::command]
pub async fn export_shell_script(
    state: State<'_, Arc<AppState>>,
    actions: Vec<OptimizationAction>,
    device_label: String,
) -> std::result::Result<String, IpcError> {
    let content = crate::export::shell_script::ShellExport::render(&actions, &device_label);
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let safe_label = sanitise_filename(&device_label);
    let path = state.data_dir.join("exports").join(format!("dozeforge-{safe_label}-{stamp}.sh"));
    std::fs::write(&path, content).map_err(|e| IpcError::from(crate::error::Error::Io(e)))?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn disable_bloatware(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<BloatwareReport, IpcError> {
    inner_bloatware(state.inner().clone(), serial, packages, true).await.map_err(Into::into)
}

#[tauri::command]
pub async fn enable_bloatware(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<BloatwareReport, IpcError> {
    inner_bloatware(state.inner().clone(), serial, packages, false).await.map_err(Into::into)
}

async fn inner_bloatware(
    state: Arc<AppState>,
    serial: String,
    packages: Vec<String>,
    disable: bool,
) -> Result<BloatwareReport> {
    let serial = checked_serial(&serial)?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial).await?;
    let installed = PackageListParser.parse(&raw)?;
    let manifest = state.manifest.read().await;

    let mgr = BloatwareManager {
        client: &state.adb,
        serial: &serial,
        manifest: &manifest,
        installed_packages: &installed,
    };
    let targets: Vec<PackageName> = packages.into_iter().map(PackageName).collect();
    if disable {
        mgr.disable_batch(&targets).await
    } else {
        mgr.enable_batch(&targets).await
    }
}

#[tauri::command]
pub async fn set_phantom_process_limit(
    state: State<'_, Arc<AppState>>,
    serial: String,
    value: u32,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    if value > 1024 {
        return Err(IpcError::from(crate::error::Error::other(format!(
            "phantom process limit {value} is unreasonable; max is 1024"
        ))));
    }
    let cmd = format!("device_config put activity_manager max_phantom_processes {}", value);
    state.adb.invoker.shell(&serial, &cmd, Duration::from_secs(5)).await
        .map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn preview_profile(
    state: State<'_, Arc<AppState>>,
    serial: String,
    profile: Profile,
    user_excludes: Option<Vec<String>>,
) -> std::result::Result<ProfilePreview, IpcError> {
    inner_preview_profile(state.inner().clone(), serial, profile, user_excludes.unwrap_or_default())
        .await
        .map_err(Into::into)
}

async fn inner_preview_profile(
    state: Arc<AppState>,
    serial: String,
    profile: Profile,
    user_excludes: Vec<String>,
) -> Result<ProfilePreview> {
    let serial = checked_serial(&serial)?;
    let capabilities = CapabilityProbe::probe(&state.adb, &serial).await?;

    let raw = safe_pm_list_packages(&state.adb.invoker, &serial).await?;
    let installed = PackageListParser.parse(&raw)?;

    // Doze whitelist (for Nuclear profile)
    let idle_raw = state
        .adb
        .invoker
        .shell(&serial, "dumpsys deviceidle", Duration::from_secs(15))
        .await
        .unwrap_or_default();
    let doze = DeviceIdleParser.parse(&idle_raw).unwrap_or(DozeWhitelist {
        user_whitelisted: Vec::new(),
        system_whitelisted: Vec::new(),
    });
    let doze_user: Vec<String> = doze.user_whitelisted.iter().map(|p| p.0.clone()).collect();

    let manifest = state.manifest.read().await;
    let exclusions = Exclusions::new_default().with_user_overrides(user_excludes);

    let builder = ProfileBuilder {
        manifest: &manifest,
        capabilities: &capabilities,
        installed: &installed,
        exclusions: &exclusions,
        doze_user_whitelist: &doze_user,
    };

    Ok(builder.build(profile))
}

#[tauri::command]
pub async fn apply_profile(
    state: State<'_, Arc<AppState>>,
    serial: String,
    profile: Profile,
    user_excludes: Option<Vec<String>>,
) -> std::result::Result<OptimizationReport, IpcError> {
    inner_apply_profile(state.inner().clone(), serial, profile, user_excludes.unwrap_or_default())
        .await
        .map_err(Into::into)
}

async fn inner_apply_profile(
    state: Arc<AppState>,
    serial: String,
    profile: Profile,
    user_excludes: Vec<String>,
) -> Result<OptimizationReport> {
    let preview = inner_preview_profile(state.clone(), serial.clone(), profile, user_excludes).await?;
    inner_apply(state, serial, preview.actions).await
}

// ===========================================================================
// V2 commands â€” Overview, Telemetry, Sleep, Battery, Miscategorized
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewSnapshot {
    pub device_serial: DeviceSerial,
    pub sdk_int: u32,
    pub battery: BatteryHealth,
    pub sleep_score: SleepScore,
    pub zombie_count: u32,
    pub hog_candidate_count: u32,
    pub active_bucket_apps: u32,
    pub ram_used_mb: Option<u64>,
    pub ram_total_mb: Option<u64>,
    pub top_offender: Option<crate::heuristics::proxy_detector::CulpritRanking>,
}

#[tauri::command]
pub async fn overview_snapshot(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<OverviewSnapshot, IpcError> {
    inner_overview(state.inner().clone(), serial).await.map_err(Into::into)
}

async fn inner_overview(state: Arc<AppState>, serial: String) -> Result<OverviewSnapshot> {
    let serial = checked_serial(&serial)?;
    let identity = state.adb.build_identity(&serial).await?;

    // 1. Battery (sysfs first, dumpsys fallback)
    let battery = read_battery_health(&state.adb, &serial).await;

    // 2. Sleep score inputs
    let api = identity.sdk_int;
    let bs_raw = state.adb.invoker
        .shell(&serial, "dumpsys batterystats --checkin", Duration::from_secs(40))
        .await.unwrap_or_default();
    let wakelocks = BatteryStatsParser::for_api(api).parse(&bs_raw).unwrap_or_default();

    let alarm_raw = state.adb.invoker
        .shell(&serial, "dumpsys alarm", Duration::from_secs(20))
        .await.unwrap_or_default();
    let alarms = AlarmParser.parse(&alarm_raw).unwrap_or_default();

    let jobs_raw = state.adb.invoker
        .shell(&serial, "dumpsys jobscheduler", Duration::from_secs(20))
        .await.unwrap_or_default();
    let jobs = JobSchedulerParser.parse(&jobs_raw).unwrap_or_default();

    let idle_raw = state.adb.invoker
        .shell(&serial, "dumpsys deviceidle", Duration::from_secs(10))
        .await.unwrap_or_default();
    let doze = DeviceIdleParser.parse(&idle_raw).unwrap_or(DozeWhitelist {
        user_whitelisted: Vec::new(), system_whitelisted: Vec::new(),
    });

    let sensors_raw = state.adb.invoker
        .shell(&serial, "dumpsys sensorservice", Duration::from_secs(10))
        .await.unwrap_or_default();
    let sensors = SensorServiceParser.parse(&sensors_raw).unwrap_or_default();

    let sleep_score = SleepScoreCalc {
        wakelocks: &wakelocks,
        alarms: &alarms,
        doze: &doze,
        sensors: &sensors,
    }.compute();

    // 3. Process snapshot for zombie/hog counts
    let top_raw = state.adb.invoker
        .shell(&serial, ProcessStatusParser::command(), Duration::from_secs(10))
        .await.unwrap_or_default();
    let proc_snap = ProcessStatusParser::parse(&top_raw).unwrap_or_default();

    // 4. Active bucket apps count (cheap: just the standby parser)
    let standby_raw = state.adb.invoker
        .shell(&serial, "dumpsys usagestats", Duration::from_secs(15))
        .await.unwrap_or_default();
    let standby = StandbyParser.parse(&standby_raw).unwrap_or_default();
    let active_count = standby.iter()
        .filter(|s| matches!(
            s.bucket,
            crate::parsers::StandbyBucket::Active
                | crate::parsers::StandbyBucket::WorkingSet
        ))
        .count() as u32;

    // 5. RAM via /proc/meminfo
    let (ram_used_mb, ram_total_mb) = read_meminfo(&state.adb, &serial).await;

    // 6. Top offender (re-use proxy_detector::rank)
    let culprits = crate::heuristics::proxy_detector::rank(&wakelocks, &alarms, &jobs);
    let top_offender = culprits.into_iter().next();

    Ok(OverviewSnapshot {
        device_serial: serial,
        sdk_int: identity.sdk_int,
        battery,
        sleep_score,
        zombie_count: proc_snap.zombie_count,
        hog_candidate_count: proc_snap.hog_candidate_count,
        active_bucket_apps: active_count,
        ram_used_mb,
        ram_total_mb,
        top_offender,
    })
}

async fn read_battery_health(adb: &crate::adb::AdbClient, serial: &DeviceSerial) -> BatteryHealth {
    // Try sysfs first
    let script = BatterySysfsParser::read_script();
    if let Ok(out) = adb.invoker.shell(serial, script, Duration::from_secs(8)).await {
        if let Ok(h) = BatterySysfsParser::parse(&out) {
            if h.source.is_some() {
                return h;
            }
        }
    }
    // Fallback to dumpsys battery
    if let Ok(out) = adb.invoker.shell(serial, "dumpsys battery", Duration::from_secs(8)).await {
        return BatterySysfsParser::parse_dumpsys(&out);
    }
    BatteryHealth::default()
}

async fn read_meminfo(adb: &crate::adb::AdbClient, serial: &DeviceSerial) -> (Option<u64>, Option<u64>) {
    let Ok(raw) = adb.invoker.shell(serial, "cat /proc/meminfo", Duration::from_secs(5)).await else {
        return (None, None);
    };
    let mut total = None;
    let mut available = None;
    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = parse_kb_to_mb(v);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            available = parse_kb_to_mb(v);
        }
    }
    let used = match (total, available) {
        (Some(t), Some(a)) => Some(t.saturating_sub(a)),
        _ => None,
    };
    (used, total)
}

fn parse_kb_to_mb(raw: &str) -> Option<u64> {
    let raw = raw.trim().trim_end_matches("kB").trim();
    raw.parse::<u64>().ok().map(|kb| kb / 1024)
}

#[tauri::command]
pub async fn battery_health(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<BatteryHealth, IpcError> {
    let serial = checked_serial(&serial)?;
    Ok(read_battery_health(&state.adb, &serial).await)
}

#[tauri::command]
pub async fn process_status(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<ProcessSnapshot, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = state.adb.invoker
        .shell(&serial, ProcessStatusParser::command(), Duration::from_secs(10))
        .await.map_err(IpcError::from)?;
    ProcessStatusParser::parse(&raw).map_err(IpcError::from)
}

#[tauri::command]
pub async fn start_telemetry_stream(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    serial: String,
    interval_secs: Option<u64>,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    streams::start(
        state.stream_state.clone(),
        state.adb.clone(),
        app,
        serial,
        interval_secs.unwrap_or(3),
    ).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_telemetry_stream(
    state: State<'_, Arc<AppState>>,
) -> std::result::Result<(), IpcError> {
    streams::stop(state.stream_state.clone()).await;
    Ok(())
}

#[tauri::command]
pub async fn miscategorized_apps(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<MiscategorizedApp>, IpcError> {
    let serial = checked_serial(&serial)?;

    let usage_raw = state.adb.invoker
        .shell(&serial, UsageStatsParser::command(), Duration::from_secs(15))
        .await.unwrap_or_default();
    let usage = UsageStatsParser::parse(&usage_raw).unwrap_or_default();

    let standby_raw = state.adb.invoker
        .shell(&serial, "dumpsys usagestats", Duration::from_secs(15))
        .await.unwrap_or_default();
    let standby = StandbyParser.parse(&standby_raw).unwrap_or_default();

    let detector = MiscategorizedDetector::new(&usage, &standby);
    Ok(detector.run())
}

#[tauri::command]
pub async fn sleep_score(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<SleepScore, IpcError> {
    let serial = checked_serial(&serial)?;
    let identity = state.adb.build_identity(&serial).await?;
    let api = identity.sdk_int;

    let bs_raw = state.adb.invoker
        .shell(&serial, "dumpsys batterystats --checkin", Duration::from_secs(40))
        .await.unwrap_or_default();
    let wakelocks = BatteryStatsParser::for_api(api).parse(&bs_raw).unwrap_or_default();
    let alarm_raw = state.adb.invoker
        .shell(&serial, "dumpsys alarm", Duration::from_secs(20))
        .await.unwrap_or_default();
    let alarms = AlarmParser.parse(&alarm_raw).unwrap_or_default();
    let idle_raw = state.adb.invoker
        .shell(&serial, "dumpsys deviceidle", Duration::from_secs(10))
        .await.unwrap_or_default();
    let doze = DeviceIdleParser.parse(&idle_raw).unwrap_or(DozeWhitelist {
        user_whitelisted: Vec::new(), system_whitelisted: Vec::new(),
    });
    let sensors_raw = state.adb.invoker
        .shell(&serial, "dumpsys sensorservice", Duration::from_secs(10))
        .await.unwrap_or_default();
    let sensors = SensorServiceParser.parse(&sensors_raw).unwrap_or_default();

    Ok(SleepScoreCalc {
        wakelocks: &wakelocks,
        alarms: &alarms,
        doze: &doze,
        sensors: &sensors,
    }.compute())
}

#[tauri::command]
pub async fn read_action_log(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> std::result::Result<Vec<crate::telemetry::ActionLogEntry>, IpcError> {
    state.action_log.tail(limit.unwrap_or(50)).map_err(Into::into)
}

// ===========================================================================
// V2.1 commands â€” Privacy module (DNS, firewall by app, clipboard guard)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyState {
    pub dns: PrivateDnsState,
    pub scan: PrivacyScan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsPresetDto {
    pub label: String,
    pub hostname: String,
    pub blocks_ads: bool,
    pub blocks_trackers: bool,
}

#[tauri::command]
pub async fn list_dns_presets() -> std::result::Result<Vec<DnsPresetDto>, IpcError> {
    Ok(DNS_PRESETS
        .iter()
        .map(|p| DnsPresetDto {
            label: p.label.to_string(),
            hostname: p.hostname.to_string(),
            blocks_ads: p.blocks_ads,
            blocks_trackers: p.blocks_trackers,
        })
        .collect())
}

#[tauri::command]
pub async fn get_dangerous_permissions(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<DangerousPermissionEntry>, IpcError> {
    let serial = checked_serial(&serial)?;
    let appops_raw = state
        .adb
        .invoker
        .shell(&serial, PrivacyOpsScanner::command(), Duration::from_secs(20))
        .await
        .unwrap_or_default();
    
    let apps = DangerousPermissionsScanner::parse(&appops_raw).unwrap_or_default();
    
    // Filter to only user-installed apps
    let all_packages = list_packages(state.clone(), serial.as_str().to_string()).await?;
    let third_party: Vec<String> = all_packages.into_iter().filter(|p| !p.is_system).map(|p| p.name.to_string()).collect();
    
    let filtered: Vec<DangerousPermissionEntry> = apps.into_iter().filter(|app| third_party.contains(&app.package.to_string())).collect();
    
    Ok(filtered)
}

#[tauri::command]
pub async fn get_privacy_state(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<PrivacyState, IpcError> {
    let serial = checked_serial(&serial)?;

    // 1. Private DNS state â€” two `settings get` calls in parallel.
    let mode_fut = state.adb.invoker.shell(
        &serial, "settings get global private_dns_mode", Duration::from_secs(5),
    );
    let host_fut = state.adb.invoker.shell(
        &serial, "settings get global private_dns_specifier", Duration::from_secs(5),
    );
    let (mode_raw, host_raw) = tokio::join!(mode_fut, host_fut);
    let dns = PrivateDnsState::parse(
        &mode_raw.unwrap_or_default(),
        &host_raw.unwrap_or_default(),
    );

    // 2. Privacy ops scan
    let appops_raw = state.adb.invoker
        .shell(&serial, PrivacyOpsScanner::command(), Duration::from_secs(20))
        .await
        .unwrap_or_default();
    let scan = PrivacyOpsScanner::parse(&appops_raw).unwrap_or_default();

    Ok(PrivacyState { dns, scan })
}

#[tauri::command]
pub async fn set_private_dns(
    state: State<'_, Arc<AppState>>,
    serial: String,
    mode: PrivateDnsMode,
    hostname: Option<String>,
) -> std::result::Result<OptimizationReport, IpcError> {
    // Sanity-check hostname when mode is Hostname.
    if matches!(mode, PrivateDnsMode::Hostname) {
        let host = hostname.as_deref().unwrap_or("").trim();
        if host.is_empty() || !host.contains('.') {
            return Err(IpcError {
                kind: "validation".to_string(),
                message: "Hostname mode requires a valid DNS-over-TLS hostname (e.g. dns.adguard-dns.com)".to_string(),
            });
        }
    }

    let actions = vec![OptimizationAction::SetPrivateDns { mode, hostname }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Apply or remove background-network restriction on a set of packages.
/// `block=true` â†’ set RUN_ANY_IN_BACKGROUND=ignore; `block=false` â†’ restore to default.
#[tauri::command]
pub async fn apply_firewall(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
    block: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let target_mode = if block { crate::parsers::AppOpMode::Ignore } else { crate::parsers::AppOpMode::Default };
    let mut actions: Vec<OptimizationAction> = Vec::with_capacity(packages.len() * 2);
    for pkg in packages {
        let pkg_name = PackageName(pkg);
        actions.push(OptimizationAction::SetAppOp {
            package: pkg_name.clone(),
            op: "RUN_ANY_IN_BACKGROUND".to_string(),
            mode: target_mode,
        });
        actions.push(OptimizationAction::SetAppOp {
            package: pkg_name,
            op: "RUN_IN_BACKGROUND".to_string(),
            mode: target_mode,
        });
    }
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Apply or remove clipboard read guard on a set of packages.
#[tauri::command]
pub async fn apply_clipboard_guard(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
    block: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let target_mode = if block { crate::parsers::AppOpMode::Ignore } else { crate::parsers::AppOpMode::Default };
    let actions: Vec<OptimizationAction> = packages
        .into_iter()
        .map(|pkg| OptimizationAction::SetAppOp {
            package: PackageName(pkg),
            op: "READ_CLIPBOARD".to_string(),
            mode: target_mode,
        })
        .collect();
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

// ===========================================================================
// V2.2 commands â€” Storage module (inventory, trim, clear-cache, dexopt)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOverviewDto {
    pub diskstats: DiskStats,
    pub inventory_total_bytes: u64,
    pub inventory_count: u32,
}

#[tauri::command]
pub async fn storage_overview(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<StorageOverviewDto, IpcError> {
    let serial = checked_serial(&serial)?;

    // diskstats can be slow on large /data partitions (does a benchmark)
    let raw = state.adb.invoker
        .shell(&serial, DiskStatsParser::command(), Duration::from_secs(20))
        .await.unwrap_or_default();
    let mut diskstats = DiskStatsParser::parse(&raw).unwrap_or_default();

    // Android 14+ fallback: df /data
    if let Ok(df_raw) = state.adb.invoker.shell(&serial, "df /data", Duration::from_secs(5)).await {
        for line in df_raw.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(total_1k), Ok(free_1k)) = (parts[1].parse::<u64>(), parts[3].parse::<u64>()) {
                    diskstats.data_total_bytes = Some(total_1k * 1024);
                    diskstats.data_free_bytes = Some(free_1k * 1024);
                    break;
                }
            }
        }
    }

    // Android 14+ fallback: df /cache
    if let Ok(df_raw) = state.adb.invoker.shell(&serial, "df /cache", Duration::from_secs(5)).await {
        for line in df_raw.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(total_1k), Ok(free_1k)) = (parts[1].parse::<u64>(), parts[3].parse::<u64>()) {
                    diskstats.cache_total_bytes = Some(total_1k * 1024);
                    diskstats.cache_free_bytes = Some(free_1k * 1024);
                    break;
                }
            }
        }
    }

    // Inventory totals — light-weight: just sum APKs counted earlier or 0.
    // We don't run the full inventory here to keep overview snappy.
    Ok(StorageOverviewDto {
        diskstats,
        inventory_total_bytes: 0,
        inventory_count: 0,
    })
}

#[tauri::command]
pub async fn storage_inventory(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<PackageSize>, IpcError> {
    let serial = checked_serial(&serial)?;
    
    // Fetch both in parallel: pm list packages -f, and du -sk
    let pm_fut = state.adb.invoker.shell(&serial, "pm list packages -f 2>/dev/null", Duration::from_secs(10));
    let du_fut = state.adb.invoker.shell(&serial, "find /data/app /system/app /system/priv-app /product/app /product/priv-app /vendor/app -maxdepth 2 -type d 2>/dev/null | xargs du -sk 2>/dev/null || true", Duration::from_secs(15));
    
    let (pm_res, du_res) = tokio::join!(pm_fut, du_fut);
    
    let pm_raw = pm_res.map_err(IpcError::from)?;
    let du_raw = du_res.unwrap_or_default(); // du might fail or return nothing if paths don't exist

    PackageSizesScanner::parse(&pm_raw, &du_raw).map_err(IpcError::from)
}

#[tauri::command]
pub async fn clear_app_cache(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions: Vec<OptimizationAction> = packages
        .into_iter()
        .map(|p| OptimizationAction::ClearAppCache { package: PackageName(p) })
        .collect();
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn trim_system_caches(
    state: State<'_, Arc<AppState>>,
    serial: String,
    target_free_bytes: u64,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::TrimSystemCaches { target_free_bytes }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn run_bg_dexopt(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::RunBgDexopt];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}


// ===========================================================================
// V2.3 commands â€” Display & Audio tuning (refresh rate, BT abs volume)
// ===========================================================================

#[tauri::command]
pub async fn get_display_settings(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<DisplaySettings, IpcError> {
    let serial = checked_serial(&serial)?;

    // 7 parallel shell reads - none of these block on the device.
    let min_fut     = state.adb.invoker.shell(&serial, "settings get system min_refresh_rate",  Duration::from_secs(5));
    let peak_fut    = state.adb.invoker.shell(&serial, "settings get system peak_refresh_rate", Duration::from_secs(5));
    let bt_fut      = state.adb.invoker.shell(&serial, "getprop persist.bluetooth.disableabsolutevolume", Duration::from_secs(5));
    let fb_fut      = state.adb.invoker.shell(&serial, "getprop ro.surface_flinger.max_frame_buffer_acquired_buffers", Duration::from_secs(5));
    let mono_fut    = state.adb.invoker.shell(&serial, "settings get system master_mono", Duration::from_secs(5));
    let spatial_fut = state.adb.invoker.shell(&serial, "settings get secure spatial_audio_enabled", Duration::from_secs(5));
    let avrcp_fut   = state.adb.invoker.shell(&serial, "getprop persist.bluetooth.avrcpversion", Duration::from_secs(5));

    let (min, peak, bt, fb, mono, spatial, avrcp) =
        tokio::join!(min_fut, peak_fut, bt_fut, fb_fut, mono_fut, spatial_fut, avrcp_fut);

    Ok(DisplaySettings::from_parts(
        &min.unwrap_or_default(),
        &peak.unwrap_or_default(),
        &bt.unwrap_or_default(),
        &fb.unwrap_or_default(),
        &mono.unwrap_or_default(),
        &spatial.unwrap_or_default(),
        &avrcp.unwrap_or_default(),
    ))
}

#[tauri::command]
pub async fn apply_refresh_rate(
    state: State<'_, Arc<AppState>>,
    serial: String,
    min_rate: Option<f32>,
    peak_rate: Option<f32>,
) -> std::result::Result<OptimizationReport, IpcError> {
    let mut actions: Vec<OptimizationAction> = Vec::new();
    if let Some(r) = min_rate {
        if !r.is_finite() || r < 1.0 || r > 240.0 {
            return Err(IpcError {
                kind: "validation".to_string(),
                message: format!("min_refresh_rate must be in 1.0..=240.0, got {r}"),
            });
        }
        actions.push(OptimizationAction::SetMinRefreshRate { rate: r });
    }
    if let Some(r) = peak_rate {
        if !r.is_finite() || r < 1.0 || r > 240.0 {
            return Err(IpcError {
                kind: "validation".to_string(),
                message: format!("peak_refresh_rate must be in 1.0..=240.0, got {r}"),
            });
        }
        actions.push(OptimizationAction::SetPeakRefreshRate { rate: r });
    }
    if actions.is_empty() {
        return Err(IpcError {
            kind: "validation".to_string(),
            message: "no refresh rate values provided".to_string(),
        });
    }
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_bluetooth_absolute_volume(
    state: State<'_, Arc<AppState>>,
    serial: String,
    disabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetBluetoothAbsoluteVolume { disabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

// ===========================================================================
// V2.4 commands — Block H: Phantom killer, Captive portal, ART compile
// ===========================================================================

#[tauri::command]
pub async fn get_system_tweaks(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<SystemTweaks, IpcError> {
    let serial = checked_serial(&serial)?;
    let phantom_fut = state.adb.invoker.shell(
        &serial, "settings get global settings_enable_monitor_phantom_procs", Duration::from_secs(5),
    );
    let captive_fut = state.adb.invoker.shell(
        &serial, "settings get global captive_portal_mode", Duration::from_secs(5),
    );
    let max_fut = state.adb.invoker.shell(
        &serial, "device_config get activity_manager max_phantom_processes", Duration::from_secs(5),
    );
    let (p, c, m) = tokio::join!(phantom_fut, captive_fut, max_fut);
    Ok(SystemTweaks::from_parts(
        &p.unwrap_or_default(),
        &c.unwrap_or_default(),
        &m.unwrap_or_default(),
    ))
}

#[tauri::command]
pub async fn set_phantom_monitor(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetPhantomMonitor { enabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_captive_portal_mode(
    state: State<'_, Arc<AppState>>,
    serial: String,
    disabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetCaptivePortalMode { disabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Valid `pm compile` modes per Android source (PackageManagerShellCommand.java).
const VALID_COMPILE_MODES: &[&str] = &[
    "speed",
    "speed-profile",
    "verify",
    "quicken",
    "everything",
    "extract",
];

#[tauri::command]
pub async fn compile_package(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    mode: String,
) -> std::result::Result<OptimizationReport, IpcError> {
    if !VALID_COMPILE_MODES.contains(&mode.as_str()) {
        return Err(IpcError {
            kind: "validation".to_string(),
            message: format!(
                "invalid compile mode '{mode}'. Valid: {}",
                VALID_COMPILE_MODES.join(", ")
            ),
        });
    }
    let pkg = package.trim();
    if pkg.is_empty() {
        return Err(IpcError {
            kind: "validation".to_string(),
            message: "package name is empty".to_string(),
        });
    }
    let actions = vec![OptimizationAction::CompilePackage {
        package: PackageName(pkg.to_string()),
        mode,
    }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn reset_compilation(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
) -> std::result::Result<OptimizationReport, IpcError> {
    let pkg = package.trim();
    if pkg.is_empty() {
        return Err(IpcError {
            kind: "validation".to_string(),
            message: "package name is empty".to_string(),
        });
    }
    let actions = vec![OptimizationAction::ResetCompilation {
        package: PackageName(pkg.to_string()),
    }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}
fn sanitise_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

// ===========================================================================
// V2.5 commands — Sleep timeline / Kernel wakelocks / Per-app battery drain
// ===========================================================================

#[tauri::command]
pub async fn sleep_timeline(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<crate::parsers::sleep_timeline::SleepTimeline, IpcError> {
    use crate::parsers::sleep_timeline::SleepTimelineParser;
    let serial = checked_serial(&serial)?;
    let raw = state
        .adb
        .invoker
        .shell(&serial, "dumpsys batterystats", Duration::from_secs(30))
        .await
        .map_err(IpcError::from)?;
    SleepTimelineParser.parse(&raw).map_err(IpcError::from)
}

#[tauri::command]
pub async fn kernel_wakelocks(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<crate::parsers::kernel_wakelocks::KernelWakelock>, IpcError> {
    use crate::parsers::kernel_wakelocks::KernelWakelocksParser;
    let serial = checked_serial(&serial)?;

    // Primary: parse the "Wakeup reasons" / "Kernel Wakelocks" section of
    // dumpsys batterystats. Works on API 30-34+, all OEMs.
    let raw = state
        .adb
        .invoker
        .shell(&serial, "dumpsys batterystats", Duration::from_secs(30))
        .await
        .map_err(IpcError::from)?;
    let primary = KernelWakelocksParser.parse(&raw).unwrap_or_default();
    if !primary.is_empty() {
        return Ok(primary);
    }

    // Fallback: read /proc/wakelocks directly. Triggered when batterystats
    // has been freshly reset (e.g. user just unplugged from charger for the
    // first time today) or when the vendor stripped the section entirely.
    // Requires no permission - the file is world-readable on every Android.
    let proc_raw = state
        .adb
        .invoker
        .shell(&serial, "cat /proc/wakelocks 2>/dev/null", Duration::from_secs(5))
        .await
        .unwrap_or_default();
    Ok(KernelWakelocksParser::parse_proc_wakelocks(&proc_raw))
}

#[tauri::command]
pub async fn battery_per_app(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<crate::parsers::battery_drain::BatteryDrain, IpcError> {
    use std::collections::HashSet;
    use crate::parsers::battery_drain::BatteryDrainParser;
    use crate::parsers::batterystats::build_uid_to_package_map;

    let serial = checked_serial(&serial)?;

    // Parallel fetch: checkin (for UID map) + textual (for drain section).
    let checkin_fut = state.adb.invoker.shell(
        &serial,
        "dumpsys batterystats --checkin",
        Duration::from_secs(30),
    );
    let text_fut = state.adb.invoker.shell(
        &serial,
        "dumpsys batterystats",
        Duration::from_secs(30),
    );
    let power_fut = state.adb.invoker.shell(
        &serial,
        "dumpsys power",
        Duration::from_secs(10),
    );
    let top_fut = state.adb.invoker.shell(
        &serial,
        "top -b -n 1 -q -o PID,USER,STAT,PCPU,RSS,ARGS",
        Duration::from_secs(15),
    );

    let (checkin_res, text_res, power_res, top_res) =
        tokio::join!(checkin_fut, text_fut, power_fut, top_fut);

    let checkin = checkin_res.map_err(IpcError::from)?;
    let text = text_res.map_err(IpcError::from)?;

    // Build cross-reference sets:
    //   - live wakelock packages (from `dumpsys power`)
    //   - zombie process packages (from `top -b`)
    let live_wakelock_pkgs: HashSet<String> = match power_res {
        Ok(raw) => crate::parsers::power::PowerParser
            .parse(&raw)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|wl| wl.package.map(|p| p.0))
            .collect(),
        Err(_) => HashSet::new(),
    };
    let zombie_pkgs: HashSet<String> = match top_res {
        Ok(raw) => crate::parsers::process_status::ProcessStatusParser::parse(&raw)
            .map(|snap| {
                snap.rows
                    .into_iter()
                    .filter(|r| r.is_zombie)
                    .filter_map(|r| r.package)
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => HashSet::new(),
    };

    let uid_to_pkg = build_uid_to_package_map(&checkin);
    BatteryDrainParser { uid_to_pkg, live_wakelock_pkgs, zombie_pkgs }
        .parse(&text)
        .map_err(IpcError::from)
}


// ===========================================================================
// V2.6 commands - App labels resolver / Audio extras / Bloatware presets
// ===========================================================================

/// Resolves human-readable application labels for every package on the
/// device in one ADB round-trip. Expensive (35-90 s on a Pixel 8 Pro with
/// 400+ packages); callers should cache aggressively.
#[tauri::command]
pub async fn resolve_app_labels(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<std::collections::HashMap<String, String>, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = state
        .adb
        .invoker
        .shell(&serial, AppLabelsResolver::command(), Duration::from_secs(120))
        .await
        .map_err(IpcError::from)?;
    AppLabelsResolver.parse(&raw).map_err(IpcError::from)
}

/// Forces both stereo channels into one (`master_mono`). Useful for users
/// with hearing loss in one ear, or for single-earbud listening.
#[tauri::command]
pub async fn set_master_mono(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetMasterMono { enabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Toggles head-tracked / spatial audio (Pixel Buds Pro, AirPods on
/// Android 13+, Sony WH-1000XM5, etc). No-op on hardware that does not
/// support it.
#[tauri::command]
pub async fn set_spatial_audio(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetSpatialAudio { enabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Valid AVRCP profile versions (Android source: `BluetoothA2dp.cpp`).
const VALID_AVRCP_VERSIONS: &[&str] = &["avrcp13", "avrcp14", "avrcp15", "avrcp16"];

/// Pins the AVRCP version used over Bluetooth. Take effect after
/// re-pairing the headset.
#[tauri::command]
pub async fn set_avrcp_version(
    state: State<'_, Arc<AppState>>,
    serial: String,
    version: String,
) -> std::result::Result<OptimizationReport, IpcError> {
    let v = version.trim().to_ascii_lowercase();
    if !VALID_AVRCP_VERSIONS.contains(&v.as_str()) {
        return Err(IpcError {
            kind: "validation".to_string(),
            message: format!(
                "invalid AVRCP version '{version}'. Valid: {}",
                VALID_AVRCP_VERSIONS.join(", ")
            ),
        });
    }
    let actions = vec![OptimizationAction::SetAvrcpVersion { version: v }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

/// Returns one `BloatwareRecommendation` per installed package, enriched
/// with the plain-language `notes` field and the category tag used by
/// presets. Cheap: this is a pure-Rust pass over the verdict list.
#[tauri::command]
pub async fn bloatware_recommendations(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<BloatwareRecommendation>, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial)
        .await
        .map_err(IpcError::from)?;
    let installed = PackageListParser.parse(&raw).map_err(IpcError::from)?;
    let manifest = state.manifest.read().await;
    let verdicts: Vec<crate::heuristics::risk::PackageVerdict> = installed
        .iter()
        .map(|p| crate::heuristics::risk::classify(p, &manifest))
        .collect();
    Ok(verdicts.iter().map(bloatware_recommendation::recommend).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloatPresetDto {
    pub id: BloatPreset,
    pub label: String,
    pub description: String,
}

/// Lists every preset with its English label + description so the UI can
/// render them without knowing the enum.
#[tauri::command]
pub async fn list_bloat_presets() -> std::result::Result<Vec<BloatPresetDto>, IpcError> {
    Ok(BloatPreset::all()
        .iter()
        .map(|p| BloatPresetDto {
            id: *p,
            label: p.label().to_string(),
            description: p.description().to_string(),
        })
        .collect())
}

/// Returns the list of packages that match a preset on this specific
/// device. Use this to populate the selection before calling
/// `disable_bloatware`.
#[tauri::command]
pub async fn preview_bloat_preset(
    state: State<'_, Arc<AppState>>,
    serial: String,
    preset: BloatPreset,
) -> std::result::Result<Vec<String>, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = safe_pm_list_packages(&state.adb.invoker, &serial)
        .await
        .map_err(IpcError::from)?;
    let installed = PackageListParser.parse(&raw).map_err(IpcError::from)?;
    let manifest = state.manifest.read().await;
    let recs: Vec<BloatwareRecommendation> = installed
        .iter()
        .map(|p| crate::heuristics::risk::classify(p, &manifest))
        .map(|v| bloatware_recommendation::recommend(&v))
        .collect();
    Ok(bloatware_recommendation::packages_for_preset(preset, &recs))
}

// ===========================================================================
// V2.7 commands - Advanced Optimizations (Performance & Background)
// ===========================================================================

use crate::parsers::performance_settings::PerformanceSettings;

#[tauri::command]
pub async fn get_performance_settings(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<PerformanceSettings, IpcError> {
    let serial = checked_serial(&serial)?;
    // Execute all reads concurrently for speed
    let (s_anim, s_trans, s_dur, s_bg, s_wifi, s_ble, s_doze, s_net) = tokio::join!(
        state.adb.invoker.shell(&serial, "settings get global window_animation_scale", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global transition_animation_scale", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global animator_duration_scale", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global background_process_limit", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global wifi_scan_always_enabled", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global ble_scan_always_enabled", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "settings get global device_idle_constants", Duration::from_secs(3)),
        state.adb.invoker.shell(&serial, "cmd netpolicy get restrict-background", Duration::from_secs(3))
    );

    Ok(PerformanceSettings::from_parts(
        &s_anim.unwrap_or_default(),
        &s_trans.unwrap_or_default(),
        &s_dur.unwrap_or_default(),
        &s_bg.unwrap_or_default(),
        &s_wifi.unwrap_or_default(),
        &s_ble.unwrap_or_default(),
        &s_doze.unwrap_or_default(),
        &s_net.unwrap_or_default(),
    ))
}

#[tauri::command]
pub async fn set_animation_scales(
    state: State<'_, Arc<AppState>>,
    serial: String,
    scale: f32,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetAnimationScales { scale }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_aggressive_doze(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetAggressiveDoze { enabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_background_scan(
    state: State<'_, Arc<AppState>>,
    serial: String,
    wifi: bool,
    ble: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetBackgroundScan { wifi, ble }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_data_saver(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetDataSaver { enabled }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn hibernate_package(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    hibernate: bool,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::HibernatePackage { package: PackageName(package), hibernate }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_game_mode(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    mode: u8,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetGameMode { package: PackageName(package), mode }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

#[tauri::command]
pub async fn set_background_process_limit(
    state: State<'_, Arc<AppState>>,
    serial: String,
    limit: Option<u32>,
) -> std::result::Result<OptimizationReport, IpcError> {
    let actions = vec![OptimizationAction::SetBackgroundProcessLimit { limit }];
    inner_apply(state.inner().clone(), serial, actions).await.map_err(Into::into)
}

use crate::parsers::deviceidle::{DozeState, DozeStateParser};

#[tauri::command]
pub async fn get_doze_state(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<DozeState, IpcError> {
    let serial = checked_serial(&serial)?;
    let raw = state.adb.invoker
        .shell(&serial, "dumpsys deviceidle", Duration::from_secs(10))
        .await
        .unwrap_or_default();
    DozeStateParser.parse(&raw).map_err(Into::into)
}

#[tauri::command]
pub async fn set_doze_whitelist(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    add: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    let op = if add { "+" } else { "-" };
    state.adb.invoker
        .shell(&serial, &format!("dumpsys deviceidle whitelist {}{}", op, package), Duration::from_secs(5))
        .await
        .map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_force_doze(
    state: State<'_, Arc<AppState>>,
    serial: String,
    force: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let cmd = if force { "dumpsys deviceidle force-idle" } else { "dumpsys deviceidle unforce" };
    state.adb.invoker.shell(&serial, cmd, Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn simulate_unplug(
    state: State<'_, Arc<AppState>>,
    serial: String,
    unplug: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let cmd = if unplug { "dumpsys battery unplug" } else { "dumpsys battery reset" };
    state.adb.invoker.shell(&serial, cmd, Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn get_art_status_batch(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<HashMap<String, String>, IpcError> {
    let serial = checked_serial(&serial)?;
    for pkg in &packages {
        crate::security::validate_pkg(pkg)?;
    }
    let mut tasks = Vec::new();

    for pkg in packages {
        let state_clone = state.inner().clone();
        let serial_clone = serial.clone();
        tasks.push(tokio::spawn(async move {
            let out = state_clone.adb.invoker.shell(&serial_clone, &format!("cmd package dump {} | grep -A2 compile", pkg), Duration::from_secs(5)).await.unwrap_or_default();
            
            let mut status = "unknown".to_string();
            for line in out.lines() {
                if let Some(idx) = line.find("status=") {
                    status = line[idx+7..].trim().to_string();
                    break;
                }
            }
            (pkg, status)
        }));
    }

    let results = futures::future::join_all(tasks).await;
    let mut map = HashMap::new();
    for res in results {
        if let Ok((pkg, status)) = res {
            map.insert(pkg, status);
        }
    }
    
    Ok(map)
}

#[tauri::command]
pub async fn clear_temp_files(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    state.adb.invoker.shell(&serial, "rm -rf /data/local/tmp/*", Duration::from_secs(10)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn get_all_standby_buckets(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<crate::parsers::StandbyAssignment>, IpcError> {
    let serial = checked_serial(&serial)?;
    let output = state.adb.invoker.shell(&serial, "dumpsys usagestats", Duration::from_secs(10)).await.map_err(IpcError::from)?;
    let parsed = crate::parsers::standby::StandbyParser.parse(&output).map_err(IpcError::from)?;
    Ok(parsed)
}

#[tauri::command]
pub async fn set_standby_bucket(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    bucket: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    crate::security::validate_token(&bucket)?;
    state.adb.invoker.shell(&serial, &format!("am set-standby-bucket {} {}", package, bucket), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_appops(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    op: String,
    mode: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    crate::security::validate_op_name(&op)?;
    crate::security::validate_token(&mode)?;
    state.adb.invoker.shell(&serial, &format!("appops set {} {} {}", package, op, mode), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn force_stop_package(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    state.adb.invoker.shell(&serial, &format!("am force-stop {}", package), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn open_app_settings(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    state.adb.invoker.shell(&serial, &format!("am start -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:{}", package), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRestrictions {
    pub package: String,
    pub wake_lock_ignored: bool,
    pub run_in_background_ignored: bool,
    pub standby_bucket: String,
}

#[tauri::command]
pub async fn get_app_restrictions_batch(
    state: State<'_, Arc<AppState>>,
    serial: String,
    packages: Vec<String>,
) -> std::result::Result<HashMap<String, AppRestrictions>, IpcError> {
    let serial = checked_serial(&serial)?;
    // Validate every package name once, up front; reject the whole batch on any failure
    // rather than partially executing dangerous shell strings.
    for pkg_raw in &packages {
        crate::security::validate_pkg(pkg_raw)?;
    }
    let mut tasks = tokio::task::JoinSet::new();

    for pkg_raw in packages {
        let state_clone = state.inner().clone();
        let ser = serial.clone();
        
        tasks.spawn(async move {
            let pkg = crate::parsers::PackageName(pkg_raw.clone());
            let mut restrictions = AppRestrictions {
                package: pkg_raw.clone(),
                wake_lock_ignored: false,
                run_in_background_ignored: false,
                standby_bucket: "unknown".to_string(),
            };

            // 1. AppOps
            if let Ok(raw_ops) = state_clone.adb.invoker.shell(
                &ser, &format!("cmd appops get {}", pkg), Duration::from_secs(5)
            ).await {
                let parser = crate::parsers::AppOpsParser { package: pkg.clone() };
                if let Ok(ops) = crate::parsers::Parser::parse(&parser, &raw_ops) {
                    for op in ops {
                        if op.op == "WAKE_LOCK" && matches!(op.mode, crate::parsers::AppOpMode::Ignore) {
                            restrictions.wake_lock_ignored = true;
                        }
                        if (op.op == "RUN_IN_BACKGROUND" || op.op == "RUN_ANY_IN_BACKGROUND") && matches!(op.mode, crate::parsers::AppOpMode::Ignore) {
                            restrictions.run_in_background_ignored = true;
                        }
                    }
                }
            }

            // 2. Standby Bucket
            if let Ok(raw_bucket) = state_clone.adb.invoker.shell(
                &ser, &format!("am get-standby-bucket {}", pkg), Duration::from_secs(5)
            ).await {
                if let Ok(n) = raw_bucket.trim().parse::<i32>() {
                    if let Some(b) = crate::parsers::StandbyBucket::from_raw(n) {
                        restrictions.standby_bucket = format!("{:?}", b).to_lowercase();
                    }
                }
            }
            
            (pkg_raw, restrictions)
        });
    }

    let mut map = HashMap::new();
    while let Some(res) = tasks.join_next().await {
        if let Ok((pkg, r)) = res {
            map.insert(pkg, r);
        }
    }
    
    Ok(map)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleAppDetails {
    pub package: String,
    pub version_name: Option<String>,
    pub is_system: bool,
    pub cache_bytes: Option<u64>,
    pub data_bytes: Option<u64>,
    pub apk_bytes: Option<u64>,
    pub restrictions: AppRestrictions,
}

#[tauri::command]
pub async fn get_single_app_details(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    root_mode: bool,
) -> std::result::Result<SingleAppDetails, IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    let pkg = crate::parsers::PackageName(package.clone());
    
    let mut restrictions = AppRestrictions {
        package: package.clone(),
        wake_lock_ignored: false,
        run_in_background_ignored: false,
        standby_bucket: "unknown".to_string(),
    };
    
    let su_prefix = if root_mode { "su -c " } else { "" };

    let cmd_ops = format!("cmd appops get {}", pkg);
    let cmd_bucket = format!("am get-standby-bucket {}", pkg);
    let cmd_du_data = format!("{}du -sk /data/data/{}", su_prefix, pkg);
    let cmd_du_user = format!("{}du -sk /data/user_de/0/{}", su_prefix, pkg);
    let cmd_du_cache = format!("{}du -sk /data/data/{}/cache", su_prefix, pkg);
    let cmd_pm = format!("pm list packages -f {}", pkg);

    let ops_fut = state.adb.invoker.shell(&serial, &cmd_ops, Duration::from_secs(5));
    let bucket_fut = state.adb.invoker.shell(&serial, &cmd_bucket, Duration::from_secs(5));
    let du_data_fut = state.adb.invoker.shell(&serial, &cmd_du_data, Duration::from_secs(5));
    let du_user_fut = state.adb.invoker.shell(&serial, &cmd_du_user, Duration::from_secs(5));
    let du_cache_fut = state.adb.invoker.shell(&serial, &cmd_du_cache, Duration::from_secs(5));
    let pm_fut = state.adb.invoker.shell(&serial, &cmd_pm, Duration::from_secs(5));

    let (ops_res, bucket_res, du_data_res, du_user_res, du_cache_res, pm_res) = tokio::join!(ops_fut, bucket_fut, du_data_fut, du_user_fut, du_cache_fut, pm_fut);

    // 1. AppOps
    if let Ok(raw_ops) = ops_res {
        let parser = crate::parsers::AppOpsParser { package: pkg.clone() };
        if let Ok(ops) = crate::parsers::Parser::parse(&parser, &raw_ops) {
            for op in ops {
                if op.op == "WAKE_LOCK" && matches!(op.mode, crate::parsers::AppOpMode::Ignore) {
                    restrictions.wake_lock_ignored = true;
                }
                if (op.op == "RUN_IN_BACKGROUND" || op.op == "RUN_ANY_IN_BACKGROUND") && matches!(op.mode, crate::parsers::AppOpMode::Ignore) {
                    restrictions.run_in_background_ignored = true;
                }
            }
        }
    }

    // 2. Standby Bucket
    if let Ok(raw_bucket) = bucket_res {
        if let Ok(n) = raw_bucket.trim().parse::<i32>() {
            if let Some(b) = crate::parsers::StandbyBucket::from_raw(n) {
                restrictions.standby_bucket = format!("{:?}", b).to_lowercase();
            }
        }
    }

    // 3. Cache & Data (App data)
    let mut data_bytes = 0;
    let mut cache_bytes = 0;
    
    if let Ok(du_raw) = du_data_res {
        if let Some(line) = du_raw.lines().next() {
            if let Some(kb_str) = line.split_whitespace().next() {
                if let Ok(kb) = kb_str.parse::<u64>() {
                    data_bytes += kb * 1024;
                }
            }
        }
    }
    if let Ok(du_raw) = du_user_res {
        if let Some(line) = du_raw.lines().next() {
            if let Some(kb_str) = line.split_whitespace().next() {
                if let Ok(kb) = kb_str.parse::<u64>() {
                    data_bytes += kb * 1024;
                }
            }
        }
    }
    if let Ok(du_raw) = du_cache_res {
        if let Some(line) = du_raw.lines().next() {
            if let Some(kb_str) = line.split_whitespace().next() {
                if let Ok(kb) = kb_str.parse::<u64>() {
                    cache_bytes += kb * 1024;
                }
            }
        }
    }
    
    // 4. APK size
    let mut apk_bytes = 0;
    if let Ok(pm_raw) = pm_res {
        for line in pm_raw.lines() {
            if let Some(no_prefix) = line.trim().strip_prefix("package:") {
                if let Some((path, p)) = no_prefix.rsplit_once('=') {
                    if p == package {
                        let su_prefix_apk = if root_mode { "su -c " } else { "" };
                        if let Ok(du_apk) = state.adb.invoker.shell(&serial, &format!("{}du -sk {}", su_prefix_apk, path), Duration::from_secs(5)).await {
                            if let Some(apk_line) = du_apk.lines().next() {
                                if let Some(kb_str) = apk_line.split_whitespace().next() {
                                    if let Ok(kb) = kb_str.parse::<u64>() {
                                        apk_bytes = kb * 1024;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Version and System flag
    let mut version_name = None;
    let mut is_system = false;
    if let Ok(dumpsys_pkg) = state.adb.invoker.shell(&serial, &format!("dumpsys package {}", pkg), Duration::from_secs(5)).await {
        for line in dumpsys_pkg.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("versionName=") {
                version_name = Some(trimmed.replace("versionName=", "").to_string());
            } else if trimmed.starts_with("flags=[") || trimmed.starts_with("pkgFlags=[") {
                if trimmed.contains(" SYSTEM ") || trimmed.contains("[SYSTEM ") {
                    is_system = true;
                }
            }
        }
    }

    Ok(SingleAppDetails {
        package,
        version_name,
        is_system,
        cache_bytes: if cache_bytes > 0 { Some(cache_bytes) } else { None },
        data_bytes: if data_bytes > 0 { Some(data_bytes) } else { None },
        apk_bytes: if apk_bytes > 0 { Some(apk_bytes) } else { None },
        restrictions,
    })
}

#[tauri::command]
pub async fn clear_app_data(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    state.adb.invoker.shell(&serial, &format!("pm clear {}", package), Duration::from_secs(10)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn uninstall_package(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_pkg(&package)?;
    state.adb.invoker.shell(&serial, &format!("pm uninstall {}", package), Duration::from_secs(15)).await.map_err(IpcError::from)?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Advanced Tweaks Commands
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn compile_all_apps(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    // This can take a while, so we increase the timeout to 5 minutes
    state.adb.invoker.shell(&serial, "pm compile -a -f -m speed", Duration::from_secs(300)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn disable_ram_plus(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    // Samsung specific
    state.adb.invoker.shell(&serial, "settings put global ram_expand_size 0", Duration::from_secs(5)).await.map_err(IpcError::from)?;
    // General ZRAM
    let _ = state.adb.invoker.shell(&serial, "settings put global zram_enabled 0", Duration::from_secs(5)).await;
    Ok(())
}

#[tauri::command]
pub async fn force_refresh_rate(
    state: State<'_, Arc<AppState>>,
    serial: String,
    rate: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_dimension(&rate)?;
    state.adb.invoker.shell(&serial, &format!("settings put system min_refresh_rate {}", rate), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    state.adb.invoker.shell(&serial, &format!("settings put system peak_refresh_rate {}", rate), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_wm_size(
    state: State<'_, Arc<AppState>>,
    serial: String,
    size: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    if size.trim().is_empty() || size.trim() == "reset" {
        state.adb.invoker.shell(&serial, "wm size reset", Duration::from_secs(5)).await.map_err(IpcError::from)?;
    } else {
        crate::security::validate_dimension(&size)?;
        state.adb.invoker.shell(&serial, &format!("wm size {}", size), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_wm_density(
    state: State<'_, Arc<AppState>>,
    serial: String,
    density: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    if density.trim().is_empty() || density.trim() == "reset" {
        state.adb.invoker.shell(&serial, "wm density reset", Duration::from_secs(5)).await.map_err(IpcError::from)?;
    } else {
        crate::security::validate_dimension(&density)?;
        state.adb.invoker.shell(&serial, &format!("wm density {}", density), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn reset_display(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    state.adb.invoker.shell(&serial, "wm size reset", Duration::from_secs(5)).await.map_err(IpcError::from)?;
    state.adb.invoker.shell(&serial, "wm density reset", Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_heads_up_notifications(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global heads_up_notifications_enabled {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_hotword_detection(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global hotword_detection_enabled {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_activity_logging(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global activity_starts_logging_enabled {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_adaptive_connectivity(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put secure adaptive_connectivity_enabled {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn reboot_device(
    state: State<'_, Arc<AppState>>,
    serial: String,
    mode: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    // Allow the frontend convention "system" to mean "no extra argument".
    let mode_arg = if mode == "system" { "" } else { &mode };
    crate::security::validate_reboot_mode(mode_arg)?;
    let mut args = vec!["-s", serial.as_str(), "reboot"];
    if !mode_arg.is_empty() {
        args.push(mode_arg);
    }
    state.adb.invoker.exec(&args, Duration::from_secs(15)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_display_density(
    state: State<'_, Arc<AppState>>,
    serial: String,
    density: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_dimension(&density)?;
    state.adb.invoker.shell(&serial, &format!("wm density {}", density), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_display_size(
    state: State<'_, Arc<AppState>>,
    serial: String,
    size: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_dimension(&size)?;
    state.adb.invoker.shell(&serial, &format!("wm size {}", size), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}



#[tauri::command]
pub async fn set_window_blurs(
    state: State<'_, Arc<AppState>>,
    serial: String,
    disabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if disabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global disable_window_blurs {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_reduce_transparency(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "1" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global accessibility_reduce_transparency {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_fixed_performance_mode(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "true" } else { "false" };
    state.adb.invoker.shell(&serial, &format!("cmd power set-fixed-performance-mode-enabled {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_dark_mode(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "2" } else { "1" };
    state.adb.invoker.shell(&serial, &format!("settings put secure ui_night_mode {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn set_stay_awake(
    state: State<'_, Arc<AppState>>,
    serial: String,
    enabled: bool,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    let val = if enabled { "7" } else { "0" };
    state.adb.invoker.shell(&serial, &format!("settings put global stay_on_while_plugged_in {}", val), Duration::from_secs(5)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn capture_screenshot(
    state: State<'_, Arc<AppState>>,
    serial: String,
    save_path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    // `save_path` is a HOST path written by ADB pull. Don't validate it as an Android path,
    // but make sure it has no shell metacharacters since we pass it through `exec(&[..])`
    // which uses argv directly (no shell). Length cap is a defensive measure.
    if save_path.is_empty() || save_path.len() > 4096 {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid save_path".into() });
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_file = format!("/sdcard/screen_{}.png", nonce);

    // 1. Take screenshot
    state.adb.invoker.shell(&serial, &format!("screencap -p {}", temp_file), Duration::from_secs(10)).await.map_err(IpcError::from)?;

    // 2. Pull it (argv-based, no shell)
    state.adb.invoker.exec(&["-s", serial.as_str(), "pull", &temp_file, &save_path], Duration::from_secs(15)).await.map_err(IpcError::from)?;

    // 3. Cleanup (temp_file is fully controlled by us; safe to interpolate)
    let _ = state.adb.invoker.shell(&serial, &format!("rm {}", temp_file), Duration::from_secs(5)).await;

    Ok(())
}

#[tauri::command]
pub async fn install_apk(
    state: State<'_, Arc<AppState>>,
    serial: String,
    apk_path: String,
    downgrade: bool,
    keep_data: bool,
) -> std::result::Result<String, IpcError> {
    let serial = checked_serial(&serial)?;
    // `apk_path` is a HOST path passed via argv to adb. No shell, but length cap.
    if apk_path.is_empty() || apk_path.len() > 4096 {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid apk_path".into() });
    }
    let mut args = vec!["-s", serial.as_str(), "install"];
    if downgrade { args.push("-d"); }
    if keep_data { args.push("-r"); }
    args.push(&apk_path);

    let res = state.adb.invoker.exec(&args, Duration::from_secs(60)).await.map_err(IpcError::from)?;
    Ok(res)
}

#[tauri::command]
pub async fn launch_scrcpy(
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    std::process::Command::new("scrcpy")
        .arg("-s")
        .arg(serial.as_str())
        .arg("--max-fps")
        .arg("120")
        .arg("--video-bit-rate")
        .arg("8M")
        .spawn()
        .map_err(|e| IpcError { kind: "scrcpy_error".into(), message: format!("Failed to launch scrcpy: {}", e) })?;
    Ok(())
}


#[tauri::command]
pub async fn extract_apk(
    state: State<'_, Arc<AppState>>,
    serial: String,
    package: String,
    save_path: String,
) -> std::result::Result<String, IpcError> {
    let serial = checked_serial(&serial)?;
    let out = state.adb.invoker.shell(&serial, &format!("pm path {}", package), std::time::Duration::from_secs(10)).await.map_err(IpcError::from)?;
    if out.trim().is_empty() { return Err(IpcError { kind: "extract_error".into(), message: "Package not found".into() }); }

    let mut paths = Vec::new();
    for line in out.lines() {
        let p = line.trim().strip_prefix("package:").unwrap_or(line.trim());
        if !p.is_empty() { paths.push(p); }
    }

    if paths.is_empty() {
        return Err(IpcError { kind: "extract_error".into(), message: "No APK paths found".into() });
    }

    if paths.len() == 1 && !save_path.ends_with(".zip") {
        state.adb.invoker.exec(&["-s", serial.as_str(), "pull", paths[0], &save_path], std::time::Duration::from_secs(120)).await.map_err(IpcError::from)?;
        return Ok(format!("APK extracted to {}", save_path));
    }

    let temp_dir = std::env::temp_dir().join(format!("dozeforge_extract_{}", package));
    std::fs::create_dir_all(&temp_dir).unwrap_or_default();

    for path in &paths {
        state.adb.invoker.exec(&["-s", serial.as_str(), "pull", path, temp_dir.to_str().unwrap()], std::time::Duration::from_secs(120)).await.map_err(IpcError::from)?;
    }

    let file = std::fs::File::create(&save_path).map_err(|e| IpcError { kind: "fs_error".into(), message: e.to_string() })?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for entry in std::fs::read_dir(&temp_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_str().unwrap();
            zip.start_file(filename, options).unwrap();
            let mut f = std::fs::File::open(&path).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).unwrap();
            std::io::Write::write_all(&mut zip, &buffer).unwrap();
        }
    }

    zip.start_file("install_me.bat", options).unwrap();
    std::io::Write::write_all(&mut zip, b"@echo off\r\ncd /d \"%~dp0\"\r\necho Installing App Bundle...\r\nadb install-multiple *.apk\r\npause").unwrap();
    
    zip.finish().unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(format!("App Bundle extracted to {}", save_path))
}


#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub date: String,
}

#[tauri::command]
pub async fn list_files(
    state: State<'_, Arc<AppState>>,
    serial: String,
    path: String,
) -> std::result::Result<Vec<FileEntry>, IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_android_path(&path)?;

    // Resolve symlinks first (e.g. /sdcard -> /storage/self/primary)
    let resolved = {
        let r = state.adb.invoker.shell(&serial, &format!("readlink -f \"{}\" 2>/dev/null || echo \"{}\"", path, path), std::time::Duration::from_secs(5)).await.unwrap_or_else(|_| path.clone());
        r.trim().to_string()
    };
    // `readlink -f` output is data from the device, not user input — but it gets re-interpolated
    // below. Re-validate to be safe (the device could be compromised).
    crate::security::validate_android_path(&resolved)?;

    // Use ls -la for full listing including hidden files; Android ls has slightly different format
    let out = state.adb.invoker.shell(
        &serial,
        &format!("ls -la \"{}\" 2>/dev/null", resolved),
        std::time::Duration::from_secs(10),
    ).await.map_err(IpcError::from)?;

    let mut entries = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        // Skip "total N" lines and empty lines
        if line.is_empty() || line.starts_with("total") { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        // Android ls -la: permissions links user group size date time name
        // e.g.: drwxrwx--x  2 root sdcard_rw 4096 2024-01-01 12:00 DCIM
        // Some Android versions: drwxrwx--x root sdcard_rw 4096 2024-01-01 12:00 DCIM (no links count)
        if parts.len() < 6 { continue; }

        let perms = parts[0];
        let is_dir = perms.starts_with('d');
        let is_link = perms.starts_with('l');

        // Detect format: if parts[1] is numeric (link count) -> standard format, else -> Android compact format
        let (size, date, name_start) = if parts[1].parse::<u64>().is_ok() && parts.len() >= 8 {
            // Standard: perms links user group size date time name...
            let sz = parts[4].parse::<u64>().unwrap_or(0);
            let dt = format!("{} {}", parts[5], parts[6]);
            (sz, dt, 7usize)
        } else if parts.len() >= 7 {
            // Android compact: perms user group size date time name...
            let sz = parts[3].parse::<u64>().unwrap_or(0);
            let dt = format!("{} {}", parts[4], parts[5]);
            (sz, dt, 6usize)
        } else {
            continue;
        };

        // Name can have spaces; join the rest
        let raw_name = parts[name_start..].join(" ");
        // For symlinks: "name -> target" — strip the arrow part
        let name = if is_link {
            raw_name.split(" -> ").next().unwrap_or(&raw_name).to_string()
        } else {
            raw_name
        };

        // Skip . and ..
        if name == "." || name == ".." { continue; }

        entries.push(FileEntry { name, is_dir: is_dir || is_link, size, date });
    }
    Ok(entries)
}


#[tauri::command]
pub async fn push_file(
    state: State<'_, Arc<AppState>>,
    serial: String,
    local_path: String,
    remote_path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    if local_path.is_empty() || local_path.len() > 4096 {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid local_path".into() });
    }
    crate::security::validate_android_path(&remote_path)?;
    state.adb.invoker.exec(&["-s", serial.as_str(), "push", &local_path, &remote_path], std::time::Duration::from_secs(300)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn pull_file(
    state: State<'_, Arc<AppState>>,
    serial: String,
    remote_path: String,
    local_path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_android_path(&remote_path)?;
    if local_path.is_empty() || local_path.len() > 4096 {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid local_path".into() });
    }
    state.adb.invoker.exec(&["-s", serial.as_str(), "pull", &remote_path, &local_path], std::time::Duration::from_secs(300)).await.map_err(IpcError::from)?;
    Ok(())
}
#[tauri::command]
pub async fn delete_file(
    state: State<'_, Arc<AppState>>,
    serial: String,
    path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_android_path(&path)?;
    // After validate_android_path the value has no shell metacharacters and no `..` segments,
    // so it is safe to interpolate.
    state.adb.invoker.shell(&serial, &format!("rm -rf \"{}\"", path), std::time::Duration::from_secs(30)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn create_directory(
    state: State<'_, Arc<AppState>>,
    serial: String,
    path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_android_path(&path)?;
    state.adb.invoker.shell(&serial, &format!("mkdir -p \"{}\"", path), std::time::Duration::from_secs(10)).await.map_err(IpcError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn fastboot_reboot(
    serial: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    tokio::time::timeout(std::time::Duration::from_secs(30), tokio::process::Command::new("fastboot").arg("-s").arg(serial.as_str()).arg("reboot").output()).await.map_err(|e| IpcError { kind: "fastboot_error".into(), message: format!("Timeout: {}", e) })?.map_err(|e| IpcError { kind: "fastboot_error".into(), message: format!("Exec failed: {}", e) })?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalStatus {
    pub raw_value: i32,
    pub label: String,
}

#[tauri::command]
pub async fn get_thermal_status(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<ThermalStatus, IpcError> {
    let serial = checked_serial(&serial)?;
    let out = state.adb.invoker.shell(&serial, "dumpsys thermalservice", Duration::from_secs(5)).await.unwrap_or_default();
    
    let mut val = -1;
    for line in out.lines() {
        if let Some(idx) = line.find("Thermal Status: ") {
            let num_str = line[idx + "Thermal Status: ".len()..].trim();
            if let Ok(n) = num_str.parse::<i32>() {
                val = n;
                break;
            }
        } else if let Some(idx) = line.find("mStatus=") {
            let num_str = line[idx + "mStatus=".len()..].trim();
            if let Ok(n) = num_str.parse::<i32>() {
                val = n;
                break;
            }
        }
    }
    
    let label = match val {
        0 => "NONE",
        1 => "LIGHT",
        2 => "MODERATE",
        3 => "SEVERE",
        4 => "CRITICAL",
        5 => "EMERGENCY",
        6 => "SHUTDOWN",
        _ => "UNKNOWN",
    };
    
    Ok(ThermalStatus { raw_value: val, label: label.to_string() })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkUsage {
    pub package: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[tauri::command]
pub async fn get_network_usage(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<Vec<NetworkUsage>, IpcError> {
    let serial = checked_serial(&serial)?;
    
    // Get all packages with their UIDs
    let all_packages = list_packages(state.clone(), serial.as_str().to_string()).await?;
    let mut uid_to_pkg = HashMap::new();
    for pkg in all_packages {
        uid_to_pkg.insert(pkg.uid.to_string(), pkg.name.to_string());
    }

    // dumpsys netstats detail is huge. Just dump netstats.
    let netstats = state.adb.invoker.shell(&serial, "dumpsys netstats", Duration::from_secs(15)).await.unwrap_or_default();
    
    let mut usage_map: HashMap<String, (u64, u64)> = HashMap::new();
    
    for line in netstats.lines() {
        let line = line.trim();
        if !line.starts_with("uid=") { continue; }
        
        // e.g. uid=10234 set=DEFAULT rovp=false rb=1024 rp=10 tb=2048 tp=20
        let parts: Vec<&str> = line.split_whitespace().collect();
        let mut uid = "";
        let mut rx = 0u64;
        let mut tx = 0u64;
        
        for p in parts {
            if let Some(v) = p.strip_prefix("uid=") { uid = v; }
            else if let Some(v) = p.strip_prefix("rb=") { rx = v.parse().unwrap_or(0); }
            else if let Some(v) = p.strip_prefix("tb=") { tx = v.parse().unwrap_or(0); }
        }
        
        if let Some(pkg) = uid_to_pkg.get(uid) {
            let entry = usage_map.entry(pkg.clone()).or_insert((0, 0));
            entry.0 += rx;
            entry.1 += tx;
        }
    }
    
    let mut res = Vec::new();
    for (pkg, (rx, tx)) in usage_map {
        if rx > 0 || tx > 0 {
            res.push(NetworkUsage { package: pkg, rx_bytes: rx, tx_bytes: tx });
        }
    }
    res.sort_by(|a, b| (b.rx_bytes + b.tx_bytes).cmp(&(a.rx_bytes + a.tx_bytes)));
    Ok(res)
}

#[tauri::command]
pub async fn fastboot_flash(
    serial: String,
    partition: String,
    image_path: String,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    crate::security::validate_partition(&partition)?;
    if image_path.is_empty() || image_path.len() > 4096 {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid image_path".into() });
    }
    tokio::time::timeout(std::time::Duration::from_secs(300), tokio::process::Command::new("fastboot").arg("-s").arg(serial.as_str()).arg("flash").arg(partition).arg(image_path).output()).await.map_err(|e| IpcError { kind: "fastboot_error".into(), message: format!("Timeout: {}", e) })?.map_err(|e| IpcError { kind: "fastboot_error".into(), message: format!("Exec failed: {}", e) })?;
    Ok(())
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NativeProfile {
    pub disabled_packages: Vec<String>,
}

#[tauri::command]
pub async fn export_native_profile(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<NativeProfile, IpcError> {
    let serial = checked_serial(&serial)?;
    let out = state.adb.invoker.shell(&serial, "pm list packages -d", std::time::Duration::from_secs(10)).await.map_err(IpcError::from)?;
    let mut disabled_packages = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.starts_with("package:") {
            let pkg = line.strip_prefix("package:").unwrap_or(line);
            disabled_packages.push(pkg.to_string());
        }
    }
    Ok(NativeProfile { disabled_packages })
}

#[tauri::command]
pub async fn import_native_profile(
    state: State<'_, Arc<AppState>>,
    serial: String,
    profile: NativeProfile,
) -> std::result::Result<(), IpcError> {
    let serial = checked_serial(&serial)?;
    for pkg in profile.disabled_packages {
        // Reject malformed packages instead of silently swallowing — the user expects an error
        // if the imported profile is corrupt.
        crate::security::validate_pkg(&pkg)?;
        let _ = state.adb.invoker.shell(&serial, &format!("pm disable-user --user 0 {}", pkg), std::time::Duration::from_secs(10)).await;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdnsService {
    pub address: String, // IP:PORT
    pub service_type: String, // _adb._tcp, _adb-tls-pairing._tcp, etc
}

#[tauri::command]
pub async fn adb_mdns_services(
    state: State<'_, Arc<AppState>>,
) -> std::result::Result<Vec<MdnsService>, IpcError> {
    let out = state.adb.invoker.exec(&["mdns", "services"], std::time::Duration::from_secs(10)).await.unwrap_or_default();
    let mut services = Vec::new();
    for line in out.lines().skip(1) { // Skip "List of discovered mdns services"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            services.push(MdnsService {
                address: parts[0].to_string(),
                service_type: parts[2].to_string(),
            });
        }
    }
    Ok(services)
}

#[tauri::command]
pub async fn adb_pair(
    state: State<'_, Arc<AppState>>,
    address: String,
    pin: String,
) -> std::result::Result<String, IpcError> {
    // `address` here is the host:port we will connect to (akin to a serial).
    crate::security::validate_serial(&address)?;
    if pin.is_empty() || pin.len() > 32 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(IpcError { kind: "invalid_input".into(), message: "invalid pairing pin".into() });
    }
    let out = state.adb.invoker.exec(&["pair", &address, &pin], std::time::Duration::from_secs(15)).await.map_err(IpcError::from)?;
    Ok(out)
}

#[tauri::command]
pub async fn adb_connect(
    state: State<'_, Arc<AppState>>,
    address: String,
) -> std::result::Result<String, IpcError> {
    crate::security::validate_serial(&address)?;
    let out = state.adb.invoker.exec(&["connect", &address], std::time::Duration::from_secs(15)).await.map_err(IpcError::from)?;
    Ok(out)
}

#[tauri::command]
pub async fn adb_tcpip(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<String, IpcError> {
    crate::security::validate_serial(&serial)?;
    let out = state.adb.invoker.exec(&["-s", &serial, "tcpip", "5555"], std::time::Duration::from_secs(10)).await.map_err(IpcError::from)?;
    Ok(out)
}
