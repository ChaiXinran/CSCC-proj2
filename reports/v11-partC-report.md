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

Status: basic V11-C functionality implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-C report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partC-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-25 | Codex | Implemented basic RegExp refinement pass, Annex B legacy globals/accessors, and focused descriptor shape coverage for V11-C | `src/builtins/mod.rs`, `src/builtins/v11.rs`, `tests/native_v11_builtins.rs`, `reports/v11-partC-report.md` | `rustfmt --edition 2024 src/builtins/v11.rs tests/native_v11_builtins.rs`; `cargo test --no-default-features --test native_v11_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262`; `cargo test --no-default-features --test native_v8_builtins`; `cargo test --no-default-features --test native_v9_builtins`; `cargo test --no-default-features --test native_v10_builtins`; `cargo test --no-default-features --test native_v6_builtins string_search_methods` | focused local gates pass; V11 scan not rerun in this basic-functionality pass |

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
