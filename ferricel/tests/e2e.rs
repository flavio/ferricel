//! End-to-end tests for the ferricel CLI
//!
//! These tests focus on the CLI interface and integration between components:
//! - Building WASM files from CEL expressions
//! - Running WASM files produced by the build command
//! - Passing input/data via command line arguments and files
//!
//! Note: Unit tests for CEL compilation logic are in src/compiler.rs

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
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

    // Verify the WASM file was created and has content
    assert!(output_path.exists(), "WASM file should exist");
    let metadata = fs::metadata(output_path).unwrap();
    assert!(metadata.len() > 0, "WASM file should not be empty");
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
// RUN COMMAND TESTS - WITHOUT VARIABLES
// ============================================================================

#[test]
fn test_run_simple_wasm_without_variables() {
    // Build a simple WASM file
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run the WASM file
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
// RUN COMMAND TESTS - WITH INPUT/DATA FROM CLI
// ============================================================================

#[test]
fn test_run_with_input_json_inline() {
    // Build WASM that uses input.age
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "input.age + 10", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with inline JSON
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--input-json", r#"{"age": 25}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("35"));
}

#[test]
fn test_run_with_data_json_inline() {
    // Build WASM that uses data.value
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "data.value * 2", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with inline JSON
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--data-json", r#"{"value": 50}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("100"));
}

#[test]
fn test_run_with_both_input_and_data_inline() {
    // Build WASM that uses both input and data
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "input.x + data.y", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with both inline
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--input-json", r#"{"x": 100}"#])
        .args(["--data-json", r#"{"y": 200}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("300"));
}

// ============================================================================
// RUN COMMAND TESTS - WITH INPUT/DATA FROM FILES
// ============================================================================

#[test]
fn test_run_with_input_from_file() {
    // Create input JSON file
    let input_file = create_json_file(r#"{"age": 42}"#);

    // Build WASM
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "input.age", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with input file
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--input-file")
        .arg(input_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("42"));
}

#[test]
fn test_run_with_data_from_file() {
    // Create data JSON file
    let data_file = create_json_file(r#"{"multiplier": 5}"#);

    // Build WASM
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "10 * data.multiplier", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with data file
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--data-file")
        .arg(data_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("50"));
}

#[test]
fn test_run_with_both_input_and_data_from_files() {
    // Create JSON files
    let input_file = create_json_file(r#"{"a": 100}"#);
    let data_file = create_json_file(r#"{"b": 50}"#);

    // Build WASM
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "input.a - data.b", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with both files
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--input-file")
        .arg(input_file.path())
        .arg("--data-file")
        .arg(data_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("50"));
}

#[test]
fn test_run_mixed_inline_json_and_file() {
    // Create data file
    let data_file = create_json_file(r#"{"y": 25}"#);

    // Build WASM
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "input.x + data.y", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    // Run with inline input and file data
    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--input-json", r#"{"x": 75}"#])
        .arg("--data-file")
        .arg(data_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("100"));
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_run_input_json_and_file_are_mutually_exclusive() {
    let wasm_file = NamedTempFile::new().unwrap();
    let input_file = create_json_file(r#"{"age": 30}"#);

    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--input-json", r#"{"age": 25}"#])
        .arg("--input-file")
        .arg(input_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_run_data_json_and_file_are_mutually_exclusive() {
    let wasm_file = NamedTempFile::new().unwrap();
    let data_file = create_json_file(r#"{"value": 10}"#);

    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .args(["--data-json", r#"{"value": 5}"#])
        .arg("--data-file")
        .arg(data_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_run_missing_input_file() {
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--input-file")
        .arg("/tmp/this_file_does_not_exist_xyz123.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_run_missing_data_file() {
    let wasm_file = NamedTempFile::new().unwrap();
    ferricel()
        .args(["build", "-e", "42", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--data-file")
        .arg("/tmp/this_file_does_not_exist_xyz456.json")
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
        .args(["build", "-e", "input.age", "-o"])
        .arg(wasm_file.path())
        .assert()
        .success();

    ferricel()
        .args(["run"])
        .arg(wasm_file.path())
        .arg("--input-file")
        .arg(bad_json_file.path())
        .assert()
        .failure();
    // The error might be in stderr or could cause a panic/error
    // Just verify it fails - the specific error message may vary
}
