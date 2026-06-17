//! Event serialization for structured logging
//!
//! Creates LogEvent structs with file/line context and structured key-value pairs

use anyhow::Result;
use ferricel_types::{LogEvent, LogLevel};
use serde_json::{Map, Value, json};
use slog::{KV, OwnedKVList, Record};

/// Serializer for extracting key-value pairs from slog records
struct SimpleSerializer {
    data: Map<String, Value>,
}

impl SimpleSerializer {
    fn new() -> Self {
        SimpleSerializer { data: Map::new() }
    }
}

impl slog::Serializer for SimpleSerializer {
    fn emit_arguments(&mut self, key: slog::Key, val: &std::fmt::Arguments) -> slog::Result {
        self.data.insert(key.to_string(), json!(format!("{}", val)));
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, val: &str) -> slog::Result {
        self.data.insert(key.to_string(), json!(val));
        Ok(())
    }

    fn emit_i64(&mut self, key: slog::Key, val: i64) -> slog::Result {
        self.data.insert(key.to_string(), json!(val));
        Ok(())
    }

    fn emit_u64(&mut self, key: slog::Key, val: u64) -> slog::Result {
        self.data.insert(key.to_string(), json!(val));
        Ok(())
    }

    fn emit_f64(&mut self, key: slog::Key, val: f64) -> slog::Result {
        self.data.insert(key.to_string(), json!(val));
        Ok(())
    }

    fn emit_bool(&mut self, key: slog::Key, val: bool) -> slog::Result {
        self.data.insert(key.to_string(), json!(val));
        Ok(())
    }

    fn emit_unit(&mut self, key: slog::Key) -> slog::Result {
        self.data.insert(key.to_string(), json!(null));
        Ok(())
    }

    fn emit_none(&mut self, key: slog::Key) -> slog::Result {
        self.data.insert(key.to_string(), json!(null));
        Ok(())
    }
}

/// Create a structured log event from slog Record
pub fn new(record: &Record, values: &OwnedKVList) -> Result<LogEvent> {
    let level = match record.level() {
        slog::Level::Debug | slog::Level::Trace => LogLevel::Debug,
        slog::Level::Info => LogLevel::Info,
        slog::Level::Warning => LogLevel::Warn,
        slog::Level::Error | slog::Level::Critical => LogLevel::Error,
    };

    // Extract structured key-value pairs
    let mut serializer = SimpleSerializer::new();
    record.kv().serialize(record, &mut serializer)?;
    values.serialize(record, &mut serializer)?;

    // Remove "runtime" key added by logger root
    serializer.data.remove("runtime");

    Ok(LogEvent {
        level,
        message: format!("{}", record.msg()),
        file: record.file().to_string(),
        line: record.line(),
        column: record.column(),
        extra: serializer.data,
    })
}
