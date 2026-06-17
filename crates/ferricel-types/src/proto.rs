//! Generated protobuf types for typed CEL value bindings.
//!
//! Re-exports types generated from:
//! - `cel/expr/value.proto` — canonical CEL value representation
//! - `bindings.proto` — typed variable bindings wrapper

/// Types from `cel.expr` package (cel/expr/value.proto)
pub mod cel {
    pub mod expr {
        include!(concat!(env!("OUT_DIR"), "/cel.expr.rs"));
    }
}

/// Types from `ferricel` package (bindings.proto).
/// `Bindings.variables` references `super::cel::expr::Value`, so this module
/// must be a sibling of the `cel` module above.
pub mod ferricel {
    include!(concat!(env!("OUT_DIR"), "/ferricel.rs"));
}

// Convenient re-export at the proto level
pub use ferricel::Bindings;
