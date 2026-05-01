// Main test executor for the CEL conformance suite.

use std::{collections::HashMap, path::Path, sync::Mutex};

use rayon::prelude::*;
use serde_json::Value as JsonValue;
use slog::{Drain, Logger, o};

use ferricel_core::compiler::Builder as CompilerBuilder;
use ferricel_core::runtime::CelEngine;

use ferricel_types::proto::Bindings as FerricelBindings;
use ferricel_types::proto::cel::expr::Value as FerricelValue;
use ferricel_types::proto::cel::expr::map_value::Entry as FerricelEntry;
use ferricel_types::proto::cel::expr::value::Kind as FerricelKind;
use ferricel_types::proto::cel::expr::{
    ListValue as FerricelListValue, MapValue as FerricelMapValue,
};
use prost::Message;

use super::proto_gen::{ExprValue, SimpleTest, SimpleTestFile, Value};
use super::types::{SkipList, TestResult, TestStats};

pub struct ConformanceTestRunner {
    pub skip_list: SkipList,
    pub logger: Logger,
    pub proto_descriptor: Option<Vec<u8>>,
}

impl ConformanceTestRunner {
    pub fn new() -> Self {
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

    pub fn list_sections(&self, test_file_path: &Path) {
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

    pub fn list_tests(&self, test_file_path: &Path, section_name: &str) {
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

    pub fn run_test_file(&self, test_file_path: &Path) -> TestStats {
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

            // Collect results for this section with parallel execution.
            // Use a custom thread pool with an 8 MB stack to avoid overflows in
            // the recursive compiler (compile_expr) on Rayon worker threads,
            // which default to only 2 MB on Linux.
            let pool = rayon::ThreadPoolBuilder::new()
                .stack_size(8 * 1024 * 1024)
                .build()
                .expect("Failed to build Rayon thread pool");
            let section_results: Vec<_> = pool.install(|| {
                tests_to_run
                    .par_iter()
                    .map(|test| {
                        let result = self.run_single_test(file_name, &section.name, test);
                        stats.record(&result);
                        (test.name.clone(), result)
                    })
                    .collect()
            });

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
            && stats.total.load(std::sync::atomic::Ordering::SeqCst) == 0
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

    pub fn run_single_test(&self, file: &str, section: &str, test: &SimpleTest) -> TestResult {
        // Check skip list
        if let Some(reason) = self.skip_list.should_skip(file, section, &test.name) {
            return TestResult::Skipped(reason);
        }

        // Skip tests whose bindings contain unsupported proto object types
        const SUPPORTED_OBJECT_URLS: &[&str] = &[
            "type.googleapis.com/google.protobuf.Duration",
            "type.googleapis.com/google.protobuf.Timestamp",
        ];
        for expr_value in test.bindings.values() {
            if let Some(super::proto_gen::cel::expr::expr_value::Kind::Value(v)) = &expr_value.kind
            {
                if let Some(super::proto_gen::cel::expr::value::Kind::ObjectValue(any)) = &v.kind {
                    if !SUPPORTED_OBJECT_URLS.contains(&any.type_url.as_str()) {
                        return TestResult::Skipped(format!(
                            "Binding uses unsupported proto type: {}",
                            any.type_url
                        ));
                    }
                }
            }
        }

        // Skip check_only tests — they require a type checker we don't implement
        if test.check_only {
            return TestResult::Skipped("check_only test (requires type checker)".to_string());
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

    pub fn execute_cel_expression(&self, test: &SimpleTest) -> Result<JsonValue, String> {
        // Step 1: Compile the CEL expression to WASM (in memory)
        let mut builder = CompilerBuilder::new().with_logger(self.logger.clone());
        if let Some(descriptor) = self.proto_descriptor.clone() {
            builder = builder.with_proto_descriptor(descriptor).map_err(|e| format!("Build failed: {}", e))?;
        }
        if !test.container.is_empty() {
            builder = builder.with_container(test.container.clone());
        }
        let compiler = builder.build();
        let wasm_bytes = match compiler.compile(&test.expr) {
            Ok(bytes) => bytes,
            Err(e) => {
                // Check if this test expects an error (eval_error or any error)
                use super::proto_gen::cel::expr::conformance::test::simple_test::ResultMatcher;
                if matches!(test.result_matcher, Some(ResultMatcher::EvalError(_))) {
                    // Build error counts as eval error for tests with disable_check
                    // Return as error marker (to match expected error format)
                    return Ok(JsonValue::String(format!("error: {}", e)));
                }
                return Err(format!("Build failed: {}", e));
            }
        };

        // Step 2: Encode bindings as protobuf (preserves full type fidelity)
        let bindings_proto = if !test.bindings.is_empty() {
            self.convert_bindings_to_proto(&test.bindings)?
        } else {
            // Empty Bindings message
            FerricelBindings::default().encode_to_vec()
        };

        // Step 3: Execute the WASM module using the proto bindings path
        let json_result = match CelEngine::new(self.logger.clone())
            .execute_proto(&wasm_bytes, &bindings_proto)
        {
            Ok(result) => result,
            Err(e) => {
                // Check if this is an expected error
                if test.result_matcher.is_some() {
                    use super::proto_gen::cel::expr::conformance::test::simple_test::ResultMatcher;
                    if matches!(test.result_matcher, Some(ResultMatcher::EvalError(_))) {
                        // This is expected to fail, return error marker as a JSON string
                        return Ok(JsonValue::String(format!("error: {}", e)));
                    }
                }
                return Err(format!("Run failed: {}", e));
            }
        };

        // Step 4: Parse the JSON result
        serde_json::from_str(&json_result)
            .map_err(|e| format!("Failed to parse output '{}': {}", json_result, e))
    }

    /// Convert conformance-test bindings to a protobuf-encoded `ferricel.Bindings` message.
    pub fn convert_bindings_to_proto(
        &self,
        bindings: &HashMap<String, ExprValue>,
    ) -> Result<Vec<u8>, String> {
        let mut variables = std::collections::HashMap::new();

        for (key, expr_value) in bindings {
            if let Some(kind) = &expr_value.kind {
                use super::proto_gen::cel::expr::expr_value::Kind;
                let value = match kind {
                    Kind::Value(v) => self.conformance_value_to_ferricel(v)?,
                    Kind::Error(_) => return Err("Cannot bind an error value".to_string()),
                    Kind::Unknown(_) => return Err("Cannot bind an unknown value".to_string()),
                };
                variables.insert(key.clone(), value);
            }
        }

        let bindings_msg = FerricelBindings { variables };
        Ok(bindings_msg.encode_to_vec())
    }

    /// Convert a conformance `cel.expr.Value` into a `ferricel_types` `cel.expr.Value`.
    /// Both are generated from the same proto schema, so the conversion is 1:1.
    pub fn conformance_value_to_ferricel(&self, v: &Value) -> Result<FerricelValue, String> {
        use super::proto_gen::cel::expr::value::Kind as ConformanceKind;

        let kind = match &v.kind {
            None => return Ok(FerricelValue { kind: None }),
            Some(ConformanceKind::NullValue(n)) => FerricelKind::NullValue(*n),
            Some(ConformanceKind::BoolValue(b)) => FerricelKind::BoolValue(*b),
            Some(ConformanceKind::Int64Value(i)) => FerricelKind::Int64Value(*i),
            Some(ConformanceKind::Uint64Value(u)) => FerricelKind::Uint64Value(*u),
            Some(ConformanceKind::DoubleValue(d)) => FerricelKind::DoubleValue(*d),
            Some(ConformanceKind::StringValue(s)) => FerricelKind::StringValue(s.clone()),
            Some(ConformanceKind::BytesValue(b)) => FerricelKind::BytesValue(b.clone()),
            Some(ConformanceKind::TypeValue(t)) => FerricelKind::TypeValue(t.clone()),
            Some(ConformanceKind::ListValue(list)) => {
                let values: Result<Vec<_>, _> = list
                    .values
                    .iter()
                    .map(|v| self.conformance_value_to_ferricel(v))
                    .collect();
                FerricelKind::ListValue(FerricelListValue { values: values? })
            }
            Some(ConformanceKind::MapValue(map)) => {
                let entries: Result<Vec<FerricelEntry>, String> = map
                    .entries
                    .iter()
                    .map(|e| -> Result<FerricelEntry, String> {
                        let key = e
                            .key
                            .as_ref()
                            .map(|k| self.conformance_value_to_ferricel(k))
                            .transpose()?;
                        let value = e
                            .value
                            .as_ref()
                            .map(|v| self.conformance_value_to_ferricel(v))
                            .transpose()?;
                        Ok(FerricelEntry { key, value })
                    })
                    .collect();
                FerricelKind::MapValue(FerricelMapValue { entries: entries? })
            }
            Some(ConformanceKind::ObjectValue(any)) => {
                // google.protobuf.Any — convert to FerricelKind::ObjectValue
                // We re-use the prost_types::Any bytes directly since both sides
                // understand the same wire format.
                use prost_types::Any;
                FerricelKind::ObjectValue(Any {
                    type_url: any.type_url.clone(),
                    value: any.value.clone(),
                })
            }
            Some(ConformanceKind::EnumValue(e)) => {
                use ferricel_types::proto::cel::expr::EnumValue as FerricelEnum;
                FerricelKind::EnumValue(FerricelEnum {
                    r#type: e.r#type.clone(),
                    value: e.value,
                })
            }
        };

        Ok(FerricelValue { kind: Some(kind) })
    }

    pub fn convert_bindings_to_json(
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

    pub fn expr_value_to_json(
        &self,
        kind: &super::proto_gen::cel::expr::expr_value::Kind,
    ) -> Result<JsonValue, String> {
        use super::proto_gen::cel::expr::expr_value::Kind;
        match kind {
            Kind::Value(v) => self.cel_value_to_json(v),
            Kind::Error(_) => Err("Cannot convert error to JSON".to_string()),
            Kind::Unknown(_) => Err("Cannot convert unknown to JSON".to_string()),
        }
    }

    pub fn cel_value_to_json(&self, value: &Value) -> Result<JsonValue, String> {
        if let Some(kind) = &value.kind {
            use super::proto_gen::cel::expr::value::Kind;
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

    pub fn cel_value_to_string(&self, value: &Value) -> Result<String, String> {
        if let Some(kind) = &value.kind {
            use super::proto_gen::cel::expr::value::Kind;
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

    pub fn compare_result(&self, test: &SimpleTest, actual: JsonValue) -> TestResult {
        use super::proto_gen::cel::expr::conformance::test::simple_test::ResultMatcher;

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
                Err(e) if e.starts_with("Unsupported object type URL:") => TestResult::Skipped(
                    format!("Expected value uses unsupported proto type: {}", e),
                ),
                Err(e) => TestResult::Failed(format!("Failed to convert expected value: {}", e)),
            },
            Some(ResultMatcher::TypedResult(typed)) => {
                // We ignore the deduced_type (no type checker) and only validate the result value.
                match &typed.result {
                    None => TestResult::Skipped("TypedResult has no result value".to_string()),
                    Some(expected_val) => match self.cel_value_to_json(expected_val) {
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
                        Err(e) if e.starts_with("Unsupported object type URL:") => {
                            TestResult::Skipped(format!(
                                "Expected value uses unsupported proto type: {}",
                                e
                            ))
                        }
                        Err(e) => {
                            TestResult::Failed(format!("Failed to convert expected value: {}", e))
                        }
                    },
                }
            }
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

    pub fn values_equal(&self, expected: &JsonValue, actual: &JsonValue) -> bool {
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

    pub fn load_test_file(&self, path: &Path) -> SimpleTestFile {
        // Convert textproto to binary using protoc
        let proto_dir = Path::new("../cel-spec/proto");

        let output = std::process::Command::new("protoc")
            .arg(format!("--proto_path={}", proto_dir.display()))
            .arg("--encode=cel.expr.conformance.test.SimpleTestFile")
            .arg("cel/expr/conformance/test/simple.proto")
            .arg("cel/expr/conformance/proto2/test_all_types.proto")
            .arg("cel/expr/conformance/proto3/test_all_types.proto")
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
