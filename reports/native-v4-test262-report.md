# Native V4 Test262 Report

Test date: 2026-06-21 (UTC+08:00)

This report covers the self-developed Native backend. Boa is not used.

## Fixed Earlier Gates

| Gate | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V1 | 6 | 6 | 0 | 0 |
| Native V2 | 15 | 15 | 0 | 0 |
| Native V3 | 26 | 26 | 0 | 0 |

V1-V3 remain fixed regression gates. V4 is intentionally not represented by a
curated "known passing" list.

## Native V4

`--native-v4` scans the full V4-area directories listed in
`NATIVE_V4_SCAN_SUITES` in [`src/test262.rs`](../src/test262.rs):

- `test/language/expressions/object`
- `test/language/expressions/array`
- `test/language/expressions/delete`
- `test/language/expressions/in`
- `test/language/expressions/instanceof`
- `test/built-ins/Object`
- `test/built-ins/Array`
- `test/built-ins/Function/prototype/call`

Latest local baseline before the command semantics were renamed:

| Scan | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V4 directories | 7,911 | 397 | 6,009 | 1,505 |

The scan treats native harness includes or explicitly unsupported compile-time
features as skipped rather than passed. Common remaining causes include
out-of-scope syntax, missing standard constructors such as `String`/`Number`,
`Proxy`/`Symbol`, harness helper dependencies, dynamic `Function` source
compilation, and standard methods beyond the V4 C2/C3 target.

## Reproduction

```sh
cargo test --test native_test262
cargo run --release -- test262 --native-v4 --jobs 4
cargo run --release -- test262 --native-v4 --jobs 4 --progress
```
