//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;

use crate::adb::AdbClient;
use crate::error::{Error, Result};
use crate::heuristics::manifest::HybridManifest;
use crate::ipc::streams::{self, StreamState};
use crate::ipc::diagnostics::{LogStreamState, new_log_stream_state};
use crate::ipc::streaming::{RamStreamState, new_ram_stream_state};
use crate::snapshot::store::SnapshotStore;
use crate::telemetry::ActionLog;

pub struct AppState {
    pub adb: Arc<AdbClient>,
    pub manifest: RwLock<HybridManifest>,
    pub snapshot_store: SnapshotStore,
    pub action_log: ActionLog,
    pub stream_state: StreamState,
    pub log_stream: LogStreamState,
    pub ram_stream: RamStreamState,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(app: AppHandle) -> Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| Error::other(format!("cannot resolve app_data_dir: {e}")))?;

        std::fs::create_dir_all(&data_dir)?;
        let snapshots_dir = data_dir.join("snapshots");
        std::fs::create_dir_all(&snapshots_dir)?;
        std::fs::create_dir_all(data_dir.join("exports"))?;
        std::fs::create_dir_all(data_dir.join("logs"))?;

        let snapshot_store = SnapshotStore::new(snapshots_dir);
        let manifest = HybridManifest::load_or_default()?;
        let adb = Arc::new(AdbClient::discover()?);
        let action_log = ActionLog::new(data_dir.join("actions.jsonl"));
        let stream_state = streams::new_state();
        let log_stream = new_log_stream_state();
        let ram_stream = new_ram_stream_state();

        Ok(Self {
            adb,
            manifest: RwLock::new(manifest),
            snapshot_store,
            action_log,
            stream_state,
            log_stream,
            ram_stream,
            data_dir,
        })
    }
}
