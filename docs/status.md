# Implementation Status

## Implemented

- Cross-platform Rust binary layout.
- Script evaluation, file execution, and persistent REPL.
- Fresh-isolate execution for independent agent actions.
- Captured `print`/`console` output.
- Loop, recursion, VM stack, backtrace, heap-object, heap-byte, and wall-clock
  limits — all categorised as `VmErrorKind::RuntimeLimit`.
- Per-isolate LRU caching of parsed and compiled scripts (native and Boa).
- Parallel Test262 discovery and execution.
- Test262 harness includes, strict variants, negative tests, async completion,
  `$262` host support, filtering, result limits, and JSON summaries.
- Cold-isolate and warm-runtime microbenchmark command (`bench`, `bench --native`).
- JetStream 2.0 CLI adapter and a pinned six-workload performance report.
- Backend-neutral `Engine`/`Runtime` facade with isolated Boa and native
  backend modules.
- **V7 — Stability, Limits, GC, and Performance Evidence** (completed):
  - `ExecutionBudget` with `check_loop`, `check_call_depth`, `check_stack_depth`,
    `check_deadline` called at all VM boundaries.
  - Non-moving mark-and-sweep GC via `Collector`/`Trace` with explicit `RootSet`
    root discovery; sweeping preserves stable object/function/environment IDs.
  - Parser depth limit (`MAX_PARSE_DEPTH = 50`) converts stack-overflow inputs
    into `ParseError` before the Rust call stack is exhausted.
  - Large allocation guards (`checked_string_repeat_len`, `checked_array_length`,
    `checked_utf16_allocation`) applied to all builtins that size from JS values.
  - Crash-safe Test262 dashboard (`native_full_test262_by_dir.rs`) with
    per-suite `SuiteStatus` (`Completed`, `Crashed`) and incremental JSON output.
    Set `AGENTJS_TEST262_SUITE_TIMEOUT_SECS` to adjust the per-child-suite timeout.
  - `--native-v7` pinned integration gate runs the native backend over the
    zero-failure, zero-skip V1-V6 Test262 files to catch regressions after V7
    stability, GC, cache, and reporting work.
  - `--native-v7-scan` diagnostic gate over selected language/builtin directories.
  - CI updated to V7: no-default-features quality checks, V7 contract test matrix
    (with and without default features), `--native-v7` CLI gate, dashboard
    compilation check, and V7 diagnostic-scan JSON smoke.

## Known Gaps

- Test262 module tests are currently skipped.
- YAML frontmatter parsing supports only the Test262 fields consumed by the
  runner, not a general YAML parser.
- Boa remains the compatibility baseline for the `eval`/`run`/`repl` CLI
  commands. `BackendKind::Native` executes the self-developed V1-V6 lexer,
  parser, bytecode, VM, runtime, and builtin path without falling back to Boa.
- The fixed Native V1-V6 gates pass 69 official Test262 files with no failures
  or skips. These curated gates are regression checks, not a full conformance
  percentage.
- After merging Tracks A, B, and C, the Native V6 core builtin scan passes
  1,499 of 2,199 selected String, Number, Math, Boolean, Error, and JSON tests,
  with 1 explicitly skipped.
- A sharded Test262 run on revision `de8e621c` executed 47,516 non-staging
  tests and passed 45,310. Treating every unexecuted non-staging test as a
  failure gives a conservative full-suite lower bound of 87.31%. See
  `reports/test262-report.md`.
- V7 bytecode groundwork exposes recursive `ChunkCacheMetadata`, rejects
  invalid chunks before interpretation, and covers high stack depth, handler
  restore invariants, nested-function validation, and source-to-cache-metadata
  lowering.
- V7 runtime/GC/cache focused tests cover wall-clock and allocation limits,
  stack-budget rejection, non-moving GC preservation/collection behavior, and
  native script-cache hit/miss and capacity-zero behavior.
- The full Test262 dashboard helper now runs child suites in separate
  processes, writes incremental JSON reports, and classifies completed,
  crashed, and timed-out child suites. Set
  `AGENTJS_TEST262_SUITE_TIMEOUT_SECS` to adjust the per-child-suite timeout.
- `--native-v7` is now the pinned V7 integration gate. It runs the native
  backend over the zero-failure, zero-skip V1-V6 Test262 files to catch
  regressions after V7 stability, GC, cache, and reporting work.
- `--native-v7-scan` selects a lightweight frontend/cache-safety Test262 sample
  of selected language and builtin directories. The recommended local command
  is `cargo run --release --no-default-features -- test262 --native-v7-scan
  --jobs 4 --json reports/native-v7-frontend-summary.json`.
  Current V7 scan results and failure classification are recorded in
  `reports/native-v7-test262-report.md`.

## Acceptance Gates

Before claiming contest readiness:

1. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass on Linux,
   macOS, and Windows. ✅ CI enforces this on `ubuntu-latest` and `windows-latest`.
2. Replace the conservative sharded result with a timeout-safe monolithic
   pinned Test262 run; the current verified lower bound already exceeds 60%.
3. JetStream 2 and project microbenchmarks are compared with native results.
   JetStream 2 report in `reports/jetstream2-report.md`; native microbenchmark
   via `cargo run --release --no-default-features -- bench --native`.
4. Binary size, cold-start latency, and warm throughput are reported with
   native release build numbers.
5. The script cache is measured against an uncached baseline. Cache hit/miss
   counts are reported by `bench --native`.

V7 engineering milestone is complete. The remaining gap before contest readiness
is item 2: a full-suite, crash-safe, wall-clock-bounded Test262 run that
produces a truthful JSON report with crashed-suite counts.

CI is defined in `.github/workflows/ci.yml`. It includes default-feature
quality checks, no-default-features native checks, V7 focused contracts, the
fixed Native V1-V6 Test262 gates plus V7 scan selector tests, dashboard
compilation, and a small V7 diagnostic-scan JSON smoke. A local focused run is
available through `scripts/test262-sample.ps1`.
