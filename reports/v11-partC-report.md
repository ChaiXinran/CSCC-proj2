# V11 Part C Report — RegExp / Annex B / Descriptor Builtins

Owner: C group
Scope: builtins / RegExp / Annex B / descriptor sweep

This report must be updated by any worker or AI agent that changes V11-C code.
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

- `test/built-ins/RegExp`
- RegExp-facing `String.prototype` methods
- `test/annexB`
- Object/Function/Array/String/Iterator descriptor sweep cases

## Current Status

Status: basic V11-C functionality plus Object/Reflect descriptor bridge implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-C report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partC-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-25 | Codex | Implemented basic RegExp refinement pass, Annex B legacy globals/accessors, and focused descriptor shape coverage for V11-C | `src/builtins/mod.rs`, `src/builtins/v11.rs`, `tests/native_v11_builtins.rs`, `reports/v11-partC-report.md` | `rustfmt --edition 2024 src/builtins/v11.rs tests/native_v11_builtins.rs`; `cargo test --no-default-features --test native_v11_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262`; `cargo test --no-default-features --test native_v8_builtins`; `cargo test --no-default-features --test native_v9_builtins`; `cargo test --no-default-features --test native_v10_builtins`; `cargo test --no-default-features --test native_v6_builtins string_search_methods` | focused local gates pass; V11 scan not rerun in this basic-functionality pass |
| 2026-06-25 | Codex / C group | Updated JS-visible `Object.defineProperty` and `Reflect.*` symbol/receiver behavior to reuse B-owned descriptor helpers instead of builtin-local patches | `src/builtins/object.rs`, `src/builtins/std_primitives.rs`, `src/runtime/context.rs`, `src/vm/interpreter.rs`, `tests/native_object_keys.rs`, `reports/native-v11-b-object-summary.json`, `reports/native-v11-b-reflect-summary.json`, `reports/v11-partC-report.md` | `cargo fmt --all -- --check`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_object_keys`; `cargo test --no-default-features --test native_regexp`; `cargo test --no-default-features --test native_test262`; Object/Reflect focused Test262 scans | `native_regexp`: 6/6 remains passing; Object scan: 2700/3411 pass; Reflect scan: 118/153 pass, up from 111/153 during this pass |

## Implemented Functionality

- Added `v11` builtin installation after V6/V8/V9/V10 setup.
- Replaced the JS-visible `RegExp` constructor after V6 installation so V11-C
  can validate flags, preserve the existing `RegExp.prototype`, and install
  `RegExp.escape`.
- Refined `RegExp.prototype.exec`, `test`, `compile`, `toString`, and
  prototype getters for `source`, `flags`, `global`, `ignoreCase`,
  `multiline`, `dotAll`, `sticky`, `unicode`, `unicodeSets`, and
  `hasIndices`.
- Added basic `lastIndex` handling for global/sticky `exec`/`test` and
  post-V6 Symbol dispatch for `@@match`, `@@matchAll`, `@@search`, and
  `@@split`.
- Added Annex B legacy globals `escape` and `unescape`.
- Added Annex B `Object.prototype.__defineGetter__`,
  `__defineSetter__`, `__lookupGetter__`, `__lookupSetter__`, and
  `__proto__` accessor behavior.
- Added `String.prototype.trimLeft` and `trimRight` aliases.
- `Object.defineProperty` with symbol keys now uses the shared descriptor
  validation path, so invalid redefinitions throw instead of silently
  overwriting non-configurable symbol properties.
- `Reflect.defineProperty`, `Reflect.getOwnPropertyDescriptor`,
  `Reflect.deleteProperty`, and `Reflect.has` now expose symbol-key descriptor
  semantics from the B-owned object model helpers.
- `Reflect.get` and `Reflect.set` now honor the explicit receiver argument for
  string and symbol keys, including accessor `this` binding and receiver-side
  writes.

## Test Results and Delta Analysis

Initial V11 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Record focused C commands and future `--native-v11-scan` deltas here.

Setup validation:

- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- Initial `--native-v11-scan`: timed out after 300s in local tool execution;
  baseline pass/fail totals remain pending.

Focused local validation for this pass:

```text
rustfmt --edition 2024 src/builtins/v11.rs tests/native_v11_builtins.rs
cargo test --no-default-features --test native_v11_builtins
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test native_v8_builtins
cargo test --no-default-features --test native_v9_builtins
cargo test --no-default-features --test native_v10_builtins
cargo test --no-default-features --test native_v6_builtins string_search_methods
```

Results:

- `native_v11_builtins`: 6 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- `native_v8_builtins`: 10 passed, 0 failed.
- `native_v9_builtins`: 9 passed, 0 failed.
- `native_v10_builtins`: 7 passed, 0 failed.
- `native_v6_builtins string_search_methods`: 1 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.

`cargo test --no-default-features --test native_v6_builtins` was also tried;
it reported 25 passed and 1 failed. The failure is the existing BigInt literal
smoke case (`1n + 2`) and is outside V11-C RegExp/Annex B scope.

The focused Test262 RegExp/Annex B commands and `--native-v11-scan` were not
rerun in this step. This pass stops at the requested basic functionality point
instead of using large-suite failures to drive a broader semantic expansion.

Object/Reflect descriptor bridge validation:

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
- `native_regexp`: 6 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- Object focused scan: 3,411 total, 2,700 passed, 711 failed, 0 skipped,
  79.1557% conformance.
- Reflect focused scan: 153 total, 118 passed, 35 failed, 0 skipped,
  77.1242% conformance. The explicit receiver change accounts for a local
  improvement from 111/153 to 118/153 during this pass.
- Full `--native-v11-scan` was not rerun in this pass because prior local
  attempts against the locked manifest timed out before producing JSON.

## Open Risks / Coordination Notes

- Coordinate with B before descriptor sweep changes depend on object-model
  behavior.
- Annex B coverage is intentionally partial and isolated to builtins that can
  be implemented without runtime object-model changes.
- RegExp matching still uses the existing Rust regex helper, so full ECMAScript
  RegExp semantics, named groups, indices arrays, unicode set behavior, and
  complete sticky/global edge cases remain future work.
- Descriptor sweep coverage is limited to newly installed/refined builtin
  entry points; broader Object/Function/Array/String/Iterator descriptor and
  property-order precision remains coordinated with V11-B.
- Object/Reflect descriptor builtins now depend on B-owned symbol and receiver
  helpers. Future C descriptor sweep work should extend those helpers when the
  missing behavior is shared, and only patch builtin-local metadata when the
  issue is genuinely JS-visible shape data.
