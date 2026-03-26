// Integration tests for Kubernetes CEL extension functions.
// Each extension group lives in its own submodule under `kubernetes/`.
//
// To add a new group (e.g. strings):
//   1. Create `tests/kubernetes/strings.rs` with the test functions.
//   2. Add `mod strings;` to `tests/kubernetes/mod.rs`.
//   No changes to this file are needed.

mod common;
mod kubernetes;
