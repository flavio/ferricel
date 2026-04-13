//! CEL extension library compiler functions.
//!
//! This module contains compiler implementations for the additional CEL functions
//! defined in the CEL extensions specification (string, list, and polymorphic operations).
//! See: <https://github.com/google/cel-spec/blob/master/extensions/>

pub mod encoders;
pub mod lists;
pub mod poly;
pub mod strings;
