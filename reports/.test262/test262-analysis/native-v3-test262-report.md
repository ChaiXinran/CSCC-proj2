# Native V3 Test262 Report

Test date: 2026-06-19 (UTC+08:00)

This report covers the self-developed Native backend. Boa is not used.

## Fixed Acceptance Gates

| Gate | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V1 | 6 | 6 | 0 | 0 |
| Native V2 | 15 | 15 | 0 | 0 |
| Native V3 | 26 | 26 | 0 | 0 |

Each fixed file is executed in the Test262 modes selected by its metadata.
Ordinary files run in both default and strict mode.

The V3 gate is defined by `NATIVE_V3_TESTS` in
[`src/test262.rs`](../src/test262.rs). It covers function declarations and
expressions, recursion, functions as values, `return` syntax and line
terminators, and basic object literal property names.

## Broader V3-Area Baseline

| Test262 directory | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| `statements/function` | 451 | 54 | 397 | 0 |
| `statements/return` | 16 | 12 | 4 | 0 |
| `expressions/object` | 1,170 | 172 | 997 | 1 |
| `expressions/array` | 52 | 0 | 52 | 0 |

These directory rates are diagnostic, not V3 conformance claims. Most failures
depend on features outside the frozen V3 scope, including `eval`, built-in
`Object`/`Function` constructors, `instanceof`, destructuring, default/rest
parameters, getters/setters, computed property names, generators, async
functions, array holes, prototype behavior, or additional harness includes.

## Reproduction

```sh
cargo test --test native_test262
cargo run --release -- test262 --native-v3 --jobs 1 --verbose
cargo run --release -- test262 --backend native \
  --suite test/language/statements/return --jobs 1 --verbose
```
