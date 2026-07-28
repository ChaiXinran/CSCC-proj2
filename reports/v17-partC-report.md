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
