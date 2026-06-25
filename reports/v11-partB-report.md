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

Status: symbol-key descriptor and receiver precision pass complete.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-B report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partB-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-24 | Codex / B group | Fixed shared array-index property-key classification so `4294967295` is an ordinary string key, preserving spec-aligned own-key order for Object/Reflect helpers | `src/runtime/object.rs`, `src/runtime/property_map.rs`, `tests/native_v11_runtime.rs`, V11 docs | `cargo test --no-default-features --test native_v11_runtime`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262` | +3 focused V11-B tests. V11 scan not rerun because the current V11 scan manifest times out locally before producing JSON. |
| 2026-06-25 | Codex / B group | Added shared symbol-key descriptor validation, symbol delete/has/set helpers, and receiver-aware Reflect get/set VM paths | `src/runtime/context.rs`, `src/vm/interpreter.rs`, `src/builtins/object.rs`, `src/builtins/std_primitives.rs`, `tests/native_object_keys.rs`, `reports/native-v11-b-object-summary.json`, `reports/native-v11-b-reflect-summary.json`, `reports/v11-partB-report.md` | `cargo fmt --all -- --check`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_object_keys`; `cargo test --no-default-features --test native_regexp`; `cargo test --no-default-features --test native_test262`; Object/Reflect focused Test262 scans | `native_object_keys`: 7/7; `native_regexp`: 6/6; `native_test262`: 15/15; Object scan: 2700/3411 pass; Reflect scan: 118/153 pass, up from 111/153 during this pass |

## Implemented Functionality

- Shared array-index classification now rejects `4294967295` (`2^32 - 1`) as
  an array index. ECMAScript array indices are only `0..2^32-2`; this keeps
  `4294967295` in ordinary string-key insertion order.
- `PropertyMap::keys()` and array/object own-property descriptor paths now use
  the corrected boundary.
- Focused tests cover `Object.keys`, `Object.getOwnPropertyNames`, and
  `Reflect.ownKeys` ordering with `4294967295`, numeric index keys, string
  keys, and symbol keys.
- Shared descriptor validation now covers symbol keys as well as string keys.
  `Object.defineProperty` and `Reflect.defineProperty` no longer blindly
  overwrite symbol properties when descriptor invariants reject the update.
- Symbol-key `Reflect.deleteProperty`, `Reflect.has`, and
  `Reflect.getOwnPropertyDescriptor` now use the object model helpers and honor
  configurability, prototype-chain lookup, and descriptor shape.
- Symbol-key VM property reads/writes now follow accessor/data descriptor paths.
  Accessor getters/setters are invoked through the VM call path, and data
  writes respect non-writable or inherited descriptors.
- `Reflect.get` and `Reflect.set` now use the explicit receiver argument for
  string and symbol keys, including accessor `this` binding and receiver-side
  data writes.

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

Second B precision fix validation:

```text
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_regexp
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-b-object-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --progress --json reports/native-v11-b-reflect-summary.json
```

Results:

- `cargo fmt --all -- --check`: passed.
- `cargo check --no-default-features --all-targets`: passed.
- `native_object_keys`: 7 passed, 0 failed.
- `native_regexp`: 6 passed, 0 failed. This was included because C-visible
  builtins use the same descriptor/receiver paths touched in this pass.
- `native_test262`: 15 passed, 0 failed.
- Object focused scan: 3,411 total, 2,700 passed, 711 failed, 0 skipped,
  79.1557% conformance.
- Reflect focused scan: 153 total, 118 passed, 35 failed, 0 skipped,
  77.1242% conformance. During this pass the same Reflect scan improved from
  111/153 to 118/153 after the explicit receiver fix.
- Full `--native-v11-scan` was not rerun in this pass because prior local
  attempts against the locked manifest timed out before producing JSON.

## Open Risks / Coordination Notes

- C group owns JS-visible RegExp and Annex B builtin behavior.
- B should fix shared descriptor/object-model helpers instead of adding one-off
  builtin patches.
- Property-order changes can affect many existing tests; run focused Object and
  native gates after each non-trivial change.
- C descriptor sweeps should rely on the corrected runtime key-order helper
  instead of adding local sorting in builtin code.
- This pass intentionally routes JS-visible Object/Reflect changes through B
  helpers so C descriptor sweep work can reuse the same symbol and receiver
  semantics instead of duplicating builtin-local rules.
