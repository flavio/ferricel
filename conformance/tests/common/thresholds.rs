// Per-suite conformance test failure thresholds.
//
// Thresholds are loaded from `conformance/thresholds.toml` at test runtime.
// Suites not listed in the file default to 0 (must be 100% passing).

use std::collections::HashMap;

const THRESHOLDS_TOML: &str = include_str!("../../thresholds.toml");

pub struct Thresholds {
    suites: HashMap<String, usize>,
}

impl Thresholds {
    pub fn load() -> Self {
        let table: toml::Table = THRESHOLDS_TOML
            .parse()
            .expect("Failed to parse thresholds.toml");

        let mut suites = HashMap::new();

        if let Some(toml::Value::Table(suite_table)) = table.get("suites") {
            for (name, value) in suite_table {
                let count = value
                    .as_integer()
                    .expect("Threshold values must be integers")
                    as usize;
                suites.insert(name.clone(), count);
            }
        }

        Self { suites }
    }

    /// Returns the maximum number of allowed failures for the given suite name.
    /// Suites not listed in `thresholds.toml` default to 0.
    pub fn max_failures(&self, suite: &str) -> usize {
        self.suites.get(suite).copied().unwrap_or(0)
    }
}
