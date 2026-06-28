# AGENTS.md — AgentJS

## Quick-start commands

```sh
# Project gate (run before any merge, in this order)
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings

# Also run this if touching native stages
cargo test --no-default-features --test native_test262

# Single test
cargo test <name>
cargo test --test <filename>         # integration test in tests/

# CLI eval / run / repl (currently Boa-backed)
cargo run -- eval "1 + 2"
cargo run -- run examples/hello.js
cargo run -- repl
```

## Fixed native gates (zero-failure, zero-skip — run before merging native changes)

```sh
cargo run --release --no-default-features -- test262 --native-v1 --jobs 1
cargo run --release --no-default-features -- test262 --native-v2 --jobs 1
# ...through --native-v7
```

Each version gate runs a curated set of Test262 files. The `-scan` variants (e.g. `--native-v7-scan`) run broader diagnostic directories. All `--native-vN-scan` flags are in `src/main.rs:172-186` and selectors are in `src/test262.rs`.

## Lightweight scan commands (default integration check after focused work)

```sh
# V8: 5,000-case manifest from prior non-passing cases
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
# V9–V11: same pattern, different manifest file
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4  --json reports/native-v9-scan-summary.json
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

The `reports/native-vN-scan-failures.txt` manifest files are locked for each version. Scan summary JSONs are tracked (`.gitignore` whitelists `reports/native-v*-scan-summary.json`). The V11 scan exceeded the 300s tool timeout on first run — future V11 workers should use a longer timeout or replace slow cases.

## Architecture & boundaries

```
src/
├── contracts.rs       # Reviewed cross-team API: re-exports Token, Program, Chunk, Instruction,
│                      #   JsValue, SourceParser, ProgramCompiler, ChunkExecutor, NativePipeline
├── engine.rs          # Backend-neutral Engine/Runtime (no Boa imports)
├── lib.rs / main.rs   # Library + CLI
├── test262.rs         # Test262 discovery/execution, NATIVE_VN_TESTS constants, scan selectors
├── lexer/ ast/ parser/ bytecode/ vm/ runtime/ builtins/
│   (native engine: lexer → ast/parser → bytecode → VM → runtime + builtins)
└── backend/
    ├── boa.rs          # Boa compatibility baseline (only file that imports boa)
    └── native.rs       # Assembles native stages; contains no stage logic
```

**One-directional dependency flow:** `lexer -> parser -> bytecode -> vm -> runtime/builtins`. Never import a downstream module upstream.

**`BackendKind::Boa`** is the default for production CLI. **`BackendKind::Native`** is the self-developed pipeline. `--native-vN` flags route Test262 through native. Never silently fall back to Boa to make tests "pass" — document unsupported behavior instead.

## Three submodule trees (never modify directly)

| tree | purpose |
|------|---------|
| `boa/` | Rust ECMAScript engine — behavior oracle, not final engine |
| `quickjs/` | C engine — architecture/performance reference |
| `test262/` | ECMAScript conformance suite |

All are pinned git submodules. Keep project code at root. Bump submodule revisions in `docs/dependencies.md` only.

## Cargo feature flags

- `default = ["boa-backend"]` — the Boa compatibility build.
- `booa-backend` — pulls in `boa_engine` + `boa_runtime` from `boa/core/engine` + `boa/core/runtime` (local paths).
- `conformance` — adds `intl_bundled`, `temporal`, `experimental` for full Test262.
- Building with `--no-default-features` produces a smaller native-only binary (no Boa dep).

## Versioned development pattern (V8–present)

Each version (V8, V9, V10, V11) uses a three-track parallel batch:

| Track | Purpose | Typical directories |
|-------|---------|-------------------|
| A | Frontend / syntax / lowering | `lexer/`, `parser/`, `ast/`, `bytecode/` |
| B | Runtime substrate / protocol | `runtime/`, `vm/`, job queue, iterator, module |
| C | Builtins / JS-visible algorithms | `builtins/`, `src/test262.rs`, reports |

Each track has a mandatory report file (`reports/vN-partA-report.md`, etc.) that must be updated in the **same change** as any code, test, or docs modification. Reports include what changed, files touched, commands run, result deltas against the locked baseline, newly exposed failures, and coordination notes for other tracks.

Each version also has:
- `docs/native-vN-scope.md` — scope doc
- `docs/native-vN-interface.md` — shared interface (**read-only**; changes require review)
- `docs/native-vN-team-plan.md` — team plan
- `reports/native-vN-scan-failures.txt` — 5,000-case manifest for lightweight scan
- `reports/native-vN-scan-summary.json` — initial baseline scan result

Setup for a new version is documented in `docs/version-development-workflow.md`.

## Report/analysis baseline rule

`reports/test262-analysis.md` is locked as the 2026-06-24 full-run baseline. Never edit it. New full analysis goes into a dated file (`reports/test262-analysis-YYYY-MM-DD.md`) or versioned file. Only tracked report files and scan files listed in `.gitignore`'s whitelist should be in `reports/`.

## Testing approach

- Put unit tests beside each module; broader integration tests under `tests/`.
- Stages are decoupled through `contracts.rs` — test one stage with fakes for the others via `NativePipeline::from_stages(...)`.
- Never count skipped Test262 cases as passes.
- Report newly-passed / newly-failed / skipped / regressed counts with behavior-affecting PRs.
- Boa-backed numbers (README baseline) and native numbers are separate — never conflate them.

## Commit style

Imperative subject, scope when helpful (`feat(lexer): ...`, `fix(vm): ...`, `test(parser): ...`). Keep scoped to one module + necessary shared-contract change. Don't mix unrelated submodule changes. Merge shared-contract changes before dependent branches.

## `src/contracts.rs` — the stable boundary

Import cross-team types/traits through `agentjs::contracts::{...}`. Changing these types or the `SourceParser`/`ProgramCompiler`/`ChunkExecutor`/`NativePipeline` traits requires team review. Keep implementations in their owning `src/<module>/` directory.

## Host surface

Tiny by design: `print` + frozen `console` facade. No filesystem/process/network. Runtime limits bound loops, recursion, VM stack, and backtrace size. LRU cache reuses parsed/compiled scripts per isolate without sharing mutable globals.

## QuickJS reference (optional build)

```sh
cd quickjs && make && make test
```

## Test262 tooling (for linting individual cases)

```sh
cd test262 && npm install
python tools/lint/lint.py --exceptions lint.exceptions test/path/to/case.js
```
