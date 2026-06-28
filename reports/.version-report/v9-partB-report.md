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

Status: first runtime substrate pass complete.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V9-B report template and locked V9 scan manifest | `reports/v9-partB-report.md`, `reports/native-v9-scan-failures.txt`, `reports/native-v9-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json` | baseline: 0/5000 passed, 5000 failed, 0 skipped |
| 2026-06-24 | Codex / B group | Added minimal Promise records, FIFO native job queue, native `drain_jobs`, and array/string iterator runtime helpers | `src/runtime/job.rs`, `src/runtime/iterator.rs`, `src/runtime/mod.rs`, `src/runtime/context.rs`, `src/backend/native.rs`, `tests/native_v9_runtime.rs`, V9 docs | `cargo fmt --all -- --check`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_v9_runtime`; `cargo test --no-default-features --test native_test262` | Rust/runtime substrate: +5 focused tests. V9 scan not rerun because no JS-visible `Promise`/collection builtins were installed in this B-only step. |

## Implemented Functionality

- `PromiseId`, `PromiseState`, `PromiseRecord`, `PromiseJob`, and
  single-settle `create_promise` / `fulfill_promise` / `reject_promise`
  helpers.
- Deterministic FIFO `JobQueue` with `Job::PromiseReaction` and
  `Job::HostCallback`.
- Native backend `run_jobs()` now drains the B queue, and native script/module
  evaluation honors `ExecutionOptions::drain_jobs`.
- GC roots include fulfilled/rejected Promise values and queued Promise job
  values.
- `IteratorRecord` plus runtime helpers for array and string fallback
  iteration:
  `get_iterator`, `iterator_next`, and `iterator_close`.
- Focused Rust tests in `tests/native_v9_runtime.rs`.

## Test Results and Delta Analysis

Initial V9 scan:

```text
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

Result: 5,000 total, 0 passed, 5,000 failed, 0 skipped, 0.00%.

Record focused B commands and future `--native-v9-scan` deltas here.

First B substrate validation:

```text
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_v9_runtime
cargo test --no-default-features --test native_test262
```

Results:

- `native_v9_runtime`: 5 passed, 0 failed.
- `native_test262`: 13 passed, 0 failed.
- No V9 scan delta is claimed for this step. The work is intentionally
  runtime-only and does not install the JS-visible `Promise` constructor,
  `Map`/`Set`, or `Iterator` builtins.

## Open Risks / Coordination Notes

- Coordinate `for...of` lowering with A group.
- Coordinate Map/Set/Iterator builtin storage and helper use with C group.
- Preserve V8 module runner behavior while adding job draining.
- Generic `Symbol.iterator` dispatch is still open; current helper supports
  array and string fallback only.
- Promise reaction lists, then/catch/finally algorithms, and the global
  `Promise` builtin remain future B/C integration work.
