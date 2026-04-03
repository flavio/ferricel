//! Shared types used across ferricel workspace

pub mod extensions;
pub mod functions;
pub mod proto;

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Log levels for ferricel runtime
///
/// Ordered from most verbose to least verbose for comparison operations.
/// The u8 representation is used for FFI and atomic storage.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum LogLevel {
    /// Debug level (most verbose)
    Debug = 0,
    /// Info level (default)
    #[default]
    Info = 1,
    /// Warning level
    Warn = 2,
    /// Error level (least verbose)
    Error = 3,
}

impl LogLevel {
    /// Convert to u8 for FFI/atomic operations
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Convert to i32 for WASM FFI
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// Default log level
    pub const fn default() -> Self {
        LogLevel::Info
    }
}

impl From<u8> for LogLevel {
    fn from(val: u8) -> Self {
        match val {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Warn,
            _ => LogLevel::Error, // 3 or higher defaults to Error
        }
    }
}

impl From<LogLevel> for u8 {
    fn from(level: LogLevel) -> u8 {
        level.as_u8()
    }
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" | "err" => Ok(LogLevel::Error),
            _ => Err(format!(
                "Invalid log level: '{}'. Valid values: debug, info, warn, error",
                s
            )),
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Structured log event passed from WASM guest to host
///
/// This struct represents a log event with file/line context and optional
/// structured key-value pairs. It's serialized to JSON in the WASM guest
/// and deserialized on the host side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    /// Log level
    pub level: LogLevel,

    /// Log message
    pub message: String,

    /// Source file where the log was emitted
    pub file: String,

    /// Line number in the source file
    pub line: u32,

    /// Column number in the source file (optional)
    #[serde(default)]
    pub column: u32,

    /// Additional structured key-value pairs from slog
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("warning".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("error".parse::<LogLevel>().unwrap(), LogLevel::Error);
        assert_eq!("err".parse::<LogLevel>().unwrap(), LogLevel::Error);

        // Case insensitive
        assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("Info".parse::<LogLevel>().unwrap(), LogLevel::Info);

        // Invalid
        assert!("invalid".parse::<LogLevel>().is_err());
    }

    #[test]
    fn test_to_string() {
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_u8_conversion() {
        assert_eq!(LogLevel::Debug.as_u8(), 0);
        assert_eq!(LogLevel::Info.as_u8(), 1);
        assert_eq!(LogLevel::Warn.as_u8(), 2);
        assert_eq!(LogLevel::Error.as_u8(), 3);

        assert_eq!(LogLevel::from(0), LogLevel::Debug);
        assert_eq!(LogLevel::from(1), LogLevel::Info);
        assert_eq!(LogLevel::from(2), LogLevel::Warn);
        assert_eq!(LogLevel::from(3), LogLevel::Error);
        assert_eq!(LogLevel::from(99), LogLevel::Error); // Out of range defaults to Error
    }

    #[test]
    fn test_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_default() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }
}
