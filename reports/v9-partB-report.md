# V9 Part B Report — Promise / Job Queue / Iterator Runtime

Owner: B group
Scope: runtime / VM / backend job draining / iterator helpers

This report must be updated by any worker or AI agent that changes V9-B code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v9-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V9 hotspots.

| Metric | Baseline |
| --- | ---: |
| V9 scan total | 5,000 |
| V9 scan passed | 0 |
| V9 scan failed | 5,000 |
| V9 scan skipped | 0 |

Primary directories:

- `test/built-ins/Promise`
- iterator runtime users from language and builtin tests
- async Test262 completion paths

## Current Status

Status: setup only.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V9-B report template and locked V9 scan manifest | `reports/v9-partB-report.md`, `reports/native-v9-scan-failures.txt`, `reports/native-v9-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json` | baseline: 0/5000 passed, 5000 failed, 0 skipped |

## Implemented Functionality

- None yet.

## Test Results and Delta Analysis

Initial V9 scan:

```text
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

Result: 5,000 total, 0 passed, 5,000 failed, 0 skipped, 0.00%.

Record focused B commands and future `--native-v9-scan` deltas here.

## Open Risks / Coordination Notes

- Coordinate `for...of` lowering with A group.
- Coordinate Map/Set/Iterator builtin storage and helper use with C group.
- Preserve V8 module runner behavior while adding job draining.
