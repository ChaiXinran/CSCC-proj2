# V8 Part B Report — Module Runner Infrastructure

Owner: B group
Scope: runtime / VM / module runner / module registry

This report must be updated by any worker or AI agent that changes V8-B code.
Do not wait for an explicit user request. Update it in the same change as the
implementation.

## Baseline

Baseline source: `reports/test262-analysis.md` locked on 2026-06-24.

| Metric | Baseline |
| --- | ---: |
| Full direct total | 53,379 |
| Full direct passed | 14,035 |
| Full direct failed | 38,507 |
| Full direct skipped | 837 |
| Full direct pass rate | 26.29% |
| `module runner not implemented yet` skips | 821 |
| Module-related parsed failures | 4 |
| V8 scan total | 5,000 |
| V8 scan passed | 0 |
| V8 scan failed | 4,504 |
| V8 scan skipped | 496 |

Primary directories:

- `test/language/module-code`
- module flagged Test262 cases
- module dependencies loaded by the Test262 runner

## Current Status

Status: first-stage infrastructure complete; waiting for A-group import/export
AST lowering before full module graph execution can pass import/export suites.

## Change Log

Add entries newest first.

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | B group | Added explicit `SourceKind::{Script, Module}`, native module eval entry, module registry/status records, import/export binding storage slots, module top-level strict mode, module top-level `this === undefined`, relative dependency loader, duplicate-evaluation guard, and module-mode Test262 failure labels | `src/engine.rs`, `src/backend/mod.rs`, `src/backend/native.rs`, `src/backend/boa.rs`, `src/runtime/module.rs`, `src/runtime/mod.rs`, `src/runtime/context.rs`, `src/contracts.rs`, `src/test262.rs`, `src/lib.rs`, `tests/native_v8_module.rs`, `reports/native-v8-b-module-summary.json`, `reports/native-v8-scan-summary.json` | `cargo fmt --all -- --check`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_v8_module`; `cargo test --no-default-features --test native_test262`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v8-b-module-summary.json`; `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | B unit tests 5/5 passed; native Test262 gate 12/12 passed; module-code focused run 201/599 passed, 398 failed, 0 skipped, 33.56%; V8 scan improved from 0/5000 passed, 4504 failed, 496 skipped to 205/5000 passed, 4795 failed, 0 skipped |
| 2026-06-24 | setup | Recorded shared V8 scan baseline | `reports/native-v8-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Created report template | `reports/v8-partB-report.md` | not run | baseline recorded |

## Implemented Functionality

- Backend-neutral `SourceKind` is available through `ExecutionOptions`.
- Test262 `flags: [module]` now enters the native module path instead of being
  skipped as `module runner not implemented yet`.
- Native module execution is strict by default.
- Module top-level `this` is `undefined`; normal script top-level `this`
  remains `globalThis`.
- Native runtime tracks `ModuleRecord`, `ModuleId`, `ModuleStatus`, and a
  per-isolate `ModuleRegistry`.
- `ModuleRecord` includes import/export binding storage slots for the A/B
  connector to fill once import/export AST is available.
- Re-evaluating the same normalized module path is deduplicated.
- Relative module specifiers (`./` and `../`) resolve from the importing file.
- Bare module specifiers return an explicit `Unsupported` failure.
- Module Test262 failures are labelled as `module mode` for report
  classification.

## Test Results and Delta Analysis

When updating this section, compare against the locked baseline above and record:

- command;
- total / passed / failed / skipped;
- pass-rate delta;
- module skip delta;
- new module failure classes after skips become executable;
- regressions.

Current B focused result:

```text
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v8-b-module-summary.json
```

Result: 599 total, 201 passed, 398 failed, 0 skipped, 33.56%.

Current V8 lightweight scan:

```text
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

Result: 5,000 total, 205 passed, 4,795 failed, 0 skipped, 4.10%.

Delta against the locked V8 scan baseline:

- passed: 0 -> 205;
- failed: 4,504 -> 4,795;
- skipped: 496 -> 0;
- conformance: 0.00% -> 4.10%.

Interpretation:

- The focused `test/language/module-code` suite no longer has module-runner
  skips; all 599 files are executable through the native module path.
- The V8 lightweight scan no longer has skipped cases after the module runner
  path was enabled; previously skipped module cases are now either passing or
  visible as parser/runtime failures.
- 201 cases already pass with strict module execution and existing parser/VM
  support.
- Most remaining failures are expected parser/front-end gaps around
  `import`/`export`, import attributes, top-level await, class/generator/async
  forms, and module linking semantics. Those need A-group AST support before B
  can wire real import/export binding storage.
- The module top-level `this` fix converted at least one focused module case
  from failure to pass.

## Open Risks / Coordination Notes

- Coordinate import/export AST shape with A group; B has the loader/registry
  substrate but does not parse or lower import/export declarations.
- Coordinate `$262` host helpers and report formatting with C group.
- Current loader supports explicit relative dependency loading from Rust-side
  connectors. Once A exposes import/export AST, B should connect those AST nodes
  to `load_module_dependency` and add import/export binding records.
- Cyclic live-binding semantics remain a later-version task; V8 reports cyclic
  module graphs as explicit unsupported behavior.
