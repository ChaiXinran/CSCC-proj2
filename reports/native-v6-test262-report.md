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
| 2,199 | 769 | 967 | 463 | 34.97% |

Skipped tests are not counted as passes. The remaining failures include
unsupported harness helpers, deferred syntax and standard-library features,
descriptor/metadata edge cases, advanced JSON reviver/replacer/space behavior,
and semantics outside the V6 core scope.

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
