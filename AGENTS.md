# Agent Development Guide

A file for [guiding coding agents](https://agents.md/).

## Commands

- **Build:** `make ferricel`
- **Test (unit):** `make unit-tests`
- **Test (integration):** `make integration-tests`
- **Test (end to end):** `make e2e-tests`
- **Test (conformance):** `make conformance-tests` (runs all conformance tests)
- **Test (conformance - specific):** `make conformance-<name>` (e.g., `make conformance-basic`, `make conformance-string`)
- **Test (conformance - list):** `make conformance-list` (lists available conformance test suites)
- **Test (conformance - list sections):** `make conformance-sections-<name>` (e.g., `make conformance-sections-basic`)
- **Test (conformance - list tests in section):** `CONFORMANCE_SECTION=<section> make conformance-sections-<name>`
- **Test (conformance - run specific section):** `CONFORMANCE_SECTION=<section> make conformance-<name>`
- **Test (conformance - run specific test):** `CONFORMANCE_SECTION=<section> CONFORMANCE_TEST=<test> make conformance-<name>`
- **Test (all):** `make tests`
- **Formatting**: `make fmt`
- **Linting**: `make lint`
- **Linting (fix some issues automatically)**: `make lint-fix`
- **Docs (all)**: `make docs` (builds both Rust API docs and the mdbook user guide)
- **Docs (Rust API)**: `make docs-api` (requires runtime to be built first)
- **Docs (user guide)**: `make docs-book` (requires `mdbook` to be installed; output in `docs/book/`)

## Workspace Structure

The project is organized as a Cargo workspace with the following crates:

### Core Libraries

- **ferricel-core**: Core compiler and runtime library. This is the reusable library that can be used by other projects
- **ferricel-types**: Shared type definitions used by both Wasm guest and host
- **runtime**: Wasm guest runtime functions
  - Compiled to `wasm32-unknown-unknown`
  - Embedded into each generated Wasm module
  - Provides runtime functions that CEL programs call during execution
  - Self-contained - each Wasm file is standalone

### CLI Application

- **ferricel**: Thin CLI wrapper around ferricel-core
  - `build` subcommand: Reads CEL expressions, compiles to Wasm files
  - `run` subcommand: Loads and executes Wasm modules, prints results
  - Tests at `tests/e2e.rs` validate CLI behavior

### Testing

- **conformance**: Official CEL conformance tests
  - Tests compliance with the CEL specification from google/cel-spec
  - Uses protobuf to parse test definitions
  - Build script (`build.rs`) compiles proto files (only when running conformance tests)
  - Run with `make conformance-tests`

## Testing Strategy

The project has three levels of testing:

### 1. Unit Tests (`make unit-tests`)

- Located in `ferricel-core/tests/compiler_tests.rs`
- Test individual compiler features (operators, types, conversions, etc.)

### 2. End-to-End Tests (`make e2e-tests`)

- Located in `ferricel/tests/e2e.rs`
- Test the CLI interface and integration between components
- Expensive to run - only add when CLI behavior changes
- Verify full workflow: build CEL → Wasm file → run Wasm → check output

### 3. Conformance Tests

- Located in `conformance/tests/conformance.rs`
- Validate compliance with the official CEL specification
- Test data from `cel-spec/` git submodule (google/cel-spec repository)
- Tests requiring unimplemented features are automatically skipped
- Results show PASSED/FAILED counts (doesn't fail the build)
- Track pass rate over time to measure specification compliance

## Development Principles

1. **CEL Specification Compliance**: The official CEL specification is the north star. All changes must respect the spec.

2. **Test Requirements**: After changes:
   - Unit tests must pass (`make unit-tests`)
   - Linter must pass (`make lint`)
   - Consider running conformance tests to check spec compliance

3. **Runtime Stability**: The `runtime` Wasm module is embedded in each output file. The internal API can change freely since there are no backward compatibility concerns. Each Wasm file is self-contained.

4. **Code Organization**:
   - Core logic belongs in `ferricel-core`
   - CLI-specific code belongs in `ferricel`
   - Tests should be close to the code they test
   - Conformance infrastructure is isolated in `conformance`

5. **API Design**: `ferricel-core` provides both:
   - Granular access via `compiler` and `runtime` modules
   - Convenient high-level functions like `evaluate_cel()`
   - This allows flexibility for different use cases
