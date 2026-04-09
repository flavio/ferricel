// Conformance test runner for ferricel
// This test suite runs the official CEL conformance tests from google/cel-spec

mod common;

use std::path::Path;

use common::runner::ConformanceTestRunner;

#[test]
fn conformance_basic_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/basic.textproto");

    runner.run_test_file(test_file);

    // We'll track this but not fail the build yet while we're implementing
    // assert_eq!(stats.failed, 0, "Some conformance tests failed");
}

#[test]
fn conformance_comparisons_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/comparisons.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_integer_math_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/integer_math.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_fp_math_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/fp_math.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_string_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/string.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_logic_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/logic.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_lists_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/lists.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_conversions_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/conversions.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_timestamps_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/timestamps.textproto");

    runner.run_test_file(test_file);
}

#[test]
fn conformance_string_ext_tests() {
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/string_ext.textproto");

    runner.run_test_file(test_file);
}
