# Native V9 Scope: Execution Semantics Expansion

Native V9 starts after the V8 module-runner/builtin-skeleton setup. V9 is a
three-track feature batch focused on execution semantics that unblock async,
iterator, Promise, and collection-heavy Test262 areas.

Shared contracts are defined in
[Native V9 Shared Interface](native-v9-interface.md), and file ownership is
defined in [Native V9 Team Plan](native-v9-team-plan.md).

## 1. Baseline

V9 uses the locked V9 lightweight scan manifest:

```text
reports/native-v9-scan-failures.txt
```

The manifest contains 5,000 previously non-passing Test262 cases sampled from
the full direct output, prioritizing:

- `test/language/statements/for-of`
- `test/language/statements/for-await-of`
- async/generator/yield language areas
- `test/built-ins/Promise`
- `test/built-ins/Iterator`
- `test/built-ins/Map`
- `test/built-ins/Set`
- `test/built-ins/WeakMap`
- `test/built-ins/WeakSet`

Initial scan command:

```sh
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

Initial result:

```text
5,000 total, 0 passed, 5,000 failed, 0 skipped, 0.00%
```

## 2. V9 Tracks

### V9-A — Async / Generator / For-of Frontend Lowering

Owner: A group.

Scope:

- generator function and `yield` syntax/lowering;
- async function and `await` syntax/lowering;
- async generator parser;
- `for...of` lowering;
- `for await...of` parser with minimal lowering.

Expected effect:

- reduce parser/compile failures in modern language control-flow areas;
- expose runtime iterator and Promise failures to B group.

### V9-B — Promise / Job Queue / Iterator Runtime

Owner: B group.

Scope:

- minimal Promise runtime substrate;
- microtask/job queue model;
- iterator protocol runtime helpers;
- iterator close behavior;
- async Test262 completion support where it depends on native job draining.

Expected effect:

- support Promise-focused tests;
- support async/for-await tests once A lowers syntax;
- provide shared iterator helpers used by C builtins.

### V9-C — Map / Set / Iterator Builtins

Owner: C group.

Scope:

- `Map`;
- `Set`;
- `WeakMap` / `WeakSet` skeletons and core observable behavior;
- `Iterator` constructor/prototype/helper skeletons;
- high-signal iterator helper methods from Test262 hotspots.

Expected effect:

- reduce missing-global failures for collection and iterator builtins;
- convert missing constructor failures into descriptor/semantic failures.

## 3. Explicit Non-Goals

V9 does not include:

- full Temporal semantics;
- full Intl formatting;
- complete TypedArray algorithms;
- full module live-binding/cycle semantics;
- RegExp property escapes/backreferences;
- complete descriptor sweep for all builtins;
- spec-perfect async generator scheduling beyond the minimal V9 target.

## 4. Focused Commands

### V9-A

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress --json reports/native-v9-a-forof-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress --json reports/native-v9-a-forawait-summary.json
```

### V9-B

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress --json reports/native-v9-b-promise-summary.json
```

### V9-C

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Map --jobs 4 --progress --json reports/native-v9-c-map-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Set --jobs 4 --progress --json reports/native-v9-c-set-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress --json reports/native-v9-c-iterator-summary.json
```

### V9 Integration

```sh
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

## 5. Completion Criteria

V9 is complete only when:

- all three tracks have merged;
- old V1-V8 regression gates remain green;
- focused A/B/C summaries are current;
- `--native-v9-scan` has a current JSON summary;
- each `reports/v9-part*-report.md` records changes and deltas;
- `AGENTS.md`, `readme.md`, `docs/status.md`, and `thoughts/newplan.md` reflect
  the V9 status;
- skipped tests are never counted as passes.
