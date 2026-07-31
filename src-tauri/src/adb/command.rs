//! Thin wrapper around `adb` invocations with structured timeouts.

use std::path::PathBuf;
use std::time::Duration;

use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::debug;

use crate::error::{Error, Result};

use super::DeviceSerial;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const TOP_TIMEOUT: Duration = Duration::from_secs(60);

/// Windows: CREATE_NO_WINDOW flag from winbase.h. Suppresses the console
/// window that would otherwise flash every time we spawn adb.exe â€” a
/// background Win32 console app spawning another console app makes a
/// visible cmd-style flash by default.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct AdbInvoker {
    pub adb_path: PathBuf,
    pub semaphore: Arc<Semaphore>,
}

impl AdbInvoker {
    pub fn new(adb_path: PathBuf) -> Self { 
        Self { 
            adb_path,
            semaphore: Arc::new(Semaphore::new(5)),
        } 
    }

    pub async fn shell(&self, serial: &DeviceSerial, cmd: &str, deadline: Duration) -> Result<String> {
        self.exec(&["-s", serial.as_str(), "shell", cmd], deadline).await
    }

    pub async fn exec(&self, args: &[&str], deadline: Duration) -> Result<String> {
        let _permit = self.semaphore.acquire().await.unwrap();
        debug!(target: "dozeforge::adb", ?args, "executing adb");

        let fut = async {
            let mut command = Command::new(&self.adb_path);
            command.args(args).kill_on_drop(true);

            #[cfg(windows)]
            command.creation_flags(CREATE_NO_WINDOW);

            let output = command.output().await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                return Err(Error::AdbCommand {
                    exit_code: output.status.code().unwrap_or(-1),
                    stderr,
                });
            }

            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        };

        match timeout(deadline, fut).await {
            Ok(result) => result,
            Err(_) => Err(Error::AdbTimeout(deadline)),
        }
    }

    /// Read-only exec with a small retry + backoff for transient ADB flakiness
    /// (a device momentarily offline, USB re-enumeration, adb server restart).
    ///
    /// SAFETY: only for **idempotent read** commands (`devices`, `getprop`,
    /// `dumpsys`…). Never use for mutations — a retried `flash`/`disable-user`
    /// that actually succeeded on the first, "timed-out" attempt would double
    /// apply.
    pub async fn exec_read(&self, args: &[&str], deadline: Duration) -> Result<String> {
        let mut last: Result<String> = Err(Error::AdbNotFound);
        for attempt in 0u32..3 {
            match self.exec(args, deadline).await {
                Ok(out) => return Ok(out),
                Err(e) => {
                    last = Err(e);
                    tokio::time::sleep(Duration::from_millis(200 * u64::from(attempt + 1))).await;
                }
            }
        }
        last
    }

    pub async fn shell_silent(&self, serial: &DeviceSerial, cmd: &str, deadline: Duration) -> Result<()> {
        self.shell(serial, cmd, deadline).await.map(|_| ())
    }
}