// Conformance test runner for ferricel
// This test suite runs the official CEL conformance tests from google/cel-spec

use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use rayon::prelude::*;
use serde_json::Value as JsonValue;
use slog::{Drain, Logger, o};

// Import compiler and runtime functions from ferricel-core
use ferricel_core::compiler::{CompilerOptions, compile_cel_to_wasm};
use ferricel_core::runtime::CelEngine;

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
    passed: AtomicUsize,
    failed: AtomicUsize,
    skipped: AtomicUsize,
    total: AtomicUsize,
}

impl TestStats {
    fn record(&self, result: &TestResult) {
        self.total.fetch_add(1, Ordering::SeqCst);
        match result {
            TestResult::Passed => {
                self.passed.fetch_add(1, Ordering::SeqCst);
            }
            TestResult::Failed(_) => {
                self.failed.fetch_add(1, Ordering::SeqCst);
            }
            TestResult::Skipped(_) => {
                self.skipped.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    fn print_summary(&self, test_file: &str, filtered: bool) {
        let passed = self.passed.load(Ordering::SeqCst);
        let failed = self.failed.load(Ordering::SeqCst);
        let skipped = self.skipped.load(Ordering::SeqCst);
        let total = self.total.load(Ordering::SeqCst);

        let total_f64 = total as f64;
        let passed_pct = if total > 0 {
            (passed as f64 / total_f64) * 100.0
        } else {
            0.0
        };
        let failed_pct = if total > 0 {
            (failed as f64 / total_f64) * 100.0
        } else {
            0.0
        };
        let skipped_pct = if total > 0 {
            (skipped as f64 / total_f64) * 100.0
        } else {
            0.0
        };

        println!("\n{:=<60}", "");
        print!("Conformance Test Results: {}", test_file);
        if filtered {
            print!(" [FILTERED]");
        }
        println!();
        println!("{:-<60}", "");
        println!(
            "PASSED:  {:>4} / {:>4}  ({:>5.1}%)",
            passed, total, passed_pct
        );
        println!(
            "FAILED:  {:>4} / {:>4}  ({:>5.1}%)",
            failed, total, failed_pct
        );
        println!(
            "SKIPPED: {:>4} / {:>4}  ({:>5.1}%)",
            skipped, total, skipped_pct
        );
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
        let rules = vec![SkipRule {
            file: Some("proto2".to_string()),
            section: None,
            test: None,
            reason: "Protocol buffer support not implemented".to_string(),
        }];

        // Note: proto3 tests are now partially supported with wrapper type semantics
        // Individual tests may still fail for unimplemented features

        Self { rules }
    }

    fn should_skip(&self, file: &str, section: &str, test: &str) -> Option<String> {
        for rule in &self.rules {
            let file_match = rule.file.as_ref().is_none_or(|f| f == file);
            let section_match = rule.section.as_ref().is_none_or(|s| s == section);
            let test_match = rule.test.as_ref().is_none_or(|t| t == test);

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
    proto_descriptor: Option<Vec<u8>>,
}

impl ConformanceTestRunner {
    fn new() -> Self {
        // Create a logger for tests
        let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let logger = Logger::root(drain, o!());

        // Load the proto descriptor generated at build time
        let proto_descriptor = std::fs::read(env!("PROTO_DESCRIPTOR_PATH")).ok();

        Self {
            skip_list: SkipList::new(),
            logger,
            proto_descriptor,
        }
    }

    fn list_sections(&self, test_file_path: &Path) {
        let file_name = test_file_path.file_stem().unwrap().to_str().unwrap();
        let test_file = self.load_test_file(test_file_path);

        println!("\nAvailable sections in: {}\n", file_name);

        let mut total_sections = 0;
        let mut total_tests = 0;

        for section in &test_file.section {
            let test_count = section.test.len();
            total_sections += 1;
            total_tests += test_count;

            println!("  {} ({} tests)", section.name, test_count);
            if !section.description.is_empty() {
                println!("    {}", section.description);
            }
            println!();
        }

        println!(
            "Total: {} sections, {} tests\n",
            total_sections, total_tests
        );
    }

    fn list_tests(&self, test_file_path: &Path, section_name: &str) {
        let file_name = test_file_path.file_stem().unwrap().to_str().unwrap();
        let test_file = self.load_test_file(test_file_path);

        // Find the specified section
        let section = test_file.section.iter().find(|s| s.name == section_name);

        match section {
            Some(section) => {
                println!("\nTests in section: {} ({})\n", section_name, file_name);

                if !section.description.is_empty() {
                    println!("{}\n", section.description);
                }

                for test in &section.test {
                    println!("  {}", test.name);
                    println!("    Expression: {:?}", test.expr);
                    println!();
                }

                println!("Total: {} tests\n", section.test.len());
            }
            None => {
                eprintln!(
                    "Error: Section '{}' not found in test file '{}'",
                    section_name, file_name
                );
                eprintln!("\nAvailable sections:");
                for section in &test_file.section {
                    eprintln!("  - {}", section.name);
                }
                std::process::exit(1);
            }
        }
    }

    fn run_test_file(&self, test_file_path: &Path) -> TestStats {
        let file_name = test_file_path.file_stem().unwrap().to_str().unwrap();

        // Check for listing mode
        let list_mode = std::env::var("CONFORMANCE_LIST").is_ok();
        if list_mode {
            let section_filter = std::env::var("CONFORMANCE_SECTION").ok();
            match section_filter {
                Some(section_name) => self.list_tests(test_file_path, &section_name),
                None => self.list_sections(test_file_path),
            }
            // Exit after listing - don't run tests
            std::process::exit(0);
        }

        // Check for filters
        let section_filter = std::env::var("CONFORMANCE_SECTION").ok();
        let test_filter = std::env::var("CONFORMANCE_TEST").ok();

        // Validate: if test filter is set, section filter must also be set
        if test_filter.is_some() && section_filter.is_none() {
            eprintln!("Error: CONFORMANCE_TEST requires CONFORMANCE_SECTION to also be set");
            eprintln!(
                "Usage: CONFORMANCE_SECTION=<section> CONFORMANCE_TEST=<test> make conformance-<suite>"
            );
            std::process::exit(1);
        }

        println!("\n{:=<60}", "");
        print!("Running conformance tests from: {}", file_name);

        // Display filter status
        if section_filter.is_some() || test_filter.is_some() {
            println!();
            print!("[Filtered: ");
            if let Some(ref section) = section_filter {
                print!("section={}", section);
                if let Some(ref test) = test_filter {
                    print!(", test={}", test);
                }
            }
            println!("]");
        } else {
            println!();
        }

        println!("{:=<60}", "");

        // Load the test file
        let test_file = self.load_test_file(test_file_path);
        let stats = TestStats::default();
        let failed_tests = Mutex::new(Vec::new());

        // Run each section sequentially (to maintain clean output)
        // but parallelize tests within each section
        for section in &test_file.section {
            // Apply section filter
            if let Some(ref filter) = section_filter
                && &section.name != filter
            {
                continue; // Skip this section
            }

            println!("\n  Section: {}", section.name);
            if !section.description.is_empty() {
                println!("    {}", section.description);
            }

            // Filter tests if needed
            let tests_to_run: Vec<_> = if let Some(ref filter) = test_filter {
                section.test.iter().filter(|t| &t.name == filter).collect()
            } else {
                section.test.iter().collect()
            };

            // Check if the test filter didn't match anything
            if let Some(ref filter) = test_filter
                && tests_to_run.is_empty()
            {
                eprintln!(
                    "\nError: Test '{}' not found in section '{}'",
                    filter, section.name
                );
                eprintln!("\nAvailable tests in this section:");
                for test in &section.test {
                    eprintln!("  - {}", test.name);
                }
                std::process::exit(1);
            }

            // Collect results for this section with parallel execution
            let section_results: Vec<_> = tests_to_run
                .par_iter()
                .map(|test| {
                    let result = self.run_single_test(file_name, &section.name, test);
                    stats.record(&result);
                    (test.name.clone(), result)
                })
                .collect();

            // Print results sequentially for cleaner output
            for (test_name, result) in section_results {
                match &result {
                    TestResult::Passed => {
                        println!("    ✓ {}", test_name);
                    }
                    TestResult::Failed(reason) => {
                        println!("    ✗ {}: {}", test_name, reason);
                        failed_tests.lock().unwrap().push((
                            section.name.clone(),
                            test_name,
                            reason.clone(),
                        ));
                    }
                    TestResult::Skipped(reason) => {
                        println!("    ⊘ {} (SKIPPED: {})", test_name, reason);
                    }
                }
            }
        }

        // Check if section filter didn't match anything
        if let Some(ref filter) = section_filter
            && stats.total.load(Ordering::SeqCst) == 0
        {
            eprintln!(
                "\nError: Section '{}' not found in test file '{}'",
                filter, file_name
            );
            eprintln!("\nAvailable sections:");
            for section in &test_file.section {
                eprintln!("  - {}", section.name);
            }
            std::process::exit(1);
        }

        // Print failed tests at the end
        let failed_tests = failed_tests.into_inner().unwrap();
        if !failed_tests.is_empty() {
            println!("\n{:-<60}", "");
            println!("Failed Tests:");
            println!("{:-<60}", "");
            for (section, test, reason) in failed_tests {
                println!("  [{}] {}", section, test);
                println!("    └─ {}", reason);
            }
        }

        stats.print_summary(file_name, section_filter.is_some() || test_filter.is_some());
        stats
    }

    fn run_single_test(&self, file: &str, section: &str, test: &SimpleTest) -> TestResult {
        // Check skip list
        if let Some(reason) = self.skip_list.should_skip(file, section, &test.name) {
            return TestResult::Skipped(reason);
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
        let compiler_options = CompilerOptions {
            proto_descriptor: self.proto_descriptor.clone(),
            container: if test.container.is_empty() {
                None
            } else {
                Some(test.container.clone())
            },
            logger: self.logger.clone(),
            extensions: vec![],
        };
        let wasm_bytes = match compile_cel_to_wasm(&test.expr, compiler_options) {
            Ok(bytes) => bytes,
            Err(e) => {
                // Check if this test expects an error (eval_error or any error)
                use cel::expr::conformance::test::simple_test::ResultMatcher;
                if matches!(test.result_matcher, Some(ResultMatcher::EvalError(_))) {
                    // Build error counts as eval error for tests with disable_check
                    // Return as error marker (to match expected error format)
                    return Ok(JsonValue::String(format!("error: {}", e)));
                }
                return Err(format!("Build failed: {}", e));
            }
        };

        // Step 2: Prepare bindings JSON if bindings are present
        let bindings_json = if !test.bindings.is_empty() {
            Some(self.convert_bindings_to_json(&test.bindings)?)
        } else {
            None
        };

        // Step 3: Execute the WASM module in-memory
        let json_result = match CelEngine::new(self.logger.clone())
            .execute(&wasm_bytes, bindings_json.as_deref())
        {
            Ok(result) => result,
            Err(e) => {
                // Check if this is an expected error
                if test.result_matcher.is_some() {
                    use cel::expr::conformance::test::simple_test::ResultMatcher;
                    if matches!(test.result_matcher, Some(ResultMatcher::EvalError(_))) {
                        // This is expected to fail, return error marker as a JSON string
                        return Ok(JsonValue::String(format!("error: {}", e)));
                    }
                }
                return Err(format!("Run failed: {}", e));
            }
        };

        // Step 4: Parse the JSON result
        let parsed: JsonValue = serde_json::from_str(&json_result)
            .map_err(|e| format!("Failed to parse output '{}': {}", json_result, e))?;

        // Step 5: Check if result is an error value and convert to expected format
        // CEL error values serialize as {"error": "message"}, but tests expect "error: message" string
        if let Some(obj) = parsed.as_object()
            && obj.len() == 1
            && obj.contains_key("error")
            && let Some(error_msg) = obj.get("error").and_then(|v| v.as_str())
        {
            return Ok(JsonValue::String(format!("error: {}", error_msg)));
        }

        Ok(parsed)
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
                Kind::TypeValue(type_name) => {
                    // Type values are represented as {"type_value": "type_name"}
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "type_value".to_string(),
                        JsonValue::String(type_name.clone()),
                    );
                    Ok(JsonValue::Object(obj))
                }
                Kind::ObjectValue(any) => {
                    // Decode a prost_types::Any into a JSON object using the established
                    // __type__-based convention used throughout the codebase for proto objects.
                    use prost::Message as _;
                    const DURATION_URL: &str = "type.googleapis.com/google.protobuf.Duration";
                    if any.type_url == DURATION_URL {
                        let dur = prost_types::Duration::decode(any.value.as_slice())
                            .map_err(|e| format!("Failed to decode Duration: {}", e))?;
                        let mut obj = serde_json::Map::new();
                        obj.insert(
                            "__type__".to_string(),
                            JsonValue::String("google.protobuf.Duration".to_string()),
                        );
                        obj.insert("seconds".to_string(), JsonValue::Number(dur.seconds.into()));
                        obj.insert("nanos".to_string(), JsonValue::Number(dur.nanos.into()));
                        Ok(JsonValue::Object(obj))
                    } else {
                        Err(format!("Unsupported object type URL: {}", any.type_url))
                    }
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
                if e.as_f64().is_some_and(|f| f.is_nan()) && a.as_f64().is_some_and(|f| f.is_nan())
                {
                    return true;
                }

                // For floating point numbers, use epsilon comparison
                // to handle rounding differences in representation
                if let (Some(ef), Some(af)) = (e.as_f64(), a.as_f64()) {
                    // Use relative epsilon for very small numbers
                    let epsilon = if ef.abs() < 1e-10 || af.abs() < 1e-10 {
                        // For very small numbers, use a small absolute epsilon
                        1e-30
                    } else {
                        // For larger numbers, use relative epsilon
                        ef.abs() * 1e-10
                    };
                    (ef - af).abs() <= epsilon
                } else {
                    // If not floats (i.e., integers), compare exactly
                    e == a
                }
            }
            (JsonValue::Object(e), JsonValue::Object(a)) => {
                // Maps are order-agnostic
                if e.len() != a.len() {
                    return false;
                }
                e.iter()
                    .all(|(k, v)| a.get(k).is_some_and(|av| self.values_equal(v, av)))
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
