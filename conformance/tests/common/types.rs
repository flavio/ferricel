// Shared data types for the conformance test runner.

use std::sync::atomic::{AtomicUsize, Ordering};

// Test result for reporting
#[derive(Debug, Clone, PartialEq)]
pub enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
}

// Statistics for test execution
#[derive(Debug, Default)]
pub struct TestStats {
    pub passed: AtomicUsize,
    pub failed: AtomicUsize,
    pub skipped: AtomicUsize,
    pub total: AtomicUsize,
}

impl TestStats {
    pub fn record(&self, result: &TestResult) {
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

    pub fn print_summary(&self, test_file: &str, filtered: bool) {
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
pub struct SkipList {
    rules: Vec<SkipRule>,
}

#[derive(Debug)]
pub struct SkipRule {
    pub file: Option<String>,
    pub section: Option<String>,
    pub test: Option<String>,
    pub reason: String,
}

impl SkipList {
    pub fn new() -> Self {
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

    pub fn should_skip(&self, file: &str, section: &str, test: &str) -> Option<String> {
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
