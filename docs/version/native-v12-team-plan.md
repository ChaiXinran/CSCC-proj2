# Native V12 Team Plan

V12 is a three-person repair continuation. The team works in parallel on
separate surfaces, but the plan intentionally limits simultaneous edits to
`src/vm/interpreter.rs` and `src/runtime/value.rs`, because those files sit on
the shared execution path for every requested fix.

Shared contracts in `native-v12-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v12-repair-contracts
codex/v12-a-de-boa-frontend
codex/v12-b-bigint-runtime
codex/v12-c-vm-propertykey-scheduler
codex/v12-integration
```

Recommended merge order:

```text
V12 contracts
  -> A de-Boa backend surface
  -> B BigIntValue substrate
  -> B BigInt VM/builtin migration
  -> C symbol PropertyKey path
  -> focused correctness fixes
  -> C VM scheduler prototype
  -> cleanup, scans, and reports
```

Reports:

- A appends to `reports/.version-report/v12-partA-report.md`.
- B appends to `reports/.version-report/v12-partB-report.md`.
- C appends to `reports/.version-report/v12-partC-report.md`.

If `v12-partA-report.md` does not exist, A creates it. B/C must preserve older
V12 report sections and add dated sections for new work.

## 2. A Group - Frontend and Backend Surface

Owned files:

```text
Cargo.toml
src/backend/
src/main.rs
src/lib.rs
src/test262.rs
src/lexer/
src/parser/
src/bytecode/compiler.rs
tests/parser_bigint.rs
tests/runtime.rs
README.md
docs/status.md
```

Primary tasks:

- Remove the embedded Boa backend feature and dispatch path.
- Remove `--backend boa` from CLI parsing and help.
- Keep `--backend native` or simplify backend parsing only after coordinating
  with B/C test commands.
- Preserve external Boa CLI comparison instructions in README/benchmark docs.
- Update runtime tests that currently assert embedded Boa selectability.
- Migrate BigInt literal compilation from local `i128` parsing to B's shared
  `runtime::bigint::parse_bigint_literal`.
- Fix `for (const x of iterable)` by creating a fresh per-iteration lexical
  binding instead of reusing an initialized binding.

A must not:

- change `JsValue::BigInt` representation directly;
- rewrite VM operator semantics;
- implement property-key symbol logic in one-off compiler code.

Independent validation:

```sh
cargo check --no-default-features --all-targets
cargo test --no-default-features runtime
cargo test --no-default-features parser_bigint
cargo test --no-default-features parser_iteration
cargo run --release --no-default-features -- eval "1 + 2"
```

Required report:

- `reports/.version-report/v12-partA-report.md`

## 3. B Group - BigInt Runtime and Call/Apply

Owned files:

```text
src/runtime/value.rs
src/runtime/bigint.rs
src/runtime/object.rs
src/runtime/mod.rs
src/bytecode/chunk.rs
src/builtins/std_primitives.rs
src/builtins/binary_data.rs
src/builtins/json.rs
src/builtins/date_intl.rs
tests/native_bigint.rs
tests/parser_bigint.rs
```

Shared review files:

```text
src/vm/interpreter.rs
src/builtins/function.rs
src/runtime/function.rs
```

Primary tasks:

- Add `src/runtime/bigint.rs`.
- Replace raw `i128` BigInt storage with `BigIntValue`.
- Update `JsValue`, `PrimitiveValue`, `Constant`, and GC/estimated-byte logic.
- Centralize literal parsing, string parsing, radix formatting, and exact
  BigInt/Number comparison.
- Move VM BigInt operators to shared helpers.
- Update `BigInt()`, `BigInt.prototype.*`, `BigInt.asIntN`, and
  `BigInt.asUintN`.
- Audit typed-array, DataView, Temporal, JSON, and primitive wrapper paths that
  pattern-match `JsValue::BigInt(i128)`.
- Tighten `Function.prototype.call` / `apply` forwarding and bound-function
  behavior after BigInt storage compiles cleanly.

B must not:

- remove embedded Boa dispatch; A owns that surface;
- rewrite the VM frame scheduler; C owns that surface;
- add parser-specific BigInt behavior outside the shared parser contract.

Independent validation:

```sh
cargo test --no-default-features parser_bigint
cargo test --no-default-features native_bigint
cargo test --no-default-features native_primitives
cargo test --no-default-features native_typed_arrays
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/BigInt --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-builtins-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/bigint --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-literals-summary.json
```

Required report:

- `reports/.version-report/v12-partB-report.md`

## 4. C Group - VM, PropertyKey, Scheduler, and Cleanup

Owned files:

```text
src/vm/
src/runtime/context.rs
src/runtime/property.rs
src/runtime/property_map.rs
src/builtins/object.rs
src/builtins/proxy.rs
tests/native_symbol.rs
tests/native_object_keys.rs
tests/native_compound_assignment.rs
```

Primary tasks:

- Route computed property operations through `PropertyKey` so symbol keys are
  accepted where ECMAScript allows them.
- Fix computed update expressions so object/key/getter/setter side effects are
  observed exactly once.
- Add any required bytecode support for precomputed computed-member stores.
- Draft the explicit-frame VM scheduler design before large interpreter edits.
- Prototype ordinary user-function trampoline scheduling only after B's BigInt
  VM migration lands.
- Remove dead code and merge duplicated tree traversals after semantic fixes.
- Evaluate script-cache stack-analysis reuse only after scheduler direction is
  stable.

C must not:

- modify BigInt storage before B's interface lands;
- change frontend parsing rules for loop bindings;
- introduce fair or round-robin scheduling for ordinary synchronous JS calls.

Independent validation:

```sh
cargo test --no-default-features native_symbol
cargo test --no-default-features native_object_keys
cargo test --no-default-features native_compound_assignment
cargo test --no-default-features --test native_test262
```

Scheduler validation, after the prototype:

```sh
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

Required report:

- `reports/.version-report/v12-partC-report.md`

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `Cargo.toml`, `src/backend/`, CLI backend parsing | A | Remove embedded Boa first; B/C rebase tests after |
| `src/runtime/value.rs` | B | No C scheduler changes until BigIntValue compiles |
| `src/runtime/bigint.rs` | B | Single home for BigInt parsing and operations |
| `src/bytecode/compiler.rs` | A | BigInt literal lowering must call B helper |
| `src/bytecode/chunk.rs` | B with A review | Constant representation affects compiler and VM |
| `src/vm/interpreter.rs` | B then C | B migrates BigInt helpers first; C owns later scheduler |
| `src/runtime/context.rs` | C with B review | PropertyKey changes can affect primitive wrappers |
| `src/builtins/std_primitives.rs` | B | BigInt constructor/prototype/static methods |
| `src/builtins/function.rs` | B with C review | call/apply forwarding touches VM call paths |
| `src/builtins/object.rs`, `src/builtins/proxy.rs` | C | Use shared PropertyKey helpers |
| `src/test262.rs` | shared | Do not add scan selectors without locked manifest |
| `docs/version/native-v12-*.md` | all groups | Contract updates before shared-file changes |

## 6. Integration Gate

Before V12 repair is considered complete:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262
```

Focused summaries should also be current:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/BigInt --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-builtins-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/bigint --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-b-bigint-literals-summary.json
```

If the scheduler prototype lands, add:

```sh
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

Reports and docs to update after integration:

- `reports/.version-report/v12-partA-report.md`
- `reports/.version-report/v12-partB-report.md`
- `reports/.version-report/v12-partC-report.md`
- `docs/status.md`
- `AGENTS.md` if commands or ownership rules change
- `README.md` for de-Boa and external Boa comparison wording
