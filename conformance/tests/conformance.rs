// Conformance test runner for ferricel
// This test suite runs the official CEL conformance tests from google/cel-spec

mod common;

use std::path::Path;

use common::{runner::ConformanceTestRunner, thresholds::Thresholds};

fn assert_suite(suite: &str, failed: usize, thresholds: &Thresholds) {
    let max = thresholds.max_failures(suite);
    assert!(
        failed <= max,
        "Regression in '{}': {} failures exceed the allowed maximum of {}. \
         Fix the regressions or update conformance/thresholds.toml.",
        suite,
        failed,
        max,
    );
}

#[test]
fn conformance_basic_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/basic.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("basic", stats.failed(), &thresholds);
}

#[test]
fn conformance_comparisons_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/comparisons.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("comparisons", stats.failed(), &thresholds);
}

#[test]
fn conformance_integer_math_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/integer_math.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("integer_math", stats.failed(), &thresholds);
}

#[test]
fn conformance_fp_math_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/fp_math.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("fp_math", stats.failed(), &thresholds);
}

#[test]
fn conformance_string_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/string.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("string", stats.failed(), &thresholds);
}

#[test]
fn conformance_logic_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/logic.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("logic", stats.failed(), &thresholds);
}

#[test]
fn conformance_lists_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/lists.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("lists", stats.failed(), &thresholds);
}

#[test]
fn conformance_conversions_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/conversions.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("conversions", stats.failed(), &thresholds);
}

#[test]
fn conformance_timestamps_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/timestamps.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("timestamps", stats.failed(), &thresholds);
}

#[test]
fn conformance_string_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/string_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("string_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_network_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/network_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("network_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_optionals_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/optionals.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("optionals", stats.failed(), &thresholds);
}

#[test]
fn conformance_encoders_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/encoders_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("encoders_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_math_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/math_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("math_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_bindings_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/bindings_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("bindings_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_block_ext_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/block_ext.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("block_ext", stats.failed(), &thresholds);
}

#[test]
fn conformance_namespace_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/namespace.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("namespace", stats.failed(), &thresholds);
}

#[test]
fn conformance_parse_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/parse.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("parse", stats.failed(), &thresholds);
}

#[test]
fn conformance_macros2_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/macros2.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("macros2", stats.failed(), &thresholds);
}

#[test]
fn conformance_macros_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/macros.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("macros", stats.failed(), &thresholds);
}

#[test]
fn conformance_type_deduction_tests() {
    let thresholds = Thresholds::load();
    let runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/type_deduction.textproto");
    let stats = runner.run_test_file(test_file);
    assert_suite("type_deduction", stats.failed(), &thresholds);
}
