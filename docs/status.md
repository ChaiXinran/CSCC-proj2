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

- Test262 module tests now have a first-stage native module path. The focused
  `test/language/module-code` run reports 201/599 passed, 398 failed, and 0
  skipped; the standard V8 scan reports 205/5,000 passed, 4,795 failed, and 0
  skipped. Remaining failures are mostly import/export parser and linking gaps.
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
- A direct full `test/` scan, including `test/staging`, now completes and writes
  `reports/native-full-test262-summary.json`. The 2026-06-24 run passed 14,035
  of 53,379 tests, failed 38,507, skipped 837, and reported 26.29%
  conformance for that exact stress command. Failure classification is recorded
  in `reports/test262-analysis.md`; the short summary is in
  `reports/test262-report.md`.
- The dominant remaining gaps are parser/modern syntax, template literal
  substitutions, missing builtin/global families, module execution, and RegExp
  feature coverage.
- Native V8 workflow setup has started. The V8 scope, shared interface, and team
  plan are recorded in `docs/native-v8-scope.md`,
  `docs/native-v8-interface.md`, and `docs/native-v8-team-plan.md`.
- V8-B module runner infrastructure has a first-stage implementation:
  `SourceKind::Module`, native module eval, strict module execution, module
  top-level `this === undefined`, module registry/status records, relative
  dependency loading, duplicate evaluation guard, and module-mode Test262
  labels. Focused command:
  `cargo run --release --no-default-features -- test262 --backend native --root
  test262 --suite test/language/module-code --jobs 4 --progress --json
  reports/native-v8-b-module-summary.json`.
- V8 worker progress is tracked in `reports/v8-partA-report.md`,
  `reports/v8-partB-report.md`, and `reports/v8-partC-report.md`. Workers and
  AI agents should update the relevant report whenever they change that track.
- The standard V8 lightweight integration command is
  `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4
  --json reports/native-v8-scan-summary.json`. It runs the locked 5,000-case
  manifest in `reports/native-v8-scan-failures.txt`. The initial summary was
  0/5,000 passed, 4,504 failed, and 496 skipped; after V8-B first-stage module
  runner work it is 205/5,000 passed, 4,795 failed, and 0 skipped.
- The version workflow now requires every future version to create per-track
  worker reports and a `--native-vN-scan` 5,000-case prior-failure manifest
  before implementation starts. See `docs/version-development-workflow.md`.
- Native V9 setup has started. Scope, interface, team plan, per-track reports,
  and the locked V9 lightweight scan are recorded in
  `docs/native-v9-scope.md`, `docs/native-v9-interface.md`,
  `docs/native-v9-team-plan.md`, `reports/v9-partA-report.md`,
  `reports/v9-partB-report.md`, `reports/v9-partC-report.md`, and
  `reports/native-v9-scan-failures.txt`. Initial V9 scan:
  0/5,000 passed, 5,000 failed, 0 skipped.
- V9-B first runtime substrate pass is implemented: minimal Promise records,
  FIFO native job queue, native `run_jobs()` draining, and array/string iterator
  fallback helpers. Focused `tests/native_v9_runtime.rs` passes 5/5; JS-visible
  Promise/collection builtin installation is still future V9-C/integration
  work.
- Native V10 setup is complete. Scope, interface, team plan, per-track reports,
  and the locked V10 lightweight scan are recorded in
  `docs/native-v10-scope.md`, `docs/native-v10-interface.md`,
  `docs/native-v10-team-plan.md`, `reports/v10-partA-report.md`,
  `reports/v10-partB-report.md`, `reports/v10-partC-report.md`, and
  `reports/native-v10-scan-failures.txt`. Initial V10 scan:
  645/5,000 passed, 4,355 failed, 0 skipped.
- V10-B first runtime substrate pass is implemented: shared ArrayBuffer byte
  storage, typed-array view records, DataView records, detach/range checks, and
  Number-backed element load/store helpers. Focused `tests/native_v10_runtime.rs`
  passes 6/6; JS-visible TypedArray/ArrayBuffer/DataView constructor migration
  is still future V10-C/integration work.
- Native V11 setup is complete. Scope, interface, team plan, per-track reports,
  and the locked V11 lightweight scan are recorded in
  `docs/native-v11-scope.md`, `docs/native-v11-interface.md`,
  `docs/native-v11-team-plan.md`, `reports/v11-partA-report.md`,
  `reports/v11-partB-report.md`, `reports/v11-partC-report.md`, and
  `reports/native-v11-scan-failures.txt`. The selector is installed and
  `tests/native_test262.rs` passes 15/15. The first local V11 scan attempt
  exceeded the 300s tool timeout and did not produce a JSON summary.

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

V7 engineering milestone is complete. The full direct Test262 command now
produces a truthful JSON report. The next work is feature development guided by
`reports/test262-analysis.md`: frontend unlockers, module runner, builtin/global
families, and semantic precision.

CI is defined in `.github/workflows/ci.yml`. It includes default-feature
quality checks, no-default-features native checks, V7 focused contracts, the
fixed Native V1-V6 Test262 gates plus V7 scan selector tests, dashboard
compilation, and a small V7 diagnostic-scan JSON smoke. A local focused run is
available through `scripts/test262-sample.ps1`.
