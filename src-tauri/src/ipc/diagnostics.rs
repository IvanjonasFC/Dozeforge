use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::state::AppState;
use crate::error::IpcError;
use crate::adb::DeviceSerial;

// Shared state for the logcat/dmesg stream task.
pub type LogStreamState = Arc<Mutex<Option<JoinHandle<()>>>>;

pub fn new_log_stream_state() -> LogStreamState {
    Arc::new(Mutex::new(None))
}

#[tauri::command]
pub async fn get_system_properties(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<HashMap<String, String>, IpcError> {
    let raw = state
        .adb
        .invoker
        .shell(&DeviceSerial(serial), "getprop", Duration::from_secs(10))
        .await
        .unwrap_or_default();

    let mut props = HashMap::new();
    // parse lines like: [ro.product.model]: [Pixel 8 Pro]
    for line in raw.lines() {
        if let Some(start) = line.find('[') {
            if let Some(end) = line[start + 1..].find(']') {
                let key = &line[start + 1..start + 1 + end];
                if let Some(val_start) = line[start + 1 + end + 1..].find('[') {
                    let offset = start + 1 + end + 1 + val_start + 1;
                    if let Some(val_end) = line[offset..].find(']') {
                        let value = &line[offset..offset + val_end];
                        props.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
    }
    Ok(props)
}

#[tauri::command]
pub async fn generate_bugreport(
    state: State<'_, Arc<AppState>>,
    serial: String,
) -> std::result::Result<String, IpcError> {
    // A full bugreport takes ~3-5 minutes
    let raw = state
        .adb
        .invoker
        .shell(&DeviceSerial(serial), "bugreportz", Duration::from_secs(300))
        .await
        .map_err(IpcError::from)?;
    Ok(raw)
}

#[tauri::command]
pub async fn start_log_stream(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    serial: String,
    mode: String,
) -> std::result::Result<(), IpcError> {
    let mut guard = state.log_stream.lock().await;
    if let Some(prev) = guard.take() {
        prev.abort();
    }

    let adb_path = state.adb.invoker.adb_path.clone();
    
    let task = tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new(adb_path);
        cmd.arg("-s").arg(&serial).arg("shell");
        
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        
        if mode == "dmesg" {
            cmd.arg("dmesg").arg("-w");
        } else {
            // Default to logcat, clearing first to get fresh logs
            // Actually, we just read the stream.
            cmd.arg("logcat");
        }
        
        cmd.stdout(std::process::Stdio::piped());
        
        if let Ok(mut child) = cmd.spawn() {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = BufReader::new(stdout).lines();
                let mut buffer = Vec::with_capacity(100);
                let mut last_emit = tokio::time::Instant::now();

                while let Ok(Some(line)) = reader.next_line().await {
                    buffer.push(line);
                    
                    if buffer.len() >= 50 || last_emit.elapsed().as_millis() >= 100 {
                        if app.emit("log-batch", &buffer).is_err() {
                            break;
                        }
                        buffer.clear();
                        last_emit = tokio::time::Instant::now();
                    }
                }
            }
            let _ = child.kill().await;
        }
    });

    *guard = Some(task);
    Ok(())
}

#[tauri::command]
pub async fn stop_log_stream(state: State<'_, Arc<AppState>>) -> std::result::Result<(), IpcError> {
    let mut guard = state.log_stream.lock().await;
    if let Some(task) = guard.take() {
        task.abort();
    }
    Ok(())
}
