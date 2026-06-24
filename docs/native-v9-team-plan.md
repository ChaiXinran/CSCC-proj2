# Native V9 Team Plan

V9 is a three-track execution-semantics batch. The three groups work in
parallel on different feature families, then merge into one V9 integration
pass.

Shared contracts in `native-v9-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v9-contracts
feat/v9-a-async-generator-forof
feat/v9-b-promise-job-iterator-runtime
feat/v9-c-map-set-iterator-builtins
test/v9-integration
```

Recommended merge order:

```text
V9 contracts
  -> shared AST/runtime helper connector patches
  -> V9-A frontend lowering
  -> V9-B Promise/job/iterator runtime
  -> V9-C collection/iterator builtins
  -> V9 integration reports and docs
```

## 2. A Group — Async / Generator / For-of Lowering

Owned files:

```text
src/lexer/
src/parser/
src/ast/
src/bytecode/compiler.rs
tests/frontend_v9.rs
```

Tasks:

- generator functions and `yield`;
- async functions and `await`;
- async generator parser;
- `for...of` lowering;
- `for await...of` parser with minimal lowering.

A must not implement Promise storage, job queue, Map/Set storage, or Iterator
builtins.

Independent validation:

```sh
cargo test --no-default-features frontend_v9
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress --json reports/native-v9-a-forof-summary.json
```

Required report:

- `reports/v9-partA-report.md`

## 3. B Group — Promise / Job Queue / Iterator Runtime

Owned files:

```text
src/runtime/
src/vm/
src/backend/
src/contracts.rs
tests/native_v9_runtime.rs
```

Tasks:

- minimal Promise state model;
- deterministic microtask/job queue;
- native job draining connected to `ExecutionOptions::drain_jobs`;
- iterator runtime helpers;
- iterator close on abrupt completion;
- async Test262 completion support.

B must not install collection builtin skeletons directly; C owns those globals.

Independent validation:

```sh
cargo test --no-default-features native_v9_runtime
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress --json reports/native-v9-b-promise-summary.json
```

Required report:

- `reports/v9-partB-report.md`

## 4. C Group — Map / Set / Iterator Builtins

Owned files:

```text
src/builtins/
src/runtime/
tests/native_v9_builtins.rs
```

Tasks:

- `Map`;
- `Set`;
- `WeakMap` / `WeakSet` skeletons and core behavior;
- `Iterator` constructor/prototype/helper skeletons;
- high-signal iterator helper methods from the V9 scan.

C must coordinate with B before adding shared collection storage or iterator
runtime helpers.

Independent validation:

```sh
cargo test --no-default-features native_v9_builtins
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Map --jobs 4 --progress --json reports/native-v9-c-map-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Set --jobs 4 --progress --json reports/native-v9-c-set-summary.json
```

Required report:

- `reports/v9-partC-report.md`

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `src/lexer/`, `src/parser/`, `src/ast/` | A | B/C request syntax changes through interface docs |
| `src/bytecode/compiler.rs` | A, with B review | Iterator/job runtime helper calls require B review |
| `src/runtime/` | B, with C coordination | C collection storage helpers require B coordination |
| `src/vm/` | B | A may request opcode support through interface docs |
| `src/builtins/` | C | Must use runtime object-model and iterator APIs |
| `src/test262.rs` | shared | Keep V9 scan selector stable |
| `docs/native-v9-*.md` | all groups | Contract updates before shared-file changes |

## 6. Integration Gate

Before V9 is considered complete:

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

Initial V9 scan baseline: 0/5,000 passed, 5,000 failed, 0 skipped.

Focused summaries should also be current:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress --json reports/native-v9-a-forof-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress --json reports/native-v9-b-promise-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Map --jobs 4 --progress --json reports/native-v9-c-map-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Set --jobs 4 --progress --json reports/native-v9-c-set-summary.json
```

Reports and docs to update after integration:

- `reports/v9-partA-report.md`
- `reports/v9-partB-report.md`
- `reports/v9-partC-report.md`
- `reports/native-v9-scan-summary.json`
- `docs/status.md`
- `AGENTS.md`
- `readme.md`
- `thoughts/newplan.md`
