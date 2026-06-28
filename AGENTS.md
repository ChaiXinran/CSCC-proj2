# AGENTS.md — AgentJS

## Quick-start commands

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

### Focused checks

```sh
cargo test <name>                    # single test (add -- --nocapture for output)
cargo test --test <file>             # integration test under tests/
cargo test --no-default-features     # native-only (no Boa dep)
cargo test --no-default-features --test native_test262  # Test262 gates

# CLI (currently Boa-backed for production, native via --native-vN flags)
# Build (use --no-default-features to skip Boa, much faster)
cargo build --release                       # full build with Boa (slow)
cargo build --release --no-default-features # native-only (fast)

# CLI (currently Boa-backed for production, native via --native-vN flags)
cargo run -- eval "1 + 2"
cargo run -- run examples/hello.js
cargo run -- repl
cargo run -- bench 1000              # benchmark comparison
```

Run `fmt → check → test → clippy` in that order before merging.

## Critical constraints

- **Never edit `boa/`, `quickjs/`, `test262/`** — pinned git submodules.
- **`cli/src/backend/boa.rs` is the only file that imports `boa_*`**. `engine.rs` and `contracts.rs` must not import Boa.
- **One-directional module flow:** `lexer → ast/parser → bytecode → vm → runtime/builtins`. Never import downstream modules upstream. `backend/native.rs` only assembles stages — no stage logic.
- **`src/contracts.rs`** re-exports shared types (`Token`, `Program`, `Chunk`, `Instruction`, `JsValue`) and traits (`SourceParser`, `ProgramCompiler`, `ChunkExecutor`, `NativePipeline`). Changing them requires team review.
- **Never silently fall back to Boa** when native fails — document unsupported behavior.
- **`BackendKind::Boa`** is the default; **`BackendKind::Native`** is the self-developed pipeline. `--native-vN` flags route Test262 through native.

## Architecture

```
src/
├── contracts.rs       # cross-team API boundary
├── engine.rs          # backend-neutral Engine/Runtime (no Boa imports)
├── lib.rs / main.rs
├── test262.rs         # Test262 discovery/execution, scan selectors
├── lexer/ ast/ parser/ bytecode/ vm/ runtime/ builtins/
└── backend/
    ├── boa.rs          # Boa compatibility baseline (only Boa-import file)
    └── native.rs       # stage assembly (no stage logic)
```

## Versioned native gates

Each V1–V7 gate is a **curated zero-failure, zero-skip** regression set. V8+ have
**diagnostic scans** (5,000-case manifest, not zero-failure):

```sh
# Fixed gates (run before merging native changes)
cargo run --release --no-default-features -- test262 --native-vN --jobs 1
#   N = 1..7

# Lightweight scans (V8–V12, 5,000-case manifest from prior failures)
cargo run --release --no-default-features -- test262 --native-vN-scan --jobs 4 --json reports/native-vN-scan-summary.json
#   N = 8..12
```

Scan manifests: `reports/native-vN-scan-failures.txt`. Summary JSON is tracked (whitelisted in `.gitignore`).

## Version docs pattern

Each version has docs in `docs/`:
- `native-vN-scope.md` — scope
- `native-vN-interface.md` — shared interface (**read-only**; changes require review)
- `native-vN-team-plan.md` — team plan

From V8 onward each track keeps a report: `reports/vN-part{A,B,C}-report.md`. Update the relevant report in the same change as implementation.

## Testing approach

- Unit tests beside each module. Broader integration tests under `tests/`.
- Decoupled stages via `NativePipeline::from_stages(...)` — test one stage with fakes for others.
- Never count skipped Test262 as passes. Report newly-passed/failed/skipped/regressed with behavior-affecting PRs.
- Boa-backed numbers (README baseline) and native numbers are separate — never conflate them.

## `src/contracts.rs` — stable boundary

Import cross-team types/traits through `agentjs::contracts::{}`. Keep implementations in their owning `src/<module>/`.

## Host surface

Tiny: `print` + frozen `console` facade. No filesystem/process/network. Runtime limits bound loops, recursion, VM stack, and backtrace size. LRU cache reuses parsed/compiled scripts per isolate without sharing mutable globals.

## Commit style

Imperative subject, scope when helpful (`feat(lexer): ...`, `fix(vm): ...`, `test(parser): ...`). Keep scoped to one module + necessary shared-contract change. Merge shared-contract changes before dependent branches.

## After resolving merge conflicts

```sh
rg "^(<<<<<<<|=======|>>>>>>>)" src tests
```
