# Native V5 Test262 Report

Test date: 2026-06-21 (UTC+08:00)

This report covers the self-developed Native backend. Boa is not used.

## Pinned Gate

`--native-v5` runs a zero-skip gate for the first V5 VM/runtime integration:

| Gate | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V5 pinned files | 4 | 4 | 0 | 0 |

Pinned files:

- `test/language/statements/try/S12.14_A18_T5.js`
- `test/language/statements/switch/S12.11_A3_T1.js`
- `test/language/statements/let/global-use-before-initialization-in-prior-statement.js`
- `test/language/statements/const/global-use-before-initialization-in-prior-statement.js`

## Diagnostic Scan Scope

`--native-v5-scan` selects the V5 feature directories and treats unsupported
files as skipped, never as passed.

| Directory | Files |
| --- | ---: |
| `test/language/statements/try` | 108 |
| `test/language/statements/switch` | 47 |
| `test/language/statements/let` | 21 |
| `test/language/statements/const` | 18 |

Latest local diagnostic scan:

| Scan | Total | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Native V5 directories | 593 | 170 | 394 | 29 |

Recent V5 completion fixes moved the following representative try/finally
checks to passing:

- `test/language/statements/try/S12.14_A2.js`
- `test/language/statements/try/S12.14_A3.js`
- `test/language/statements/try/S12.14_A13_T2.js`

Remaining failures are dominated by deferred destructuring/rest syntax, `eval`,
standard error constructors used by `assert.throws`, broader early-error
coverage, and Test262 harness helpers.

## Reproduction

```sh
cargo test --test native_v5 --test native_v5_runtime
cargo test --test native_test262 native_v5
cargo run -- test262 --native-v5 --jobs 1 --verbose
cargo run -- test262 --native-v5-scan --jobs 4 --progress
```
