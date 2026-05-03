//! Slog drain implementation that sends log events to the host via cel_log

use anyhow::Result;
use ferricel_types::LogLevel;
use slog::{Drain, Level, OwnedKVList, Record};

pub struct FerricelDrain {}

impl FerricelDrain {
    pub fn new() -> Self {
        FerricelDrain {}
    }
}

// Host function declaration - will be provided by wasmtime
// For tests, provide a no-op stub since we're not running in Wasm
#[cfg(not(test))]
unsafe extern "C" {
    fn cel_log(ptr: i32, len: i32);
}

#[cfg(test)]
unsafe extern "C" fn cel_log(_ptr: i32, _len: i32) {
    // No-op stub for tests - logging is not available outside Wasm
}

impl Drain for FerricelDrain {
    type Ok = ();
    type Err = anyhow::Error;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<()> {
        // Convert slog level to our LogLevel for comparison
        let record_level = match record.level() {
            Level::Debug | Level::Trace => LogLevel::Debug,
            Level::Info => LogLevel::Info,
            Level::Warning => LogLevel::Warn,
            Level::Error | Level::Critical => LogLevel::Error,
        };

        let min_level = super::get_log_level();
        if record_level < min_level {
            return Ok(()); // Skip logging - below threshold
        }

        // Serialize event to JSON
        let event = super::event::new(record, values)?;
        let json = serde_json::to_vec(&event)?;

        // Call host function
        unsafe {
            cel_log(json.as_ptr() as i32, json.len() as i32);
        }

        Ok(())
    }
}
