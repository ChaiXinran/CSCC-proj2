# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

AgentJS is a lightweight JavaScript engine in Rust (crate `agentjs`, edition 2024, rust 1.91+) targeting short-lived, high-frequency AI-agent workloads. It ships a working **Boa-backed compatibility runtime today** while a **self-developed native engine** (lexer → parser → bytecode → VM → runtime → builtins) is built in parallel.

The contest release gate is the *native* engine reaching >60% Test262 conformance — **not** a wrapper around Boa. Boa is the behavior oracle and executable baseline; QuickJS is an architecture/perf reference. See [readme.md](readme.md), [AGENTS.md](AGENTS.md), and [docs/architecture.md](docs/architecture.md) for the authoritative narrative.

## Critical constraints

- **Never edit `boa/`, `quickjs/`, or `test262/`.** They are pinned git submodules used as backend/reference/conformance. Keep all project code at the repo root. Record any submodule revision bump in [docs/dependencies.md](docs/dependencies.md).
- **Both Boa and the native engine live in this same crate.** Boa is reached *only* through `src/backend/boa.rs`; `engine.rs` and `contracts.rs` must stay free of Boa imports. Don't leak Boa types past the backend boundary.
- **`BackendKind::Native` executes the self-developed V1–V6 pipeline end-to-end.** The CLI uses `BackendKind::Boa` for production commands; `--native-vN` flags route Test262 through the native path. Don't silently fall back to Boa to make something "pass" — document unsupported behavior instead.
- **`src/contracts.rs` is a reviewed cross-team boundary.** It re-exports the shared types (`Token`, `Program`, `Chunk`, `Instruction`, `JsValue`) and the `SourceParser` / `ProgramCompiler` / `ChunkExecutor` / `NativePipeline` traits. Changing these requires team review; keep implementation details inside their owning `src/<module>/` directory.
- **Respect one-directional module flow:** `lexer → ast/parser → bytecode → vm → runtime/builtins`. `backend/native.rs` only *assembles* stages — it must contain no lexer/parser/VM/object-model logic.

## Architecture map (`src/`)

- `engine.rs` — backend-neutral `Engine` (fresh isolate per unrelated execution) and `Runtime` (one persistent isolate, e.g. REPL), plus `RuntimeConfig`, errors, reports. No Boa.
- `contracts.rs` — the stable native-engine collaboration boundary (see above).
- `backend/mod.rs` — `BackendKind` and the internal `RuntimeBackend` trait used by CLI + Test262.
- `backend/boa.rs` — the complete Boa compatibility implementation: context creation, host functions, limits, script caching, jobs, error conversion.
- `backend/native.rs` — entry point for the self-developed engine; V1–V6 are live end-to-end without Boa fallback.
- `lexer/`, `ast/`, `parser/` — native front end. `bytecode/` — compiler/chunk/opcodes. `vm/` — interpreter/frames. `runtime/` — values, objects, environments, heap, GC. `builtins/` — Object/Function/Array etc., no host APIs exposed.
- `test262.rs` — parallel Test262 discovery/execution with strict+non-strict variants, harness includes, negative/async handling, `$262`, per-case panic isolation, JSON summaries.

Current state: V1–V5 (front end, bytecode, VM, object model, exceptions/lexical scope) are live and gated; V6 (core builtins — String, Number, Math, Boolean, Error, JSON) is live with fixed gates plus an ongoing diagnostic scan. The fixed Native V1–V6 gates pass 69 curated Test262 files with no failures or skips — these are regression checks, not a conformance percentage. Each version has a `docs/native-vN-scope.md` and `docs/native-vN-interface.md` — the interface files are read-only; changes require review before any implementation PR merges. The version development checklist lives in [docs/version-development-workflow.md](docs/version-development-workflow.md).

Planning notes and test strategy rationale live in `thoughts/` (not authoritative, but useful context).

Host surface is deliberately tiny: `print` + a frozen `console` facade. No filesystem/process/network. Runtime limits bound loops, recursion, VM stack, and backtrace size. A bounded per-isolate LRU reuses parsed/compiled scripts without sharing mutable globals across isolates.

The default `conformance` Cargo feature pulls in larger Intl/Temporal/experimental Boa components; disabling default features yields the smaller agent binary with the same isolation API.

## Commands

Project gate (run before any merge):

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
```

Run a single test: `cargo test <name>` (add `-- --nocapture` for output; `cargo test --test <file>` for a specific integration test under `tests/`).

After resolving merge conflicts, verify no stale markers remain: `rg "^(<<<<<<<|=======|>>>>>>>)" src tests`

CLI (all currently Boa-backed):

```sh
cargo run -- eval "1 + 2"
cargo run -- run examples/hello.js
cargo run -- repl
cargo build --release
```

Test262 — start with the feature directory affected by a change, not the full suite:

```sh
cargo run --release -- test262 --root test262 --suite test/language/expressions --limit 100 --jobs 8 --verbose
# Scan native conformance for a directory (does not add to fixed gates):
cargo run --release -- test262 --native-v6-scan --jobs 4 --progress
```

Windows focused run: `.\scripts\test262-sample.ps1 -Suite test/language/expressions -Limit 100`

Benchmarks: `cargo run --release -- bench 1000` (compares cold isolates, warm uncached, warm cached). JetStream2 lives under `benchmarks/`, `scripts/`, [docs/benchmark.md](docs/benchmark.md), and [reports/](reports/).

## Testing approach

Put normal / boundary / error-path unit tests beside each module; broader behavior tests under `tests/`. Because stages are decoupled through `contracts.rs`, you can test one stage with fakes for the others via `NativePipeline::from_stages(...)` — parser tests without a compiler/VM, VM tests with hand-built `Chunk`s, etc. Boa differential tests compare values/output/error categories, but the ECMAScript spec and Test262 are authoritative when behavior differs.

**Never count skipped Test262 cases as passes.** Behavior-affecting PRs should report newly passed / newly failed / skipped / regressed counts. Existing Test262 (45,310/47,516) and benchmark numbers are **Boa-backed baselines** — native results must be reported separately. Current status and gaps: [docs/status.md](docs/status.md).

## Team structure

Four groups own different stages. When adding or changing code, stay within the owning group's boundary:

- **A — Frontend** (`lexer/`, `ast/`, `parser/`): tokens, AST nodes, parser — no compiler or runtime imports.
- **B — Compiler** (`bytecode/`): opcode definitions, chunk emission — depends on AST, not on VM internals.
- **C — VM/Runtime/Builtins** (`vm/`, `runtime/`, `builtins/`): interpreter, object model, property descriptors, built-in functions.
- **D — Integration** (`test262.rs`, `tests/`): `NATIVE_VN_TESTS`, `--native-vN` flags, Test262 scanning and reporting.

Shared contracts (AST node shapes, `Instruction` enum, `PropertyDescriptor`, `JsValue`, builtin signatures) are defined in `docs/native-vN-interface.md` and `src/contracts.rs`. Contract changes must be merged before any dependent implementation PRs.

### Native engine test gates

Run all versioned gates before merging anything that touches native stages:

```sh
cargo run --release -- test262 --native-v1 --jobs 1
cargo run --release -- test262 --native-v2 --jobs 1
cargo run --release -- test262 --native-v3 --jobs 1
cargo run --release -- test262 --native-v4 --jobs 1
cargo run --release -- test262 --native-v5 --jobs 1
cargo run --release -- test262 --native-v6 --jobs 1
```

All versioned gates must stay at zero regressions and zero new skips.

## Commits

Imperative subject, scope when helpful (e.g. `feat(lexer): tokenize numeric literals`, `fix(vm): preserve operands across calls`). Keep commits scoped to one module + any necessary shared-contract change; don't mix unrelated upstream-submodule changes. Merge shared-contract changes before dependent branches.
