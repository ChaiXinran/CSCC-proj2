# V9 Part C Report — Map / Set / Iterator Builtins

Owner: C group
Scope: builtins / collection objects / iterator builtin surface

This report must be updated by any worker or AI agent that changes V9-C code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v9-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V9 hotspots.

| Metric | Baseline |
| --- | ---: |
| V9 scan total | 5,000 |
| V9 scan passed | 0 |
| V9 scan failed | 5,000 |
| V9 scan skipped | 0 |

Primary directories:

- `test/built-ins/Map`
- `test/built-ins/Set`
- `test/built-ins/WeakMap`
- `test/built-ins/WeakSet`
- `test/built-ins/Iterator`

## Current Status

Status: basic C-track collection and iterator builtins implemented.

V9-C now installs the `Map`, `Set`, `WeakMap`, `WeakSet`, and `Iterator`
globals. The implementation covers constructor/prototype/descriptor shape,
basic ordered Map/Set storage, weak collection object-key behavior, collection
iterators, and a small eager Iterator helper surface. This is intentionally not
a complete ES collection/iterator implementation; complete iterator-close,
species construction, Set algebra, Map grouping, weak reachability semantics,
and lazy iterator helper pipelines remain future bug-fix or feature work.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | C / Codex | Implemented V9-C basic collection/iterator builtins: Map/Set ordered storage and iteration, WeakMap/WeakSet object-key storage, Iterator constructor/prototype plus eager helper basics, and focused regression tests | `src/builtins/v9.rs`, `src/builtins/mod.rs`, `tests/native_v9_builtins.rs`, `reports/v9-partC-report.md`; local generated summaries `reports/native-v9-c-map-summary.json`, `reports/native-v9-c-set-summary.json`, `reports/native-v9-c-iterator-summary.json` | `cargo fmt --all`; `cargo test --no-default-features --test native_v9_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262`; `cargo test --no-default-features --test native_v8_builtins`; focused Map/Set/Iterator Test262 commands listed below | V9-C unit test 9/9 passed; all-target check passed; native Test262 selector/regression tests 13/13 passed; V8-C regression 10/10 passed; Map 139/204 passed, Set 198/383 passed, Iterator 96/514 passed |
| 2026-06-24 | setup | Created V9-C report template and locked V9 scan manifest | `reports/v9-partC-report.md`, `reports/native-v9-scan-failures.txt`, `reports/native-v9-scan-summary.json` | `cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json` | baseline: 0/5000 passed, 5000 failed, 0 skipped |

## Implemented Functionality

- `Map`:
  constructor-only call behavior, null/undefined initializer support, array-like
  pair initialization, `get`, `set`, `has`, `delete`, `clear`, `size`,
  `keys`, `values`, `entries`, `forEach`, `Symbol.iterator`,
  `Symbol.species`, and `Symbol.toStringTag`.
- `Set`:
  constructor-only call behavior, null/undefined initializer support, array-like
  value initialization, `add`, `has`, `delete`, `clear`, `size`, `values`,
  `keys`, `entries`, `forEach`, `Symbol.iterator`, `Symbol.species`, and
  `Symbol.toStringTag`.
- `WeakMap` / `WeakSet`:
  constructor/prototype skeletons and core object-key behavior for `get`/`set`/
  `add`/`has`/`delete`; non-object keys are rejected where mutation requires it.
  Storage is intentionally strong hidden-slot storage in this C-track pass.
- `Iterator`:
  constructor/prototype/global shape, `Iterator.from` for existing iterator
  objects, `next`, `values`, `Symbol.iterator`, `toArray`, `forEach`, `some`,
  `every`, and `find`. Lazy pipeline helpers (`map`, `filter`, `take`, `drop`,
  `flatMap`, `reduce`) are installed as explicit TypeError skeletons.
- Collection iterators:
  Map/Set iterator objects preserve insertion order for active entries and
  produce standard `{ value, done }` result objects for `next()`.

## Test Results and Delta Analysis

Initial V9 scan baseline:

```text
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
```

Result: 5,000 total, 0 passed, 5,000 failed, 0 skipped, 0.00%.

Current C-track checks:

```text
cargo fmt --all
cargo test --no-default-features --test native_v9_builtins
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test native_v8_builtins
```

Results:

- V9-C unit test: 9 passed, 0 failed.
- All-target check: passed.
- Native Test262 selector/regression tests: 13 passed, 0 failed.
- V8-C regression: 10 passed, 0 failed.

Focused V9-C diagnostics:

```text
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Map --jobs 4 --json reports/native-v9-c-map-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Set --jobs 4 --json reports/native-v9-c-set-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress --json reports/native-v9-c-iterator-summary.json
```

Results:

- Map: 204 total, 139 passed, 65 failed, 0 skipped, 68.14%.
- Set: 383 total, 198 passed, 185 failed, 0 skipped, 51.70%.
- Iterator: 514 total, 96 passed, 418 failed, 0 skipped, 18.68%.

`--native-v9-scan` was not rerun in this pass. The requested goal was basic
V9-C functionality; complete-suite scan deltas should drive later bug-fix work,
not a broad semantic expansion in this patch.

## Open Risks / Coordination Notes

- Coordinate shared collection storage with B group.
- Use normal object-model APIs for constructor/prototype/descriptor shape.
- Map/Set storage currently uses hidden ordinary properties with linear scans;
  this preserves observable insertion order for the implemented methods, but a
  shared runtime collection substrate should replace it before complete
  semantics or large workloads.
- Remaining focused failures are expected for this basic pass: complete iterator
  protocol integration, iterator close on abrupt completion, `Iterator.concat`,
  `Iterator.zip`/`zipKeyed`, lazy helper pipelines, Set algebra methods,
  `Map.groupBy`, `Map.prototype.getOrInsert*`, full subclass/species behavior,
  cross-realm `$262.createRealm`, BigInt key distinctions, and frontend parser
  gaps in newer Test262 helper syntax.
