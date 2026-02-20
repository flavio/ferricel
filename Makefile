.PHONY: all clean runtime ferricel help

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

# Help target
help:
	@echo "Available targets:"
	@echo "  all      - Build ferricel and runtime (default)"
	@echo "  runtime  - Build only the runtime WASM module"
	@echo "  ferricel - Build runtime and ferricel binary (runtime is embedded at compile-time)"
	@echo "  clean    - Remove all build artifacts"
	@echo "  help     - Show this help message"
	@echo ""
	@echo "Note: The runtime WASM must be built before ferricel, as it's embedded using include_bytes!"
	@echo ""
	@echo "Usage examples:"
	@echo "  make ferricel"
	@echo "  cargo run -p ferricel -- build --expression '10 + 20'"
	@echo "  cargo run -p ferricel -- build -e '5 + 15' -o output.wasm"
	@echo "  cargo run -p ferricel -- run output.wasm"
	@echo ""
	@echo "Workflow example:"
	@echo "  cargo run -p ferricel -- build -e '10 + 20' && cargo run -p ferricel -- run final_cel_program.wasm"
