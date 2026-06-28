# V10 Part C Report — Date / Intl / Temporal Builtin Semantics

Owner: C group
Scope: builtins / Date / Intl / Temporal / JS-visible typed-array integration

This report must be updated by any worker or AI agent that changes V10-C code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v10-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V10 hotspots.

| Metric | Baseline |
| --- | ---: |
| V10 scan total | 5,000 |
| V10 scan passed | 645 |
| V10 scan failed | 4,355 |
| V10 scan skipped | 0 |

Primary directories:

- `test/built-ins/Date`
- `test/annexB/built-ins/Date`
- `test/intl402`
- `test/built-ins/Temporal`

## Current Status

Status: basic V10-C functionality implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V10-C report template and locked V10 scan manifest | `reports/v10-partC-report.md`, `reports/native-v10-scan-failures.txt`, `reports/native-v10-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json` | baseline: 645/5000 passed, 4355 failed, 0 skipped |
| 2026-06-24 | Codex | Implemented basic Date, deterministic Intl fallback extensions, and selected Temporal core builtins for V10-C | `src/builtins/mod.rs`, `src/builtins/v10.rs`, `tests/native_v10_builtins.rs`, `reports/v10-partC-report.md` | `rustfmt --edition 2024 src/builtins/v10.rs tests/native_v10_builtins.rs`; `cargo test --no-default-features --test native_v10_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_v8_builtins`; `cargo test --no-default-features --test native_v9_builtins`; `cargo test --no-default-features --test native_test262` | focused local gates pass; V10 scan not rerun in this basic-functionality pass |

## Implemented Functionality

- Added `v10` builtin installation after V6/V8/V9 builtin setup.
- Installed global `Date` with constructor/function behavior, `Date.now`,
  `Date.parse`, `Date.UTC`, hidden date value slots, UTC-oriented getters,
  `setTime`, `toISOString`, `toJSON`, `toString`, `toUTCString`, and
  deterministic UTC locale-string fallbacks.
- Extended `Intl` with deterministic fallback methods for
  `DateTimeFormat.format`, `formatToParts`, `formatRange`,
  `formatRangeToParts`, and `NumberFormat.formatToParts`.
- Added basic `Intl.getCanonicalLocales`, `Intl.PluralRules`,
  `Intl.RelativeTimeFormat`, `Intl.ListFormat`, and `Intl.Locale` behavior.
- Installed `Temporal` namespace with basic `Duration`, `Instant`,
  `PlainDate`, `PlainTime`, `PlainDateTime`, and `Now` support.
- Temporal constructors/prototypes expose basic construction, `from`,
  `compare` where implemented, `toString`/`toJSON`, and explicit `TypeError`
  primitive conversion behavior.

## Test Results and Delta Analysis

Initial V10 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

Record focused C commands and future `--native-v10-scan` deltas here.

Initial V10 scan result: 5,000 total, 645 passed, 4,355 failed, 0 skipped,
12.90%.

Focused local validation for this pass:

```text
rustfmt --edition 2024 src/builtins/v10.rs tests/native_v10_builtins.rs
cargo test --no-default-features --test native_v10_builtins
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_v8_builtins
cargo test --no-default-features --test native_v9_builtins
cargo test --no-default-features --test native_test262
```

Results:

- `native_v10_builtins`: 7 passed, 0 failed.
- `native_v8_builtins`: 10 passed, 0 failed.
- `native_v9_builtins`: 9 passed, 0 failed.
- `native_test262`: 14 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- `cargo fmt --all -- --check` was not used as a success gate because it
  currently wants to reformat `tests/frontend_v9.rs`, which is outside V10-C
  ownership for this pass.

The V10 scan and focused Test262 Date/Intl/Temporal sweeps were not rerun in
this step. The user requested stopping once the basic feature surface is in
place rather than using comprehensive failures to drive large follow-up
semantic work.

## Open Risks / Coordination Notes

- Date local-time and locale-sensitive behavior is intentionally mapped to a
  deterministic UTC fallback for V10-C basic functionality.
- Intl formatting is not ICU-quality; it returns stable `en-US`/UTC-oriented
  fallback strings and option objects.
- Temporal coverage is a selected core subset, not full Temporal spec
  conformance.
- Coordinate with B before exposing JS-visible typed-array objects backed by
  runtime storage.
