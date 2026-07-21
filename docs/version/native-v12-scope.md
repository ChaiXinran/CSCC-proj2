# Native V12 Scope: BigInt, Backend Simplification, and VM Scheduling

Native V12 is a repair continuation focused on removing the embedded Boa
dispatch path, unblocking BigInt correctness beyond the current `i128` range,
and preparing the VM for explicit-frame scheduling. It also includes the
highest-priority semantic fixes identified during final preparation:
`Function.prototype.call` / `apply`, per-iteration `const` loop bindings,
computed update evaluation order, and symbol property keys.

Shared contracts are defined in
[Native V12 Shared Interface](native-v12-interface.md), and file ownership is
defined in [Native V12 Team Plan](native-v12-team-plan.md).

Existing V12 reports may already contain earlier iterator, promise, allocation,
and numeric fast-path work. This scope treats those entries as historical V12
content. New work must append dated sections to the same V12 report files
instead of rewriting prior results.

## 1. Baseline

V12 starts from the current native-only default build:

```text
cargo run --release --no-default-features -- eval "1 + 2"
```

Boa remains a valid external comparison engine through the pinned submodule,
but it is no longer part of the AgentJS runtime dispatch surface after the
de-Boa track merges:

```text
cargo build --release --manifest-path boa/Cargo.toml -p boa_cli
```

The BigInt baseline is intentionally limited:

- `JsValue::BigInt(i128)` stores only native signed 128-bit values.
- `parse_bigint_literal` in `src/bytecode/compiler.rs` rejects literals outside
  the `i128` range.
- BigInt parsing logic is duplicated in VM and builtin paths.
- BigInt operators are partially implemented, but overflow is reported as a
  native range error instead of using arbitrary precision.
- `BigInt()` and several prototype/static methods exist, but their internal
  representation is range-limited.

## 2. V12 Tracks

### V12-A - Frontend / Backend Surface

Owner: A group.

Scope:

- remove the embedded Boa backend from AgentJS dispatch;
- preserve external Boa CLI comparison instructions;
- update CLI help, Cargo features, runtime tests, and documentation;
- migrate BigInt literal compilation to the shared BigInt parser;
- fix `for (const x of iterable)` per-iteration binding lowering.

Expected effect:

- native-only build and public architecture are simpler;
- external comparison experiments remain reproducible;
- BigInt literals no longer fail only because they exceed `i128`;
- `const` for-of loops stop reusing an initialized binding across iterations.

### V12-B - BigInt Runtime and Correctness

Owner: B group.

Scope:

- introduce the shared `BigIntValue` representation;
- centralize BigInt literal/string parsing and radix formatting;
- update `JsValue`, constants, primitive wrappers, JSON, typed-array, Temporal,
  and builtin call sites that currently assume `i128`;
- implement arbitrary-precision arithmetic, bitwise, shift, comparison,
  increment/decrement, `BigInt()`, `BigInt.asIntN`, and `BigInt.asUintN`;
- tighten `Function.prototype.call` / `apply` semantics where they interact
  with builtin forwarding and bound functions.

Expected effect:

- BigInt can parse, compile, and execute values beyond signed 128-bit range;
- mixed `Number`/`BigInt` arithmetic continues to throw `TypeError`;
- BigInt behavior is owned by one runtime module instead of scattered helpers.

### V12-C - VM / PropertyKey / Scheduling

Owner: C group.

Scope:

- preserve symbol keys through a shared `PropertyKey` path instead of forcing
  computed keys through string conversion;
- fix computed update expressions such as `obj[key]++` so object, key, and
  getter side effects are evaluated exactly once;
- design and stage an explicit-frame VM scheduler;
- remove dead VM/builtin code and merge duplicate AST/tree traversals only
  after semantic fixes land;
- coordinate script-cache and bytecode stack-analysis cleanup with A/B.

Expected effect:

- symbol property access is no longer rejected by string-only coercion paths;
- computed update operators match ECMAScript evaluation order;
- recursive Rust calls for JS user functions can be migrated toward a
  trampoline-style frame loop without changing synchronous JS ordering.

## 3. Explicit Non-Goals

V12 does not include:

- browser or Web API compatibility;
- full Temporal/Intl conformance beyond call sites touched by BigInt storage;
- changing Test262 skips into passes without actually running them;
- replacing the native VM with Boa or silently falling back to Boa;
- fair scheduling of ordinary synchronous JS calls;
- a complete async-generator or module-cycle rewrite;
- broad formatting or style-only churn outside touched files.

## 4. Focused Commands

### V12-A

```sh
cargo check --no-default-features --all-targets
cargo test --no-default-features runtime
cargo test --no-default-features parser_bigint
cargo run --release --no-default-features -- eval "1n + 2n"
```

### V12-B

```sh
cargo test --no-default-features parser_bigint
cargo test --no-default-features native_bigint
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/BigInt --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-builtins-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/bigint --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-literals-summary.json
```

### V12-C

```sh
cargo test --no-default-features native_symbol
cargo test --no-default-features native_compound_assignment
cargo test --no-default-features native_object_keys
cargo test --no-default-features --test native_test262
```

### V12 Integration

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

If a V12 scan selector is added later, use the locked V12 manifest and summary
path documented with that selector. Until then, do not claim a V12 scan result.

## 5. Completion Criteria

V12 is complete only when:

- the embedded Boa backend is removed from runtime dispatch and CLI selection;
- external Boa comparison commands remain documented;
- `JsValue::BigInt` no longer stores raw `i128`;
- all BigInt parsing routes use the shared parser;
- BigInt arithmetic, bitwise, shift, comparison, `BigInt()`, prototype methods,
  `asIntN`, and `asUintN` have focused coverage;
- `Function.prototype.call` / `apply`, `for const...of`, computed update
  evaluation order, and symbol property keys have regression tests;
- the VM scheduler design is documented before any large interpreter rewrite;
- each touched track appends to its V12 part report with commands and deltas;
- old V1-V11 gates remain green or documented with pre-existing blockers.
