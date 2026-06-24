# V11 Part B Report — Object Model / Descriptor Precision

Owner: B group
Scope: runtime / VM / contracts / object-model precision

This report must be updated by any worker or AI agent that changes V11-B code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v11-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V11 hotspots.

| Metric | Baseline |
| --- | ---: |
| V11 scan total | 5,000 |
| V11 scan passed | pending |
| V11 scan failed | pending |
| V11 scan skipped | pending |

Primary directories:

- `test/built-ins/Object`
- `test/built-ins/Function`
- `test/built-ins/Array`
- descriptor/property-order/receiver precision cases

## Current Status

Status: first object-key ordering precision fix complete.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-B report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partB-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-24 | Codex / B group | Fixed shared array-index property-key classification so `4294967295` is an ordinary string key, preserving spec-aligned own-key order for Object/Reflect helpers | `src/runtime/object.rs`, `src/runtime/property_map.rs`, `tests/native_v11_runtime.rs`, V11 docs | `cargo test --no-default-features --test native_v11_runtime`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262` | +3 focused V11-B tests. V11 scan not rerun because the current V11 scan manifest times out locally before producing JSON. |

## Implemented Functionality

- Shared array-index classification now rejects `4294967295` (`2^32 - 1`) as
  an array index. ECMAScript array indices are only `0..2^32-2`; this keeps
  `4294967295` in ordinary string-key insertion order.
- `PropertyMap::keys()` and array/object own-property descriptor paths now use
  the corrected boundary.
- Focused tests cover `Object.keys`, `Object.getOwnPropertyNames`, and
  `Reflect.ownKeys` ordering with `4294967295`, numeric index keys, string
  keys, and symbol keys.

## Test Results and Delta Analysis

Initial V11 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Record focused B commands and future `--native-v11-scan` deltas here.

Setup validation:

- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- Initial `--native-v11-scan`: timed out after 300s in local tool execution;
  baseline pass/fail totals remain pending.

First B precision fix validation:

```text
cargo test --no-default-features --test native_v11_runtime
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
```

Results:

- `native_v11_runtime`: 3 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- V11 scan was not rerun because the current manifest timed out locally during
  setup before producing `reports/native-v11-scan-summary.json`.

## Open Risks / Coordination Notes

- C group owns JS-visible RegExp and Annex B builtin behavior.
- B should fix shared descriptor/object-model helpers instead of adding one-off
  builtin patches.
- Property-order changes can affect many existing tests; run focused Object and
  native gates after each non-trivial change.
- C descriptor sweeps should rely on the corrected runtime key-order helper
  instead of adding local sorting in builtin code.
