# Native V7 Team Plan

V7 is an engineering-hardening milestone. Work is divided to keep crash
reporting, runtime limits, GC, caching, and benchmarking from colliding in the
same files. Shared contracts in `native-v7-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v7-contracts
feat/v7-reporting
feat/v7-budgets
feat/v7-gc
feat/v7-cache
test/v7-benchmarks
```

Recommended merge order:

```text
V7 contracts
  -> crash-safe reporting baseline
  -> resource budget APIs
  -> GC tracing and sweeping
  -> native script cache and hot-path cleanup
  -> benchmark/status/readme integration
```

The reporting baseline merges before risky runtime work so later crashes are
visible instead of anecdotal.

## 2. A Group — Frontend and Source Safety

Owned files:

```text
src/lexer/
src/parser/
src/ast/
tests/frontend_v7.rs
```

Tasks:

- keep V7 free of new syntax requirements;
- add parser stress tests for deeply nested expressions, large literals, and
  syntax-error recovery paths that previously risked recursion overflow;
- ensure parse failures remain `SyntaxError`/`Unsupported`, not panics;
- verify cached parse results are independent of `NativeContext`.

A must not add benchmark-specific grammar shortcuts.

## 3. B Group — Compiler, Chunk, and Stack Contracts

Owned files:

```text
src/bytecode/
tests/bytecode_v7_contract.rs
tests/frontend_bytecode_v7.rs
```

Tasks:

- preserve `Chunk::analyze_stack` as the source of VM preallocation data;
- add regression tests for high stack-depth chunks and handler stack restore
  invariants;
- ensure compiled chunks contain no runtime heap IDs and are safe to cache;
- expose cache-safe metadata needed by the native script cache;
- reject invalid chunks before execution.

B must not add GC or limit behavior directly to opcodes unless the interface
document is updated first.

## 4. C Group — Runtime, VM, GC, and Cache

### C0 — Budgets and Allocation Guards

Owned files:

```text
src/engine.rs
src/backend/native.rs
src/runtime/context.rs
src/vm/
src/builtins/string.rs
src/builtins/array.rs
src/builtins/json.rs
tests/native_v7_runtime.rs
```

Tasks:

- add wall-clock deadline and heap byte-budget configuration;
- classify all resource exhaustion as `RuntimeLimit`;
- guard large String, Array, JSON, and enumeration allocations;
- add VM budget checks at loop, call, stack, and native builtin loop
  boundaries.

### C1 — Mark-and-Sweep GC

Owned files:

```text
src/runtime/gc.rs
src/runtime/heap.rs
src/runtime/object.rs
src/runtime/environment.rs
src/runtime/function.rs
src/runtime/value.rs
src/vm/interpreter.rs
tests/native_v7_gc.rs
```

Tasks:

- implement root snapshots from `NativeContext` and `Vm`;
- implement tracing for all heap-referencing runtime structures;
- sweep unreachable object, environment, and function slots without moving IDs;
- expose collection statistics;
- prove prototypes, closures, bound functions, symbol properties, accessors,
  and pending exceptions remain reachable.

### C2 — Native Script Cache and Hot Paths

Owned files:

```text
src/backend/native.rs
src/bytecode/chunk.rs
src/vm/
src/runtime/property_map.rs
tests/native_v7_cache.rs
```

Tasks:

- implement isolate-local LRU parse/compile cache;
- key cached chunks by source hash and strictness;
- verify capacity `0` disables caching;
- measure cache hit/miss counts in benchmarks;
- make only semantics-preserving hot-path improvements.

## 5. D Group — Test262, Benchmarks, CI, and Documentation

Owned files:

```text
src/test262.rs
src/main.rs
tests/native_full_test262_by_dir.rs
tests/native_test262.rs
docs/benchmark.md
docs/status.md
readme.md
reports/
.github/workflows/ci.yml
scripts/
```

Tasks:

- maintain the top-level and child-suite Test262 dashboards;
- add timeouts and crashed-suite classification where the CLI runner needs it;
- add a V7 pinned gate only after zero-failure, zero-skip candidates are
  verified;
- record full-suite, `built-ins`, and `language` baselines;
- update benchmark reports with native, Boa, QuickJS, and Node/V8 references
  where applicable;
- add CI jobs for no-default-features native builds and V1-V7 regression gates.

D must not hide crashed suites by excluding them from totals without recording
their status.

## 6. Shared-File Lock

| File | Owner |
| --- | --- |
| `src/engine.rs` | C0 |
| `src/backend/native.rs` | C0/C2, coordinated by D for CLI-facing behavior |
| `src/runtime/heap.rs` | C1 |
| `src/runtime/gc.rs` | C1 |
| `src/runtime/context.rs` | C0/C1 shared, interface changes first |
| `src/vm/interpreter.rs` | C0/C1 |
| `src/bytecode/chunk.rs` | B/C2 |
| `src/test262.rs`, `src/main.rs` | D |
| `tests/native_full_test262_by_dir.rs` | D |
| `docs/native-v7-*.md` | D after contracts merge |

If two groups need the same file, the interface document is updated first and a
small connector branch merges before feature branches continue.

## 7. Independent Validation

```text
A: source -> tokens/AST under deep or large inputs
B: hand-built AST/chunk -> validation and cache-safe metadata
C0: direct runtime budget and allocation-limit tests
C1: direct heap/GC root and sweep tests
C2: repeated native execution with cache enabled/disabled
D: child-process Test262 dashboards and benchmark reports
```

Each branch runs:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --release --no-default-features --test native_full_test262_by_dir --no-run
```

## 8. Merge Gate

Before V7 integration is considered complete:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release --no-default-features
cargo run --release --no-default-features -- test262 --native-v1 --jobs 1
cargo run --release --no-default-features -- test262 --native-v2 --jobs 1
cargo run --release --no-default-features -- test262 --native-v3 --jobs 1
cargo run --release --no-default-features -- test262 --native-v4 --jobs 1
cargo run --release --no-default-features -- test262 --native-v5 --jobs 1
cargo run --release --no-default-features -- test262 --native-v6 --jobs 1
cargo run --release --no-default-features -- test262 --native-v7 --jobs 1
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/native-v7-frontend-summary.json
```

Recommended dashboard runs:

```powershell
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/built-ins"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/language"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

Dashboard environment variables:

- `AGENTJS_TEST262_JOBS`: worker count, defaults to `4`;
- `AGENTJS_TEST262_SUITE`: child suite used by child-dashboard and failure
  sample modes;
- `AGENTJS_TEST262_REPORT`: output JSON path;
- `AGENTJS_TEST262_SUITE_TIMEOUT_SECS`: per-child-suite dashboard timeout,
  defaults to `300`;
- `AGENTJS_TEST262_SAMPLE_LIMIT`: failure sample cap.

The dashboard percentages are diagnostic. V7 completion depends on stable,
truthful reporting and bounded execution, not pretending unsupported tests pass.
