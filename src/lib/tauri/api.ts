import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ActionLogEntry,
  AuditReport,
  AvrcpVersion,
  BatteryHealth,
  BatteryDrain,
  BloatPreset,
  BloatPresetDto,
  BloatwareRecommendation,
  BloatwareReport,
  CpuAggregate,
  Device,
  DisplaySettings,
  KernelWakelock,
  SleepTimeline,
  SystemTweaks,
  CompileMode,
  DeviceCapabilities,
  DnsPreset,
  InstalledPackage,
  IpcError,
  MiscategorizedApp,
  OptimizationAction,
  OptimizationReport,
  OverviewSnapshot,
  PackageSize,
  PackageVerdict,
  PerformanceSettings,
  PrivacyState,
  PrivateDnsMode,
  ProcessSnapshot,
  Profile,
  ProfilePreview,
  RollbackReport,
  SleepScore,
  SnapshotMeta,
  StorageOverview,
  TelemetryTick,
  WakeupSources
} from '$types';

export class DozeForgeError extends Error {
  readonly kind: string;

  constructor(payload: IpcError | unknown) {
    if (typeof payload === 'object' && payload !== null && 'kind' in payload) {
      const ipc = payload as IpcError;
      super(ipc.message);
      this.kind = ipc.kind;
    } else if (payload instanceof Error) {
      super(payload.message);
      this.kind = 'unknown';
    } else {
      super(String(payload));
      this.kind = 'unknown';
    }
    this.name = 'DozeForgeError';
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new DozeForgeError(e);
  }
}

export const api = {
  listDevices: () => call<Device[]>('list_devices'),

  probeCapabilities: (serial: string) =>
    call<DeviceCapabilities>('probe_capabilities', { serial }),

  auditDevice: (serial: string) => call<AuditReport>('audit_device', { serial }),

  sampleCpu: (serial: string, durationSecs?: number) =>
    call<CpuAggregate[]>('sample_cpu', { serial, durationSecs }),

  listWakeupSources: (serial: string) =>
    call<WakeupSources>('list_wakeup_sources', { serial }),

  listPackages: (serial: string) =>
    call<InstalledPackage[]>('list_packages', { serial }),

  classifyPackages: (serial: string, packages: string[] = []) =>
    call<PackageVerdict[]>('classify_packages', { serial, packages }),

  applyOptimization: (serial: string, actions: OptimizationAction[]) =>
    call<OptimizationReport>('apply_optimization', { serial, actions }),

  takeSnapshot: (serial: string, packages: string[], label?: string) =>
    call<SnapshotMeta>('take_snapshot', { serial, packages, label }),

  listSnapshots: () => call<SnapshotMeta[]>('list_snapshots'),

  rollbackSnapshot: (serial: string, snapshotId: string, only?: string[]) =>
    call<RollbackReport>('rollback_snapshot', { serial, snapshotId, only }),

  exportShellScript: (actions: OptimizationAction[], deviceLabel: string) =>
    call<string>('export_shell_script', { actions, deviceLabel }),

  disableBloatware: (serial: string, packages: string[]) =>
    call<BloatwareReport>('disable_bloatware', { serial, packages }),

  enableBloatware: (serial: string, packages: string[]) =>
    call<BloatwareReport>('enable_bloatware', { serial, packages }),

  setPhantomProcessLimit: (serial: string, value: number) =>
    call<void>('set_phantom_process_limit', { serial, value }),

  previewProfile: (serial: string, profile: Profile, userExcludes?: string[]) =>
    call<ProfilePreview>('preview_profile', { serial, profile, userExcludes }),

  applyProfile: (serial: string, profile: Profile, userExcludes?: string[]) =>
    call<OptimizationReport>('apply_profile', { serial, profile, userExcludes }),

  // ---- V2 ----
  overviewSnapshot: (serial: string) =>
    call<OverviewSnapshot>('overview_snapshot', { serial }),

  batteryHealth: (serial: string) =>
    call<BatteryHealth>('battery_health', { serial }),

  processStatus: (serial: string) =>
    call<ProcessSnapshot>('process_status', { serial }),

  startTelemetryStream: (serial: string, intervalSecs = 3) =>
    call<void>('start_telemetry_stream', { serial, intervalSecs }),

  stopTelemetryStream: () =>
    call<void>('stop_telemetry_stream'),

  onTelemetryTick: (handler: (tick: TelemetryTick) => void): Promise<UnlistenFn> =>
    listen<TelemetryTick>('telemetry_tick', (e) => handler(e.payload)),

  miscategorizedApps: (serial: string) =>
    call<MiscategorizedApp[]>('miscategorized_apps', { serial }),

  sleepScore: (serial: string) =>
    call<SleepScore>('sleep_score', { serial }),

  readActionLog: (limit = 50) =>
    call<ActionLogEntry[]>('read_action_log', { limit }),

  // ---- Privacy ----
  listDnsPresets: () =>
    call<DnsPreset[]>('list_dns_presets'),

  getPrivacyState: (serial: string) =>
    call<PrivacyState>('get_privacy_state', { serial }),

  getDangerousPermissions: (serial: string) =>
    call<Array<{ package: string, permissions: Record<string, string> }>>('get_dangerous_permissions', { serial }),

  setPrivateDns: (serial: string, mode: PrivateDnsMode, hostname: string | null) =>
    call<OptimizationReport>('set_private_dns', { serial, mode, hostname }),

  applyFirewall: (serial: string, packages: string[], block: boolean) =>
    call<OptimizationReport>('apply_firewall', { serial, packages, block }),

  applyClipboardGuard: (serial: string, packages: string[], block: boolean) =>
    call<OptimizationReport>('apply_clipboard_guard', { serial, packages, block }),

  // ---- Storage ----
  storageOverview: (serial: string) =>
    call<StorageOverview>('storage_overview', { serial }),

  storageInventory: (serial: string) =>
    call<PackageSize[]>('storage_inventory', { serial }),

  clearAppCache: (serial: string, packages: string[]) =>
    call<OptimizationReport>('clear_app_cache', { serial, packages }),

  trimSystemCaches: (serial: string, targetFreeBytes: number) =>
    call<OptimizationReport>('trim_system_caches', { serial, targetFreeBytes }),

  runBgDexopt: (serial: string) =>
    call<OptimizationReport>('run_bg_dexopt', { serial }),

  // ---- Display & Audio ----
  getDisplaySettings: (serial: string) =>
    call<DisplaySettings>('get_display_settings', { serial }),

  applyRefreshRate: (serial: string, minRate: number | null, peakRate: number | null) =>
    call<OptimizationReport>('apply_refresh_rate', { serial, minRate, peakRate }),

  setBluetoothAbsoluteVolume: (serial: string, disabled: boolean) =>
    call<OptimizationReport>('set_bluetooth_absolute_volume', { serial, disabled }),

  // ---- Block H: System tweaks ----
  getSystemTweaks: (serial: string) =>
    call<SystemTweaks>('get_system_tweaks', { serial }),

  setPhantomMonitor: (serial: string, enabled: boolean) =>
    call<OptimizationReport>('set_phantom_monitor', { serial, enabled }),

  setCaptivePortalMode: (serial: string, disabled: boolean) =>
    call<OptimizationReport>('set_captive_portal_mode', { serial, disabled }),

  compilePackage: (serial: string, pkg: string, mode: CompileMode) =>
    call<OptimizationReport>('compile_package', { serial, package: pkg, mode }),

  resetCompilation: (serial: string, pkg: string) =>
    call<OptimizationReport>('reset_compilation', { serial, package: pkg }),

  // ---- V2.5: Sleep timeline / Kernel wakelocks / Per-app battery drain ----
  sleepTimeline: (serial: string) =>
    call<SleepTimeline>('sleep_timeline', { serial }),

  kernelWakelocks: (serial: string) =>
    call<KernelWakelock[]>('kernel_wakelocks', { serial }),

  batteryPerApp: (serial: string) =>
    call<BatteryDrain>('battery_per_app', { serial }),

  // ---- V2.6: App labels resolver / Audio extras / Bloatware presets ----
  resolveAppLabels: (serial: string) =>
    call<Record<string, string>>('resolve_app_labels', { serial }),

  setMasterMono: (serial: string, enabled: boolean) =>
    call<OptimizationReport>('set_master_mono', { serial, enabled }),

  setSpatialAudio: (serial: string, enabled: boolean) =>
    call<OptimizationReport>('set_spatial_audio', { serial, enabled }),

  setAvrcpVersion: (serial: string, version: AvrcpVersion) =>
    call<OptimizationReport>('set_avrcp_version', { serial, version }),

  bloatwareRecommendations: (serial: string) =>
    call<BloatwareRecommendation[]>('bloatware_recommendations', { serial }),

  listBloatPresets: () =>
    call<BloatPresetDto[]>('list_bloat_presets'),

  previewBloatPreset: (serial: string, preset: BloatPreset) =>
    call<string[]>('preview_bloat_preset', { serial, preset }),

  // ---- V2.7: Advanced Optimizations ----
  getPerformanceSettings: (serial: string) =>
    call<PerformanceSettings>('get_performance_settings', { serial }),

  setAnimationScales: (serial: string, scale: number) =>
    call<OptimizationReport>('set_animation_scales', { serial, scale }),

  setAggressiveDoze: (serial: string, enabled: boolean) =>
    call<OptimizationReport>('set_aggressive_doze', { serial, enabled }),

  setBackgroundScan: (serial: string, wifi: boolean, ble: boolean) =>
    call<OptimizationReport>('set_background_scan', { serial, wifi, ble }),

  setDataSaver: (serial: string, enabled: boolean) =>
    call<OptimizationReport>('set_data_saver', { serial, enabled }),

  hibernatePackage: (serial: string, pkg: string, hibernate: boolean) =>
    call<OptimizationReport>('hibernate_package', { serial, package: pkg, hibernate }),

  setGameMode: (serial: string, pkg: string, mode: number) =>
    call<OptimizationReport>('set_game_mode', { serial, package: pkg, mode }),

  setBackgroundProcessLimit: (serial: string, limit: number | null) =>
    call<OptimizationReport>('set_background_process_limit', { serial, limit }),

  // ---- V2.8: Sleep/Doze Interactive Commands ----
  getDozeState: (serial: string) =>
    call<DozeState>('get_doze_state', { serial }),

  setDozeWhitelist: (serial: string, pkg: string, add: boolean) =>
    call<void>('set_doze_whitelist', { serial, package: pkg, add }),

  setForceDoze: (serial: string, force: boolean) =>
    call<void>('set_force_doze', { serial, force }),

  simulateUnplug: (serial: string, unplug: boolean) =>
    call<void>('simulate_unplug', { serial, unplug }),

  // ---- V2.9: Buckets & App Actions ----
  getAllStandbyBuckets: (serial: string) =>
    call<{ package: string, bucket: string }[]>('get_all_standby_buckets', { serial }),

  setStandbyBucket: (serial: string, pkg: string, bucket: string) =>
    call<void>('set_standby_bucket', { serial, package: pkg, bucket }),

  setAppOps: (serial: string, pkg: string, op: string, mode: string) =>
    call<void>('set_appops', { serial, package: pkg, op, mode }),

  forceStopPackage: (serial: string, pkg: string) =>
    call<void>('force_stop_package', { serial, package: pkg }),

  openAppSettings: (serial: string, pkg: string) =>
    call<void>('open_app_settings', { serial, package: pkg }),

  getAppRestrictionsBatch: (serial: string, packages: string[]) =>
    call<Record<string, any>>('get_app_restrictions_batch', { serial, packages }),

  getSingleAppDetails: (serial: string, pkg: string) =>
    call<any>('get_single_app_details', { serial, package: pkg }),

  clearAppData: (serial: string, pkg: string) =>
    call<void>('clear_app_data', { serial, package: pkg }),

  uninstallPackage: (serial: string, pkg: string) =>
    call<void>('uninstall_package', { serial, package: pkg }),


  compileAllApps: (serial: string) =>
    call<void>('compile_all_apps', { serial }),

  disableRamPlus: (serial: string) =>
    call<void>('disable_ram_plus', { serial }),

  forceRefreshRate: (serial: string, rate: string) =>
    call<void>('force_refresh_rate', { serial, rate }),

  setHeadsUpNotifications: (serial: string, enabled: boolean) =>
    call<void>('set_heads_up_notifications', { serial, enabled }),

  setHotwordDetection: (serial: string, enabled: boolean) =>
    call<void>('set_hotword_detection', { serial, enabled }),

  setActivityLogging: (serial: string, enabled: boolean) =>
    call<void>('set_activity_logging', { serial, enabled }),

  setAdaptiveConnectivity: (serial: string, enabled: boolean) =>
    call<void>('set_adaptive_connectivity', { serial, enabled }),

  // ---- V2.10: System Actions ----
  rebootDevice: (serial: string, mode: string) =>
    call<void>('reboot_device', { serial, mode }),

  setDisplayDensity: (serial: string, density: string) =>
    call<void>('set_display_density', { serial, density }),

  setDisplaySize: (serial: string, size: string) =>
    call<void>('set_display_size', { serial, size }),

  resetDisplay: (serial: string) =>
    call<void>('reset_display', { serial }),

  setWindowBlurs: (serial: string, disabled: boolean) =>
    call<void>('set_window_blurs', { serial, disabled }),

  setReduceTransparency: (serial: string, enabled: boolean) =>
    call<void>('set_reduce_transparency', { serial, enabled }),

  setFixedPerformanceMode: (serial: string, enabled: boolean) =>
    call<void>('set_fixed_performance_mode', { serial, enabled }),

  setDarkMode: (serial: string, enabled: boolean) =>
    call<void>('set_dark_mode', { serial, enabled }),

  setStayAwake: (serial: string, enabled: boolean) =>
    call<void>('set_stay_awake', { serial, enabled }),

  captureScreenshot: (serial: string, savePath: string) =>
    call<void>('capture_screenshot', { serial, savePath }),

  installApk: (serial: string, apkPath: string, downgrade: boolean, keepData: boolean) =>
    call<string>('install_apk', { serial, apkPath, downgrade, keepData }),
    
  extractApk: (serial: string, package_name: string, savePath: string) =>
    call<string>('extract_apk', { serial, package: package_name, savePath }),
    
  listFiles: (serial: string, path: string) =>
    call<Array<{ name: string; is_dir: boolean; size: number; date: string }>>('list_files', { serial, path }),
    
  pushFile: (serial: string, localPath: string, remotePath: string) =>
    call<void>('push_file', { serial, localPath, remotePath }),
    
  pullFile: (serial: string, remotePath: string, localPath: string) =>
    call<void>('pull_file', { serial, remotePath, localPath }),
    
  deleteFile: (serial: string, path: string) =>
    call<void>('delete_file', { serial, path }),
    
  createDirectory: (serial: string, path: string) =>
    call<void>('create_directory', { serial, path }),
    
  fastbootReboot: (serial: string) =>
    call<void>('fastboot_reboot', { serial }),
    
  fastbootFlash: (serial: string, partition: string, imagePath: string) =>
    call<void>('fastboot_flash', { serial, partition, imagePath }),
    
  getThermalStatus: (serial: string) =>
    call<{ raw_value: number, label: string }>('get_thermal_status', { serial }),
    
  getNetworkUsage: (serial: string) =>
    call<Array<{ package: string, rx_bytes: number, tx_bytes: number }>>('get_network_usage', { serial }),
    
  exportNativeProfile: (serial: string) =>
    call<{ disabled_packages: string[] }>('export_native_profile', { serial }),
    
  importNativeProfile: (serial: string, profile: { disabled_packages: string[] }) =>
    call<void>('import_native_profile', { serial, profile }),
    
  launchScrcpy: (serial: string) =>
    call<void>('launch_scrcpy', { serial }),

  // ---- V2.10: Storage Advanced Commands ----
  getArtStatusBatch: (serial: string, packages: string[]) =>
    call<Record<string, string>>('get_art_status_batch', { serial, packages }),

  clearTempFiles: (serial: string) =>
    call<void>('clear_temp_files', { serial }),

  // ---- V3.0: Diagnostics ----
  getSystemProperties: (serial: string) =>
    call<Record<string, string>>('get_system_properties', { serial }),

  generateBugreport: (serial: string) =>
    call<string>('generate_bugreport', { serial }),

  startLogStream: (serial: string, mode: string) =>
    call<void>('start_log_stream', { serial, mode }),

  stopLogStream: () =>
    call<void>('stop_log_stream'),

  onLogBatch: (handler: (lines: string[]) => void): Promise<UnlistenFn> =>
    listen<string[]>('log-batch', (e) => handler(e.payload))
};
