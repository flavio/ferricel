.PHONY: all clean runtime ferricel help unit-tests e2e-tests tests conformance-tests conformance-list conformance-%

# Default target
all: ferricel

# Variables
RUNTIME_TARGET := target/wasm32-unknown-unknown/release/runtime.wasm
RUNTIME_SOURCES := $(shell find runtime/src -type f -name '*.rs' 2>/dev/null)
RUNTIME_CARGO := runtime/Cargo.toml

FERRICEL_SOURCES := $(shell find ferricel/src -type f -name '*.rs' 2>/dev/null)
FERRICEL_CARGO := ferricel/Cargo.toml

WORKSPACE_CARGO := Cargo.toml Cargo.lock

# Build the runtime WASM module
runtime: $(RUNTIME_TARGET)

$(RUNTIME_TARGET): $(RUNTIME_SOURCES) $(RUNTIME_CARGO) $(WORKSPACE_CARGO)
	@echo "Building runtime for wasm32-unknown-unknown..."
	cargo build -p runtime --target wasm32-unknown-unknown --release

# Build and run ferricel (depends on runtime)
# NOTE: Runtime WASM is embedded at compile-time using include_bytes!
ferricel: $(RUNTIME_TARGET) $(FERRICEL_SOURCES) $(FERRICEL_CARGO) $(WORKSPACE_CARGO)
	@echo "Building ferricel..."
	cargo build -p ferricel --release

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Run only unit tests (compiler and runtime unit tests)
unit-tests: $(RUNTIME_TARGET)
	@echo "Running unit tests..."
	cargo test --workspace --exclude conformance --lib --bins

# Run only end-to-end tests (CLI integration tests)
e2e-tests: $(RUNTIME_TARGET)
	@echo "Running end-to-end CLI tests..."
	cargo test --package ferricel --test e2e

# Run all tests (unit + e2e + conformance)
tests: unit-tests e2e-tests conformance-tests

# Run CEL conformance tests (separate crate)
conformance-tests: $(RUNTIME_TARGET)
	@echo "Running CEL conformance tests..."
	@echo "Note: Building ferricel-core first..."
	cargo build --package ferricel-core
	cargo test --package conformance --test conformance -- --nocapture

# Run a specific conformance test suite
# Usage: make conformance-basic, make conformance-string, etc.
conformance-%: $(RUNTIME_TARGET)
	@cargo build --package ferricel-core
	@case "$*" in \
		basic) \
			cargo test --package conformance --test conformance conformance_basic_tests -- --nocapture ;; \
		comparisons) \
			cargo test --package conformance --test conformance conformance_comparisons_tests -- --nocapture ;; \
		conversions) \
			cargo test --package conformance --test conformance conformance_conversions_tests -- --nocapture ;; \
		fp-math) \
			cargo test --package conformance --test conformance conformance_fp_math_tests -- --nocapture ;; \
		int-math) \
			cargo test --package conformance --test conformance conformance_integer_math_tests -- --nocapture ;; \
		lists) \
			cargo test --package conformance --test conformance conformance_lists_tests -- --nocapture ;; \
		logic) \
			cargo test --package conformance --test conformance conformance_logic_tests -- --nocapture ;; \
		string) \
			cargo test --package conformance --test conformance conformance_string_tests -- --nocapture ;; \
		timestamps) \
			cargo test --package conformance --test conformance conformance_timestamps_tests -- --nocapture ;; \
		all) \
			$(MAKE) conformance-tests ;; \
		list) \
			$(MAKE) conformance-list ;; \
		*) \
			echo "Error: Unknown conformance test suite '$*'"; \
			echo ""; \
			echo "Available conformance test suites:"; \
			echo "  conformance-basic       - Basic CEL features (literals, operators, variables)"; \
			echo "  conformance-comparisons - Comparison operators (==, !=, <, >, <=, >=)"; \
			echo "  conformance-conversions - Type conversions (int(), uint(), double(), etc.)"; \
			echo "  conformance-fp-math     - Floating point math operations"; \
			echo "  conformance-int-math    - Integer math operations"; \
			echo "  conformance-lists       - List operations (indexing, size, in, etc.)"; \
			echo "  conformance-logic       - Logical operators (&&, ||, !, ? :)"; \
			echo "  conformance-string      - String operations (size, contains, matches, etc.)"; \
			echo "  conformance-timestamps  - Timestamp and duration operations"; \
			echo "  conformance-all         - Run all conformance tests"; \
			echo "  conformance-list        - Show this list"; \
			echo ""; \
			echo "Examples:"; \
			echo "  make conformance-basic"; \
			echo "  make conformance-string"; \
			echo "  make conformance-all"; \
			exit 1 ;; \
	esac

# List available conformance test suites
conformance-list:
	@echo "Available conformance test suites:"
	@echo ""
	@echo "  conformance-basic       - Basic CEL features (literals, operators, variables)"
	@echo "  conformance-comparisons - Comparison operators (==, !=, <, >, <=, >=)"
	@echo "  conformance-conversions - Type conversions (int(), uint(), double(), etc.)"
	@echo "  conformance-fp-math     - Floating point math operations"
	@echo "  conformance-int-math    - Integer math operations"
	@echo "  conformance-lists       - List operations (indexing, size, in, etc.)"
	@echo "  conformance-logic       - Logical operators (&&, ||, !, ? :)"
	@echo "  conformance-string      - String operations (size, contains, matches, etc.)"
	@echo "  conformance-timestamps  - Timestamp and duration operations"
	@echo "  conformance-all         - Run all conformance tests"
	@echo ""
	@echo "Usage:"
	@echo "  make conformance-basic    # Run basic tests"
	@echo "  make conformance-string   # Run string tests"
	@echo "  make conformance-all      # Run all tests"

# Check code formatting (does not modify files)
.PHONY: fmt
fmt:
	cargo fmt --all

# Run clippy lints with warnings treated as errors
.PHONY: lint
lint:
	cargo fmt --all -- --check
	cargo clippy --workspace -- -D warnings

# Auto-fix clippy warnings where possible
.PHONY: lint-fix
lint-fix:
	cargo clippy --workspace --fix --allow-dirty --allow-staged

# Check that the code compiles without building artifacts
.PHONY: check
check:
	cargo check --workspace

# Help target
help:
	@echo "Available targets:"
	@echo "  all              - Build ferricel and runtime (default)"
	@echo "  runtime          - Build only the runtime WASM module"
	@echo "  ferricel         - Build runtime and ferricel binary (runtime is embedded at compile-time)"
	@echo "  clean            - Remove all build artifacts"
	@echo "  unit-tests       - Run unit tests (ferricel-core, runtime)"
	@echo "  e2e-tests        - Run CLI integration tests"
	@echo "  tests            - Run all tests (unit + e2e + conformance)"
	@echo "  conformance-tests - Run all CEL conformance tests"
	@echo "  conformance-<name> - Run specific conformance test suite"
	@echo "  conformance-list - List available conformance test suites"
	@echo "  fmt              - Check code formatting (does not modify files)"
	@echo "  lint             - Run clippy lints with warnings as errors"
	@echo "  lint-fix         - Auto-fix clippy warnings where possible"
	@echo "  check            - Check that code compiles without building"
	@echo "  help             - Show this help message"
	@echo ""
	@echo "Note: The runtime WASM must be built before ferricel, as it's embedded using include_bytes!"
	@echo ""
	@echo "Workspace structure:"
	@echo "  ferricel-core    - Core compiler and runtime library"
	@echo "  ferricel         - CLI binary (thin wrapper)"
	@echo "  conformance      - CEL conformance tests"
	@echo "  runtime          - WASM guest runtime"
	@echo "  ferricel-types   - Shared types"
	@echo ""
	@echo "Usage examples:"
	@echo "  make ferricel"
	@echo "  make unit-tests"
	@echo "  make e2e-tests"
	@echo "  make conformance-tests"
	@echo "  make conformance-basic"
	@echo "  make conformance-list"
	@echo "  cargo run -p ferricel -- build --expression '10 + 20'"
	@echo "  cargo run -p ferricel -- build -e '5 + 15' -o output.wasm"
	@echo "  cargo run -p ferricel -- run output.wasm"
	@echo "  cargo run -p ferricel -- run output.wasm --input-json '{\"age\": 25}'"
	@echo ""
	@echo "Workflow example:"
	@echo "  cargo run -p ferricel -- build -e '10 + 20' && cargo run -p ferricel -- run final_cel_program.wasm"
