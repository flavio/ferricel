# Limitations

Known limitations and upstream bugs that affect ferricel's conformance with the CEL specification.

---

## Upstream `cel` crate (v0.13) parser bugs

These issues are in the [`cel`](https://crates.io/crates/cel) crate's parser and cannot be fixed in ferricel directly. They need to be reported/patched upstream.

### 1. Even-count unary `!` collapses incorrectly

**File:** `src/parser/parser.rs`, `visit_LogicalNot` (~line 692)

**Affected conformance tests:** `parse` suite — `not`

`!!true` (or any even number of `!`) produces `_!_(true)` instead of evaluating to `true`. The parser holds all `!` operators in a flat `ctx.ops` list and tries to optimise even counts, but discards the visit result and then wraps in a single `_!_` anyway:

```rust
if ctx.ops.len() % 2 == 0 {
    self.visit(member.as_ref()); // result discarded
}
let target = self.visit(member.as_ref());
self.global_call_or_macro(op_id, LOGICAL_NOT, vec![target]) // always wraps in one !
```

Expected: `!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!true` (32 `!`) → `true`  
Got: `false` (single negation applied)

### 2. Even-count unary `-` collapses incorrectly

**File:** `src/parser/parser.rs`, `visit_Negate` (~line 708)

**Affected conformance tests:** `parse` suite — `unary_neg`

Same bug as above but for `-`. `--19` (or any even count of `-`) produces `_-_(19)` instead of `19`.

Expected: `--------------------------------19` (32 `-`) → `19`  
Got: `-19` (single negation applied)

### 3. Triple-quoted bytes literals include delimiters in value

**File:** `src/parser/` (bytes literal parsing)

**Affected conformance tests:** `parse` suite — `bytes_literals` section (triple-quoted variants)

`b'''hello'''` produces the bytes `''hello''` (the inner quotes are included) instead of `hello`.

### 4. Bytes literals reject common escape sequences

**File:** `src/parser/parse.rs`, `parse_bytes`

**Affected conformance tests:** `parse` suite — `bytes_literals` section

Bytes literals do not accept `\\`, `\n`, `\r`, `\a`, `\b`, `\f`, `\t`, `\v` escape sequences, returning `InvalidEscape` errors. These are all valid per the CEL spec.

### 5. `\X` (uppercase hex) not recognised in strings or bytes

**File:** `src/parser/parse.rs`

**Affected conformance tests:** `parse` suite — `string_literals` and `bytes_literals` sections (`*_upper_x_escapes` tests)

`\X00` etc. should be accepted as a hex escape (same as `\x00`) in both string and bytes literals, but the parser rejects it with `InvalidEscape`.

### 6. Cross-quote escaping in strings

**File:** `src/parser/parse.rs`, `parse_string`

**Affected conformance tests:** `parse` suite — `string_literals` section (`*_escaped_punctuation` tests)

- In single-quoted strings, `\"` should unescape to `"` but is kept as `\"`
- In double-quoted strings, `\'` should unescape to `'` but is kept as `\'`

### 7. Raw strings incorrectly unescape quote characters

**File:** `src/parser/parse.rs`, `parse_raw_string`

**Affected conformance tests:** `parse` suite — `string_literals` section (`raw_triple_*_escapes` tests)

In raw strings, `\'` and `\"` should remain as the two-character sequences `\'` and `\"` (no escape processing). The parser currently unescapes them to `'` and `"` respectively.

### 8. Triple-quoted strings include delimiters in value (unescaped punctuation tests)

**File:** `src/parser/` (string literal parsing)

**Affected conformance tests:** `parse` suite — `string_literals` section (`triple_*_unescaped_punctuation` tests)

Similar to issue #3 for bytes: triple-quoted strings that contain the single-char delimiter (e.g. `'''a'b'''`) may include the delimiter characters in the parsed value.

---

## Out-of-scope features

These are not bugs but features not yet implemented in ferricel:

- **Proto message construction and field access** — tests using `TestAllTypes` proto messages are automatically skipped in the conformance runner.

---

## `cel.iterVar`/`cel.accuVar` as comprehension iteration variables blocked by `cel` crate parser

The `block_ext` CSE constructs use `cel.iterVar(N, M)` and `cel.accuVar(N, M)` to refer to
comprehension iteration and accumulator variables by nesting depth. When these appear as the
*iteration variable* argument to macros like `map`, `filter`, `exists` (the first positional arg),
the `cel` crate parser rejects them with `"argument must be a simple name"` because its
`extract_ident` helper requires a bare `Ident` node.

`cel.iterVar`/`cel.accuVar` work correctly when used *inside* comprehension bodies.

**Affected conformance tests (`block_ext` suite):**
- `basic/multiple_macros_1` through `multiple_macros_3`
- `basic/nested_macros_1`, `nested_macros_2`
- `basic/adjacent_macros`
- `basic/macro_shadowed_variable_1`, `macro_shadowed_variable_2`

---

## Leading-dot identifier resolution blocked by `cel` crate parser

The CEL spec allows a leading dot on identifiers (`.y`, `.y.z`) to force resolution in
the root scope, bypassing both the active container and comprehension-local variables.
Example: inside `[1].exists(y, .y == 1)`, `.y` refers to the global variable `y`, not
the iteration variable.

The `cel` crate (v0.13.0) silently drops the leading dot for plain `Ident` nodes in
`visit_Ident` (`src/parser/parser.rs`), making `.y` indistinguishable from `y` in the
AST. It correctly preserves leading dots for `GlobalCall` and `CreateMessage` nodes, but
not for bare identifiers.

**Affected conformance tests (`namespace` suite):**
- `namespace_shadowing/disambiguation` — `.y` should resolve to root-scope `y` (bypassing container `com.example`)
- `namespace_shadowing/comprehension_shadowing_disambiguation` — `.y` inside comprehension should bypass local `y`
- `namespace_shadowing/comprehension_shadowing_namespaced_selector_disambiguation` — `.y.z` inside comprehension should bypass local `y` and container
