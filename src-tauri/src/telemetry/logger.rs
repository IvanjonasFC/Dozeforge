//! Structured logging setup.

use std::io;

use tracing_appender::rolling;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::error::Result;

pub fn init_default() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("dozeforge=info,tauri=info,warn"));

    let stderr_layer = fmt::layer()
        .with_writer(io::stderr)
        .with_target(true)
        .with_ansi(true);

    let file_layer = if std::env::var("DOZEFORGE_NO_LOG").is_ok() {
        None
    } else if let Some(data) = dirs::data_dir() {
        let logs_dir = data.join("DozeForge").join("logs");
        if std::fs::create_dir_all(&logs_dir).is_ok() {
            let file_appender = rolling::daily(&logs_dir, "dozeforge.log");
            Some(
                fmt::layer()
                    .json()
                    .with_writer(file_appender)
                    .with_target(true)
                    .with_current_span(false)
                    .with_span_list(false),
            )
        } else {
            None
        }
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry().with(filter).with(stderr_layer);

    if let Some(file_layer) = file_layer {
        subscriber.with(file_layer).try_init().ok();
    } else {
        subscriber.try_init().ok();
    }

    Ok(())
}
