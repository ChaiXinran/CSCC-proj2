# V8 Part A Report — Frontend Unlockers

Owner: A group
Scope: lexer / parser / AST / bytecode compiler

This report must be updated by any worker or AI agent that changes V8-A code.
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
| Parser syntax gap | 16,259 |
| Template literal substitutions unsupported | 5,307 |
| Lexer/static syntax gap | 530 |
| V8 scan total | 5,000 |
| V8 scan passed | 0 |
| V8 scan failed | 4,504 |
| V8 scan skipped | 496 |

Primary directories:

- `test/language`
- `test/built-ins/String`
- class, template, spread/rest, and destructuring related language directories

## Current Status

Status: not started.

## Change Log

Add entries newest first.

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Recorded shared V8 scan baseline | `reports/native-v8-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | 0/5000 passed, 4504 failed, 496 skipped |
| 2026-06-24 | setup | Created report template | `reports/v8-partA-report.md` | not run | baseline recorded |

## Implemented Functionality

- None yet.

## Test Results and Delta Analysis

When updating this section, compare against the locked baseline above and record:

- command;
- total / passed / failed / skipped;
- pass-rate delta;
- relevant failure-class delta, especially parser/template/lexer classes;
- newly passing directories;
- regressions.

## Open Risks / Coordination Notes

- Coordinate any AST shape or opcode changes with B group before implementation.
- Do not implement builtin skeletons in this track.
- Tagged templates, complete `super`, and deep destructuring may be deferred if
  ordinary templates/classes/spread are unblocked first.
