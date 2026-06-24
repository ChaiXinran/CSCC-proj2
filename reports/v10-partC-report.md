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

Status: setup only.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V10-C report template and locked V10 scan manifest | `reports/v10-partC-report.md`, `reports/native-v10-scan-failures.txt`, `reports/native-v10-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json` | baseline: 645/5000 passed, 4355 failed, 0 skipped |

## Implemented Functionality

- None yet.

## Test Results and Delta Analysis

Initial V10 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

Record focused C commands and future `--native-v10-scan` deltas here.

Initial V10 scan result: 5,000 total, 645 passed, 4,355 failed, 0 skipped,
12.90%.

## Open Risks / Coordination Notes

- Date/Intl/Temporal behavior should use normal builtin installation and
  descriptor paths.
- Deterministic fallback formatting is acceptable only when documented here.
- Coordinate with B before exposing JS-visible typed-array objects backed by
  runtime storage.
