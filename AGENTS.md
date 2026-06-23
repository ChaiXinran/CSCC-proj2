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

---

## Native V7 — A-line (Frontend / Syntax / Early-errors / Operand Compilation)

### Owner

A-group (frontend contributors): `src/lexer/`, `src/parser/`, `src/ast/`

### Status — COMPLETE (as of 2026-06-24)

All A-line items from the Native V7 Test262 parallel fix plan have been implemented,
tested, and verified against all V1–V7 gates.

### What was done

#### 1. Strict-mode source tracking (`src/lexer/token.rs`, `src/lexer/mod.rs`)

- Added `has_legacy_escape: bool` to `Token`.  Set to `true` by the lexer for
  any `String` token that contains a legacy octal escape (`\1`–`\7`, `\0x` where
  `x` is a digit) or a non-octal decimal escape (`\8`, `\9`).
- Added side-channel field `string_has_legacy_escape: bool` to `Lexer` so
  `read_string_escape()` can pass the flag to `read_string()` without changing
  the escape-reading method signature.

#### 2. `"use strict"` directive prologue detection (`src/parser/mod.rs`, `src/parser/statement.rs`)

- Added `pub(super) is_strict: bool` to `Parser` (default `false`).
- `parse_program()` now calls `consume_directive_prologue()` before the statement
  loop.
- `parse_function_body()` saves `outer_strict`, calls `consume_directive_prologue()`,
  and restores `outer_strict` on exit, so function-level strict mode is correctly
  scoped.
- `consume_directive_prologue()` scans consecutive `ExpressionStatement(StringLiteral)`
  nodes at the start of a body.  It sets `self.is_strict = true` on a `"use strict"`
  directive, and immediately rejects any string in the directive position that has
  `has_legacy_escape` while `self.is_strict` is already true.

#### 3. U+2028/U+2029 as valid string content (`src/lexer/mod.rs`)

- Changed the line-terminator guard inside `read_string()` from
  `is_line_terminator(ch)` (which caught LS and PS) to
  `matches!(ch, '\n' | '\r')` (only CR and LF terminate strings).
- U+2028 (LINE SEPARATOR) and U+2029 (PARAGRAPH SEPARATOR) are now legal
  unescaped string content per ES2019+.

#### 4. Strict-mode early error for legacy escapes in expressions (`src/parser/expression.rs`)

- `parse_primary()` checks `self.is_strict && token.has_legacy_escape` before
  consuming a `String` token and returns a `ParseError` on violation.
- `delete identifier` in strict mode is rejected at parse time:
  `self.is_strict && matches!(argument, Expression::Identifier(_))` →
  `ParseError("cannot delete an unqualified identifier in strict mode")`.

#### 5. `++`/`--` on member expressions (`src/bytecode/compiler.rs`)

Extended `compile_update()` to handle all valid assignment targets:

| Operand form | Prefix | Postfix |
|---|---|---|
| `identifier` | ✅ (V6) | ✅ (V6) |
| `obj.prop` (static member) | ✅ new | ✅ new |
| `obj[key]` (computed member) | ✅ new | ❌ unsupported (needs rotate instruction) |

- Static member prefix `++obj.prop`: `Duplicate` → `GetProperty` → `UnaryPlus` → `Constant(1)` → `Add` → `SetProperty`.
- Static member postfix `obj.prop++`: uses `DuplicatePair` to preserve [obj, old_num], computes and stores the new value, then double-`Pop` to leave old_num.
- Computed member prefix `++obj[key]`: `DuplicatePair` saves [obj, key], `GetElement`, `UnaryPlus`, `Constant(1)`, `Add`, `SetElement`.
- Computed member postfix `obj[key]++`: returns `CompileError("unsupported")` — no rotate/tuck instruction available; requires a dedicated VM instruction to implement cleanly.

#### 6. `delete` non-member operands (`src/bytecode/compiler.rs`)

Extended `compile_delete()`:

- `delete identifier` (sloppy mode): emits `Constant(false)` — declared bindings are non-configurable.
- `delete (non-reference expression)`: evaluates the operand for side-effects, `Pop`s it, emits `Constant(true)` — deleting a non-reference always succeeds per spec.
- Static and computed member forms: unchanged from V6.

### Tests added (`tests/frontend_v7.rs`)

Twelve new tests cover:
- `strict_mode_rejects_legacy_octal_string_escape`
- `strict_mode_rejects_non_octal_decimal_escape`
- `sloppy_mode_allows_legacy_octal_string_escape`
- `use_strict_directive_detected_in_function`
- `strict_mode_rejects_legacy_octal_in_second_directive`
- `ls_and_ps_allowed_in_string_literals`
- `strict_mode_delete_identifier_is_early_error`
- `sloppy_mode_delete_identifier_is_allowed`
- `delete_member_expression_compiles`
- `delete_literal_compiles_to_true`
- `prefix_static_member_update_parses`
- `postfix_static_member_update_parses`
- `prefix_computed_member_update_parses`

### Contracts/Boundaries

`Token.has_legacy_escape` is a new additive field.  It carries metadata from the
lexer into the parser and does NOT cross the `contracts.rs` stable boundary (it
stays within `src/lexer/token.rs`, which is internal to the A-group).

### Known remaining gaps (not A-line scope)

- Postfix computed-member update (`obj[key]++`) requires a `Rotate`/`Tuck` VM
  instruction — deferred to a future VM milestone.
- Class fields, optional chaining (`?.`), nullish coalescing (`??`), `async`/
  `await`, `for…of`, destructuring assignment, and spread/rest remain outside
  the V7 scope.
- The `--native-v7-scan` diagnostic baseline (58.37%) may improve further as
  B-line and C-line fixes land.
