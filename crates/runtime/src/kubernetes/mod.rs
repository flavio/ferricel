//! Kubernetes CEL extensions.
//!
//! This module contains implementations of the additional CEL functions that
//! Kubernetes adds on top of the standard CEL specification.
//! See: <https://kubernetes.io/docs/reference/using-api/cel/#kubernetes-cel-libraries>

pub mod cidr;
pub mod dispatch;
pub mod format;
pub mod ip;
pub mod lists;
pub mod quantity;
pub mod regex;
pub mod semver;
pub mod url;

#[cfg(test)]
pub(super) mod test_helpers {
    // Re-export the crate-level helpers so existing `super::super::test_helpers::*`
    // imports in kubernetes submodules continue to work unchanged.
    pub(super) use crate::test_helpers::{make_array, make_int, make_str, make_val, read_val};
}
