# Repository Guidelines

## Project Structure & Module Organization

The root is the AgentJS implementation. It uses three bundled upstream trees for its current backend and evaluation:

- `src/`: AgentJS runtime, CLI, and Test262 runner. `engine.rs` is
  backend-neutral; backend implementations live in `src/backend/`. Integration
  tests live in `tests/`; runnable JavaScript samples live in `examples/`.
- `src/contracts.rs`: stable native-engine collaboration boundary. Import
  cross-team types and traits here; change its public contracts only with team
  review. Keep implementations in their owning `src/<module>/` directory.
- `docs/`: architecture, status, and benchmark methodology.
- `boa/`: current Rust ECMAScript backend and implementation reference. Do not modify it for AgentJS features unless an upstream patch is intentional.
- `quickjs/`: C implementation used as a compact engine reference. Sources are at the directory root, with examples, tests, and fuzz targets under `examples/`, `tests/`, and `fuzz/`.
- `test262/`: official ECMAScript conformance suite. Tests are under `test262/test/`, shared helpers under `harness/`, generators under `src/`, and tooling under `tools/`.

Keep new project code at the root rather than placing it inside an upstream tree.
Treat `boa/`, `quickjs/`, and `test262/` as pinned submodules; document revision
updates in `docs/dependencies.md`.

## Build, Test, and Development Commands

```sh
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo run -- eval "1 + 2"
cargo run --release -- test262 --root test262 --suite test --jobs 8
```

Build the QuickJS comparison binary on Linux, macOS, WSL, or MSYS2:

```sh
cd quickjs
make
make test
```

For Test262 tooling:

```sh
cd test262
npm install
npm test -- --hostPath /path/to/engine
python tools/lint/lint.py --exceptions lint.exceptions test/path/to/case.js
```

## Coding Style & Naming Conventions

Root Rust code uses four spaces and `rustfmt`; use `snake_case` for functions/modules and `CamelCase` for types. Keep public APIs documented and prefer explicit error propagation over panics. Follow each upstream subtree's own `.editorconfig`. Keep files UTF-8, LF-terminated, and free of trailing whitespace.

## Testing Guidelines

Add unit tests beside private modules and public behavior tests under `tests/`. Every runtime change should cover successful execution plus its isolation or limit behavior. Prefer focused Test262 runs before the full suite. Never count skipped cases as passes, and attach pass-rate or benchmark deltas to engine changes.

## Commit & Pull Request Guidelines

History varies by subtree: Boa commonly uses scoped Conventional Commit subjects such as `fix(vm): ...`, while QuickJS and Test262 favor concise imperative summaries. Use an imperative subject, add a scope when helpful, and avoid mixing unrelated upstream changes. Pull requests should identify the affected subtree, explain behavior and specification impact, list commands run, link relevant issues, and include benchmark or Test262 results when performance or compatibility changes.
