# Conformance Tests

Official CEL (Common Expression Language) conformance tests for ferricel.

## Overview

This crate runs the official conformance tests from [google/cel-spec](https://github.com/google/cel-spec) to validate that ferricel correctly implements the CEL specification.

## Structure

- **`tests/conformance.rs`**: Test runner that loads and executes conformance test files
- **`build.rs`**: Compiles protobuf definitions from cel-spec at build time
- **Test data**: Located in `../cel-spec/` git submodule

## Running Tests

```bash
# From workspace root
make conformance-tests

# Or directly with cargo
cargo test --package conformance --test conformance -- --nocapture
```

## Test Coverage

Currently runs tests from:
- `basic.textproto`: Fundamental CEL features (literals, operators, variables, functions)

### Test Results

Tests are categorized as:
- **PASSED**: Test executed successfully and produced expected result
- **FAILED**: Test failed - indicates a bug or missing feature
- **SKIPPED**: Test requires unimplemented features (automatically detected)

The conformance tests do not fail the build. They provide visibility into CEL specification compliance.

### Skip List

Tests are automatically skipped when they require:
- Type environments (not yet supported)
- Protocol buffer types (not yet supported)
- Certain unbound variable patterns

## Implementation Details

The test runner:
1. Parses protobuf test definitions from cel-spec
2. Compiles each CEL expression to WASM in-memory using `ferricel-core`
3. Executes the WASM module with test bindings
4. Compares the result against expected values
5. Reports statistics (PASSED/FAILED/SKIPPED)

This approach uses in-memory compilation and execution, avoiding filesystem I/O for better performance.

## Current Status

As of the last run:
- **Pass rate**: ~74%
- **Known issues**:
  - Null literal support
  - Negative hex literal parsing
  - Some string escape sequences
  - Unbound variable error messages

Track pass rate over time to measure progress toward full CEL spec compliance.

## Dependencies

The protobuf compilation only happens when building this crate, keeping the main `ferricel` binary build fast and lean.

Build dependencies:
- `prost-build`: Compiles .proto files to Rust code
- `prost`, `prost-types`: Runtime protobuf support

## Contributing

When adding new CEL features:
1. Run conformance tests to see which tests now pass
2. Update skip list if new feature categories are supported
3. Track the pass rate improvement

## License

See the root workspace LICENSE file.
