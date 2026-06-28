# Native Fix2 Test262 Report

## Scope

This report summarizes the latest Native backend full Test262 diagnostic run for `ChaiXinran/CSCC-proj2`.

The report follows the structure of `reports/native-v7-test262-report.md`: scope, diagnostic scan, result summary, failure hotspots, failure classification, skipped tests, interpretation, follow-up order, and quality gates. The command was executed through the self-developed Native path: lexer, parser/AST, bytecode compiler, VM, runtime, builtins, heap, and Test262 runner. Boa is not counted as a fallback for these results.

Input log: `test262-fixbug2-output.txt`.

## Diagnostic Fix2 Scan

Command:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

Build / run evidence:

```text
Finished `release` profile [optimized] target(s) in 1m 45s
Running `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`
```

The run did **not** reach the normal full-scan endpoint. The last captured progress line is:

```text
[52748/53379  98.8%] pass=26803 fail=25943 skip=2
```

The process then ended with:

```text
memory allocation of 34359738368 bytes failed
error: process didn't exit successfully: `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress` (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)
```

Therefore, this report treats Fix2 as a **last-captured diagnostic state**, not a clean complete full-scan result.

## Result Summary

| Metric | Value |
| --- | --- |
| Selected total | 53,379 |
| Last captured processed | 52,748 |
| Passed | 26,803 |
| Failed | 25,943 |
| Skipped | 2 |
| Conformance vs selected total | 50.21% |
| Conformance vs captured processed | 50.81% |
| 60% target | 32,028 |
| Shortfall from target | 5,225 |
| Minimum shortfall even if all unprocessed pass | 4,594 |

Skipped tests are not counted as passes.

The 60% target over 53,379 selected tests is 32,028 passing tests. At the last captured state, Fix2 has 26,803 passing tests and is 5,225 passes below the 60% line. Because 631 selected tests were not captured after the crash, even an unrealistically perfect tail would still leave at least 4,594 additional passes needed.

## Historical Progress

| Run | Processed | Selected total | Passed | Failed | Skipped | Passed / selected total |
| --- | --- | --- | --- | --- | --- | --- |
| native-v11 baseline | 53,377 | 53,379 | 18,050 | 35,325 | 2 | 33.81% |
| native-v11-fixbug | 53,377 | 53,379 | 19,027 | 34,348 | 2 | 35.65% |
| fix | 53,377 | 53,379 | 24,950 | 28,425 | 2 | 46.74% |
| fix2 last captured | 52,748 | 53,379 | 26,803 | 25,943 | 2 | 50.21% |

Notes:

- `fix2` should be compared cautiously because it stopped at 52,748 processed tests rather than the normal 53,377 captured endpoint.
- At the same processed position, the previous `fix` run had `pass=24728 fail=28018 skip=2`; Fix2 has `pass=26803 fail=25943 skip=2`, a **+2,075 pass / -2,075 fail** improvement over the comparable prefix.
- Compared with the previous complete-ish `fix` endpoint, Fix2 is already **+1,853 passes** despite stopping 629 processed tests earlier.

## Failure Hotspots by Area

The progress log records failure paths and aggregate counters, but it does not list passing file paths. Therefore, the following table is failure-centric rather than a per-directory pass-rate table.

| Area | Failures | Share of captured failures |
| --- | --- | --- |
| `language/expressions` | 4,799 | 18.5% |
| `language/statements` | 4,776 | 18.4% |
| `built-ins/Temporal` | 4,194 | 16.2% |
| `intl402/Temporal` | 2,022 | 7.8% |
| `built-ins/RegExp` | 857 | 3.3% |
| `built-ins/Array` | 742 | 2.9% |
| `built-ins/TypedArray` | 687 | 2.6% |
| `built-ins/Promise` | 597 | 2.3% |
| `staging/sm` | 452 | 1.7% |
| `language/module-code` | 410 | 1.6% |
| `built-ins/Atomics` | 388 | 1.5% |
| `built-ins/Object` | 371 | 1.4% |
| `built-ins/TypedArrayConstructors` | 334 | 1.3% |
| `annexB/language` | 326 | 1.3% |
| `built-ins/Iterator` | 326 | 1.3% |
| `built-ins/Proxy` | 311 | 1.2% |
| `language/eval-code` | 276 | 1.1% |
| `built-ins/Date` | 246 | 0.9% |
| `built-ins/Function` | 240 | 0.9% |
| `built-ins/DataView` | 213 | 0.8% |
| `intl402/NumberFormat` | 207 | 0.8% |
| `intl402/DateTimeFormat` | 191 | 0.7% |
| `built-ins/Set` | 174 | 0.7% |
| `annexB/built-ins` | 165 | 0.6% |
| `built-ins/String` | 129 | 0.5% |

## Failure Hotspots by Sub-area

| Sub-area | Failures | Share of captured failures |
| --- | --- | --- |
| `language/statements/class` | 2,111 | 8.1% |
| `language/expressions/class` | 1,915 | 7.4% |
| `language/statements/for-await-of` | 1,146 | 4.4% |
| `built-ins/Temporal/ZonedDateTime` | 899 | 3.5% |
| `built-ins/Temporal/PlainDateTime` | 677 | 2.6% |
| `built-ins/TypedArray/prototype` | 671 | 2.6% |
| `language/expressions/dynamic-import` | 628 | 2.4% |
| `built-ins/Array/prototype` | 614 | 2.4% |
| `intl402/Temporal/ZonedDateTime` | 583 | 2.2% |
| `built-ins/Temporal/PlainDate` | 577 | 2.2% |
| `language/expressions/async-generator` | 542 | 2.1% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 2.0% |
| `language/expressions/object` | 506 | 2.0% |
| `intl402/Temporal/PlainDate` | 489 | 1.9% |
| `intl402/Temporal/PlainDateTime` | 481 | 1.9% |
| `built-ins/Temporal/Duration` | 453 | 1.7% |
| `built-ins/Temporal/PlainTime` | 431 | 1.7% |
| `built-ins/Temporal/Instant` | 414 | 1.6% |
| `built-ins/RegExp/property-escapes` | 345 | 1.3% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.3% |
| `language/statements/for-of` | 271 | 1.0% |
| `language/statements/async-generator` | 267 | 1.0% |
| `language/eval-code/direct` | 249 | 1.0% |
| `language/module-code/top-level-await` | 244 | 0.9% |
| `built-ins/Iterator/prototype` | 239 | 0.9% |

The remaining failure mass is still dominated by:

1. `language/expressions` and `language/statements`, especially class, async-generator, for-await-of, dynamic import, object expressions, and for-of.
2. `built-ins/Temporal` and `intl402/Temporal`, which remain very large but high-cost.
3. Array/TypedArray/RegExp/Promise/Object/Iterator areas, where object-model and protocol precision can still produce practical gains.

## Failure Classification

The following categories are derived from failure path and message heuristics. They are diagnostic groups, not a formal ECMAScript taxonomy.

| Failure class | Count | Share of captured failures |
| --- | --- | --- |
| Temporal / Intl Temporal not implemented or incomplete | 6,294 | 24.3% |
| Async / Promise semantics gap | 5,042 | 19.4% |
| Assertion / semantic mismatch | 3,409 | 13.1% |
| Other runtime failure | 2,884 | 11.1% |
| Unsupported feature | 1,427 | 5.5% |
| Frontend syntax / static-semantics gap | 984 | 3.8% |
| Missing builtin method / undefined target | 977 | 3.8% |
| Property descriptor / builtin shape gap | 835 | 3.2% |
| BigInt semantics gap | 807 | 3.1% |
| Iterator / for-of protocol gap | 674 | 2.6% |
| Binding / environment semantics gap | 673 | 2.6% |
| Module syntax / module-loader gap | 583 | 2.2% |
| Unsupported Proxy / ShadowRealm | 536 | 2.1% |
| Generator / yield semantics gap | 388 | 1.5% |
| RegExp syntax / regexp-engine gap | 315 | 1.2% |
| Unsupported BigInt | 101 | 0.4% |
| Annex B HTML comment syntax gap | 11 | 0.0% |
| TypeError / runtime object-model gap | 3 | 0.0% |

## First Visible Error Kind

| First visible error kind | Count | Share |
| --- | --- | --- |
| TypeError | 8,082 | 31.2% |
| Test262Error | 7,511 | 29.0% |
| SyntaxError | 5,163 | 19.9% |
| ReferenceError | 1,765 | 6.8% |
| Unsupported | 1,436 | 5.5% |
| Other | 1,139 | 4.4% |
| Error | 725 | 2.8% |
| RangeError | 119 | 0.5% |
| EvalError | 3 | 0.0% |

## Representative Failure Classes

| Failure class | Count | Representative files |
| --- | --- | --- |
| Temporal / Intl Temporal not implemented or incomplete | 6,294 | `built-ins/Date/prototype/toTemporalInstant/length.js`<br>`built-ins/Date/prototype/toTemporalInstant/name.js` |
| Async / Promise semantics gap | 5,042 | `built-ins/Array/fromAsync/async-iterable-async-mapped-awaits-once.js`<br>`built-ins/Array/fromAsync/async-iterable-input-does-not-await-input.js` |
| Assertion / semantic mismatch | 3,409 | `annexB/built-ins/Date/prototype/getYear/this-not-date.js`<br>`annexB/built-ins/Date/prototype/setYear/this-not-date.js` |
| Other runtime failure | 2,884 | `annexB/built-ins/Date/prototype/getYear/not-a-constructor.js`<br>`annexB/built-ins/Date/prototype/setYear/not-a-constructor.js` |
| Unsupported feature | 1,427 | `annexB/language/statements/for-await-of/iterator-close-return-emulates-undefined-throws-when-called.j`<br>`annexB/language/statements/for-of/iterator-close-return-emulates-undefined-throws-when-called.js` |
| Frontend syntax / static-semantics gap | 984 | `annexB/language/expressions/assignmenttargettype/callexpression-as-for-in-lhs.js`<br>`annexB/language/expressions/assignmenttargettype/callexpression-as-for-of-lhs.js` |
| Missing builtin method / undefined target | 977 | `annexB/built-ins/Date/prototype/getYear/nan.js`<br>`annexB/built-ins/Date/prototype/getYear/return-value.js` |
| Property descriptor / builtin shape gap | 835 | `annexB/built-ins/Date/prototype/getYear/length.js`<br>`annexB/built-ins/Date/prototype/getYear/name.js` |
| BigInt semantics gap | 807 | `built-ins/Atomics/add/bigint/bad-range.js`<br>`built-ins/Atomics/add/bigint/good-views.js` |
| Iterator / for-of protocol gap | 674 | `annexB/built-ins/Array/from/iterator-method-emulates-undefined.js`<br>`annexB/built-ins/TypedArrayConstructors/from/iterator-method-emulates-undefined.js` |
| Binding / environment semantics gap | 673 | `annexB/language/eval-code/direct/func-block-decl-eval-func-block-scoping.js`<br>`annexB/language/eval-code/direct/func-block-decl-eval-func-existing-block-fn-no-init.js` |
| Module syntax / module-loader gap | 583 | `language/expressions/dynamic-import/always-create-new-promise.js`<br>`language/expressions/dynamic-import/assign-expr-get-value-abrupt-throws.js` |

## Fix2 Delta versus Previous Fix at the Same Processed Count

The previous `fix` run was also inspected at `[52748/53379]`, where it had `pass=24728 fail=28018 skip=2`. Fix2 at the same processed count has `pass=26803 fail=25943 skip=2`.

### Largest Failure Reductions by Area

| Area | fix failures at 52,748 processed | fix2 failures | Reduction |
| --- | --- | --- | --- |
| `language/statements` | 5,572 | 4,776 | +796 |
| `language/expressions` | 5,524 | 4,799 | +725 |
| `built-ins/Array` | 850 | 742 | +108 |
| `built-ins/Promise` | 703 | 597 | +106 |
| `staging/sm` | 503 | 452 | +51 |
| `annexB/language` | 374 | 326 | +48 |
| `built-ins/TypedArray` | 731 | 687 | +44 |
| `built-ins/String` | 169 | 129 | +40 |
| `built-ins/Iterator` | 366 | 326 | +40 |
| `built-ins/Object` | 408 | 371 | +37 |
| `built-ins/TypedArrayConstructors` | 356 | 334 | +22 |
| `built-ins/DataView` | 230 | 213 | +17 |
| `built-ins/Symbol` | 42 | 26 | +16 |
| `built-ins/Reflect` | 24 | 10 | +14 |
| `language/identifiers` | 82 | 68 | +14 |

### Largest Regressions by Area

| Area | fix failures at 52,748 processed | fix2 failures | Delta |
| --- | --- | --- | --- |
| `built-ins/RegExp` | 736 | 857 | -121 |
| `language/directive-prologue` | 14 | 17 | -3 |
| `language/eval-code` | 275 | 276 | -1 |

### Largest Failure Reductions by Sub-area

| Sub-area | fix failures at 52,748 processed | fix2 failures | Reduction |
| --- | --- | --- | --- |
| `language/statements/class` | 2,619 | 2,111 | +508 |
| `language/expressions/class` | 2,399 | 1,915 | +484 |
| `language/statements/for-of` | 527 | 271 | +256 |
| `language/expressions/assignment` | 237 | 136 | +101 |
| `built-ins/Array/prototype` | 700 | 614 | +86 |
| `language/expressions/object` | 548 | 506 | +42 |
| `built-ins/TypedArray/prototype` | 711 | 671 | +40 |
| `built-ins/String/prototype` | 153 | 117 | +36 |
| `annexB/language/eval-code` | 209 | 177 | +32 |
| `built-ins/Promise/prototype` | 124 | 93 | +31 |
| `language/statements/function` | 135 | 112 | +23 |
| `built-ins/Array/from` | 18 | 0 | +18 |
| `annexB/language/global-code` | 57 | 41 | +16 |
| `staging/sm/Iterator` | 100 | 84 | +16 |
| `language/expressions/arrow-function` | 89 | 73 | +16 |

### Largest Regressions by Sub-area

| Sub-area | fix failures at 52,748 processed | fix2 failures | Delta |
| --- | --- | --- | --- |
| `built-ins/RegExp/property-escapes` | 213 | 345 | -132 |
| `language/statements/for` | 123 | 144 | -21 |
| `language/directive-prologue/func-expr-no-semi-runtime.js` | 0 | 1 | -1 |
| `built-ins/DataView/custom-proto-if-not-object-fallbacks-to-default-prototype.js` | 0 | 1 | -1 |
| `language/eval-code/direct` | 248 | 249 | -1 |
| `language/directive-prologue/get-accsr-runtime.js` | 0 | 1 | -1 |
| `built-ins/DataView/custom-proto-access-resizes-buffer-valid-by-length.js` | 0 | 1 | -1 |
| `language/directive-prologue/func-decl-no-semi-runtime.js` | 0 | 1 | -1 |
| `language/directive-prologue/set-accsr-not-first-runtime.js` | 0 | 1 | -1 |
| `language/directive-prologue/func-expr-inside-func-decl-runtime.js` | 0 | 1 | -1 |

The biggest wins are in `language/statements/class`, `language/expressions/class`, `language/statements/for-of`, assignment/destructuring, Array prototype, Promise prototype, and TypedArray prototype. The most visible regression is `built-ins/RegExp/property-escapes`; this should be protected by a pinned regression gate before more frontend and regexp work is merged.

## Skipped Tests

| Skipped file |
| --- |
| `test262/test/built-ins/Atomics/wait/bigint/cannot-suspend-throws.js` |
| `test262/test/built-ins/Atomics/wait/cannot-suspend-throws.js` |

The two skipped Atomics wait tests remain explicit unsupported host/runtime cases. They are not counted as passes.

## Interpretation

Fix2 is a major conformance improvement over the previous fix baseline, but the scan is not yet stable enough to serve as a final release gate. The key findings are:

1. The project is now at roughly **50.21%** pass rate over the selected total at the last captured state.
2. The comparable-prefix improvement over the previous fix run is **+2,075 passes**.
3. The remaining distance to 60% is at least **5,225 passes**, and at least **4,594 passes** even if all unprocessed tail tests were assumed to pass.
4. The largest remaining count is still language/class/async/iterator-related, not GC/cache.
5. Temporal/Intl Temporal is the largest isolated builtin family, but it is too large to be the fastest short-term path unless only descriptor-level stubs are targeted.
6. The final crash must be fixed; otherwise, future full-scan data will remain less trustworthy than the underlying conformance work.

## Suggested Follow-Up Order

1. **Stabilize full-scan reporting first.** The runner should not allocate a 32 GiB buffer or crash with `STATUS_STACK_BUFFER_OVERRUN` during final reporting. Stream failure records, truncate per-test messages, and avoid formatting all failures into one giant string.
2. **Protect Fix2 wins with pinned gates.** Add representative class, for-of, Array, Promise, TypedArray, and descriptor tests that flipped from fail to pass.
3. **Fix the RegExp property-escape regression.** `built-ins/RegExp/property-escapes` regressed by 132 failures versus the previous fix prefix.
4. **Continue language/class/destructuring work.** `language/statements/class` and `language/expressions/class` are still the largest non-Temporal sub-areas and also produced the biggest Fix2 reductions.
5. **Prioritize async / Promise / iterator support.** `for-await-of`, async generators, Promise, Iterator, and Array.fromAsync failures are now among the largest practical gains.
6. **Improve object-model and descriptor precision.** This directly affects Object, Array, TypedArray, String/Date Annex B, RegExp legacy accessors, and builtin `name` / `length` / descriptor tests.
7. **Treat Temporal / Intl Temporal as a separate milestone.** If time is short, only implement low-cost constructor/property-descriptor skeletons; avoid attempting full Temporal semantics before reaching 60%.

## Quality Gates

The following commands should be used before merging another broad Fix2 follow-up:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features
```

Recommended high-yield Test262 gates:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress
```

Final full-scan gate after the reporting crash is fixed:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress *> reports\fix2-test262-output.txt
```

Failed, skipped, crashed, timed-out, or unprocessed suites must not be counted as passes.
