// Mirrors the Rust types serialised through Tauri IPC.
// Keep this file in sync with src-tauri/src/ipc/commands.rs and the parser
// modules' #[derive(Serialize)] structs.

export type DeviceState =
  | 'device'
  | 'unauthorized'
  | 'offline'
  | 'recovery'
  | 'sideload'
  | 'bootloader';

export interface Device {
  serial: string;
  state: DeviceState;
  model: string | null;
  manufacturer: string | null;
  product: string | null;
}

export interface BuildIdentity {
  sdk_int: number;
  security_patch_year: number;
  security_patch_month: number;
  fingerprint: string;
}

export interface DeviceCapabilities {
  appops_set: boolean;
  appops_get: boolean;
  am_set_standby_bucket: boolean;
  pm_disable_user: boolean;
  device_config_put: boolean;
  dumpsys_jobscheduler: boolean;
  dumpsys_deviceidle: boolean;
  dumpsys_sensorservice: boolean;
  write_secure_settings: boolean;
}

export interface WakelockEntry {
  package: string;
  uid: number;
  total_ms: number;
  count: number;
}

export type AlarmKind = 'wakeup_rtc' | 'wakeup_elapsed' | 'non_wakeup';

export interface AlarmAttribution {
  target_package: string;
  triggering_package: string;
  kind: AlarmKind;
  wake_count: number;
}

export interface JobAttribution {
  package: string;
  job_count: number;
  periodic_count: number;
}

export interface DozeWhitelist {
  user_whitelisted: string[];
  system_whitelisted: string[];
}

export interface LiveWakelock {
  tag: string;
  package: string | null;
  flags: string;
}

export interface SensorClient {
  package: string;
  sensors: string[];
}

export interface InstalledPackage {
  name: string;
  uid: number;
  install_path: string;
  is_system: boolean;
  label: string | null;
}

export type StandbyBucket =
  | 'exempted'
  | 'active'
  | 'working_set'
  | 'frequent'
  | 'rare'
  | 'restricted'
  | 'never';

export type AppOpMode = 'allow' | 'deny' | 'ignore' | 'default' | 'foreground';

export interface AppOpState {
  package: string;
  op: string;
  mode: AppOpMode;
}

export interface CpuAggregate {
  package: string | null;
  pid: number;
  samples: number;
  p50: number;
  p95: number;
  max: number;
}

export type RiskTier = 'critical' | 'elevated' | 'moderate';

export interface PackageVerdict {
  package: string;
  uid: number;
  install_path: string;
  tier: RiskTier;
  reasons: string[];
}

export interface CulpritRanking {
  package: string;
  wakelock_ms: number;
  wakeup_count: number;
  job_count: number;
  redirected_from_proxy: string | null;
  score: number;
}

export type OptimizationAction =
  | { kind: 'set_standby_bucket'; package: string; bucket: StandbyBucket }
  | { kind: 'set_app_op'; package: string; op: string; mode: AppOpMode }
  | { kind: 'kill_package'; package: string }
  | { kind: 'disable_package'; package: string }
  | { kind: 'enable_package'; package: string }
  | { kind: 'remove_doze_whitelist'; package: string }
  | { kind: 'add_doze_whitelist'; package: string }
  | { kind: 'set_phantom_process_limit'; value: number }
  | { kind: 'raw_shell'; command: string };

export interface OptimizationOutcome {
  action: OptimizationAction;
  success: boolean;
  message: string;
}

export interface OptimizationReport {
  snapshot_id: string;
  outcomes: OptimizationOutcome[];
}

export interface BloatwareReport {
  disabled: string[];
  failed: Array<[string, string]>;
}

export interface SnapshotMeta {
  id: string;
  created_at: string;
  device_serial: string;
  sdk_int: number;
  packages: number;
  label: string | null;
}

export interface RollbackReport {
  commands: string[];
  applied: number;
  failed: Array<[string, string]>;
}

export interface AuditReport {
  device_serial: string;
  sdk_int: number;
  wakelocks: WakelockEntry[];
  alarms: AlarmAttribution[];
  jobs: JobAttribution[];
  culprits: CulpritRanking[];
}

export interface WakeupSources {
  alarms: AlarmAttribution[];
  jobs: JobAttribution[];
  doze_whitelist: DozeWhitelist;
  live_wakelocks: LiveWakelock[];
  sensors: SensorClient[];
}

export interface IpcError {
  kind: string;
  message: string;
}

// ---- Profiles ----

export type Profile = 'conservative' | 'balanced' | 'aggressive' | 'nuclear';

export interface ProfileSummary {
  apps_restricted: number;
  bloatware_disabled: number;
  wakelocks_revoked: number;
  doze_whitelist_cleaned: number;
  total_actions: number;
  packages_excluded: number;
}

export interface ProfilePreview {
  profile: Profile;
  actions: OptimizationAction[];
  /** [package, reason] pairs */
  excluded_packages: Array<[string, string]>;
  summary: ProfileSummary;
}

// ---- V2: Overview / Battery / Sleep / Telemetry / Miscategorized ----

export interface BatteryHealth {
  cycle_count: number | null;
  charge_full_uah: number | null;
  charge_full_design_uah: number | null;
  health_percent: number | null;
  level_percent: number | null;
  temperature_c: number | null;
  voltage_v: number | null;
  status: string | null;
  health_status: string | null;
  source: string | null;
}

export type SleepTier = 'excellent' | 'good' | 'mediocre' | 'bad';

export interface Penalty {
  label: string;
  points: number;
}

export interface SleepScore {
  score: number;
  tier: SleepTier;
  penalties: Penalty[];
}

export type ProcessState = 'running' | 'sleeping' | 'uninterruptiblesleep' | 'zombie' | 'stopped' | 'idle' | 'unknown';

export interface ProcessRow {
  pid: number;
  user: string;
  state: ProcessState;
  cpu_percent: number;
  rss_kb: number;
  args: string;
  package: string | null;
  is_hog_candidate: boolean;
  is_smart_hog: boolean;
  is_zombie: boolean;
}

export interface ProcessSnapshot {
  rows: ProcessRow[];
  zombie_count: number;
  hog_candidate_count: number;
  total_cpu_percent: number;
  total_rss_kb: number;
  cpu_user: number;
  cpu_sys: number;
  cpu_iowait: number;
  mem_available_mb: number;
  swap_free_mb: number;
  swap_total_mb: number;
}

export interface TelemetryTick {
  device_serial: string;
  snapshot: ProcessSnapshot;
  ts_ms: number;
  cpu_history: number[];
}

export interface MiscategorizedApp {
  package: string;
  current_bucket: StandbyBucket;
  recommended_bucket: StandbyBucket;
  days_since_used: number;
  reason: string;
}

export interface OverviewSnapshot {
  device_serial: string;
  sdk_int: number;
  battery: BatteryHealth;
  sleep_score: SleepScore;
  zombie_count: number;
  hog_candidate_count: number;
  active_bucket_apps: number;
  ram_used_mb: number | null;
  ram_total_mb: number | null;
  top_offender: CulpritRanking | null;
}

export interface ActionLogEntry {
  ts: string;
  device_serial: string;
  action: OptimizationAction;
  success: boolean;
  message: string;
  snapshot_id: string | null;
}

// ---- V2.1: Privacy module ----

export type PrivateDnsMode = 'off' | 'opportunistic' | 'hostname';

export interface PrivateDnsState {
  mode: PrivateDnsMode;
  hostname: string | null;
}

export interface DnsPreset {
  label: string;
  hostname: string;
  blocks_ads: boolean;
  blocks_trackers: boolean;
}

export interface PrivacyAppEntry {
  package: string;
  ops: Record<string, AppOpMode>;
  firewall_active: boolean;
  clipboard_blocked: boolean;
}

export interface PrivacyScan {
  apps: PrivacyAppEntry[];
}

export interface PrivacyState {
  dns: PrivateDnsState;
  scan: PrivacyScan;
}

// ---- V2.2: Storage module ----

export interface DiskStats {
  cache_free_bytes: number | null;
  cache_total_bytes: number | null;
  system_free_bytes: number | null;
  system_total_bytes: number | null;
  data_free_bytes: number | null;
  data_total_bytes: number | null;
  recent_write_speed_kb_s: number | null;
  file_based_encryption: boolean | null;
}

export interface StorageOverview {
  diskstats: DiskStats;
  inventory_total_bytes: number;
  inventory_count: number;
}

export interface PackageSize {
  package: string;
  apk_bytes: number;
  split_count: number;
}


// ---- V2.3: Display & Audio tuning ----

export interface DisplaySettings {
  min_refresh_rate: number | null;
  peak_refresh_rate: number | null;
  bt_absolute_volume_disabled: boolean;
  max_frame_buffer_buffers: number | null;
  master_mono: boolean;
  spatial_audio_enabled: boolean | null;
  avrcp_version: string | null;
}

export type AvrcpVersion = 'avrcp13' | 'avrcp14' | 'avrcp15' | 'avrcp16';

// ---- V2.4: Block H — System tweaks (phantom killer, captive portal, ART compile) ----

export interface SystemTweaks {
  phantom_monitor_enabled: boolean | null;
  captive_portal_mode: number | null;
  max_phantom_processes: number | null;
}

export type CompileMode =
  | 'speed'
  | 'speed-profile'
  | 'verify'
  | 'quicken'
  | 'everything'
  | 'extract';

// ---- V2.5: Sleep timeline / Kernel wakelocks / Per-app battery drain ----

export type SleepEfficiencyTier = 'excellent' | 'good' | 'mediocre' | 'bad';

export interface SleepTimeline {
  on_battery_realtime_ms: number;
  on_battery_uptime_ms: number;
  screen_off_realtime_ms: number;
  screen_off_uptime_ms: number;
  deep_sleep_ms: number;
  efficiency_ratio: number;
  tier: SleepEfficiencyTier;
}

export type KernelWakelockSeverity = 'negligible' | 'low' | 'moderate' | 'high' | 'critical';

export interface KernelWakelock {
  name: string;
  total_ms: number;
  count: number;
  explanation: string;
  severity: KernelWakelockSeverity;
}

export type AppDrainVerdict =
  | 'negligible'
  | 'legitimate_foreground'
  | 'legitimate_media'
  | 'background_hog'
  | 'zombie'
  | 'radio_hog';

export interface AppDrainEntry {
  package: string;
  uid: number;
  drain_mah: number;
  drain_share: number;
  breakdown: Record<string, number>;
  has_live_wakelock: boolean;
  is_zombie: boolean;
  verdict: AppDrainVerdict;
}

export interface BatteryDrain {
  capacity_mah: number | null;
  computed_drain_mah: number;
  actual_drain_min_mah: number | null;
  actual_drain_max_mah: number | null;
  entries: AppDrainEntry[];
}

// ---- V2.6: Bloatware recommendations & presets ----

export type Recommendation =
  | 'safe_to_disable'
  | 'preinstalled_bloat'
  | 'system_use_with_care'
  | 'do_not_touch';

export type BloatCategory =
  | 'google_optional_apps'
  | 'google_assistant'
  | 'google_ads'
  | 'samsung_bixby'
  | 'samsung_optional_apps'
  | 'samsung_ads'
  | 'xiaomi_ads'
  | 'xiaomi_optional_apps'
  | 'oneplus_optional_apps'
  | 'huawei_optional_apps'
  | 'oppo_vivo_optional_apps'
  | 'motorola_optional_apps'
  | 'carrier_apps'
  | 'preloaded_social'
  | 'preloaded_microsoft'
  | 'preloaded_netflix';

export interface BloatwareRecommendation {
  package: string;
  tier: RiskTier;
  recommendation: Recommendation;
  notes: string;
  category: BloatCategory | null;
  /** Corroborated by the UAD-NG community database (human-reviewed). */
  community_verified?: boolean;
}

export type BloatPreset =
  | 'debloat_google'
  | 'debloat_oem'
  | 'debloat_ads_and_trackers'
  | 'debloat_partner_apps'
  | 'debloat_carrier';

export interface BloatPresetDto {
  id: BloatPreset;
  label: string;
  description: string;
}

// ---- V2.7: Advanced Optimizations (Performance & Background) ----

export interface PerformanceSettings {
  window_animation_scale: number | null;
  transition_animation_scale: number | null;
  animator_duration_scale: number | null;
  background_process_limit: number | null;
  wifi_scan_always_enabled: boolean | null;
  ble_scan_always_enabled: boolean | null;
  restrict_background_data: boolean;
  aggressive_doze_enabled: boolean;
}

// ---- V2.8: Sleep/Doze Interactive Commands ----

export interface DozeState {
  state: string;
  deep_enabled: boolean;
  force_idle: boolean;
  screen_on: boolean;
  charging: boolean;
  next_alarm_elapsed: string | null;
}
