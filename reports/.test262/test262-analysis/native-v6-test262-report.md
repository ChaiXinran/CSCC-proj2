# Native V6 Test262 Report

## Scope

Native V6 covers the shared coercion and primitive-wrapper foundation plus the
String, Number, Boolean, Math, Error, and core JSON builtin families. Tests run
through the self-developed Native lexer, parser, bytecode compiler, VM,
runtime, and builtin implementation. Boa is not used as a fallback.

## Acceptance Gate

Command:

```powershell
cargo run -- test262 --native-v6 --jobs 1 --verbose
```

Result:

| Total | Passed | Failed | Skipped | Conformance |
| ---: | ---: | ---: | ---: | ---: |
| 7 | 7 | 0 | 0 | 100.00% |

The pinned files cover String, Number, Math, Boolean, Error, JSON parsing, and
JSON stringification. This is the zero-regression merge gate, not the complete
builtin conformance percentage.

## Diagnostic Directory Scan

Command:

```powershell
cargo run -- test262 --native-v6-scan --jobs 4 --json reports/native-v6-summary.json
```

Scanned directories:

- `test/built-ins/String`
- `test/built-ins/Number`
- `test/built-ins/Math`
- `test/built-ins/Boolean`
- `test/built-ins/Error`
- `test/built-ins/JSON`

Result:

| Total | Passed | Failed | Skipped | Conformance |
| ---: | ---: | ---: | ---: | ---: |
| 2,199 | 1,499 | 699 | 1 | 68.17% |

This is the latest Native scan after merging Track A `arguments` support,
Track B Error/JSON/Math fixes, and Track C compound-assignment support.
Compared with the A+B baseline, Track C adds 10 passes with no regressions.
Skipped tests are not counted as passes. The remaining failures are concentrated in deferred
Symbol/Proxy/RegExp/Realm support, frontend syntax gaps, and advanced
object-model behavior.

Per-directory merged results:

| Area | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: |
| String | 791 | 431 | 1 | 64.68% |
| Number | 262 | 78 | 0 | 77.06% |
| Math | 240 | 87 | 0 | 73.39% |
| Boolean | 37 | 14 | 0 | 72.55% |
| Error | 59 | 34 | 0 | 63.44% |
| JSON | 110 | 55 | 0 | 66.67% |

## Project Quality Gates

The following commands pass on the tested Windows environment:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --all-targets
cargo clippy --no-default-features --all-targets -- -D warnings
```

The V1-V6 pinned Test262 gates pass 69 of 69 files and remain mandatory
zero-regression requirements.
