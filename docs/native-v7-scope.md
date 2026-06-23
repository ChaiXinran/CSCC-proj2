# Native V7 Scope: Stability, Limits, GC, and Performance Evidence

Native V7 turns the native backend from a feature-growing prototype into a
contest-ready runtime that can survive broad Test262 scans and sustained
benchmarks. V7 intentionally avoids new JavaScript syntax and large new
standard-library families. Its goal is stability, bounded resource use,
repeatable performance evidence, and better diagnostics for the remaining
conformance work.

Shared contracts are defined in
[Native V7 Shared Interface](native-v7-interface.md), and file ownership is
defined in [Native V7 Team Plan](native-v7-team-plan.md).

## 1. Baseline

The native backend has completed the V1-V6 feature milestones sufficiently to
support the curated gates and the core-builtin diagnostic scan. The immediate
engineering gaps exposed by broader scans are:

- `test/built-ins` can trigger very large host allocations;
- `test/language` can trigger native stack overflow;
- full-suite reporting needs crash isolation and resumable summaries;
- the heap has an object-count cap but not a full byte-budgeted GC contract;
- native script caching and performance evidence are not yet comparable to the
  Boa-backed baseline.

V7 treats those as product-quality blockers.

## 2. Delivery Stages

### V7.0 — Crash-Safe Reporting and Baselines

- Keep the ignored dashboard tests for full Test262 reporting.
- Ensure top-level and child-suite dashboards write incremental JSON summaries.
- Classify child suite status as completed, crashed, or timed out.
- Record per-suite totals, conformance percentage, crashed-suite count, and a
  capped failure sample file for focused suites.
- Establish starting baselines for:
  - `test`;
  - `test/built-ins`;
  - `test/language`;
  - the V6 diagnostic directories.

### V7.1 — Resource Budgets and Allocation Guards

- Add wall-clock deadline support to native evaluation.
- Add heap byte-budget configuration and accounting.
- Convert large string, array, JSON, and property-enumeration allocations into
  checked operations.
- Ensure stack overflow and dangerous recursion paths become `RuntimeLimit`
  failures instead of host aborts.
- Add direct runtime tests for loop, recursion, stack, heap object, heap byte,
  deadline, and large allocation limits.

### V7.2 — Non-Moving Mark-and-Sweep GC

- Implement explicit root discovery from the VM and `NativeContext`.
- Trace `JsValue`, objects, property descriptors, environments, functions,
  closures, bound functions, and pending exceptions.
- Sweep unreachable heap slots without moving live IDs.
- Trigger collection from allocation thresholds and explicit test hooks.
- Report collection statistics and verify no reachable object, prototype,
  closure binding, or environment is collected.

### V7.3 — Native Script Cache and Hot-Path Cleanup

- Implement isolate-local native parse/compile cache using
  `script_cache_capacity`.
- Prove cached chunks contain no context-local runtime IDs.
- Measure cold, warm uncached, and warm cached execution.
- Add small, semantics-preserving optimizations:
  - avoid repeated property-key allocation in hot paths;
  - avoid unnecessary UTF-16 conversion in String methods;
  - pre-size VM stacks from `Chunk::analyze_stack`;
  - preserve sparse-array semantics while improving dense-array common cases.

### V7.4 — Release Evidence Package

- Update benchmark reports with native results and same-machine references.
- Update status docs with full Test262 dashboard output, crashed-suite counts,
  and known remaining unsupported areas.
- Verify `--no-default-features` release builds do not link the Boa backend.
- Keep V1-V6 gates green.

## 3. Explicit Non-Goals

V7 does not include:

- new syntax such as modules, async functions, generators, decorators, or
  advanced class features;
- new standard-library families such as Map, Set, Promise, Proxy, TypedArray,
  Intl, Temporal, or Atomics;
- JIT compilation;
- moving or generational GC;
- full browser JetStream 2 coverage;
- silently skipping crashed or timed-out Test262 suites.

Bug fixes to existing V1-V6 features are allowed when they are necessary for
stability, limit handling, or benchmark correctness.

## 4. Test and Report Areas

V7 relies on layered validation rather than one giant in-process test:

| Layer | Purpose |
| --- | --- |
| `cargo test --test native_v7_runtime` | direct budget, heap, GC, and cache tests |
| top-level Test262 dashboard | crash-safe whole-suite visibility |
| child-suite dashboards | diagnose `built-ins` and `language` hotspots |
| failure-sample report | focused root-cause inspection without unbounded JSON |
| benchmark report | cold/warm/cache/JetStream evidence |

The lightweight V7 diagnostic scan is selected by `--native-v7-scan`. It is
intended for frontend/cache-safety smoke coverage over a few thousand files:

| Area | Purpose |
| --- | --- |
| `test/language/literals` | lexical and literal parsing stress |
| `test/language/types` | primitive type grammar and runtime front door |
| `test/language/block-scope` | scope-oriented parser/compiler coverage |
| `test/language/function-code` | function-body frontend coverage |
| `test/language/global-code` | global script frontend coverage |
| `test/built-ins/Function` | generic call/construct shapes |
| `test/built-ins/String` | string literal/coercion-heavy scripts |
| `test/built-ins/Symbol` | modern identifier/member/builtin shapes |
| `test/built-ins/Reflect` | reflective call/construct/property shapes |

Recommended command:

```powershell
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/native-v7-frontend-summary.json
```

The V7 pinned gate should contain only zero-failure, zero-skip regression tests
for newly stabilized engineering behavior. Broad dashboards are diagnostic and
must not count crashed, timed-out, or skipped suites as passes.

## 5. Completion Criteria

V7 is complete only when:

- V1-V6 pinned gates pass with no regressions;
- resource-limit failures are reported as `RuntimeLimit`, not host OOM, stack
  overflow, or process aborts;
- the heap exposes object and byte statistics;
- mark-and-sweep GC collects unreachable objects while preserving all reachable
  JS values and stable IDs;
- native script caching is measured and can be disabled with capacity `0`;
- full Test262 dashboards survive crashed child suites and produce valid JSON;
- `built-ins` and `language` child dashboards identify remaining hot spots;
- benchmark and status reports include native numbers, not only Boa-backed
  baselines;
- format, check, tests, Clippy, and no-default-features release builds pass.
