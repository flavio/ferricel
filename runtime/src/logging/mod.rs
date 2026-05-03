//! Logging infrastructure for ferricel runtime
//!
//! This module provides structured logging capabilities that work across the Wasm boundary.
//! The runtime uses `slog` for structured logging, and messages are sent to the host via
//! the `cel_log` host function.
//!
//! Log levels: Debug(0), Info(1), Warn(2), Error(3)
//! Default level: Info

mod drain;
mod event;

use ferricel_types::LogLevel;
use once_cell::sync::OnceCell;
use slog::{Drain, Logger, o};
use std::sync::atomic::{AtomicU8, Ordering};

// Global log level (0=Debug, 1=Info, 2=Warn, 3=Error)
// Stored as u8 for atomic operations, but accessed as LogLevel
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

// Global logger instance
static LOGGER: OnceCell<Logger> = OnceCell::new();

/// Get the global logger instance
pub fn get_logger() -> Logger {
    LOGGER
        .get_or_init(|| {
            let drain = drain::FerricelDrain::new().fuse();
            Logger::root(drain, o!("runtime" => "ferricel"))
        })
        .clone()
}

/// Set minimum log level (exposed to Wasm host)
/// 0=Debug, 1=Info, 2=Warn, 3=Error
#[unsafe(no_mangle)]
pub extern "C" fn cel_set_log_level(level: i32) {
    let level = level.clamp(0, 3) as u8;
    LOG_LEVEL.store(level, Ordering::Relaxed);
}

/// Get current log level
pub(crate) fn get_log_level() -> LogLevel {
    LogLevel::from(LOG_LEVEL.load(Ordering::Relaxed))
}
