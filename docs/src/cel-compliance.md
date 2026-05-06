# CEL Compliance

Ferricel targets full compliance with the [CEL specification](https://github.com/google/cel-spec) and the [cel-go extension libraries](https://pkg.go.dev/github.com/google/cel-go/ext).
Conformance is validated against the official CEL conformance test suite.

## Specification Coverage

The next sections outline which parts of the different CEL specifications are currently supported
by the ferricel compiler.

### Core CEL Specification

| Feature                                                                           | Status    |
| --------------------------------------------------------------------------------- | --------- |
| Integer arithmetic (`+`, `-`, `*`, `/`, `%`) with overflow detection              | Supported |
| Floating-point arithmetic                                                         | Supported |
| Unsigned integer (`uint`) arithmetic                                              | Supported |
| Boolean logic (`&&`, `\|\|`, `!`) with short-circuit evaluation                   | Supported |
| Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)                           | Supported |
| String operations (`+`, `size`, `contains`, `startsWith`, `endsWith`, `matches`)  | Supported |
| Bytes operations                                                                  | Supported |
| List literals, indexing, membership (`in`)                                        | Supported |
| Map literals, field access, indexing                                              | Supported |
| Conditional expressions (`? :`)                                                   | Supported |
| Null handling and null propagation                                                | Supported |
| Type conversions (`int()`, `uint()`, `double()`, `string()`, `bytes()`, `bool()`) | Supported |
| Timestamp and Duration types                                                      | Supported |
| Timestamp/Duration arithmetic and field accessors                                 | Supported |
| `size()` function                                                                 | Supported |
| Single-variable comprehensions (`all`, `exists`, `exists_one`, `map`, `filter`)   | Supported |
| Logical error propagation through `&&` / `\|\|`                                   | Supported |
| Optional types (`optional.of`, `optional.none`, `.orValue`, `.value`)             | Supported |
| Protocol Buffer message construction and field access                             | Supported |
| Protobuf wrapper type semantics (`google.protobuf.*Value`)                        | Supported |
| `dyn()` type erasure                                                              | Supported |
| `type()` introspection                                                            | Supported |

### cel-go Extension Libraries

| Extension                | Functions                                                        | Status        |
| ------------------------ | ---------------------------------------------------------------- | ------------- |
| **Bindings**             | `cel.bind(var, init, body)`                                      | Supported     |
| **Encoders**             | `base64.encode`, `base64.decode`                                 | Supported     |
| **Math**                 | `math.greatest`, `math.least`                                    | Supported     |
|                          | `math.bitOr`, `math.bitAnd`, `math.bitXor`, `math.bitNot`        | Supported     |
|                          | `math.bitShiftLeft`, `math.bitShiftRight`                        | Supported     |
|                          | `math.ceil`, `math.floor`, `math.round`, `math.trunc`            | Supported     |
|                          | `math.abs`, `math.sign`                                          | Supported     |
|                          | `math.isInf`, `math.isNaN`, `math.isFinite`                      | Supported     |
|                          | `math.sqrt`                                                      | Supported     |
| **Strings**              | `charAt`, `indexOf`, `lastIndexOf`                               | Supported     |
|                          | `lowerAscii`, `upperAscii`, `trim`                               | Supported     |
|                          | `replace`, `split`, `substring`, `join`                          | Supported     |
|                          | `reverse`, `strings.quote`                                       | Supported     |
|                          | `format` (string interpolation)                                  | Supported     |
| **Lists**                | `slice`, `flatten`, `distinct`, `reverse`                        | Supported     |
|                          | `sort`, `sortBy`                                                 | Supported     |
|                          | `first`, `last`                                                  | Supported     |
|                          | `lists.range(n)`                                                 | Supported     |
| **Sets**                 | `sets.contains`, `sets.equivalent`, `sets.intersects`            | Supported     |
| **TwoVarComprehensions** | `all(i, v, pred)`, `exists(i, v, pred)`, `existsOne(i, v, pred)` | Supported     |
|                          | `transformList(i, v, [filter,] expr)`                            | Supported     |
|                          | `transformMap(i, v, [filter,] expr)`                             | Supported     |
|                          | `transformMapEntry(i, v, [filter,] mapExpr)`                     | Supported     |
| **Regex**                | `regex.replace`, `regex.extract`, `regex.extractAll`             | Supported     |
| **Protos**               | `proto.getExt`, `proto.hasExt`                                   | Not supported |

### Kubernetes CEL Extensions

Ferricel also supports the [Kubernetes CEL validation libraries](https://kubernetes.io/docs/reference/using-api/cel/):

| Extension                                                                          | Status    |
| ---------------------------------------------------------------------------------- | --------- |
| IP address functions (`ip()`, `isIP()`, `family()`, etc.)                          | Supported |
| CIDR functions (`cidr()`, `isCIDR()`, `containsIP()`, etc.)                        | Supported |
| URL functions (`url()`, `isURL()`, `getHost()`, etc.)                              | Supported |
| Quantity functions (`quantity()`, `isQuantity()`, `add()`, `sub()`, etc.)          | Supported |
| Semver functions (`semver()`, `isSemver()`, `major()`, `minor()`, `patch()`, etc.) | Supported |
| Format validation (`format.named()`, `format.dns1123Label()`, etc.)                | Supported |
| List extensions (`isSorted()`, `sum()`, `min()`, `max()`)                          | Supported |
| Regex extensions (`find()`, `findAll()`)                                           | Supported |

## Conformance Tests

This is an overview of the current status of the conformance tests:

| Test Suite     | Successful | Failed | Skipped |
| -------------- | ---------: | -----: | ------: |
| basic          |         41 |      2 |       0 |
| bindings_ext   |          8 |      0 |       0 |
| block_ext      |         18 |      8 |      11 |
| comparisons    |        406 |      0 |       0 |
| conversions    |        109 |      0 |       0 |
| encoders_ext   |          4 |      0 |       0 |
| fp_math        |         30 |      0 |       0 |
| integer_math   |         64 |      0 |       0 |
| lists          |         39 |      0 |       0 |
| logic          |         30 |      0 |       0 |
| macros2        |         46 |      0 |       0 |
| macros         |         44 |      0 |       0 |
| math_ext       |        199 |      0 |       0 |
| namespace      |         11 |      3 |       0 |
| network_ext    |         69 |      0 |       0 |
| optionals      |         70 |      0 |       0 |
| parse          |        128 |     74 |      17 |
| string_ext     |        216 |      0 |       0 |
| string         |         51 |      0 |       0 |
| timestamps     |         76 |      0 |       0 |
| type_deduction |         17 |      1 |      29 |
| **Total**      |       1676 |     88 |      57 |
