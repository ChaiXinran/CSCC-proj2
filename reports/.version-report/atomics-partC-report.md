# Atomics Part C Report

Date: 2026-07-30

## Owner And Scope

Part C is limited to the Test262 agent harness and Atomics runtime support:

- `$262.agent.start`, `broadcast`, `receiveBroadcast`, `report`, `getReport`,
  `sleep`, `monotonicNow`, and `leaving`.
- Per-`NativeContext` worker, report queue, waiter, notification, timeout, and
  monotonic-clock state.
- `Atomics.wait`, `Atomics.waitAsync`, and `Atomics.notify` integration.
- Atomics index/count coercion needed by the focused suite.

No Temporal, RegExp, parser, lexer, module, or upstream Test262 files were
changed.

## Implementation

- Added a per-runtime `AgentManager`; each Test262 case receives fresh agent
  state and therefore cannot leak workers, reports, or waiters into another
  case.
- Agent source registers a broadcast receiver, and broadcasts run registered
  receivers against the shared buffer value.
- Reports are queued in insertion order. Cooperative waiter markers are
  resolved to `ok` or `timed-out` when notified or timed out.
- Agent callbacks are included in GC roots.
- `Atomics.notify` now returns the number of matching waiters actually woken,
  with an omitted count treated as positive infinity.
- `Atomics.wait` and `Atomics.waitAsync` register matching waits, preserve
  `not-equal` and zero-timeout behavior, and expose the required async result
  object/Promise shape.
- Undefined Atomics indices correctly coerce to zero.

The manager is cooperative and owns no operating-system worker threads, so
the worker-thread count is zero at every test boundary.

## Correctness Delta

Locked baseline:

```text
test/built-ins/Atomics
total=390 passed=270 failed=118 skipped=2
```

Final release result:

```text
test/built-ins/Atomics
total=390 passed=372 failed=16 skipped=2
conformance=95.38%
```

Net change: **+102 passed, -102 failed, unchanged skips**.

## Stability Verification

```text
jobs=1:
372 passed / 16 failed / 2 skipped

jobs=4 run 1:
372 passed / 16 failed / 2 skipped

jobs=4 run 2:
372 passed / 16 failed / 2 skipped

jobs=4 run 3:
372 passed / 16 failed / 2 skipped
```

Artifacts:

- `reports/.native-test262-tmp/c-atomics-release-j1.json`
- `reports/.native-test262-tmp/c-atomics-release-j4-run1.json`
- `reports/.native-test262-tmp/c-atomics-release-j4-run2.json`
- `reports/.native-test262-tmp/c-atomics-release-j4-run3.json`

## Rust Gates

```text
cargo fmt --all -- --check
PASS

cargo build --release --no-default-features
PASS

cargo test --no-default-features --all-targets
PASS

cargo clippy --no-default-features --all-targets -- -D warnings
PASS
```

## Remaining Atomics Failures

The 16 remaining failures are confined to:

- four advanced multi-waiter ordering/report-timing cases;
- four notified-wait duration edge cases;
- two `Atomics.store` coercion cases;
- four finite `waitAsync` Promise scheduling/timeout cases;
- two boolean-timeout async completion cases.

They are failures, not newly skipped cases.

## Module Runtime Continuation

Date: 2026-07-31

After the Atomics stage, Part C continued with the explicitly assigned module
runtime, dynamic import, import attributes, and JobQueue-facing module work.

Implemented:

- Static modules with dependencies now use the same load, instantiate, link,
  namespace, and evaluate graph used by dynamic import.
- Dependency-free modules retain the direct execution path, avoiding
  regressions in syntax-only top-level-await cases.
- Exported `var` destructuring bindings participate in module early-error
  validation and bytecode hoisting.
- Module declaration instantiation creates all destructuring binding cells.
- `import.source()` preserves its source phase and rejects source-text modules
  with a `SyntaxError` Promise.
- Static and dynamic import attributes validate and preserve the module type.
- JSON, text, and bytes requests create synthetic default-export modules.
- Bytes-module ArrayBuffers are marked immutable before importer evaluation.
- `import defer * as ns` is represented separately in the module record;
  deferred dependencies are linked but are not eagerly evaluated.

Locked baselines and final debug verification:

```text
module-code:    426 / 599 -> 500 / 599   +74
import:          14 / 127 ->  72 / 127   +58
dynamic-import: 941 /1004 -> 980 /1004   +39
------------------------------------------------
net module gain                         +171
```

Atomics remained unchanged at `372 / 390`, with 2 pre-existing skips.

Release `jobs=1` and `jobs=4` produced the same three failure/pass sets:

```text
module-code:    500 passed / 99 failed / 0 skipped
import:          72 passed / 55 failed / 0 skipped
dynamic-import: 980 passed / 24 failed / 0 skipped
```

Module artifacts:

- `reports/.native-test262-tmp/c-module-final-release-j1.json`
- `reports/.native-test262-tmp/c-module-final-release-j4.json`
- `reports/.native-test262-tmp/c-import-final-release-j1.json`
- `reports/.native-test262-tmp/c-import-final-release-j4.json`
- `reports/.native-test262-tmp/c-dynamic-final-release-j1.json`
- `reports/.native-test262-tmp/c-dynamic-final-release-j4.json`

Continuation gates:

```text
cargo fmt --all -- --check
PASS

cargo build --release --no-default-features
PASS

cargo test --no-default-features --all-targets
PASS

cargo clippy --no-default-features --all-targets -- -D warnings
PASS
```

No Test262 input, Temporal, Intl, or RegExp implementation file was changed.

## Module / Async Runtime Second Continuation

Date: 2026-07-31

This continuation stayed inside Part C ownership: module loading/linking,
namespace behavior, dynamic import, Promise-facing async generator intrinsics,
and JobQueue-triggered deferred evaluation.

Implemented:

- module namespace keys are materialized during graph instantiation and kept
  synchronized with exported binding updates;
- namespace operations trigger a linked deferred module through the shared
  module lifecycle instead of an import-site special case;
- re-entering an already instantiated graph reuses its environments and
  namespace objects;
- nested module throws preserve their JavaScript error kind and value;
- module-item parsing distinguishes static `import` declarations from
  expression-position `import()` and `import.defer()`;
- anonymous default exports receive the public name `default`;
- async generator functions now receive the required prototype object, shared
  async-generator methods, and the async-iterator prototype chain.

Release scoped comparison against the immediately preceding Part C results:

```text
module-code:    500 / 599 -> 522 / 599   +22
import:          72 / 127 ->  88 / 127   +16
dynamic-import: 980 /1004 -> 996 /1004   +16
------------------------------------------------
second-continuation module gain           +54
```

The C4 async completion expansion used fixed before/after directories. Across
module-code, import, dynamic-import, Atomics, Promise, async-function,
async-generator, for-await-of, AsyncGeneratorPrototype,
AsyncIteratorPrototype, and async-generator expressions:

```text
before: 4780 passed
after:  4859 passed
net:     +79
```

The two async-generator source-form directories each lost one passing case;
these are recorded rather than hidden. The main positive async changes were
`AsyncGeneratorPrototype` `18 -> 32` and `AsyncIteratorPrototype` `0 -> 13`.

Against the locked module-stage baseline recorded above, the three assigned
module directories now total:

```text
module-code:    426 -> 522   +96
import:          14 ->  88   +74
dynamic-import: 941 -> 996   +55
--------------------------------
cumulative module gain       +225
```

Thus the cumulative module-stage target exceeds 200. The stricter
"immediately preceding result plus another 200" interpretation is not met by
this continuation; its independently measured net gain is +79.

Final scoped artifacts:

- `reports/.native-test262-tmp/c2-final-module.json`
- `reports/.native-test262-tmp/c2-final-import.json`
- `reports/.native-test262-tmp/c2-final-dynamic.json`
- `reports/.native-test262-tmp/c2-final-atomics.json`
- `reports/.native-test262-tmp/c2-final-promise.json`
- `reports/.native-test262-tmp/c2-final-async_fn_stmt.json`
- `reports/.native-test262-tmp/c2-final-async_gen_stmt.json`
- `reports/.native-test262-tmp/c2-final-for_await.json`
- `reports/.native-test262-tmp/c2-final-async_gen_proto.json`
- `reports/.native-test262-tmp/c2-final-async_iter_proto.json`
- `reports/.native-test262-tmp/c2-final-async_gen_expr.json`

Verification:

```text
cargo fmt --all -- --check
PASS

cargo build --release --no-default-features
PASS

cargo test --locked --all-targets
PASS

cargo clippy --locked --all-targets -- -D warnings
PASS
```

As requested, no full Test262 run was performed.
