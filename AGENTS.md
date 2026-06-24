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

B-line parser/lexer early-error integration is complete as of the current V7
merge pass. Implemented items include:

- Numeric separators in number literals across decimal, fractional, exponent,
  binary, octal, hexadecimal, and the current temporary BigInt token path.
- Lexical redeclaration early errors for same-scope `let` / `const` /
  `function` names.
- `var` redeclaration conflicts against same-scope lexical names, including
  nested block `var` and `for (var ... in ...)` cases.
- Strict-mode rejection of function declarations in single-statement
  `if` / `else` / `while` / `for` bodies while still allowing braced block
  bodies.

Current B-line hot files are `src/lexer/mod.rs` and
`src/parser/statement.rs`. Focused coverage lives in their inline unit tests.

Coordination notes:

- A-line regexp work should coordinate before changing shared lexer scanning
  helpers in `src/lexer/mod.rs`.
- C-line work should avoid folding Unicode identifier escape, strict `this`,
  `Object.prototype.toString`, or String builtin changes into the B-line parser
  patch; those remain C-line/runtime responsibilities.
- Do not broaden B-line early-error validation into runtime or builtin files
  unless the V7 owners explicitly rescope the work.

Latest local V7 results:

- V7 pinned gate: 69/69 passed, 0 failed, 0 skipped.
- V7 diagnostic scan: 2,229/3,034 passed, 805 failed, 0 skipped,
  73.47% conformance.
- Net gain over the referenced B-line baseline: +243 passing Test262 cases.

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

### A-line regexp additions (same session, follow-up)

After the compiler/parser work, four additional regexp sub-tasks were completed:

**A1 – RegExp flag validation** (`src/lexer/mod.rs`):
`read_regex_literal_at()` now validates that each flag is in `[dgimsuy]`,
rejects duplicates, and rejects `u`+`v` combination — all per ES2023.

**A3 – RegExp instance properties** (`src/runtime/context.rs`):
`create_regexp()` now writes 11 data properties on every new RegExp object:
`source`, `flags` (sorted: d g i m s u v y), `global`, `ignoreCase`, `multiline`,
`dotAll`, `sticky`, `unicode`, `unicodeSets`, `hasIndices`, plus `lastIndex` (writable,
non-enumerable, non-configurable, initially 0). Helper `sort_regexp_flags()` added.

**A4a – JS replacement patterns** (`src/builtins/regexp.rs`):
Rewrote `replace_first()` and `replace_all()` to expand `$&`, `` $` ``, `$'`, `$$`,
and `$1`–`$99` via a new `expand_replacement()` helper (pure Rust, no external crate).

**A4b – Function replacement callback** (`src/builtins/v6.rs`):
`string_replace()` and `string_replace_all()` now detect a callable second argument
and invoke it per match via `vm.call_value_from_builtin()` with args
`(match, p1, …, offset, inputString)`.

**A4c – String.split capture groups** (`src/builtins/v6.rs`, `src/builtins/regexp.rs`):
`regexp::split()` now uses `captures_iter()` instead of `regex.split()` and
interleaves capture groups between substrings as required by the ES spec.

**A4d – String.prototype.matchAll** (`src/builtins/v6.rs`):
Basic eager implementation: requires `g` or `y` flag, materialises all matches
as an array of exec-style arrays (each with `index` and `input` properties).

**Result**: V7 scan improved from 65.46% → 66.71% (+38 newly passing cases,
3034 total, 2024 passed, 1010 failed).

### A4 – Symbol dispatch (follow-up session)

**A4 – Symbol dispatch** (`src/runtime/symbol.rs`, `src/builtins/v6.rs`):
Added `Symbol.match`, `Symbol.replace`, `Symbol.split`, `Symbol.matchAll`, `Symbol.search`
as well-known symbols (IDs 6–10). `RegExp.prototype` now has `[Symbol.match]`,
`[Symbol.replace]`, `[Symbol.split]`, `[Symbol.matchAll]`, `[Symbol.search]` methods
installed via symbol-keyed property descriptors. `String.prototype.match/replace/split/
matchAll/search/replaceAll` each check `@@Symbol` dispatch on the first argument before
falling back to direct logic via a shared `try_symbol_dispatch()` helper.

**Result**: V7 scan improved from 66.71% → 68.69% (+98 additional cases, 2084 passed).

### A1 – RegExp body validation (second follow-up session)

**A1 – Regex body early errors** (`src/lexer/mod.rs`):
Added `validate_regex_body(body, flags, lex_start)` called after the body is lexed.
Detects and reports lex-time `SyntaxError` for:

- **U+2028 / U+2029** in regex body (ES line terminators — forbidden unescaped): +13 tests.
- **Arithmetic modifier groups** `(?add-remove:...)` — always unsupported, always rejected
  at parse phase: +58 tests.
- **Inline modifier groups** `(?flags:...)` where flags contain anything other than `i`, `m`, `s`,
  or contain duplicate flag letters (including case-folded variants and non-ASCII chars): +25 tests.
- **Named capture group** `(?<name>...)` validation: empty name, invalid identifier characters
  (non-letter start, punctuators, digits at start), unterminated `(?<name` without `>`,
  duplicate group names: +20 tests.
- **Named backreferences** `\k<name>` with no corresponding capture group (dangling reference),
  unterminated `\k<name` without `>`: +11 tests.
- **Bare `\k`** (without `<name>`) in Unicode mode (`/u` or `/v`) or when named groups exist: +2 tests.

**Result**: V7 scan improved from 68.69% → 73.20% (+137 additional cases,
3034 total, 2221 passed, 813 failed).

### A2 – eval() operand stack underflow (third follow-up session)

**A2 – eval stack isolation** (`src/vm/interpreter.rs`, `src/builtins/function.rs`):

Root cause: `execute_with_context` called `self.stack.clear()` before running a chunk.
When `eval()` was called mid-expression (e.g. as a function argument), this wiped the
outer program's in-progress operand stack; the outer expression then found an empty
stack and threw "operand stack underflow".

Fix: Added `Vm::eval_execute()` — like `execute_with_context` but does NOT clear the stack.
It records `saved_depth = self.stack.len()` before calling `run_completion`, then
`self.stack.truncate(saved_depth)` after (whether success or error). The eval'd code
runs on top of the existing stack and the return value travels via `Result`, not via
the stack. `eval_call` in `function.rs` now calls `vm.eval_execute()` instead of
`vm.execute_with_context()`.

**Result**: V7 scan improved from 73.20% → 73.47% (+8 additional cases,
3034 total, 2229 passed, 805 failed).

### Known remaining gaps (not A-line scope)

- Postfix computed-member update (`obj[key]++`) requires a `Rotate`/`Tuck` VM
  instruction — deferred to a future VM milestone.
- Class fields, optional chaining (`?.`), nullish coalescing (`??`), `async`/
  `await`, `for…of`, destructuring assignment, and spread/rest remain outside
  the V7 scope.
- RegExp named capture groups (`(?<name>…)`) parse correctly but do not execute
  via Rust `regex` crate (which uses `(?P<name>...)` syntax) — runtime support
  is a future milestone.
- `\k` without `<name>` in non-Unicode sloppy mode (no named groups) is not
  flagged (intentional — ES spec allows it in non-Unicode mode).
- `(?<a>\a)/u` invalid identity escape inside Unicode named group: not detected.
- Duplicate named groups in different `|` alternatives (ES2025 `regexp-duplicate-named-groups`)
  are incorrectly rejected; the `CanBothParticipate` rule is not implemented.
- Further improvement depends on C-line (Number/JSON/Error/Math) fixes landing.
