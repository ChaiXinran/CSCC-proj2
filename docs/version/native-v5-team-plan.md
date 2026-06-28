# Native V5 Team Plan

V5 starts in parallel with the remaining V4 repair. The immediate objective is
to finish contracts and conflict-light A/B/D work without editing V4 runtime
hot spots.

## 1. Branch and Merge Strategy

Create branches from the latest agreed stable commit:

```text
docs/v5-contracts
feat/v5-parser_basics
feat/v5-bytecode
test/v5-test262
feat/v5-runtime       # starts after V4 repair merges
```

Recommended merge order:

```text
V5 contracts
  -> A parser_basics
  -> B bytecode
  -> V4 repair
  -> C VM/runtime
  -> D integration/Test262
```

No V5 branch should merge partial runtime work into `main` before the V4 repair
branch is accepted.

## 2. A Group — Lexer, AST, Parser

Owned files:

```text
src/lexer/
src/ast/
src/parser/
tests/parser_closures.rs
```

Tasks:

- add `try`, `catch`, `finally`, `switch`, `case`, and `default`;
- implement the frozen `Try` and `Switch` AST;
- parse optional catch binding;
- allow `break` in switch but reject switch-only `continue`;
- enforce one default clause and required try handler/finalizer;
- keep `let`/`const` declarations represented by existing `VariableKind`.

Validation is source/tokens to AST only. Do not edit compiler or runtime code.

Current status: **completed**.

Delivered:

- V5 keyword tokenization, including previously unrecognized `let`/`const`;
- frozen `CatchClause`, `SwitchCase`, `Try`, and `Switch` AST nodes;
- all three try forms and optional catch binding;
- ordered switch cases, fall-through, one optional default, and switch `break`;
- `let`/`const` parsing and required `const` initializers;
- duplicate lexical declaration checks for scripts, blocks, functions, catch
  bodies, and the shared switch lexical scope;
- nested functions no longer inherit outer loop/switch breakable contexts;
- independent coverage in `tests/parser_closures.rs`.

The only cross-owner change is a compiler compatibility arm that returns an
explicit `CompileError` for V5 statements until B implements bytecode lowering.

## 3. B Group — Bytecode Compiler

Owned files:

```text
src/bytecode/
tests/bytecode_closures.rs
tests/parser_closures_bytecode.rs
```

Tasks:

- add handler tables and lexical-environment instructions;
- validate handler ranges, targets, stack depth, and environment balance;
- compile switch using strict equality and existing jumps;
- patch switch `break` targets independently from loop targets;
- compile lexical declarations into create-then-initialize phases;
- test try/finally using hand-built AST before VM integration.

B must not encode catch/finally using magic builtin calls.

Current status: **completed**.

Delivered:

- `ExceptionHandler`/`HandlerKind` tables with stack and environment restore
  depths;
- handler range, target, stack-depth, and lexical-environment validation;
- fixed stack contracts for duplicate, exception, lexical binding, and
  finally instructions;
- `switch` lowering through `Duplicate`, `StrictEqual`, and generic jumps;
- independent switch break targets with nested lexical-environment cleanup;
- catch/finally handler metadata and catch exception loading;
- two-phase `let`/`const` creation and initialization, including lexical name
  loads/stores;
- hand-built AST/Chunk tests in `tests/bytecode_closures.rs`;
- A→B source integration in `tests/parser_closures_bytecode.rs`.

The VM contains only a compatibility rejection for unimplemented V5 runtime
instructions; execution semantics remain C-group work.

## 4. C Group — VM and Runtime

Start only after the V4 repair baseline is merged.

Owned files:

```text
src/vm/
src/runtime/environment.rs
src/runtime/context.rs
src/contracts.rs
tests/native_closures_runtime.rs
```

Tasks:

- propagate JavaScript `Completion` through nested execution;
- implement handler lookup and stack/environment restoration;
- implement catch binding and finally override semantics;
- expose lexical binding initialization, TDZ, and const errors;
- ensure return/throw/break/continue all clean up environments;
- preserve V4 object, function, and builtin behavior.

Changes to `runtime/value.rs`, `builtins/`, or object storage require explicit
review; V5 Core should not need them.

## 5. D Group — Integration and Test262

Owned files:

```text
src/test262.rs
src/main.rs
tests/native_closures.rs
tests/native_test262.rs
reports/.test262/test262-analysis/native-v5-test262-report.md
.github/workflows/ci.yml
readme.md
```

Tasks:

- prepare `--native-v5` without claiming unsupported tests as passes;
- record baseline scans for try, switch, let, and const directories;
- select a small zero-skip pinned gate for each completed feature;
- retain V1–V4 regression gates;
- classify parser, compiler, runtime, harness, timeout, and unsupported failures.

## 6. Shared-File Lock

| File | Owner while V4 repair is active |
| --- | --- |
| `src/contracts.rs` | V5 contract branch, then C |
| `src/ast/*` | A |
| `src/bytecode/*` | B |
| `src/runtime/context.rs` | V4 repair team |
| `src/runtime/value.rs` | V4 repair team |
| `src/vm/interpreter.rs` | V4 repair team |
| `src/builtins/*` | V4 repair team |
| `src/test262.rs` | D |

## 7. Merge Gate

Every V5 merge runs:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run -- test262 --native-v1 --jobs 1
cargo run -- test262 --native-v2 --jobs 1
cargo run -- test262 --native-v3 --jobs 1
cargo run -- test262 --native-v4 --jobs 4
```

Once available, append the pinned `--native-v5` gate. Diagnostic V4/V5
directory pass rates are reported separately from fixed acceptance gates.
