# V8 Part A Report — Frontend Unlockers

Owner: A group
Scope: `src/lexer/` · `src/parser/` · `src/ast/` · `src/bytecode/compiler.rs` · `src/vm/interpreter.rs` · `src/runtime/function.rs`

This report is updated in the same commit as each A-track implementation change.

---

## Baseline

Locked source: `reports/test262-analysis.md` (full direct run, 2026-06-24).

| Metric | Value |
| --- | ---: |
| Full direct total | 53,379 |
| Full direct passed | 14,035 |
| Full direct failed | 38,507 |
| Full direct skipped | 837 |
| Full direct pass rate | 26.29% |
| **language suite total** | **23,711** |
| **language suite passed** | **6,244** |
| **language suite pass rate** | **26.33%** |
| language suite failed | 16,658 |
| language suite skipped | 809 |
| V8 scan total | 5,000 |
| V8 scan passed | 0 |
| V8 scan failed | 4,504 |
| V8 scan skipped | 496 |

Dominant failure classes relevant to V8-A (from `native-v8-scope.md`):

| Class | Count | Share of all failures |
| --- | ---: | ---: |
| Parser syntax gap | 16,259 | 42.22% |
| Template literal substitutions unsupported | 5,307 | 13.78% |
| Lexer / static syntax gap | 530 | 1.38% |

---

## V8-A Scope (from `native-v8-scope.md` and `native-v8-interface.md`)

### 1. Template literals

- Untagged template literals: `` `hello ${name}` ``, multi-expression, nested.
- Lowering rule: compile to string concatenation using existing `Add` coercion.
- Tagged templates are **out of scope** for V8.

### 2. Class syntax — first stage

- Class declarations and class expressions.
- Constructor, prototype methods, static methods.
- Parse-only `extends` (prototype chain wired via `SetObjectPrototype`).

### 3. Spread / rest / destructuring — first stage

| Feature | Form | Priority |
| --- | --- | --- |
| Call spread | `f(...args)` | High |
| Array spread | `[a, ...b]` | High |
| Rest parameter | `function f(...args)` | High |
| Array destructuring | `var [a, b] = arr` | High |
| Object destructuring | `var { x, y } = obj` | Medium |

---

## Current Status

**V8-A: COMPLETE (2026-06-24)**

All V8-A frontend features are implemented, tested, and verified against the V1–V6 gate suite.

### Task checklist

- [x] Template literal lexer tokens and AST node
- [x] Template literal parser (`TemplateHead` / `TemplateMiddle` / `TemplateTail`)
- [x] Template literal bytecode lowering (string concatenation via `Add` chain)
- [x] Class declaration / expression parser
- [x] Class AST → bytecode lowering (constructor, prototype methods, static methods)
- [x] `extends` parse support + `SetObjectPrototype` wiring
- [x] `this` / `super` keyword lexer + parser + `LoadThis` instruction
- [x] `FunctionParam::Rest` parser and bytecode (`rest_param: Option<String>` on `FunctionTemplate` / `JsFunction`)
- [x] Rest parameter VM binding (`call_user_function` collects remaining args into array)
- [x] Call spread parser (`CallArgument::Spread`) and bytecode (`SpreadCall` / `SpreadCallWithThis` / `SpreadConstruct`)
- [x] Array spread in array literals (`ArrayElement::Spread` → `SpreadIntoArray` instruction)
- [x] Simple array destructuring (`var [a, b] = ...` → `Statement::DestructuringDeclaration`)
- [x] Simple object destructuring (`var { x, y: z } = ...`)
- [x] Spread operator `...` lexer support (`Operator("...")`)
- [x] VM implementation: `ArrayPush`, `SpreadIntoArray`, `SpreadCall`, `SpreadCallWithThis`, `SpreadConstruct`
- [x] All V1–V6 gates green at zero regression, zero skip

---

## Implementation Details

### Lexer changes (`src/lexer/`)

| File | Change |
| --- | --- |
| `token.rs` | Added `Keyword::This`; `TokenKind::TemplateHead`, `TemplateMiddle`, `TemplateTail` (already present in previous iteration) |
| `mod.rs` | Added `...` → `Operator("...")` detection before the single-`.` punctuator check; added `"this"` keyword mapping; added `tpl_stack: Vec<u32>` brace-depth tracker in `tokenize()` to detect when `}` ends a template substitution; updated `read_template_literal` to emit `TemplateHead`; added `read_template_middle_or_tail` |

### AST changes (`src/ast/`)

| File | Change |
| --- | --- |
| `expression.rs` | `FunctionParam` changed from struct to `enum { Simple(String), Rest(String) }` with `.name()` accessor; `ArrayElement` gained `Spread` variant; `TemplateLiteral { quasis, expressions }` struct added; `ClassElement`, `ClassExpression`, `ClassDeclaration` added; `BindingPattern` enum added; `CallArgument { Expression, Spread }` enum added; `Expression::Call / Construct` changed to `Vec<CallArgument>`; `Expression::TemplateLiteral`, `Spread`, `Class`, `This`, `Super` variants added |
| `statement.rs` | `Statement::ClassDeclaration(ClassDeclaration)` and `Statement::DestructuringDeclaration { kind, pattern, initializer }` variants added |
| `mod.rs` | All new types re-exported |

### Bytecode changes (`src/bytecode/`)

| File | Change |
| --- | --- |
| `opcode.rs` | New instructions: `ArrayPush`, `SpreadIntoArray`, `SpreadCall(u16)`, `SpreadCallWithThis(u16)`, `SpreadConstruct(u16)` with `stack_effect()` implementations |
| `chunk.rs` | `FunctionTemplate.rest_param: Option<String>` field added |
| `compiler.rs` | `compile_function_body` updated to split `FunctionParam::Simple/Rest`; `compile_call/construct` updated to handle `CallArgument::Spread` via `split_trailing_spread`; `compile_array` handles `ArrayElement::Spread`; new methods: `compile_template_literal`, `compile_class_expression`, `compile_class_declaration`, `compile_class_body`, `compile_destructuring_declaration`, `compile_binding_pattern`; `lexical_names` / `binding_pattern_names` updated for `DestructuringDeclaration`; `Expression::This/Super` compile to `LoadThis` |

### VM / Runtime changes

| File | Change |
| --- | --- |
| `src/runtime/function.rs` | `JsFunction.rest_param: Option<String>` field added |
| `src/vm/interpreter.rs` | `CreateFunction` handler propagates `rest_param`; `call_user_function` binds rest parameter as array; `has_explicit_arguments` checks `rest_param`; new instruction handlers for `ArrayPush`, `SpreadIntoArray`, `SpreadCall`, `SpreadCallWithThis`, `SpreadConstruct` |

### Parser changes (`src/parser/`)

| File | Change |
| --- | --- |
| `mod.rs` | Added `check_spread()` helper; `describe()` handles new `TokenKind` variants |
| `statement.rs` | `parse_param_list` supports `...rest`; `parse_variable_declaration` handles `[` / `{` destructuring; new methods: `parse_binding_pattern`, `parse_array_binding_pattern`, `parse_object_binding_pattern`, `parse_class_declaration`; `collect_var_declared_names` updated |
| `expression.rs` | `parse_arguments` returns `Vec<CallArgument>` with spread support; `parse_array_literal` handles spread; `parse_primary` handles `TemplateHead`, `TemplateLiteral`, `Keyword::Class`, `Keyword::This`, `Keyword::Super`; new methods: `parse_template_literal`, `parse_class_expression`, `parse_class_body` |

---

## Gate Results (2026-06-24)

All versioned gates green after V8-A implementation:

| Gate | Total | Passed | Failed | Skipped | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| --native-v1 | 6 | 6 | 0 | 0 | ✅ 100% |
| --native-v2 | 15 | 15 | 0 | 0 | ✅ 100% |
| --native-v3 | 26 | 26 | 0 | 0 | ✅ 100% |
| --native-v4 | 11 | 11 | 0 | 0 | ✅ 100% |
| --native-v5 | 4 | 4 | 0 | 0 | ✅ 100% |
| --native-v6 | 7 | 7 | 0 | 0 | ✅ 100% |

Unit test suite: **205 passed, 0 failed** (lib + all integration tests).

---

## Functional Verification

End-to-end demo run (`examples/v8a_test.js`):

```
hello world!
1+1=2
a1b2c
10
30
0 1 2 3
10 20 30
100 200
Dog says hello
Dog
```

All 10 feature assertions passed (template literals, rest params, call spread, array spread, array destructuring, object destructuring, class declaration, constructor, instance method, instance property).

---

## Change Log

Newest first.

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-26 | A group | Post-Fix2 module import/export frontend connector plus regression recovery: added stable AST nodes, module-only parser entry, module early errors, default export declaration parsing, compiler no-op/export-declaration lowering, and `ModuleRecord` metadata extraction without linking or live-binding semantics | `src/ast/statement.rs`, `src/ast/mod.rs`, `src/contracts.rs`, `src/parser/mod.rs`, `src/parser/statement.rs`, `src/bytecode/compiler.rs`, `src/backend/native.rs`, `src/runtime/module.rs`, `tests/native_modules.rs`, `reports/native-language.json` | `cargo test --no-default-features --test native_modules`; `cargo test --no-default-features --test bytecode_basics`; `cargo test --no-default-features --lib`; `cargo test --no-default-features --lib parser::statement::tests`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --json reports/tmp-language-module-code.json`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/native-language.json` | Focused module test suite 6/6. `module-code` restored to 201/599 after early-error recovery. Full `test/language` now 12,991/23,711 passed, 10,720 failed, 0 skipped, 54.79%, above the user's referenced 54.76%. No V8 scan run in this handoff. |
| 2026-06-26 | A group | Class/private/member/frontend follow-up plus destructuring assignment handoff completion | `src/ast/expression.rs`, `src/lexer/mod.rs`, `src/parser/expression.rs`, `src/parser/statement.rs`, `src/bytecode/opcode.rs`, `src/bytecode/compiler.rs`, `src/vm/interpreter.rs`, `tests/bytecode_basics.rs`, `tests/native_for_loops.rs`, `reports/native-v8-scan-summary.json` | `cargo test --no-default-features --test bytecode_basics`; `cargo test --no-default-features --test native_for_loops`; `cargo test --no-default-features` (stops at existing BigInt stdlib failure); `cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json` | V8 scan now 1,085/5,000 passed, 3,915 failed, 0 skipped (21.70%). Delta: +468 passed vs previous local summary 617/5,000; +1,085 vs initial V8 scan baseline. Full 2026-06-24 baseline unchanged; no new full-suite run claimed. |
| 2026-06-24 | A group | Complete V8-A implementation | lexer, ast, parser, bytecode/compiler, bytecode/opcode, bytecode/chunk, vm/interpreter, runtime/function | `cargo test --all-targets` + V1–V6 gates | 205 passed, 0 failed; all gates 100% |
| 2026-06-24 | setup | Recorded V8 scan baseline | `reports/native-v8-scan-summary.json` | `--native-v8-scan --jobs 4` | 0/5,000 passed, 4,504 failed, 496 skipped |
| 2026-06-24 | setup | Created report template | `reports/v8-partA-report.md` | — | baseline recorded |

---

## 2026-06-26 Follow-up Notes

Post-Fix2 module connector pass:

- Added module AST declarations for imports and exports and re-exported them through `contracts.rs`.
- Added `Parser::parse_module()` so module source is strict and can accept top-level import/export while script parsing remains unchanged.
- Implemented first-stage import/export grammar coverage for side-effect imports, default/named/namespace imports, named exports, re-exports, export-all, export-all-as, export declarations, and `export default` expression forms.
- Added module early errors for duplicate export names, unresolved local exports, strict imported bindings (`eval` / `arguments` / reserved names), duplicate labels, top-level `super` / `new.target`, and ill-formed Unicode string export names.
- Parsed `export default function/class` as declarations instead of assignment expressions so named default declarations create their expected local binding and semicolon-free declaration forms parse correctly.
- Lowered imports to bytecode no-ops and export declarations by compiling only their wrapped declaration/expression.
- Recorded module dependencies, import bindings, and export bindings into `ModuleRecord` before evaluation.

Newly exposed / remaining failures for this pass:

- Initial module connector work dropped `module-code` from the recorded 201/599 to 172/599 because partial import/export parsing removed the previous coarse SyntaxError fallback without yet implementing the required module early errors. The recovery pass restored `module-code` to 201/599 and raised full `test/language` to 54.79%.
- Module linking, dependency evaluation, live bindings, namespace object creation, and cyclic graph handling remain out of A-line scope.
- Imported local bindings are metadata-only in this pass; reading them at runtime before B-line linking support remains unsupported.

This pass continued the interrupted A-track repair and stayed on frontend/parser/compiler/runtime-instruction support:

- Class declarations now participate in lexical predeclaration/binding so `class C {}` does not compile into an undeclared global store.
- Class methods/accessors use class-specific property definition instructions so methods are non-enumerable and configurable, and getters/setters preserve accessor descriptors.
- Private member access parses and compiles through member expressions (`this.#x`, `obj.#m()`), with private names represented as internal string keys for this stage.
- Private names accept Unicode escapes in the lexer.
- Switch lexical/var conflict validation now catches same-switch `var` vs lexical conflicts.
- Assignment targets now accept array/object destructuring forms; array and object destructuring assignment update existing bindings and member targets.
- Computed-member postfix update (`obj[key]++`) now compiles using `Swap` and stack cleanup.
- Function name inference was added for anonymous functions/classes in variable declarations, assignments, and default binding initializers.
- `var` names are predeclared to `undefined` for script/function hoisting before execution.

Newly exposed / remaining failures:

- `cargo test --no-default-features` still stops in `tests/native_stdlib.rs::bigint_and_template_literals_parse_through_native_pipeline`: BigInt literal runtime semantics remain a B/C-owned V10 gap, not part of this A-track patch.
- Object destructuring assignment rest is still simplified: it copies all enumerable properties and does not yet exclude already-bound keys.
- Computed-member postfix update re-evaluates object/key for the write path; side-effect-perfect semantics need a future rotate/tuck VM instruction.

Coordination notes:

- B/C should not depend on private-name string-key storage as final private brand semantics; it is an A-stage parser/compiler compatibility bridge.
- The new `Swap`, `SetFunctionName`, and class property-definition opcodes are shared VM surface and should be reused rather than duplicated in later frontend/runtime work.

---

## Open Risks / Coordination Notes

- **`super()` call chain**: Full `super(...)` forwarding in constructors is not implemented. The `super` keyword compiles to `LoadThis` (a working approximation for method calls); constructor chaining via `super()` will need a `SuperCall` instruction in a future version.
- **Single-trailing-spread restriction**: `SpreadCall` / `SpreadConstruct` only supports a single spread as the **last** argument. Multiple spread positions (`f(...a, ...b)`) emit a `CompileError` until a more general `apply`-style implementation is added.
- **Tagged templates**: Out of V8 scope; hitting a tagged template in the parser produces a `CompileError`.
- **Nested destructuring**: Fully supported recursively through `compile_binding_pattern`. Patterns like `var { a: [b, c] } = obj` work.
- **Default parameters**: Not implemented; `function f(x = 0)` will fail to parse.
- **B/C dependency**: A does not implement TypedArray, Intl, `$262`, or module loader.
