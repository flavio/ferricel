.PHONY: all clean runtime ferricel help unit-tests e2e-tests tests

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

# Run only unit tests (tests within src/)
unit-tests:
	@echo "Running unit tests..."
	cargo test --package runtime --lib
	cargo test --package ferricel --bins

# Run only end-to-end tests (tests/ directory)
e2e-tests: $(RUNTIME_TARGET)
	@echo "Running end-to-end tests..."
	cargo test --package ferricel --test e2e

# Run all tests (unit + e2e)
tests: $(RUNTIME_TARGET)
	@echo "Running all tests..."
	cargo test --package runtime
	cargo test --package ferricel

# Help target
help:
	@echo "Available targets:"
	@echo "  all        - Build ferricel and runtime (default)"
	@echo "  runtime    - Build only the runtime WASM module"
	@echo "  ferricel   - Build runtime and ferricel binary (runtime is embedded at compile-time)"
	@echo "  clean      - Remove all build artifacts"
	@echo "  unit-tests - Run only unit tests (tests within src/)"
	@echo "  e2e-tests  - Run only end-to-end CLI tests (tests/ directory)"
	@echo "  tests      - Run all tests (unit + e2e)"
	@echo "  help       - Show this help message"
	@echo ""
	@echo "Note: The runtime WASM must be built before ferricel, as it's embedded using include_bytes!"
	@echo ""
	@echo "Usage examples:"
	@echo "  make ferricel"
	@echo "  make tests"
	@echo "  make e2e-tests"
	@echo "  cargo run -p ferricel -- build --expression '10 + 20'"
	@echo "  cargo run -p ferricel -- build -e '5 + 15' -o output.wasm"
	@echo "  cargo run -p ferricel -- run output.wasm"
	@echo "  cargo run -p ferricel -- run output.wasm --input-json '{\"age\": 25}'"
	@echo ""
	@echo "Workflow example:"
	@echo "  cargo run -p ferricel -- build -e '10 + 20' && cargo run -p ferricel -- run final_cel_program.wasm"
