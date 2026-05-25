//! Structured telemetry via `tracing` + persistent action log.

pub mod action_log;
pub mod logger;

pub use action_log::{ActionLog, ActionLogEntry};
pub use logger::init_default;
