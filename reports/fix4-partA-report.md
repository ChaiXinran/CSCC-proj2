# Fix4 Part A Report - Class and Destructuring Frontend

Owner: A group

Scope: `src/lexer/`, `src/parser/`, `src/ast/`, and narrowly required
class/destructuring lowering in `src/bytecode/compiler.rs`.

Locked baseline source: user-provided `test/language` baseline before this
Fix4-A batch.

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| `test/language` total | 23,711 | 23,711 | 0 |
| `test/language` passed | 13,399 | 13,499 | +100 |
| `test/language` failed | 10,312 | 10,212 | -100 |
| `test/language` skipped | 0 | 0 | 0 |
| `test/language` conformance | 56.51% | 56.93% | +0.42 pp |

## Change Log

### 2026-06-27 - A group

What changed:

- Fixed class computed member name parsing so `[AssignmentExpression]` re-enables
  the `in` operator even inside a surrounding `for (...)` header.
- Added `ClassElement::StaticBlock` and parser/compiler support for
  `static { ... }` class blocks.
- Added static-block early-error checks for direct `arguments`, direct `await`,
  `return`, `super()`, and outer loop/switch/label control flow leakage while
  preserving function-body boundaries.
- Added generator-parameter early errors for `yield` in default values and
  computed binding names across declarations, expressions, and class methods.
- Tightened strict-mode destructuring assignment targets so nested `eval` and
  `arguments` targets are rejected as `SyntaxError`.
- Fixed `var` destructuring declarations and C-style `for (var [x] = ...)`
  lowering to write into hoisted `var` bindings without breaking function
  parameter destructuring.

Functionality added or fixed:

- Class static blocks execute in source order with static fields and receive the
  class constructor as `this`.
- Class computed member names such as `['x' in empty]` parse correctly in class
  bodies nested under `for` headers.
- Static-block syntax now rejects the high-volume early-error cases that were
  previously reaching runtime or parsing incorrectly.
- `var` destructuring declarations now work with the existing hoisting model in
  top-level/function bodies and `for` initializers.

Files touched:

- `src/ast/expression.rs`: added `ClassElement::StaticBlock`.
- `src/parser/expression.rs`: class computed-name parsing fix, static block
  parsing, static-block early-error walker, strict destructuring target checks,
  generator method parameter validation, and parser coverage.
- `src/parser/statement.rs`: shared generator parameter `yield` validation for
  function declarations/expressions.
- `src/bytecode/compiler.rs`: static block lowering, `var` destructuring store
  path, and `var` destructuring name hoisting.
- `tests/native_objects.rs`: native execution coverage for class static blocks.
- `tests/native_for_loops.rs`: native execution coverage for `var`
  destructuring declarations and `for` initializers.
- `reports/native-language.json`: refreshed with the requested full
  `test/language` verification command.

Commands/tests run:

- `rustfmt --edition 2024 src/ast/expression.rs src/bytecode/compiler.rs src/parser/expression.rs src/parser/statement.rs tests/native_for_loops.rs tests/native_objects.rs`
- `cargo test --no-default-features --lib`
- `cargo test --no-default-features --test native_objects executes_class_static_blocks_with_class_this_and_source_order`
- `cargo test --no-default-features --test native_for_loops`
- `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class/dstr --jobs 4 --json reports/tmp-fix4-a-expressions-class-dstr-final.json`
- `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for/dstr --jobs 4 --json reports/tmp-fix4-a-for-dstr-final.json`
- `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/native-language.json`

Result deltas:

- Full `test/language`: 13,399/23,711 -> 13,499/23,711, +100 passing cases,
  56.51% -> 56.93%.
- Focused `test/language/statements/for/dstr`: 175/285 -> 222/285, +47 passing
  cases.
- Focused `test/language/expressions/assignment/dstr`: 278/368 -> 279/368, +1
  passing case.
- Focused `test/language/expressions/class/dstr`: final verification remains
  1,040/1,920 after fixing an intermediate `var` binding regression.

Newly exposed failures or regressions:

- No final regressions observed in the requested full `test/language` scan.
- During development, a too-broad `VariableKind::Var` destructuring change caused
  class destructuring parameter failures. It was fixed by restoring declaration
  semantics in `compile_binding_pattern(Var)` and using the `StoreName` helper
  only in hoisted `var` declaration execution paths.
- Remaining high-volume failures are mostly outside A scope: async/generator
  execution and Promise/iterator behavior are B-line work; descriptor/builtin
  shape precision is C-line work.

Coordination notes:

- No B-owned VM opcode or runtime protocol changes were introduced.
- No C-owned builtin/descriptor behavior was changed.
- Object rest destructuring still uses the existing simplified lowering; full
  excluded-key copy semantics should wait for the shared runtime helper described
  in `docs/fix4-team-plan.md`.
