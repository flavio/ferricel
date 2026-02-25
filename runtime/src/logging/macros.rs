//! Logging macros for ferricel runtime
//!
//! Provides convenient macros for logging at different levels and
//! a special cel_panic! macro that logs before panicking.

// Re-export slog macros with cel_ prefix
pub use slog::debug as cel_debug;
pub use slog::error as cel_error;
pub use slog::info as cel_info;
pub use slog::warn as cel_warn;

/// Macro for logging an error and then panicking
///
/// # Examples
///
/// ```ignore
/// cel_panic!(log, "Division by zero");
/// cel_panic!(log, "Type mismatch"; "expected" => "Int", "actual" => "String");
/// ```
#[macro_export]
macro_rules! cel_panic {
    ($log:expr, $msg:expr) => {{
        slog::error!($log, $msg);
        panic!($msg);
    }};
    ($log:expr, $msg:expr; $($key:tt => $val:expr),+ $(,)?) => {{
        slog::error!($log, $msg; $($key => $val),+);
        panic!($msg);
    }};
}
