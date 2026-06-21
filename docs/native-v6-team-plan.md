# Native V6 Team Plan

V6 is runtime-heavy, so work is divided by file ownership rather than forcing
unrelated parser or compiler changes. Shared coercion and intrinsic contracts
merge before builtin implementations.

## 1. Branch and Merge Strategy

Recommended branches:

```text
docs/v6-contracts
feat/v6-coercion
feat/v6-string
feat/v6-numeric
feat/v6-json
test/v6-test262
```

Recommended merge order:

```text
V6 contracts
  -> coercion/wrapper runtime
  -> independent builtin modules
  -> builtin registration integration
  -> Test262 gate and reports
```

Do not develop all standard objects in `src/builtins/mod.rs`; that recreates
the merge-conflict pattern seen in V4/V5.

## 2. A Group — Frontend Compatibility

Owned files:

```text
src/lexer/
src/ast/
src/parser/
tests/frontend_v6.rs
```

Tasks:

- verify builtin names remain ordinary identifiers and member names;
- add parser regression coverage for chained constructor/method calls;
- verify string escape and Unicode literal inputs used by V6 tests;
- classify syntax-dependent Test262 failures without implementing deferred
  syntax.

V6 requires no new AST variants or keywords. A must not special-case builtin
names.

## 3. B Group — Generic Call/Construct Contract

Owned files:

```text
src/bytecode/
tests/bytecode_v6_contract.rs
tests/frontend_bytecode_v6.rs
```

Tasks:

- verify constructor and method calls use existing generic instructions;
- preserve receiver binding for primitive and object method calls;
- verify left-to-right evaluation and nested coercion call stack behavior;
- add no String-, Number-, Math-, Error-, or JSON-specific opcode.

B should need tests and small generic fixes only. Any proposed magic builtin
opcode requires team review.

## 4. C Group — Runtime and Builtins

### C0 — Coercion and Wrappers

Owned files:

```text
src/runtime/coercion.rs
src/runtime/object.rs
src/runtime/context.rs
src/runtime/intrinsics.rs
src/vm/
tests/native_v6_runtime.rs
```

Implement the frozen conversion API, primitive wrapper internal slots,
intrinsic identities, property access on boxed primitives, and exception
propagation. Changes to VM call completion must retain the V5 tests.

### C1 — String

Owned files:

```text
src/builtins/string.rs
tests/native_v6_string.rs
```

Implement V6.1 using C0 conversion and UTF-16 helpers. Do not edit numeric,
JSON, Object, or Array modules.

### C2 — Numeric, Boolean, Math, and Error

Owned files:

```text
src/builtins/number.rs
src/builtins/boolean.rs
src/builtins/math.rs
src/builtins/error.rs
tests/native_v6_numeric.rs
```

Move the existing partial implementations out of `builtins/mod.rs`, then
correct descriptors, edge cases, prototypes, and error hierarchy.

### C3 — JSON

Owned files:

```text
src/builtins/json.rs
tests/native_v6_json.rs
```

Implement the standalone JSON parser/stringifier and direct runtime tests.
Reviver/replacer callbacks are a second merge after core JSON passes.

## 5. D Group — Integration and Test262

Owned files:

```text
src/builtins/mod.rs
src/test262.rs
src/main.rs
tests/native_test262.rs
reports/native-v6-test262-report.md
.github/workflows/ci.yml
readme.md
```

Tasks:

- integrate module installers after their branches pass independently;
- add `--native-v6` and `--native-v6-scan`;
- scan String, Number, Math, Boolean, Error, and JSON directories;
- select a zero-failure, zero-skip pinned gate for each completed stage;
- retain every V1-V5 gate;
- report parser, compiler, runtime, harness, unsupported-feature, timeout, and
  assertion failures separately.

## 6. Shared-File Lock

| File | Owner |
| --- | --- |
| `src/runtime/object.rs` | C0 |
| `src/runtime/context.rs` | C0 |
| `src/vm/interpreter.rs` | C0 |
| `src/builtins/mod.rs` | D |
| `src/builtins/string.rs` | C1 |
| `src/builtins/number.rs`, `boolean.rs`, `math.rs`, `error.rs` | C2 |
| `src/builtins/json.rs` | C3 |
| `src/test262.rs`, `src/main.rs` | D |

Object, Array, and Function modules are regression surfaces. Modify them only
for a focused V6 coercion fix with corresponding V4 and V6 tests.

## 7. Independent Validation

```text
A: source -> AST
B: hand-built AST -> Chunk
C0: direct conversion/wrapper/runtime API
C1-C3: direct builtin calls in a controlled NativeContext
D: source -> Native result -> Test262 summary
```

Each branch runs its focused tests plus:

```powershell
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

## 8. Merge Gate

After integration:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run -- test262 --native-v1 --jobs 1
cargo run -- test262 --native-v2 --jobs 1
cargo run -- test262 --native-v3 --jobs 1
cargo run -- test262 --native-v4 --jobs 1
cargo run -- test262 --native-v5 --jobs 1
cargo run -- test262 --native-v6 --jobs 1
cargo run -- test262 --native-v6-scan --jobs 4
```

The scan percentage is diagnostic. Only the pinned gate is a merge blocker,
and skipped tests never count as passes.
