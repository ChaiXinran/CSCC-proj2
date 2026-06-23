# Implementation Status

## Implemented

- Cross-platform Rust binary layout.
- Script evaluation, file execution, and persistent REPL.
- Fresh-isolate execution for independent agent actions.
- Captured `print`/`console` output.
- Loop, recursion, VM stack, and backtrace limits.
- Per-isolate LRU caching of parsed and compiled scripts.
- Parallel Test262 discovery and execution.
- Test262 harness includes, strict variants, negative tests, async completion,
  `$262` host support, filtering, result limits, and JSON summaries.
- Cold-isolate and warm-runtime microbenchmark command.
- JetStream 2.0 CLI adapter and a pinned six-workload performance report.
- Backend-neutral `Engine`/`Runtime` facade with isolated Boa and native
  backend modules.

## Known Gaps

- Test262 module tests are currently skipped.
- YAML frontmatter parsing intentionally supports the Test262 fields consumed
  by the runner rather than implementing a general YAML parser.
- Runtime limits do not yet include a hard heap-byte budget or wall-clock
  preemption.
- Boa remains the compatibility baseline, but `BackendKind::Native` now
  executes the self-developed V1-V5 lexer, parser, bytecode, VM, runtime, and
  builtin path without falling back to Boa.
- The fixed Native V1-V6 gates pass 69 official Test262 files with no failures
  or skips. These curated gates are regression checks, not a full conformance
  percentage.
- The completed V5 diagnostic scan passes 191 of 593 selected try, switch,
  let, and const tests; unsupported and failed cases remain separately
  reported.
- After merging Tracks A, B, and C, the Native V6 core builtin scan passes
  1,499 of 2,199 selected String, Number, Math, Boolean, Error, and JSON tests,
  with 1 explicitly skipped. Track C compound assignments added 10 passes over
  the A+B baseline without regressions.
- A sharded Test262 run on revision `de8e621c` executed 47,516 non-staging
  tests and passed 45,310. Treating every unexecuted non-staging test as a
  failure gives a conservative full-suite lower bound of 87.31%. See
  `reports/test262-report.md`.
- Native V7 planning is frozen in `docs/native-v7-scope.md`,
  `docs/native-v7-interface.md`, and `docs/native-v7-team-plan.md`. V7 targets
  the remaining engineering gaps above: hard byte budgets, cooperative
  deadlines, non-moving GC, crash-safe Test262 dashboards, native script
  caching, and native benchmark evidence.

## Acceptance Gates

Before claiming contest readiness:

1. `cargo test` and `cargo clippy --all-targets -- -D warnings` pass on Linux,
   macOS, and Windows.
2. Replace the conservative sharded result with a timeout-safe monolithic
   pinned Test262 run; the current verified lower bound already exceeds 60%.
3. JetStream 2 and project microbenchmarks are compared with Boa and QuickJS.
4. Binary size, cold-start latency, peak RSS, and warm throughput are reported.
5. The script cache and subsequent native optimizations are measured against
   an uncached baseline.

V7 is the next planned milestone for items 2-5: it is considered complete only
when broad native scans produce truthful crash-safe reports, resource
exhaustion is categorized as `RuntimeLimit`, and benchmark reports use native
release builds rather than Boa-backed baselines.

CI is defined in `.github/workflows/ci.yml`. A local focused run is available
through `scripts/test262-sample.ps1`.
