# Native V4 Test262 Report

Test date: 2026-06-21 (UTC+08:00)

This report covers the self-developed Native backend. Boa is not used.

## Fixed Earlier Gates

| Gate | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V1 | 6 | 6 | 0 | 0 |
| Native V2 | 15 | 15 | 0 | 0 |
| Native V3 | 26 | 26 | 0 | 0 |
| Native V4 | 11 | 11 | 0 | 0 |

V1-V4 remain fixed regression gates: every pinned file must pass in all
required modes, with zero failures and zero skips.

## Native V4 Gate

`--native-v4` runs the pinned `NATIVE_V4_TESTS` list in
[`src/test262.rs`](../src/test262.rs). The initial 11 files are grouped by the
V4 scope batches:

| Group | Basis | Files |
| --- | --- | --- |
| V4.1 | Descriptor/prototype/property operators | `delete`, `in`, object `__proto__` |
| V4.2 | Constructor/prototype call model | `instanceof`, duplicate property overwrite, `Function.prototype.call.length` |
| V4.3 | Array and minimal builtins | array nesting/length, `Array`, `Object.create` |

Files enter this gate only when they are official Test262 files in the V4
priority areas, pass both default/strict variants required by metadata, do not
need unsupported harness includes, and do not depend on out-of-scope features
such as class, Symbol, Proxy, destructuring, spread, or iterators.

## Native V4 Scan

`--native-v4-scan` scans the full V4-area directories listed in
`NATIVE_V4_SCAN_SUITES`:

- `test/language/expressions/object`
- `test/language/expressions/array`
- `test/language/expressions/delete`
- `test/language/expressions/in`
- `test/language/expressions/instanceof`
- `test/built-ins/Object`
- `test/built-ins/Array`
- `test/built-ins/Function/prototype/call`

Latest local diagnostic baseline after the `--native-v4-scan` split:

| Scan | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V4 directories | 7,911 | 437 | 5,968 | 1,506 |

The scan treats native harness includes or explicitly unsupported compile-time
features as skipped rather than passed. Common remaining causes include
out-of-scope syntax, missing standard constructors such as `String`/`Number`,
`Proxy`/`Symbol`, harness helper dependencies, dynamic `Function` source
compilation, and standard methods beyond the V4 C2/C3 target.

## Reproduction

```sh
cargo test --test native_test262
cargo run --release -- test262 --native-v4 --jobs 1 --verbose
cargo run --release -- test262 --native-v4-scan --jobs 4 --progress
```
