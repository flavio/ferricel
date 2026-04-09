// Integration tests for CEL extended library functions.
// Each extension group lives in its own submodule under `ext/`.
//
// To add a new group (e.g. encoders):
//   1. Create `tests/ext/encoders.rs` with the test functions.
//   2. Add `mod encoders;` to `tests/ext/mod.rs`.
//   No changes to this file are needed.

mod common;
mod ext;
