//! End-to-end tests for the ferricel CLI
//!
//! These tests focus on the CLI interface and integration between components:
//! - Building Wasm files from CEL expressions
//! - Running Wasm files produced by the build command
//! - Passing bindings via command line arguments and files
//!
//! Note: Unit tests for CEL compilation logic are in src/compiler.rs

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Helper function to get a Command for the ferricel binary
#[allow(deprecated)]
fn ferricel() -> Command {
    Command::cargo_bin("ferricel").expect("Failed to find ferricel binary")
}

/// Helper to create a temporary JSON file with given content
fn create_json_file(content: &str) -> NamedTempFile {
    let file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(file.path(), content).expect("Failed to write to temp file");
    file
}

/// Helper to create a temporary CEL file with given expression
fn create_cel_file(content: &str) -> NamedTempFile {
    let file = NamedTempFile::new().expect("Failed to create temp CEL file");
    fs::write(file.path(), content).expect("Failed to write CEL expression to file");
    file
}

// ============================================================================
// BUILD COMMAND TESTS
// ============================================================================

#[test]
fn test_build_creates_wasm_file() {
    let output_file = NamedTempFile::new().unwrap();
    let output_path = output_file.path();

    ferricel()
        .args(["build", "-e", "5 + 10", "-o"])
        .arg(output_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully compiled"));

    // Verify the Wasm file was created and has content
    assert!(output_path.exists(), "Wasm file should exist");
    let metadata = fs::metadata(output_path).unwrap();
    assert!(metadata.len() > 0, "Wasm file should not be empty");
}

#[test]
fn test_build_with_default_output() {
    // Build without -o flag should create final_cel_program.wasm in current dir
    // We'll use a temp directory to avoid polluting the workspace
    let temp_dir = tempfile::tempdir().unwrap();

    ferricel()
        .current_dir(temp_dir.path())
        .args(["build", "-e", "10 * 2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("final_cel_program.wasm"));

    let default_output = temp_dir.path().join("final_cel_program.wasm");
    assert!(default_output.exists(), "Default output file should exist");
}

#[test]
fn test_build_missing_expression_flag() {
    ferricel()
        .args(["build"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_build_invalid_cel_expression() {
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "-e", "this is not valid CEL $$$ @@@", "-o"])
        .arg(output_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Parse")));
}

// ============================================================================
// BUILD COMMAND TESTS - WITH FILE INPUT
// ============================================================================

#[test]
fn test_build_from_file_complex_expression() {
    // Create a CEL file with a more complex expression
    let cel_file = create_cel_file("x > 10 && y < 20");
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "--expression-file"])
        .arg(cel_file.path())
        .arg("-o")
        .arg(output_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully compiled"));

    // Verify the Wasm file was created and has content
    let metadata = fs::metadata(output_file.path()).unwrap();
    assert!(metadata.len() > 0, "Wasm file should not be empty");
}

#[test]
fn test_build_from_file_with_custom_output() {
    let cel_file = create_cel_file("5 + 10");
    let output_file = NamedTempFile::new().unwrap();
    let output_path = output_file.path();

    ferricel()
        .args(["build", "--expression-file"])
        .arg(cel_file.path())
        .args(["-o"])
        .arg(output_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(output_path.to_str().unwrap()));

    assert!(output_path.exists(), "Custom output path should be used");
}

#[test]
fn test_build_expression_file_not_found() {
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "--expression-file", "nonexistent-file.cel", "-o"])
        .arg(output_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read CEL file"));
}

#[test]
fn test_build_expression_file_invalid_content() {
    let cel_file = create_cel_file("this is invalid CEL syntax !@# $$$");
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "--expression-file"])
        .arg(cel_file.path())
        .arg("-o")
        .arg(output_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Parse")));
}

#[test]
fn test_build_mutual_exclusivity_error() {
    let cel_file = create_cel_file("42");
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "-e", "100", "--expression-file"])
        .arg(cel_file.path())
        .arg("-o")
        .arg(output_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_build_missing_both_flags() {
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "-o"])
        .arg(output_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ============================================================================
// RUN COMMAND TESTS - WITHOUT VARIABLES
// ============================================================================

#[test]
fn test_run_simple_wasm_without_variables() {
    // Build a simple Wasm file
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run the Wasm file
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("42"));
}

#[test]
fn test_run_nonexistent_wasm_file() {
    ferricel()
        .args(["run", "/tmp/nonexistent_file_xyz123.wasm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ============================================================================
// RUN COMMAND TESTS - WITH BINDINGS FROM CLI
// ============================================================================

#[test]
fn test_run_with_bindings_json_inline() {
    // Build Wasm that uses age variable
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "age + 10", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with inline JSON
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--bindings-json", r#"{"age": 25}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("35"));
}

#[test]
fn test_run_with_bindings_json_multiple_vars() {
    // Build Wasm that uses multiple variables
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "x + y", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with both variables in bindings
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--bindings-json", r#"{"x": 100, "y": 200}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("300"));
}

// ============================================================================
// RUN COMMAND TESTS - WITH BINDINGS FROM FILES
// ============================================================================

#[test]
fn test_run_with_bindings_from_file() {
    // Create bindings JSON file
    let bindings_file = create_json_file(r#"{"age": 42}"#);

    // Build Wasm
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "age", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with bindings file
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--bindings-file")
        .arg(bindings_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("42"));
}

#[test]
fn test_run_with_bindings_from_file_multiple_vars() {
    // Create bindings JSON file with multiple variables
    let bindings_file = create_json_file(r#"{"a": 100, "b": 50}"#);

    // Build Wasm
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "a - b", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with bindings file
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--bindings-file")
        .arg(bindings_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("50"));
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_run_bindings_json_and_file_are_mutually_exclusive() {
    let wasm_file = NamedTempFile::new().unwrap();
    let bindings_file = create_json_file(r#"{"age": 30}"#);

    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--bindings-json", r#"{"age": 25}"#])
        .arg("--bindings-file")
        .arg(bindings_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_run_missing_bindings_file() {
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--bindings-file")
        .arg("/tmp/this_file_does_not_exist_xyz123.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_run_invalid_json_in_file() {
    // Create a file with invalid JSON
    let bad_json_file = create_json_file("this is not valid JSON { @@@ }");

    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "age", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--bindings-file")
        .arg(bad_json_file.path())
        .assert()
        .failure();
    // The error might be in stderr or could cause a panic/error
    // Just verify it fails - the specific error message may vary
}

// ============================================================================
// EXTENSION DECLARATION TESTS
// ============================================================================
// Note: parsing and file-loading logic is unit-tested in src/cmd/extensions.rs.
// Only CLI-level concerns (clap mutual exclusivity, full build→run pipeline)
// belong here.

#[test]
fn test_build_extensions_and_extensions_file_are_mutually_exclusive() {
    let ext_file = create_json_file(r#"[]"#);
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "-e", "abs(x)", "-o"])
        .arg(output_file.path())
        .args(["--extensions", "abs:global:1"])
        .arg("--extensions-file")
        .arg(ext_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_run_with_extension_produces_runtime_error_when_not_implemented() {
    // Build a Wasm that calls an extension, then run it without an implementation.
    // The runtime returns a CEL-level error encoded in the result JSON.
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "abs(x)", "-o"])
        .arg(wasm_file.path())
        .args(["--extensions", "abs:global:1"])
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--bindings-json", r#"{"x": -5}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("Extension not found"));
}

#[test]
fn test_build_no_warning_without_structs_no_proto() {
    // When expression doesn't use structs and no proto is provided,
    // there should be no warnings
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args(["build", "-e", "5 + 10", "-o"])
        .arg(output_file.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("ERRO").not())
        .stderr(predicate::str::contains("schema").not());
}

#[test]
fn test_build_warning_struct_without_proto() {
    // When using a protobuf-looking struct without providing proto descriptor,
    // a warning should be shown
    let output_file = NamedTempFile::new().unwrap();

    ferricel()
        .args([
            "build",
            "-e",
            "google.protobuf.BoolValue{value: true}",
            "-o",
        ])
        .arg(output_file.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("ERRO"))
        .stderr(predicate::str::contains("looks like a protobuf message"))
        .stderr(predicate::str::contains("no schema provided"))
        .stderr(predicate::str::contains("--proto-descriptor"));
}

#[test]
fn test_build_warning_proto_missing_type_definition() {
    // When proto descriptor is provided but doesn't contain the type being used,
    // a warning should be shown
    let output_file = NamedTempFile::new().unwrap();
    let proto_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty_types.pb");

    ferricel()
        .args(["build", "-e", "test.missing.UnknownType{field: 42}", "-o"])
        .arg(output_file.path())
        .arg("--proto-descriptor")
        .arg(&proto_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("ERRO"))
        .stderr(predicate::str::contains("looks like a protobuf message"))
        .stderr(predicate::str::contains(
            "not defined in the provided schema",
        ));
}

#[test]
fn test_build_no_warning_with_matching_proto() {
    // When struct type is properly defined in the provided proto,
    // no warning should be shown
    let output_file = NamedTempFile::new().unwrap();
    let proto_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple_types.pb");

    ferricel()
        .args([
            "build",
            "-e",
            "test.fixtures.TestMessage{id: 123, name: 'test'}",
            "-o",
        ])
        .arg(output_file.path())
        .arg("--proto-descriptor")
        .arg(&proto_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("ERRO").not());
}
