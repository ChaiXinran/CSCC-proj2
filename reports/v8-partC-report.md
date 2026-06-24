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

Status: not started.

## Change Log

Add entries newest first.

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Added locked 5,000-case `--native-v8-scan` manifest and CLI/docs requirements | `src/test262.rs`, `src/main.rs`, `tests/native_test262.rs`, `reports/native-v8-scan-failures.txt`, docs | `cargo test --no-default-features --test native_test262 native_v8_scan_selects_the_locked_failed_case_manifest`; `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | selector test passed; V8 scan baseline is 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Recorded shared V8 scan baseline | `reports/native-v8-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Created report template | `reports/v8-partC-report.md` | not run | baseline recorded |

## Implemented Functionality

- None yet.

## Test Results and Delta Analysis

When updating this section, compare against the locked baseline above and record:

- command;
- total / passed / failed / skipped;
- pass-rate delta;
- missing-global delta;
- descriptor/semantic failures newly exposed after skeletons are installed;
- regressions.

## Open Risks / Coordination Notes

- Use runtime object-model APIs for constructors, prototypes, and descriptors.
- Coordinate storage/runtime helper needs with B group.
- Do not edit `reports/test262-analysis.md`; create a new dated/versioned
  analysis file for future full-suite analysis.
