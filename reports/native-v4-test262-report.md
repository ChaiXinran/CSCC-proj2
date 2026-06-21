# Native V4 Test262 Report

Test date: 2026-06-21 (UTC+08:00)

This report covers the self-developed Native backend. Boa is not used.

## Fixed Acceptance Gates

| Gate | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V1 | 6 | 6 | 0 | 0 |
| Native V2 | 15 | 15 | 0 | 0 |
| Native V3 | 26 | 26 | 0 | 0 |
| Native V4 | 11 | 11 | 0 | 0 |

The V4 gate is defined by `NATIVE_V4_TESTS` in
[`src/test262.rs`](../src/test262.rs). It covers property deletion, array
element deletion, deleting missing properties, the `in` operator, distinction
between absent and `undefined` properties, a trailing array hole, and duplicate
`__proto__` early errors.

## Broader V4-Area Baseline

| Test262 directory | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| `expressions/delete` | 69 | 7 | 62 | 0 |
| `expressions/in` | 36 | 11 | 25 | 0 |
| `expressions/instanceof` | 43 | 0 | 43 | 0 |
| `expressions/array` | 52 | 1 | 51 | 0 |

These directory rates are diagnostic, not broad conformance claims. Remaining
tests commonly require `eval`, `try/catch`, lexical declarations, private
fields, standard `Object`/`Array`/`Function` constructors, descriptor built-ins,
or harness includes that are outside the current native subset.

Some `in` directory tests report success only because their expected syntax
error is triggered by another unsupported construct. They are excluded from
the fixed V4 gate.

## Reproduction

```sh
cargo test --test native_test262
cargo run --release -- test262 --native-v4 --jobs 1 --verbose
cargo run --release -- test262 --backend native \
  --suite test/language/expressions/delete --jobs 1
```
