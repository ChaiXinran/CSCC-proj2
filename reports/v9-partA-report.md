# V9-A Implementation Report — Generator / Async / for-of Frontend

**Date:** 2026-06-24  
**Owner:** Group A (Frontend: lexer / parser / AST / bytecode compiler)  
**Branch:** main

---

## 1. Scope

V9-A covers the **syntax-and-lowering** layer for ECMAScript async/generator features:

| Feature | Status |
|---|---|
| `yield` / `yield*` keyword lexing & parsing | ✅ Done |
| `await` keyword lexing & parsing | ✅ Done |
| `function*` generator function declaration & expression | ✅ Done |
| `async function` declaration & expression | ✅ Done |
| `async function*` async-generator (parsing only) | ✅ Done |
| `async () =>` async arrow function | ✅ Done |
| `for (x of iterable)` lowering to iterator protocol opcodes | ✅ Done |
| `for await (x of gen)` parser support | ✅ Done |
| V9-B runtime stubs (iterator object, Promise, GeneratorObject) | ⏳ Pending (V9-B) |

---

## 2. Files Changed

### Lexer
- **`src/lexer/token.rs`** — Added `Keyword::Yield` and `Keyword::Await` variants; `as_str()` returns `"yield"` / `"await"`.
- **`src/lexer/mod.rs`** — `read_identifier_or_keyword` maps `"yield"` → `Keyword::Yield`, `"await"` → `Keyword::Await`. `async` and `of` remain contextual identifiers.

### AST
- **`src/ast/expression.rs`** — `FunctionLiteral` extended with `is_async: bool` and `is_generator: bool`; `Expression` extended with `Yield { argument, delegate }` and `Await(Box<Expression>)`.
- **`src/ast/statement.rs`** — `Statement::FunctionDeclaration` extended with `is_async: bool`, `is_generator: bool`; added `ForBinding` enum; added `Statement::ForOf { left, right, body, is_await }`.
- **`src/ast/mod.rs`** — Re-exports `ForBinding`.

### Opcodes
- **`src/bytecode/opcode.rs`** — 8 new instructions:
  - `GetIterator` — calls `[Symbol.iterator]()`, pushes iterator
  - `IteratorNext` — pushes `[value, is_done]`
  - `IteratorClose` — calls `.return()` if present
  - `CreateGenerator(u16)` — stub; creates generator from function template
  - `YieldValue` — stub; suspends generator
  - `YieldDelegate` — stub; `yield*` delegation
  - `CreateAsyncFunction(u16)` — stub; wraps function as async
  - `AwaitValue` — stub; suspends async function
  All stack effects are declared.

### VM
- **`src/vm/interpreter.rs`** — 8 exhaustive match arms added; each stub pops required values then returns `VmError::runtime("... not yet implemented (V9-B pending)")`. `CreateGenerator` and `CreateAsyncFunction` validate their function-table index before erroring.

### Compiler
- **`src/bytecode/compiler.rs`** — `compile_statement` dispatches `Statement::ForOf`; `compile_expression` handles `Expression::Yield` and `Expression::Await`; `compile_function_expression` routes `is_generator` → `CreateGenerator`, `is_async` → `CreateAsyncFunction`, else `CreateFunction`; added `compile_for_of` implementing the full iterator-protocol loop pattern.

### Parser
- **`src/parser/statement.rs`** — `parse_function_declaration` handles `function*`; added `parse_async_function_declaration` for `async function [*] name()`; `parse_statement` dispatches `async function` declarations; `parse_for` extended to detect `for (x of …)` and `for await (x of …)` for all LHS forms (declaration with identifier/destructuring, existing target).
- **`src/parser/expression.rs`** — `parse_assignment` handles `yield [*] [expr]` and `await expr` before normal assignment; `parse_function_expression` handles `function*`; added `parse_async_expression` for `async function [*] name()` and `async (…) =>`; `parse_primary` Identifier arm detects `async` + same-line lookahead; class-body parser detects `*` generator methods; all `FunctionLiteral {}` construction sites include `is_async` and `is_generator`.

### Tests
- **`tests/frontend_v9.rs`** (new) — 19 tests covering:
  - Generator declaration/expression parsing, `yield`, `yield*`, bare `yield`
  - Async declaration/expression/generator parsing, `await`
  - `for...of` with `let`, `const`, `var`, bare target
  - `for await...of` parsing
  - Bytecode emission checks (`GetIterator`, `IteratorNext`, `DeclareFunction`)
  - Runtime stub exposure (`for-of`, generator call, async+await call → V9-B error)

---

## 3. Design Decisions

### 3.1 "Expose runtime failures" pattern
V9-A does not implement runtime semantics. Tests that use `for...of`, generator execution, or async/await will fail at runtime with a clear `"... not yet implemented (V9-B pending)"` message. This converts parse-time failures (no AST support) into runtime failures (correct AST, stubs in VM) so Group B can implement semantics independently.

### 3.2 `async` and `of` stay as Identifiers
Both `async` and `of` are contextual keywords in ECMAScript. They tokenize as `Identifier` to avoid breaking existing code like `var async = 1` or `obj.of`. The parser uses lookahead to distinguish them.

### 3.3 Stack ordering for `IteratorNext`
`IteratorNext` pushes `[value, is_done]` with `is_done` on top. This lets the loop header use a bare `JumpIfTrue(exit)` without a peek instruction. The exit path pops `is_done=true` then `value=undefined` (2 pops).

### 3.4 Hidden `\u{0}forof_iter` binding
The iterator object is stored in a lexical binding named `"\u{0}forof_iter"` — a name that cannot appear in user JS (starts with U+0000). It lives in a `CreateLexicalEnvironment` scope that wraps the entire for-of body.

### 3.5 `for await...of` compile-time error
`compile_for_of` rejects `is_await: true` with `CompileError::unsupported(...)`. The parser correctly captures the flag; compile-time rejection is the correct V9-A behavior until V9-B provides async iterators.

---

## 4. Gate Results

All V1–V6 fixed gates remain at zero failures and zero skips:

| Gate | Total | Passed | Failed | Skipped | Conformance |
|------|-------|--------|--------|---------|-------------|
| V1 | 6 | 6 | 0 | 0 | 100.00% |
| V2 | 15 | 15 | 0 | 0 | 100.00% |
| V3 | 26 | 26 | 0 | 0 | 100.00% |
| V4 | 11 | 11 | 0 | 0 | 100.00% |
| V5 | 4 | 4 | 0 | 0 | 100.00% |
| V6 | 7 | 7 | 0 | 0 | 100.00% |

Unit tests: **205/205** lib tests pass; all integration test suites pass.  
V9-A frontend tests: **19/19** pass.

---

## 5. Diagnostic Scan Results

These are exploratory (non-gated) scans. Failures are expected — runtime semantics are V9-B's responsibility.

| Suite | Total | Passed | Failed | Conformance | Notes |
|-------|-------|--------|--------|-------------|-------|
| `--native-v9-scan` (5000 curated V9 cases) | 5000 | 1 | 4999 | 0.02% | Parser/early-error cases; runtime stubs always fail |
| `test/language/statements/for-of` | 751 | 82 | 669 | 10.92% | Passing cases are early-error / non-execution tests |
| `test/language/statements/generators` | 266 | 32 | 234 | 12.03% | Parser correctness tests pass; execution stubs fail |
| `test/language/expressions/generators` | 290 | 35 | 255 | 12.07% | Same pattern |
| `test/language/statements/async-function` | 74 | 16 | 58 | 21.62% | Higher rate: many non-execution syntax tests |
| `test/language/expressions/async-function` | 93 | 20 | 73 | 21.51% | Same |

### Why rates are low
The passing cases are **early-error and syntax-validation tests** (e.g., `yield` outside generator is a SyntaxError, `await` in wrong context, `for-of` with invalid LHS). All tests that require actually running a generator, awaiting a Promise, or iterating through `[Symbol.iterator]()` fail at runtime because V9-B has not yet implemented:
- `GetIterator` / `IteratorNext` / `IteratorClose` runtime semantics
- `GeneratorObject` with suspend/resume protocol
- Promise microtask queue and `async/await` execution model

---

## 6. Handoff Notes for V9-B

V9-B needs to implement in `src/vm/interpreter.rs`:

1. **`GetIterator`**: call `object[Symbol.iterator]()` and push the returned iterator.
2. **`IteratorNext`**: call `iterator.next()`, push `value` then `is_done` (bool on top).
3. **`IteratorClose`**: call `iterator.return()` if it exists.
4. **`CreateGenerator(fn_idx)`**: create a suspended `GeneratorObject` that captures the current frame and the function template. Push it as a callable.
5. **`YieldValue`**: suspend current frame, return the value to the caller (`.next(sent_value)` protocol).
6. **`YieldDelegate`**: `yield*` — delegate to another iterable and propagate its `return` value.
7. **`CreateAsyncFunction(fn_idx)`**: wrap a function so that calling it returns a Promise; the body runs in a microtask.
8. **`AwaitValue`**: suspend the current async frame, attach a `.then()` to the Promise, resume when resolved.

The opcodes and their stack contracts are already in `src/bytecode/opcode.rs`. The VM match skeleton is in `src/vm/interpreter.rs` (search for `V9-A stubs`).
