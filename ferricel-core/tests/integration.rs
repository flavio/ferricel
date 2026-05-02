// Single integration test entry point for ferricel-core.
//
// All test modules are declared here so they compile into one binary.
// This means the `dead_code` lint sees the full picture: a helper in
// `common` is considered "used" if any submodule uses it.
//
// To add a new top-level test file:
//   1. Create `tests/my_tests.rs`.
//   2. Add `mod my_tests;` below.
//
// To add a new test group in a subdirectory:
//   1. Create `tests/mygroup/my_tests.rs`.
//   2. Add `mod my_tests;` to `tests/mygroup/mod.rs`.
//   3. Add `mod mygroup;` below (if not already present).

mod common;

mod arithmetic_tests;
mod compiler_tests;
mod container_tests;
mod double_tests;
mod extension_tests;
mod has_tests;
mod in_operator_tests;
mod json_output_tests;
mod list_tests;
mod namespace_tests;
mod numeric_tests;
mod string_tests;
mod struct_tests;
mod variable_tests;

mod ext;
mod kubernetes;
