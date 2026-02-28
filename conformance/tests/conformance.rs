// Conformance test runner for ferricel
// This test suite runs the official CEL conformance tests from google/cel-spec

use std::{collections::HashMap, path::Path};

use ferricel_types::LogLevel;
use serde_json::Value as JsonValue;
use slog::{Drain, Logger, o};

// Import compiler and runtime functions from ferricel-core
use ferricel_core::compiler::compile_cel_to_wasm;
use ferricel_core::runtime;

// Include the generated protobuf types
mod cel {
    pub mod expr {
        include!(concat!(env!("OUT_DIR"), "/cel.expr.rs"));

        pub mod conformance {
            pub mod test {
                include!(concat!(env!("OUT_DIR"), "/cel.expr.conformance.test.rs"));
            }
        }
    }
}

use cel::expr::conformance::test::{SimpleTest, SimpleTestFile};
use cel::expr::{ExprValue, Value};

// Test result for reporting
#[derive(Debug, Clone, PartialEq)]
enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
}

// Statistics for test execution
#[derive(Debug, Default)]
struct TestStats {
    passed: usize,
    failed: usize,
    skipped: usize,
    total: usize,
}

impl TestStats {
    fn record(&mut self, result: &TestResult) {
        self.total += 1;
        match result {
            TestResult::Passed => self.passed += 1,
            TestResult::Failed(_) => self.failed += 1,
            TestResult::Skipped(_) => self.skipped += 1,
        }
    }

    fn print_summary(&self, test_file: &str) {
        println!("\n{:=<60}", "");
        println!("Conformance Test Results: {}", test_file);
        println!("{:-<60}", "");
        println!("PASSED:  {:>4}", self.passed);
        println!("FAILED:  {:>4}", self.failed);
        println!("SKIPPED: {:>4}", self.skipped);
        println!("{:-<60}", "");
        println!("TOTAL:   {:>4}", self.total);
        println!("{:=<60}\n", "");
    }
}

// Skip list configuration
struct SkipList {
    rules: Vec<SkipRule>,
}

#[derive(Debug)]
struct SkipRule {
    file: Option<String>,
    section: Option<String>,
    test: Option<String>,
    reason: String,
}

impl SkipList {
    fn new() -> Self {
        // Start with a default skip list for known limitations
        let mut rules = Vec::new();

        // Skip protocol buffer tests (not supported yet)
        rules.push(SkipRule {
            file: Some("proto2".to_string()),
            section: None,
            test: None,
            reason: "Protocol buffer support not implemented".to_string(),
        });

        rules.push(SkipRule {
            file: Some("proto3".to_string()),
            section: None,
            test: None,
            reason: "Protocol buffer support not implemented".to_string(),
        });

        Self { rules }
    }

    fn should_skip(&self, file: &str, section: &str, test: &str) -> Option<String> {
        for rule in &self.rules {
            let file_match = rule.file.as_ref().map_or(true, |f| f == file);
            let section_match = rule.section.as_ref().map_or(true, |s| s == section);
            let test_match = rule.test.as_ref().map_or(true, |t| t == test);

            if file_match && section_match && test_match {
                return Some(rule.reason.clone());
            }
        }
        None
    }
}

// Main test executor
struct ConformanceTestRunner {
    skip_list: SkipList,
    logger: Logger,
}

impl ConformanceTestRunner {
    fn new() -> Self {
        // Create a logger for tests
        let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let logger = Logger::root(drain, o!());

        Self {
            skip_list: SkipList::new(),
            logger,
        }
    }

    fn run_test_file(&mut self, test_file_path: &Path) -> TestStats {
        let file_name = test_file_path.file_stem().unwrap().to_str().unwrap();

        println!("\n{:=<60}", "");
        println!("Running conformance tests from: {}", file_name);
        println!("{:=<60}", "");

        // Load the test file
        let test_file = self.load_test_file(test_file_path);
        let mut stats = TestStats::default();
        let mut failed_tests = Vec::new();

        // Run each section
        for section in &test_file.section {
            println!("\n  Section: {}", section.name);
            if !section.description.is_empty() {
                println!("    {}", section.description);
            }

            // Run each test
            for test in &section.test {
                let result = self.run_single_test(file_name, &section.name, test);
                stats.record(&result);

                match &result {
                    TestResult::Passed => {
                        println!("    ✓ {}", test.name);
                    }
                    TestResult::Failed(reason) => {
                        println!("    ✗ {}: {}", test.name, reason);
                        failed_tests.push((
                            section.name.clone(),
                            test.name.clone(),
                            reason.clone(),
                        ));
                    }
                    TestResult::Skipped(reason) => {
                        println!("    ⊘ {} (SKIPPED: {})", test.name, reason);
                    }
                }
            }
        }

        // Print failed tests at the end
        if !failed_tests.is_empty() {
            println!("\n{:-<60}", "");
            println!("Failed Tests:");
            println!("{:-<60}", "");
            for (section, test, reason) in failed_tests {
                println!("  [{}] {}", section, test);
                println!("    └─ {}", reason);
            }
        }

        stats.print_summary(file_name);
        stats
    }

    fn run_single_test(&self, file: &str, section: &str, test: &SimpleTest) -> TestResult {
        // Check skip list
        if let Some(reason) = self.skip_list.should_skip(file, section, &test.name) {
            return TestResult::Skipped(reason);
        }

        // Skip tests that require features we know aren't supported
        if !test.type_env.is_empty() {
            return TestResult::Skipped("Type environment not yet supported".to_string());
        }

        // Execute the test
        match self.execute_cel_expression(test) {
            Ok(actual_value) => {
                // Compare with expected result
                self.compare_result(test, actual_value)
            }
            Err(e) => TestResult::Failed(format!("Execution error: {}", e)),
        }
    }

    fn execute_cel_expression(&self, test: &SimpleTest) -> Result<JsonValue, String> {
        // Step 1: Compile the CEL expression to WASM (in memory)
        let wasm_bytes =
            compile_cel_to_wasm(&test.expr).map_err(|e| format!("Build failed: {}", e))?;

        // Step 2: Prepare input/data JSON if bindings are present
        let (input_json, data_json) = if !test.bindings.is_empty() {
            let bindings_json = self.convert_bindings_to_json(&test.bindings)?;
            // For now, put all bindings in 'input'
            // TODO: In the future, we might need to distinguish input vs data
            (Some(bindings_json), None)
        } else {
            (None, None)
        };

        // Step 3: Execute the WASM module in-memory
        let json_result = runtime::execute_wasm_with_vars(
            &wasm_bytes,
            input_json.as_deref(),
            data_json,
            LogLevel::Error, // Use Error level to reduce noise in test output
            self.logger.clone(),
        )
        .map_err(|e| {
            // Check if this is an expected error
            if test.result_matcher.is_some() {
                use cel::expr::conformance::test::simple_test::ResultMatcher;
                if matches!(test.result_matcher, Some(ResultMatcher::EvalError(_))) {
                    // This is expected to fail, return error marker
                    return format!("error: {}", e);
                }
            }
            format!("Run failed: {}", e)
        })?;

        // Step 4: Parse the JSON result
        serde_json::from_str(&json_result)
            .map_err(|e| format!("Failed to parse output '{}': {}", json_result, e))
    }

    fn convert_bindings_to_json(
        &self,
        bindings: &HashMap<String, ExprValue>,
    ) -> Result<String, String> {
        let mut json_bindings = serde_json::Map::new();

        for (key, expr_value) in bindings {
            if let Some(kind) = &expr_value.kind {
                let json_value = self.expr_value_to_json(kind)?;
                json_bindings.insert(key.clone(), json_value);
            }
        }

        serde_json::to_string(&json_bindings)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }

    fn expr_value_to_json(&self, kind: &cel::expr::expr_value::Kind) -> Result<JsonValue, String> {
        use cel::expr::expr_value::Kind;
        match kind {
            Kind::Value(v) => self.cel_value_to_json(v),
            Kind::Error(_) => Err("Cannot convert error to JSON".to_string()),
            Kind::Unknown(_) => Err("Cannot convert unknown to JSON".to_string()),
        }
    }

    fn cel_value_to_json(&self, value: &Value) -> Result<JsonValue, String> {
        if let Some(kind) = &value.kind {
            use cel::expr::value::Kind;
            match kind {
                Kind::NullValue(_) => Ok(JsonValue::Null),
                Kind::BoolValue(b) => Ok(JsonValue::Bool(*b)),
                Kind::Int64Value(i) => Ok(JsonValue::Number((*i).into())),
                Kind::Uint64Value(u) => Ok(JsonValue::Number((*u).into())),
                Kind::DoubleValue(d) => {
                    // Handle special float values (infinity, NaN) as strings
                    if d.is_infinite() || d.is_nan() {
                        if d.is_infinite() && d.is_sign_positive() {
                            Ok(JsonValue::String("Infinity".to_string()))
                        } else if d.is_infinite() && d.is_sign_negative() {
                            Ok(JsonValue::String("-Infinity".to_string()))
                        } else {
                            Ok(JsonValue::String("NaN".to_string()))
                        }
                    } else {
                        serde_json::Number::from_f64(*d)
                            .map(JsonValue::Number)
                            .ok_or_else(|| "Invalid double value".to_string())
                    }
                }
                Kind::StringValue(s) => Ok(JsonValue::String(s.clone())),
                Kind::BytesValue(b) => {
                    use base64::Engine as _;
                    Ok(JsonValue::String(
                        base64::engine::general_purpose::STANDARD.encode(b),
                    ))
                }
                Kind::ListValue(list) => {
                    let values: Result<Vec<_>, _> = list
                        .values
                        .iter()
                        .map(|v| self.cel_value_to_json(v))
                        .collect();
                    Ok(JsonValue::Array(values?))
                }
                Kind::MapValue(map) => {
                    let mut obj = serde_json::Map::new();
                    for entry in &map.entries {
                        if let Some(key) = &entry.key {
                            let key_str = self.cel_value_to_string(key)?;
                            if let Some(val) = &entry.value {
                                obj.insert(key_str, self.cel_value_to_json(val)?);
                            }
                        }
                    }
                    Ok(JsonValue::Object(obj))
                }
                _ => Err("Unsupported value type".to_string()),
            }
        } else {
            Err("Value has no kind".to_string())
        }
    }

    fn cel_value_to_string(&self, value: &Value) -> Result<String, String> {
        if let Some(kind) = &value.kind {
            use cel::expr::value::Kind;
            match kind {
                Kind::StringValue(s) => Ok(s.clone()),
                Kind::Int64Value(i) => Ok(i.to_string()),
                Kind::Uint64Value(u) => Ok(u.to_string()),
                _ => Err("Cannot convert value to string key".to_string()),
            }
        } else {
            Err("Value has no kind".to_string())
        }
    }

    fn compare_result(&self, test: &SimpleTest, actual: JsonValue) -> TestResult {
        use cel::expr::conformance::test::simple_test::ResultMatcher;

        match &test.result_matcher {
            Some(ResultMatcher::Value(expected)) => match self.cel_value_to_json(expected) {
                Ok(expected_json) => {
                    if self.values_equal(&expected_json, &actual) {
                        TestResult::Passed
                    } else {
                        TestResult::Failed(format!(
                            "Expected {:?}, got {:?}",
                            expected_json, actual
                        ))
                    }
                }
                Err(e) => TestResult::Failed(format!("Failed to convert expected value: {}", e)),
            },
            Some(ResultMatcher::EvalError(_)) => {
                // Test expects an error
                if actual.is_string() && actual.as_str().unwrap().starts_with("error:") {
                    TestResult::Passed
                } else {
                    TestResult::Failed(format!("Expected error, got: {:?}", actual))
                }
            }
            None => {
                // Default matcher: expects true
                if actual == JsonValue::Bool(true) {
                    TestResult::Passed
                } else {
                    TestResult::Failed(format!("Expected true, got {:?}", actual))
                }
            }
            _ => TestResult::Skipped("Unsupported result matcher type".to_string()),
        }
    }

    fn values_equal(&self, expected: &JsonValue, actual: &JsonValue) -> bool {
        match (expected, actual) {
            (JsonValue::Number(e), JsonValue::Number(a)) => {
                // Handle NaN specially
                if e.as_f64().map_or(false, |f| f.is_nan())
                    && a.as_f64().map_or(false, |f| f.is_nan())
                {
                    return true;
                }
                e == a
            }
            (JsonValue::Object(e), JsonValue::Object(a)) => {
                // Maps are order-agnostic
                if e.len() != a.len() {
                    return false;
                }
                e.iter()
                    .all(|(k, v)| a.get(k).map_or(false, |av| self.values_equal(v, av)))
            }
            _ => expected == actual,
        }
    }

    fn load_test_file(&self, path: &Path) -> SimpleTestFile {
        // Convert textproto to binary using protoc
        let proto_dir = Path::new("../cel-spec/proto");

        let output = std::process::Command::new("protoc")
            .arg(format!("--proto_path={}", proto_dir.display()))
            .arg("--encode=cel.expr.conformance.test.SimpleTestFile")
            .arg("cel/expr/conformance/test/simple.proto")
            .stdin(std::process::Stdio::from(
                std::fs::File::open(path).unwrap(),
            ))
            .output()
            .expect("Failed to run protoc");

        if !output.status.success() {
            panic!("protoc failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        // Decode with prost
        prost::Message::decode(&output.stdout[..]).expect("Failed to decode protobuf")
    }
}

#[test]
fn conformance_basic_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/basic.textproto");

    let stats = runner.run_test_file(test_file);

    // Report results - don't fail the test, just report
    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );

    // We'll track this but not fail the build yet while we're implementing
    // assert_eq!(stats.failed, 0, "Some conformance tests failed");
}

#[test]
fn conformance_comparisons_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/comparisons.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_integer_math_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/integer_math.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_fp_math_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/fp_math.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_string_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/string.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_logic_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/logic.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_lists_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/lists.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_conversions_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/conversions.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}

#[test]
fn conformance_timestamps_tests() {
    let mut runner = ConformanceTestRunner::new();
    let test_file = Path::new("../cel-spec/tests/simple/testdata/timestamps.textproto");

    let stats = runner.run_test_file(test_file);

    println!("\nConformance test summary:");
    println!(
        "  Pass rate: {:.1}%",
        (stats.passed as f64 / stats.total as f64) * 100.0
    );
}
