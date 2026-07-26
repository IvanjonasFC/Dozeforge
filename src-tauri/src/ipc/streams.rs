//! Telemetry stream — emits process-snapshot events every 3s while a session
//! is active. Frontend listens via `listen("telemetry_tick", ...)`.
//!
//! Lifecycle: `start_telemetry_stream(serial)` spawns a tokio task that loops
//! `top -b -n 1` -> emit. `stop_telemetry_stream()` cancels via a flag.
//! Re-entrant: calling start again replaces the previous session safely.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, warn};

use crate::adb::{AdbClient, DeviceSerial};
use crate::parsers::process_status::{ProcessSnapshot, ProcessStatusParser};

/// Event payload emitted to the frontend on each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryTick {
    pub device_serial: DeviceSerial,
    pub snapshot: ProcessSnapshot,
    pub ts_ms: i64,
    pub cpu_history: Vec<f32>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SystemStatsRaw {
    pub user: u64,
    pub sys: u64,
    pub io: u64,
    pub idle: u64,
    pub total: u64,
}

#[derive(Debug, Default)]
pub struct TelemetryHistory {
    pub cpu_points: std::collections::VecDeque<f32>,
    pub process_history: std::collections::HashMap<u32, ProcessHistory>,
    pub prev_stat: Option<SystemStatsRaw>,
}

#[derive(Debug, Default, Clone)]
pub struct ProcessHistory {
    pub count: u32,
    pub avg_cpu: f32,
    pub max_rss: u64,
}

impl TelemetryHistory {
    pub fn update(&mut self, snap: &mut ProcessSnapshot) {
        if let (Some(prev), Some(curr)) = (&self.prev_stat, &snap.raw_stat) {
            let total_diff = curr.total.saturating_sub(prev.total) as f32;
            if total_diff > 0.0 {
                snap.cpu_user = (curr.user.saturating_sub(prev.user) as f32 / total_diff) * 100.0;
                snap.cpu_sys = (curr.sys.saturating_sub(prev.sys) as f32 / total_diff) * 100.0;
                snap.cpu_iowait = (curr.io.saturating_sub(prev.io) as f32 / total_diff) * 100.0;
            }
        }
        if let Some(curr) = snap.raw_stat.clone() {
            self.prev_stat = Some(curr);
        }

        let total_cpu = snap.cpu_user + snap.cpu_sys + snap.cpu_iowait;
        self.cpu_points.push_back(total_cpu);
        if self.cpu_points.len() > 60 {
            self.cpu_points.pop_front();
        }

        let mut current_pids = std::collections::HashSet::new();
        for row in &mut snap.rows {
            current_pids.insert(row.pid);
            let hist = self.process_history.entry(row.pid).or_default();
            hist.count += 1;
            hist.avg_cpu = (hist.avg_cpu * (hist.count as f32 - 1.0) + row.cpu_percent) / hist.count as f32;
            hist.max_rss = hist.max_rss.max(row.rss_kb);

            // Smart Hog Detection logic (tracking history for >15s (5 ticks of 3s), %CPU >10%, or RSS >500MB + %CPU >5%).
            if hist.count >= 5 {
                if hist.avg_cpu > 10.0 || (hist.max_rss > 500 * 1024 && hist.avg_cpu > 5.0) {
                    row.is_smart_hog = true;
                }
            }
        }
        self.process_history.retain(|pid, _| current_pids.contains(pid));
    }
}

/// Shared, optional running session. Replacing it cancels the previous task.
pub type StreamState = Arc<Mutex<Option<JoinHandle<()>>>>;

pub fn new_state() -> StreamState {
    Arc::new(Mutex::new(None))
}

pub async fn start(
    state: StreamState,
    history_state: Arc<tokio::sync::Mutex<TelemetryHistory>>,
    adb: Arc<AdbClient>,
    app: AppHandle,
    serial: DeviceSerial,
    interval_secs: u64,
) {
    // Cancel any existing task first.
    let mut guard = state.lock().await;
    if let Some(prev) = guard.take() {
        prev.abort();
        debug!(target: "dozeforge::stream", "previous telemetry stream cancelled");
    }

    let task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(interval_secs.max(1)));
        // Skip the immediate first tick of `interval` (fires at t=0).
        ticker.tick().await;

        let cmd = ProcessStatusParser::command();
        loop {
            // Run ADB top with a generous timeout. If it takes longer than the
            // poll interval, the next tick is naturally delayed (tokio::interval
            // skips ticks when behind, so we never queue up requests).
            let raw = match adb.invoker.shell(&serial, cmd, Duration::from_secs(8)).await {
                Ok(out) => out,
                Err(e) => {
                    warn!(target: "dozeforge::stream", "top failed: {e}");
                    ticker.tick().await;
                    continue;
                }
            };

            let mut snapshot = match ProcessStatusParser::parse(&raw) {
                Ok(s) => s,
                Err(e) => {
                    warn!(target: "dozeforge::stream", "parse failed: {e}");
                    ticker.tick().await;
                    continue;
                }
            };

            let cpu_history = {
                let mut hist = history_state.lock().await;
                hist.update(&mut snapshot);
                hist.cpu_points.iter().copied().collect()
            };

            let tick = TelemetryTick {
                device_serial: serial.clone(),
                snapshot,
                ts_ms: chrono::Utc::now().timestamp_millis(),
                cpu_history,
            };

            if let Err(e) = app.emit("telemetry_tick", &tick) {
                warn!(target: "dozeforge::stream", "emit failed, stopping: {e}");
                break;
            }

            ticker.tick().await;
        }
    });

    *guard = Some(task);
}

pub async fn stop(state: StreamState) {
    let mut guard = state.lock().await;
    if let Some(task) = guard.take() {
        task.abort();
    }
}
