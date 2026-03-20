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
