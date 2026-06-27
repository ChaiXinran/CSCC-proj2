# Native V12 Part B Report

## Owner and Scope

B owns Iterator execution, generator/async suspension, Promise state and job
queue behavior, and the protocol used by iterable-consuming builtins. C owns
JS-visible descriptor exactness and missing globals; A owns syntax and
destructuring lowering.

Locked Fixbug5 baseline: 32,097/53,379 passed, 21,280 failed, 2 skipped
(60.13%). Source: `reports/fixbug5-test262-analysis.md`.

## Initial Focused Baseline — 2026-06-27

```text
Iterator                              190 / 514  (36.96%)
Promise                               232 / 703  (33.00%)
Array/from                             47 / 47   (100.00%)
TypedArrayConstructors/from            46 / 59   (77.97%)
language/statements/for-of            523 / 751  (69.64%)
language/statements/for-await-of      983 / 1234 (79.66%)
language/expressions/yield             33 / 63   (52.38%)
language/expressions/async-generator  349 / 623  (56.02%)
```

## Implemented

- Reworked `Promise.all`, `allSettled`, `any`, and `race` to resolve every
  input through the existing thenable-resolution path and FIFO Promise queue.
- Added shared aggregate callbacks for ordered results, rejection propagation,
  all-settled records, and first-settlement behavior.
- Converted iterator acquisition/step exceptions into rejected combinator
  promises instead of synchronous throws.
- Added VM-scoped builtin roots so target promises and aggregate state survive
  low-threshold garbage collection.
- Added a generic Promise capability executor. `resolve`, `reject`, `try`,
  `withResolvers`, and all four combinators now validate and construct through
  their `this` receiver instead of hard-coding the global Promise constructor.
- Promise construction now uses `newTarget.prototype`; resolving-function
  names are the required empty string. Combinators call `this.resolve` once per
  input and register through the returned value's `then` method.
- Corrected the synchronous generator prototype chain to inherit iterator
  helpers through the per-function prototype and `Iterator.prototype`.
- Added one VM IteratorClose path for terminal helpers. `some`, `every`,
  `find`, `forEach`, and `reduce` now close on validation failure, early exit,
  and callback throw while preserving the original thrown value.
- Replaced eager `map`, `filter`, `take`, `drop`, and `flatMap` materialization
  with one shared lazy Iterator Helper prototype and state machine. It caches
  the source `next` method, advances only on helper `next()`, forwards
  `return()`, closes `take` at its limit, and tracks flatMap inner iterators.
- `take` and `drop` now apply VM `ToNumber`, including object coercion, before
  reading the iterator `next` method.

Files changed: `src/builtins/promise.rs`, `src/builtins/collections.rs`,
`src/vm/interpreter.rs`, `tests/native_promise.rs`, and
`tests/native_collections.rs`.

## Focused Results

```text
Iterator  190 -> 352 / 514  (+162, 68.48%)
Promise   232 -> 436 / 703  (+204, 62.02%)
```

Focused net gain: 366 passing cases. A formal full 53,379-case scan was not
run, so this is not claimed as a full-suite conformance delta.

Lazy helper focused results:

```text
map      28 / 36
filter   30 / 37
take     25 / 33
drop     26 / 34
flatMap  37 / 44
```

Regression gates after the final patch:

```text
Array/from                            47 / 47
TypedArrayConstructors/from           46 / 59
for-of                               523 / 751
for-await-of                         983 / 1234
yield                                 33 / 63
async-generator                      349 / 623
```

Commands run include `cargo check --all-targets --no-default-features`, the
`native_promise` and `native_collections` integration tests, and each focused
Test262 directory above with the release native backend and four jobs.

`cargo test --release --no-default-features --all-targets` reached one existing
unrelated failure in `tests/parser_control_flow.rs`:
`compiles_multiple_var_declarators_from_frontend_contract` expected two
instructions but the current compiler emits four. B-focused integration tests
all pass. `cargo clippy --all-targets --no-default-features -- -D warnings`
remains blocked by 20 pre-existing warnings across A/C/shared files; none point
to the new B code. `git diff --check` passes.

## Remaining Failures and Coordination

- Generic Promise constructor capability and constructor-specific `resolve`
  dispatch are implemented. Remaining subclass failures include species-aware
  `then` results and A-owned class static/default-derived-constructor lowering.
- `AggregateError` is not installed as a JS-visible constructor; C should add
  it through the shared builtin installer before B improves `Promise.any`
  rejection shape and its `errors` property.
- Iterator helper residuals are now mostly constructor/prototype edge cases,
  generator-running errors, and precise close precedence rather than eager
  consumption.
- Some Promise race IteratorClose tests hit the wall-clock limit. They require
  interleaved iteration/resolution rather than the current eager collection.
- No regression remains in the fixed B gates. The temporary async-generator
  prototype regression was detected and removed before this report.
- The required V12 5,000-case selector/manifest is not present in `src/` or
  `reports/`; D must install it before a `--native-v12-scan` delta can be run.

Next action: finish Promise species-aware `then` results and the remaining
`yield*` return/throw delegation cases, while coordinating class inheritance
failures with A.
