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
| 2,199 | 1,489 | 709 | 1 | 67.71% |

This is the latest Native scan after merging Track A `arguments` support with
the Track B Error, JSON, and Math fixes. Skipped tests are not counted as
passes. The remaining failures are concentrated in deferred
Symbol/Proxy/RegExp/Realm support, frontend syntax gaps, and advanced
object-model behavior.

Per-directory merged results:

| Area | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: |
| String | 784 | 438 | 1 | 64.10% |
| Number | 262 | 78 | 0 | 77.06% |
| Math | 240 | 87 | 0 | 73.39% |
| Boolean | 37 | 14 | 0 | 72.55% |
| Error | 59 | 34 | 0 | 63.44% |
| JSON | 107 | 58 | 0 | 64.85% |

## Project Quality Gates

The following commands pass on the tested Windows environment:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

The V1-V5 pinned Test262 gates remain regression requirements alongside the V6
gate.
