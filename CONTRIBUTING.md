# Running Conformance Tests

```bash
# Run all conformance tests
make conformance-tests

# List available test suites
make conformance-list
```

## Conformance Test Discovery and Filtering

To run specific conformance tests or explore what's available, follow this discovery workflow:

**1. List available test suites:**

```bash
make conformance-list
```

**2. List sections in a suite:**

```bash
make conformance-sections-basic
```

**3. List tests in a specific section:**

```bash
CONFORMANCE_SECTION="self_eval_zeroish" make conformance-sections-basic
```

**4. Run all tests in a specific section:**

```bash
CONFORMANCE_SECTION="self_eval_zeroish" make conformance-basic
```

**5. Run a single specific test:**

```bash
CONFORMANCE_SECTION="self_eval_zeroish" CONFORMANCE_TEST="self_eval_int_zero" make conformance-basic
```

**Examples:**

```bash
# List sections in the string test suite
make conformance-sections-string

# See all tests in the "string_ops" section
CONFORMANCE_SECTION="string_ops" make conformance-sections-string

# Run only the "string_ops" section
CONFORMANCE_SECTION="string_ops" make conformance-string

# Run one specific test
CONFORMANCE_SECTION="string_ops" CONFORMANCE_TEST="size" make conformance-string
```

This filtering works with any conformance test suite (basic, string, lists, logic, etc.).

## Publishing to crates.io

The workspace contains two publishable crates: `ferricel-core` and `ferricel`. Because `ferricel` depends on `ferricel-core`, they must be published in order.

`ferricel-core` embeds the `runtime` WASM at compile time. When building inside the workspace, the build script finds the WASM automatically in `target/`. When building from the published crate (outside the workspace), the WASM must be bundled inside the crate package. The `publish-prep` Makefile target handles this.

**Steps:**

**1. Build the runtime and run all tests:**

```bash
make tests
```

**2. Prepare `ferricel-core` for publishing:**

```bash
make publish-prep
```

This copies `target/wasm32-unknown-unknown/release/runtime.wasm` into `ferricel-core/runtime.wasm` so it is included in the crate package. The file is gitignored and only needed at publish time.

**3. Publish `ferricel-core`:**

```bash
cargo publish -p ferricel-core
```

**4. Publish `ferricel`:**

Once `ferricel-core` is available on crates.io (may take a minute to index), update the `ferricel-core` dependency in `ferricel/Cargo.toml` from a path dependency to a version dependency, then publish:

```bash
cargo publish -p ferricel
```

**5. Clean up:**

```bash
rm ferricel-core/runtime.wasm
```
