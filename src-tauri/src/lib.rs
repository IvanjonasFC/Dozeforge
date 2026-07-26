//! DozeForge - Tauri backend entry point.
//!
//! Module map:
//! - `adb`         : async ADB client, device discovery, capability probing
//! - `parsers`     : version-aware dumpsys parsers with API-level dispatch
//! - `heuristics`  : risk classification, GMS proxy detection, CPU sampling
//! - `optimizer`   : appops/standby/kill/disable-user actions
//! - `snapshot`    : differential snapshots and rollback
//! - `export`      : shell-script and MacroDroid exporters
//! - `ipc`         : Tauri commands exposed to the SvelteKit UI
//! - `security`    : input validators that gate every Tauri command
//! - `telemetry`   : structured logging via `tracing`
//! - `state`       : shared `AppState` held by Tauri
//! - `error`       : crate-wide `Error` and `Result` types

pub mod adb;
pub mod error;
pub mod export;
pub mod heuristics;
pub mod ipc;
pub mod optimizer;
pub mod parsers;
pub mod security;
pub mod snapshot;
pub mod state;
pub mod telemetry;

use std::sync::Arc;

use state::AppState;
use tauri::Manager;

/// Bootstraps logging, builds the shared application state, registers IPC
/// commands and launches the Tauri runtime.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Telemetry must be initialised before anything else so we capture early
    // errors. Failures here are logged via eprintln only -- never abort.
    if let Err(err) = telemetry::init_default() {
        eprintln!("[dozeforge] telemetry init failed: {err}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = Arc::new(AppState::new(handle)?);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::commands::list_devices,
            ipc::commands::probe_capabilities,
            ipc::commands::check_root,
            ipc::commands::audit_device,
            ipc::commands::sample_cpu,
            ipc::commands::list_wakeup_sources,
            ipc::commands::get_live_ram,
            ipc::commands::get_io_stats,
            ipc::commands::list_packages,
            ipc::commands::classify_packages,
            ipc::commands::apply_optimization,
            ipc::commands::take_snapshot,
            ipc::commands::list_snapshots,
            ipc::commands::rollback_snapshot,
            ipc::commands::export_shell_script,
            ipc::commands::disable_bloatware,
            ipc::commands::enable_bloatware,
            ipc::commands::set_phantom_process_limit,
            ipc::commands::preview_profile,
            ipc::commands::apply_profile,
            ipc::commands::overview_snapshot,
            ipc::commands::battery_health,
            ipc::commands::process_status,
            ipc::commands::start_telemetry_stream,
            ipc::commands::stop_telemetry_stream,
            ipc::commands::miscategorized_apps,
            ipc::commands::sleep_score,
            ipc::commands::read_action_log,
            ipc::commands::list_dns_presets,
            ipc::commands::get_privacy_state,
            ipc::commands::get_dangerous_permissions,
            ipc::commands::set_private_dns,
            ipc::commands::apply_firewall,
            ipc::commands::apply_clipboard_guard,
            ipc::commands::storage_overview,
            ipc::commands::storage_inventory,
            ipc::commands::clear_app_cache,
            ipc::commands::trim_system_caches,
            ipc::commands::run_bg_dexopt,
            ipc::commands::get_display_settings,
            ipc::commands::apply_refresh_rate,
            ipc::commands::set_bluetooth_absolute_volume,
            ipc::commands::get_system_tweaks,
            ipc::commands::set_phantom_monitor,
            ipc::commands::set_captive_portal_mode,
            ipc::commands::compile_package,
            ipc::commands::reset_compilation,
            ipc::commands::sleep_timeline,
            ipc::commands::kernel_wakelocks,
            ipc::commands::battery_per_app,
            ipc::commands::resolve_app_labels,
            ipc::commands::set_master_mono,
            ipc::commands::set_spatial_audio,
            ipc::commands::set_avrcp_version,
            ipc::commands::bloatware_recommendations,
            ipc::commands::list_bloat_presets,
            ipc::commands::preview_bloat_preset,
            ipc::commands::get_performance_settings,
            ipc::commands::set_animation_scales,
            ipc::commands::set_aggressive_doze,
            ipc::commands::set_background_scan,
            ipc::commands::set_data_saver,
            ipc::commands::hibernate_package,
            ipc::commands::set_game_mode,
            ipc::commands::set_background_process_limit,
            ipc::commands::get_doze_state,
            ipc::commands::set_doze_whitelist,
            ipc::commands::set_force_doze,
            ipc::commands::simulate_unplug,
            ipc::commands::get_art_status_batch,
            ipc::commands::clear_temp_files,
            ipc::commands::get_all_standby_buckets,
            ipc::commands::set_standby_bucket,
            ipc::commands::set_appops,
            ipc::commands::force_stop_package,
            ipc::commands::open_app_settings,
            ipc::commands::get_app_restrictions_batch,
            ipc::commands::get_single_app_details,
            ipc::commands::clear_app_data,
            ipc::commands::uninstall_package,
            ipc::commands::compile_all_apps,
            ipc::commands::disable_ram_plus,
            ipc::commands::force_refresh_rate,
            ipc::commands::set_heads_up_notifications,
            ipc::commands::set_hotword_detection,
            ipc::commands::set_activity_logging,
            ipc::commands::set_adaptive_connectivity,
            ipc::commands::reboot_device,
            ipc::commands::set_display_density,
            ipc::commands::set_display_size,
            ipc::commands::reset_display,
            ipc::commands::set_window_blurs,
            ipc::commands::set_reduce_transparency,
            ipc::commands::set_fixed_performance_mode,
            ipc::commands::set_dark_mode,
            ipc::commands::set_stay_awake,
            ipc::commands::capture_screenshot,
            ipc::commands::install_apk,
            ipc::commands::extract_apk,
            ipc::commands::list_files,
            ipc::commands::push_file,
            ipc::commands::pull_file,
            ipc::commands::delete_file,
            ipc::commands::create_directory,
            ipc::commands::get_thermal_status,
            ipc::commands::get_network_usage,
            ipc::commands::kill_all_zombies,
            ipc::commands::trim_memory,
            ipc::commands::run_shell,
            ipc::commands::set_immersive_mode,
            ipc::commands::install_apks_multiple,
            ipc::commands::fastboot_reboot,
            ipc::commands::fastboot_flash,
            ipc::commands::export_native_profile,
            ipc::commands::import_native_profile,
            ipc::commands::adb_mdns_services,
            ipc::commands::adb_pair,
            ipc::commands::adb_connect,
            ipc::commands::adb_tcpip,
            ipc::commands::launch_scrcpy,
            ipc::diagnostics::get_system_properties,
            ipc::diagnostics::generate_bugreport,
            ipc::diagnostics::start_log_stream,
            ipc::diagnostics::stop_log_stream,
            ipc::streaming::start_ram_stream,
            ipc::streaming::stop_ram_stream,
            ipc::commands::set_charge_bypass,
            ipc::commands::disable_doze_motion,
            ipc::commands::tune_gms_heartbeat,
            ipc::commands::run_fstrim,
            ipc::commands::clean_orphaned_data,
            ipc::commands::set_tcp_congestion,
            ipc::commands::force_network_mode,
            ipc::commands::sideload_apk,
            ipc::commands::backup_app_data,
            ipc::commands::restore_backup,
            ipc::commands::enable_sensors_off_tile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DozeForge");
}
