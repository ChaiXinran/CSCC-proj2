# Fix4 Part B Report

## Owner and Scope

B group owns generator resume, iterator protocol execution, Promise jobs, and
async/for-await runtime lowering. Locked full-suite baseline from
`docs/fix4-team-plan.md`: 28,468/53,379 passed (53.33%).

## 2026-06-27 — Async Completion, Async Iterator, and Resolution Pass

Implemented:

- Native Test262 async variants now load `doneprintHandle.js`; the native host
  installs `print()` and forwards output through `NativeContext::push_output`.
- Async-generator `next`, `return`, and `throw` reuse the generator VM and wrap
  results or throws in native Promises.
- Added `Symbol.asyncIterator`, `GetAsyncIterator`, and `AsyncIteratorNext`.
  `for await` prefers the async method, awaits async next-results, and falls
  back to the existing sync iterator while awaiting yielded values.
- Centralized Promise resolution for native Promise adoption, foreign
  thenables, poisoned `then` getters, callback throws, and self-resolution.
- Added focused end-to-end tests for `$DONE`, async generators, custom async
  iterators, opcode stack effects, and Promise resolution.

Focused Test262 delta against the start of this pass:

- for-await-of: 90/1,234 → 983/1,234 (+893, 79.66%)
- async-generator expressions: 81/623 → 348/623 (+267, 55.86%)
- async-generator statements: 34/301 → 165/301 (+131, 54.82%)
- Promise: 113/703 → 232/703 (+119, 33.00%)
- Iterator: stable at 191/514
- generator expressions/statements: stable at 188/290 and 173/266

Remaining high-density failures:

- Most remaining for-await cases are A-owned destructuring IteratorClose,
  abrupt completion, default initializer, name inference, and early errors.
- Promise combinators still need per-element resolution and aggregate state;
  pending Promise continuation capture remains incomplete.
- Async-generator rejection/finally precision still depends on injecting
  abrupt completions into suspended generator frames.

Coordination: `src/test262.rs` and `src/builtins/mod.rs` received the minimum
documented D/C host bridge required to measure B async behavior accurately.

## 2026-06-27 — Async and Shared Iterator Pass

Implemented:

- Preserved async/generator kind flags from bytecode templates to runtime
  functions; async functions are non-constructable and return native Promises.
- Implemented `AwaitValue` for ordinary values and native Promises that settle
  through the current job queue. Rejections become catchable completions.
- Added Async-from-Sync lowering for `for await ... of` by reusing the existing
  for-of iterator path and awaiting each yielded value.
- Replaced Promise combinators' array-only collector with the VM's shared
  iterator protocol and rooted iterator wrappers across callback allocations.
- Added `docs/fix4-shared-interface.md` and focused Promise/async/GC tests.

Files touched: `src/bytecode/{chunk,compiler}.rs`,
`src/runtime/{context,function}.rs`, `src/vm/interpreter.rs`,
`src/builtins/{function,promise}.rs`, and focused tests.

Focused results before this pass:

- yield: 33/63
- for-await-of: 88/1,234
- Promise: 111/703
- Iterator: 188/514

Latest measured results after the final GC-root rerun:

- yield: 33/63 (no change)
- for-await-of: 90/1,234 (+2)
- Promise: 113/703 (+2)
- Iterator: 190/514 (+2; an earlier parallel run reported a non-reproducible
  191/514, while jobs=1 and the final run both reported 190)

Commands run:

```powershell
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --test parser_iteration
cargo test --release --no-default-features --test native_iteration
cargo test --release --no-default-features --test native_promise
```

Merge-gate status:

- `cargo check --release --no-default-features --all-targets`: passed.
- `cargo fmt --all -- --check`: this change is formatted; the repository gate
  remains blocked by a pre-existing formatting difference in
  `src/builtins/std_primitives.rs`.
- `cargo test --release --no-default-features --all-targets`: B tests passed;
  the suite stopped at the unrelated existing
  `parser_control_flow::compiles_multiple_var_declarators_from_frontend_contract`
  assertion (`left: 4`, `right: 2`).
- `cargo clippy --release --no-default-features --all-targets -- -D warnings`:
  the B-introduced argument-count warning was removed; 17 pre-existing A/C/VM
  warnings still block the repository-wide gate.

Newly exposed gaps and coordination:

- Native Test262 does not install `doneprintHandle.js`; many Promise cases fail
  because `$DONE` is undefined or completion is never observed. D/runner should
  fix this in `src/test262.rs`.
- Most for-await-of failures are destructuring matrices and remain blocked on A
  group binding-pattern lowering.
- Pending Promise continuation capture, `Symbol.asyncIterator`, true async
  iterator records, and async generators remain B follow-up work.
- C should route iterable-consuming builtins through the shared VM helper.
