# Native V17 Part C report

## Ownership and scope

This batch covers shared Temporal PlainDate, PlainDateTime, and PlainTime
argument normalization and ISO date range validation.

## Changes

- Truncate PlainDate and PlainDateTime constructor date arguments after
  `ToNumber`, as required by Temporal integer conversion.
- Validate the inclusive Temporal ISO date range using its exact civil-date
  endpoints. This avoids the negative-boundary off-by-one in epoch-day
  conversion.
- Treat an explicit `undefined` time component as the Temporal default zero,
  without changing the distinct legacy `Date` constructor conversion rules.

Files touched:

- `src/builtins/date_intl.rs`
- `reports/test262-latest-common-errors-2026-07-28.md`

## Focused Test262 results

| Suite | Baseline failures | Final failures | Newly passing | Regressions |
|---|---:|---:|---:|---:|
| `built-ins/Temporal/PlainDate` | 136 | 125 | 11 | 0 |
| `built-ins/Temporal/PlainDateTime` | 132 | 131 | 1 | 0 |
| `built-ins/Temporal/PlainTime` | 67 | 64 | 3 | 0 |
| `built-ins/Temporal/PlainDate` (`monthCode`) | 125 | 123 | 2 | 0 |
| `built-ins/Temporal/PlainDateTime` (`monthCode`) | 131 | 129 | 2 | 0 |
| `built-ins/Temporal/PlainYearMonth` (`monthCode`) | 124 | 123 | 1 | 0 |
| `built-ins/Temporal/PlainMonthDay` (`monthCode`) | 47 | 47 | 0 | 0 |

Focused cases:

- `built-ins/Temporal/PlainDate/limits.js`: pass
- `built-ins/Temporal/PlainDate/argument-convert.js`: pass
- `built-ins/Temporal/PlainDateTime/argument-convert.js`: pass
- `built-ins/Temporal/PlainDate/from/monthcode-invalid.js`: pass

## Repository lint cleanup

- Applied repository-wide rustfmt output.
- Applied safe Clippy suggestions and resolved all remaining diagnostics.
- Removed redundant Set-like object bindings, simplified Date argument
  conversion loops, and used narrow lint allowances for stable internal
  functions whose spec-driven signatures exceed Clippy's argument threshold.
- No public contract signature was changed.

## Commands

```text
cargo build --release
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainDate\" --jobs 4
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainDateTime\" --jobs 4
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainTime\" --jobs 4
cargo check --all-targets
cargo test --all-targets
rustfmt --edition 2024 --check src/builtins/date_intl.rs
```

## Coordination notes

- No shared contract was changed.
- No Test262, Boa, or QuickJS submodule file was modified.
- `cargo check --all-targets` and `cargo test --all-targets` pass.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test --no-default-features --test native_test262` passes (15/15).
# WeakRef / FinalizationRegistry correctness follow-up

- Added the missing `WeakRef` constructor/prototype surface, target validation,
  `deref`, custom `new.target.prototype` handling, and `Symbol.toStringTag`.
- Added the missing `FinalizationRegistry` constructor/prototype surface with
  cleanup-callback validation, `register`/`unregister`, weak-target/token
  validation, token identity removal, and descriptor-visible builtin shape.
- The current collector does not yet enqueue cleanup callbacks. The
  implementation deliberately covers deterministic ECMAScript observability;
  nondeterministic collection scheduling remains deferred.

Validation:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/WeakRef` | 0 / 29 | 25 / 29 | +25 |
| `test/built-ins/FinalizationRegistry` | 0 / 47 | 43 / 47 | +43 |
| Full Test262 | 45,654 / 53,379 | 45,725 / 53,379 | +71 |

Full conformance moved from **85.5280%** to **85.6610%** (+0.1330 percentage
points), with failures decreasing from 7,723 to 7,652 and skips unchanged at
2. The focused suites account for +68 passes; the full scan records a further
+3 net cross-suite improvement.

Commands:

```powershell
cargo test --locked --test native_weak_refs
cargo check --locked --all-targets
cargo test --locked --all-targets
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\WeakRef --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\FinalizationRegistry --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --jobs 4 --progress --json reports\full-test262-summary.json
```

## JetStream RegExp compatibility follow-up

- Translate JavaScript control-letter escapes such as `\cX` to the
  corresponding control code before compiling with the Rust regex backend.
- Install the `RegExp` `Symbol.toStringTag`, allowing validatorjs to recognize
  regular expressions through `Object.prototype.toString.call(value)`.
- Added focused tests for control escapes and both literal/constructed RegExp
  branding.

Validation:

| Suite | Result |
|---|---:|
| `test/built-ins/RegExp` | 1464/1879, 0 skipped |
| control-letter translation unit test | pass |
| RegExp branding integration test | pass |

validatorjs now executes its assertion corpus until it reaches a pattern with
numeric backreferences. The current Rust `regex` backend does not support
backreferences; resolving that remaining item requires a backtracking-capable
backend or a separate compatibility execution path.
