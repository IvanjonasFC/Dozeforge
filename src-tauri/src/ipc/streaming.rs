use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::state::AppState;
use crate::error::IpcError;
use crate::parsers::process_status::ProcessStatusParser;

pub type RamStreamState = Arc<Mutex<Option<JoinHandle<()>>>>;

pub fn new_ram_stream_state() -> RamStreamState {
    Arc::new(Mutex::new(None))
}

#[tauri::command]
pub async fn start_ram_stream(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    serial: String,
) -> std::result::Result<(), IpcError> {
    let mut guard = state.ram_stream.lock().await;
    if let Some(prev) = guard.take() {
        prev.abort();
    }

    let adb_path = state.adb.invoker.adb_path.clone();
    
    let task = tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new(adb_path);
        // top -d 1 -o PID,USER,S,%CPU,RSS,ARGS continuously
        cmd.arg("-s").arg(&serial).arg("shell").arg("top -b -d 1 -o PID,USER,S,%CPU,RSS,ARGS");
        
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        
        cmd.stdout(std::process::Stdio::piped());
        
        if let Ok(mut child) = cmd.spawn() {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = BufReader::new(stdout).lines();
                let mut buffer = String::new();
                
                while let Ok(Some(line)) = reader.next_line().await {
                    let is_empty = line.trim().is_empty();
                    let is_header = line.contains("PID") && line.contains("USER");
                    
                    if is_empty || is_header {
                        if !buffer.trim().is_empty() {
                            if let Ok(snap) = ProcessStatusParser::parse(&buffer) {
                                let _ = app.emit("ram_update", snap);
                            }
                            buffer.clear();
                        }
                    } else {
                        buffer.push_str(&line);
                        buffer.push('\n');
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
pub async fn stop_ram_stream(state: State<'_, Arc<AppState>>) -> std::result::Result<(), IpcError> {
    let mut guard = state.ram_stream.lock().await;
    if let Some(task) = guard.take() {
        task.abort();
    }
    Ok(())
}
