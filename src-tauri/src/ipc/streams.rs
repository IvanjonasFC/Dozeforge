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
}

/// Shared, optional running session. Replacing it cancels the previous task.
pub type StreamState = Arc<Mutex<Option<JoinHandle<()>>>>;

pub fn new_state() -> StreamState {
    Arc::new(Mutex::new(None))
}

pub async fn start(
    state: StreamState,
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

            let snapshot = match ProcessStatusParser::parse(&raw) {
                Ok(s) => s,
                Err(e) => {
                    warn!(target: "dozeforge::stream", "parse failed: {e}");
                    ticker.tick().await;
                    continue;
                }
            };

            let tick = TelemetryTick {
                device_serial: serial.clone(),
                snapshot,
                ts_ms: chrono::Utc::now().timestamp_millis(),
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
