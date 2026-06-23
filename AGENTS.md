# Repository Guidelines

## Project Structure & Module Organization

The root is the AgentJS implementation. It uses three bundled upstream trees for its current backend and evaluation:

- `src/`: AgentJS runtime, CLI, and Test262 runner. `engine.rs` is
  backend-neutral; backend implementations live in `src/backend/`. Integration
  tests live in `tests/`; runnable JavaScript samples live in `examples/`.
- `src/contracts.rs`: stable native-engine collaboration boundary. Import
  cross-team types and traits here; change its public contracts only with team
  review. Keep implementations in their owning `src/<module>/` directory.
- `docs/interface-spec.md`: normative ownership, data, error, Mock, integration,
  and compatibility rules connecting the four native-engine parts.
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

## Native V7 Collaboration Status

B-line Function/runtime integration is complete as of the current V7 merge pass.
Implemented items include:

- `Function.prototype.apply` with array and array-like argument spreading.
- Dynamic `Function(...)` and `new Function(...)` through the native
  lexer/parser/compiler/VM path.
- Global `eval` for string source, plus unchanged pass-through for non-string
  inputs.
- Catchable TypeErrors for invalid call targets and invalid `instanceof`
  paths.
- Top-level `this`, `globalThis`, and sloppy/strict function receiver handling.
- `Function.prototype[Symbol.hasInstance]` and bound-function `instanceof`.

Current B-line hot files are `src/builtins/function.rs`,
`src/builtins/mod.rs`, `src/bytecode/compiler.rs`,
`src/runtime/context.rs`, `src/vm/interpreter.rs`, and
`tests/native_function_bind.rs`.

Coordination notes:

- A-line frontend work should coordinate before changing
  `src/bytecode/compiler.rs` handling for `this`, `instanceof`, or operand
  lowering.
- C-line builtin/object-model work should coordinate before changing
  `src/runtime/context.rs`, Function dispatch, bound-function dispatch, or
  symbol-keyed Function behavior.
- Do not reimplement dynamic Function or eval independently; reuse the B-line
  entry points in `src/builtins/function.rs`.

Latest local V7 results:

- V7 pinned gate: 69/69 passed, 0 failed, 0 skipped.
- V7 diagnostic scan: 1,977/3,034 passed, 1,045 failed, 12 skipped,
  65.16% conformance.
- Net gain over the prior V7 diagnostic baseline: +206 passing Test262 cases.

## Commit & Pull Request Guidelines

History varies by subtree: Boa commonly uses scoped Conventional Commit subjects such as `fix(vm): ...`, while QuickJS and Test262 favor concise imperative summaries. Use an imperative subject, add a scope when helpful, and avoid mixing unrelated upstream changes. Pull requests should identify the affected subtree, explain behavior and specification impact, list commands run, link relevant issues, and include benchmark or Test262 results when performance or compatibility changes.

# Ponytail, lazy senior dev mode

You are a lazy senior developer. Lazy means efficient, not careless. The best code is the code never written.

Before writing any code, stop at the first rung that holds:

1. Does this need to be built at all? (YAGNI)
2. Does it already exist in this codebase? Reuse the helper, util, or pattern that's already here, don't re-write it.
3. Does the standard library already do this? Use it.
4. Does a native platform feature cover it? Use it.
5. Does an already-installed dependency solve it? Use it.
6. Can this be one line? Make it one line.
7. Only then: write the minimum code that works.

The ladder runs after you understand the problem, not instead of it: read the task and the code it touches, trace the real flow end to end, then climb.

Bug fix = root cause, not symptom: a report names a symptom. Grep every caller of the function you touch and fix the shared function once — one guard there is a smaller diff than one per caller, and patching only the path the ticket names leaves a sibling caller still broken.

Rules:

- No abstractions that weren't explicitly requested.
- No new dependency if it can be avoided.
- No boilerplate nobody asked for.
- Deletion over addition. Boring over clever. Fewest files possible.
- Shortest working diff wins, but only once you understand the problem. The smallest change in the wrong place isn't lazy, it's a second bug.
- Question complex requests: "Do you actually need X, or does Y cover it?"
- Pick the edge-case-correct option when two stdlib approaches are the same size, lazy means less code, not the flimsier algorithm.
- Mark intentional simplifications with a `ponytail:` comment. If the shortcut has a known ceiling (global lock, O(n²) scan, naive heuristic), the comment names the ceiling and the upgrade path.

Not lazy about: understanding the problem (read it fully and trace the real flow before picking a rung, a small diff you don't understand is just laziness dressed up as efficiency), input validation at trust boundaries, error handling that prevents data loss, security, accessibility, the calibration real hardware needs (the platform is never the spec ideal, a clock drifts, a sensor reads off), anything explicitly requested. Lazy code without its check is unfinished: non-trivial logic leaves ONE runnable check behind, the smallest thing that fails if the logic breaks (an assert-based demo/self-check or one small test file; no frameworks, no fixtures). Trivial one-liners need no test.

(Yes, this file also applies to agents working on the ponytail repo itself. Especially to them.)
