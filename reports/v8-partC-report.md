# V8 Part C Report — Builtin Skeletons and Test262 Host

Owner: C group
Scope: builtins / `$262` host helpers / Test262 reports

This report must be updated by any worker or AI agent that changes V8-C code.
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
| Missing global / builtin / harness helper | 9,219 |
| `Float64Array is not defined` | 2,018 |
| `Intl is not defined` | 597 |
| `$262 is not defined` | 230 |
| V8 scan total | 5,000 |
| V8 scan passed | 0 |
| V8 scan failed | 4,504 |
| V8 scan skipped | 496 |

Primary directories:

- `test/built-ins/TypedArray`
- `test/built-ins/ArrayBuffer`
- `test/intl402`
- Test262 harness or `$262`-dependent cases

## Current Status

Status: basic C-track skeletons implemented.

The V8-C pass now installs the first-batch ArrayBuffer, DataView, TypedArray,
Intl, and `$262` host shapes. The implementation is intentionally a skeleton:
descriptor/prototype metadata and deterministic host helpers are present, while
real byte storage, indexed element algorithms, resizable buffers, SharedArrayBuffer,
cross-realm creation, and complete Intl locale behavior remain future bug-fix or
feature work.

## Change Log

Add entries newest first.

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | C / Codex | Implemented V8-C builtin skeletons and Test262 host helpers: ArrayBuffer/DataView metadata, concrete TypedArray constructors plus reachable `%TypedArray%` shape, Intl namespace constructors, `$262.global/evalScript/gc/detachArrayBuffer/createRealm`, and symbol-key Object descriptor support needed by builtin descriptors | `src/builtins/v8.rs`, `src/builtins/mod.rs`, `src/builtins/object.rs`, `tests/native_v8_builtins.rs`, `reports/native-v8-c-arraybuffer-summary.json`, `reports/native-v8-c-typedarray-summary.json`, `reports/v8-partC-report.md` | `cargo fmt --all`; `cargo test --no-default-features --test native_v8_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --progress --json reports/native-v8-c-arraybuffer-summary.json`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v8-c-typedarray-summary.json` | V8-C unit test 10/10 passed; all-target check passed; native Test262 selector/regression tests 12/12 passed; ArrayBuffer focused run 113/221 passed, 108 failed, 0 skipped; TypedArray focused run 9/1446 passed, 1437 failed, 0 skipped |
| 2026-06-24 | setup | Added locked 5,000-case `--native-v8-scan` manifest and CLI/docs requirements | `src/test262.rs`, `src/main.rs`, `tests/native_test262.rs`, `reports/native-v8-scan-failures.txt`, docs | `cargo test --no-default-features --test native_test262 native_v8_scan_selects_the_locked_failed_case_manifest`; `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | selector test passed; V8 scan baseline is 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Recorded shared V8 scan baseline | `reports/native-v8-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Created report template | `reports/v8-partC-report.md` | not run | baseline recorded |

## Implemented Functionality

- ArrayBuffer constructor/prototype skeleton:
  constructor-only call behavior, `byteLength`, `maxByteLength`, `resizable`,
  `detached`, `immutable`, `ArrayBuffer.isView`, `Symbol.species`, and
  `Symbol.toStringTag`.
- DataView constructor/prototype skeleton:
  ArrayBuffer receiver validation, `buffer`, `byteLength`, `byteOffset`,
  `Symbol.toStringTag`, and explicit TypeError for byte storage methods.
- TypedArray skeletons:
  concrete global constructors for Int8/Uint8/Uint8Clamped/Int16/Uint16/Int32/
  Uint32/Float32/Float64/BigInt64/BigUint64 arrays, `BYTES_PER_ELEMENT`,
  instance metadata getters, `ArrayBuffer.isView`, `Symbol.toStringTag`, and a
  reachable `%TypedArray%` intrinsic via `Object.getPrototypeOf(Int8Array)`.
- Intl skeletons:
  `Intl`, `Intl.DateTimeFormat`, `Intl.NumberFormat`, and `Intl.Collator`, with
  deterministic `resolvedOptions()`, `supportedLocalesOf()`, simple
  `NumberFormat.prototype.format`, and simple `Collator.prototype.compare`.
- Test262 host helper skeleton:
  `$262.global`, `$262.evalScript`, `$262.gc`, `$262.detachArrayBuffer`, and an
  explicit unsupported `$262.createRealm`.
- Object descriptor support needed by symbol-key builtin properties:
  `Object.getOwnPropertyDescriptor`, `hasOwnProperty`, and
  `propertyIsEnumerable` now handle symbol keys.

## Test Results and Delta Analysis

- `cargo fmt --all`: passed.
- `cargo test --no-default-features --test native_v8_builtins`: 10 passed, 0
  failed.
- `cargo check --no-default-features --all-targets`: passed.
- `cargo test --no-default-features --test native_test262`: 12 passed, 0 failed.
- `cargo run --release --no-default-features -- test262 --backend native --root
  test262 --suite test/built-ins/ArrayBuffer --jobs 4 --progress --json
  reports/native-v8-c-arraybuffer-summary.json`: 221 total, 113 passed, 108
  failed, 0 skipped, 51.13%.
- `cargo run --release --no-default-features -- test262 --backend native --root
  test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json
  reports/native-v8-c-typedarray-summary.json`: 1,446 total, 9 passed, 1,437
  failed, 0 skipped, 0.62%.

`test/intl402` and `--native-v8-scan` were not rerun in this pass. The user
asked to stop once basic functionality was implemented and leave complete-suite
coverage for later bug-fix work.

## Open Risks / Coordination Notes

- Use runtime object-model APIs for constructors, prototypes, and descriptors.
- Coordinate storage/runtime helper needs with B group.
- Do not edit `reports/test262-analysis.md`; create a new dated/versioned
  analysis file for future full-suite analysis.
- Remaining focused failures are expected for this C skeleton pass: real
  ArrayBuffer bytes, DataView read/write, TypedArray indexed storage and
  iteration algorithms, resizable buffers, SharedArrayBuffer, complete
  `%TypedArray%` semantics, `$262.createRealm`, parser gaps in newer Test262
  helpers, and full Intl locale-sensitive behavior are still open.
