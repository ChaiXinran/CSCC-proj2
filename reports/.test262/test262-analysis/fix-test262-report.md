# Fix Test262 Report

## Scope

This report records the latest full Test262 diagnostic run for the Native backend after the current fix round.

The structure follows `reports/native-v7-test262-report.md`: it separates the broad diagnostic scan from any zero-regression gate, reports aggregate results, groups the remaining failures, and ends with suggested follow-up work and quality gates. The current run is much broader than the V7 pinned gate: it scans `test262/test` through the self-developed Native lexer, parser, bytecode compiler, VM, runtime, builtins, object model, and Test262 host skeleton. Boa is not used as a Native fallback.

This report should therefore be read as a full-suite diagnostic baseline, not as a zero-failure merge gate.

## Test Command

Captured command:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

Captured build/run information:

- `agentjs v0.1.0` was compiled from `D:\00_OS\CSCC`.
- The release build completed successfully in `33.45s`.
- The executed binary command was `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`.

## Data Completeness Note

The uploaded log is a PowerShell-captured progress log, not a final JSON summary. The last captured progress line is:

```text
[53377/53379 100.0%] pass=24950 fail=28425 skip=2
```

The runner-selected total is 53,379, while the last visible processed count is 53,377. The final two records are not visible in the uploaded log, so this report uses the last captured state and does not invent a missing final summary.

## Overall Result

| Selected total | Last captured processed | Passed | Failed | Skipped | Conformance |
| --- | --- | --- | --- | --- | --- |
| 53,379 | 53,377 | 24,950 | 28,425 | 2 | 46.74% |

Skipped tests are not counted as passes. Conformance is computed as `passed / selected total`, matching the reporting style used in the V7 report.

The current 60% target requires at least `32,028` passing tests. With `24,950` current passes, the remaining gap is `7,078` additional passing tests.

## Change from the Previous V11 Fix Baseline

The previous V11 fix baseline ended at `[53377/53379] pass=19027 fail=34348 skip=2`. The current run improves the full-suite result significantly:

| Run | Selected total | Last captured processed | Passed | Failed | Skipped | Conformance |
| --- | --- | --- | --- | --- | --- | --- |
| Previous V11 fix baseline | 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |
| Current fix run | 53,379 | 53,377 | 24,950 | 28,425 | 2 | 46.74% |
| Change | 0 | 0 | +5,923 | -5,923 | +0 | +11.10 pp |

Interpretation:

1. Pass count increased by `5,923`.
2. Failure count decreased by `5,923`.
3. Full-suite conformance increased by `11.10` percentage points.
4. The largest gains are no longer limited to RegExp property escapes; this fix round also strongly improved class, expression, statement, TypedArray, Object, Array, and DataView areas.

## Areas with the Largest Failure Reduction

The largest second-level reductions are:

| Area | Failure change | Previous failures | Current failures |
| --- | --- | --- | --- |
| `language/expressions` | -1,934 | 7,458 | 5,524 |
| `language/statements` | -1,806 | 7,378 | 5,572 |
| `built-ins/TypedArray` | -424 | 1,155 | 731 |
| `built-ins/Object` | -303 | 711 | 408 |
| `built-ins/Array` | -259 | 1,109 | 850 |
| `built-ins/DataView` | -129 | 359 | 230 |
| `annexB/language` | -107 | 481 | 374 |
| `built-ins/TypedArrayConstructors` | -106 | 462 | 356 |
| `staging/sm` | -95 | 1,003 | 908 |
| `built-ins/RegExp` | -81 | 817 | 736 |
| `language/arguments-object` | -70 | 192 | 122 |
| `built-ins/ArrayBuffer` | -68 | 112 | 44 |
| `built-ins/BigInt` | -65 | 77 | 12 |
| `built-ins/Iterator` | -52 | 418 | 366 |
| `built-ins/String` | -48 | 217 | 169 |
| `language/literals` | -46 | 60 | 14 |
| `language/statementList` | -44 | 48 | 4 |
| `language/eval-code` | -29 | 304 | 275 |
| `language/block-scope` | -25 | 32 | 7 |
| `built-ins/GeneratorPrototype` | -21 | 61 | 40 |

The largest third-level reductions are:

| Subarea | Failure change | Previous failures | Current failures |
| --- | --- | --- | --- |
| `language/statements/class` | -1,122 | 3,741 | 2,619 |
| `language/expressions/class` | -766 | 3,165 | 2,399 |
| `built-ins/TypedArray/prototype` | -425 | 1,136 | 711 |
| `built-ins/Array/prototype` | -250 | 950 | 700 |
| `language/expressions/object` | -210 | 758 | 548 |
| `language/statements/for` | -183 | 306 | 123 |
| `language/expressions/compound-assignment` | -172 | 317 | 145 |
| `language/expressions/generators` | -148 | 253 | 105 |
| `language/statements/generators` | -132 | 226 | 94 |
| `built-ins/DataView/prototype` | -126 | 322 | 196 |
| `annexB/language/eval-code` | -104 | 313 | 209 |
| `language/expressions/arrow-function` | -94 | 183 | 89 |
| `language/expressions/assignment` | -85 | 322 | 237 |
| `built-ins/Object/seal` | -81 | 94 | 13 |
| `built-ins/RegExp/prototype` | -73 | 274 | 201 |
| `language/statements/for-of` | -68 | 595 | 527 |
| `language/statements/function` | -62 | 197 | 135 |
| `language/statements/variable` | -58 | 104 | 46 |
| `built-ins/ArrayBuffer/prototype` | -57 | 91 | 34 |
| `built-ins/Object/hasOwn` | -56 | 59 | 3 |
| `language/expressions/logical-assignment` | -43 | 66 | 23 |
| `built-ins/Iterator/prototype` | -42 | 293 | 251 |
| `built-ins/String/prototype` | -38 | 191 | 153 |
| `built-ins/Object/preventExtensions` | -36 | 40 | 4 |
| `language/expressions/left-shift` | -35 | 45 | 10 |

Small new/regressed buckets are:

| Area | Failure change | Previous failures | Current failures |
| --- | --- | --- | --- |
| `language/module-code` | +12 | 398 | 410 |
| `language/reserved-words` | +3 | 1 | 4 |
| `language/global-code` | +2 | 32 | 34 |
| `built-ins/eval` | +1 | 3 | 4 |
| `language/function-code` | +1 | 22 | 23 |
| `built-ins/Date` | +1 | 258 | 259 |

The regressions are much smaller than the improvements. They should still be guarded with a pinned regression list before the next merge.

## Current Remaining Failure Hotspots

Second-level path distribution:

| Area | Remaining failures | Share of failures |
| --- | --- | --- |
| `language/statements` | 5,572 | 19.60% |
| `language/expressions` | 5,524 | 19.43% |
| `built-ins/Temporal` | 4,196 | 14.76% |
| `intl402/Temporal` | 2,026 | 7.13% |
| `staging/sm` | 908 | 3.19% |
| `built-ins/Array` | 850 | 2.99% |
| `built-ins/RegExp` | 736 | 2.59% |
| `built-ins/TypedArray` | 731 | 2.57% |
| `built-ins/Promise` | 703 | 2.47% |
| `language/module-code` | 410 | 1.44% |
| `built-ins/Object` | 408 | 1.44% |
| `built-ins/Atomics` | 388 | 1.36% |
| `annexB/language` | 374 | 1.32% |
| `built-ins/Iterator` | 366 | 1.29% |
| `built-ins/TypedArrayConstructors` | 356 | 1.25% |
| `built-ins/Proxy` | 311 | 1.09% |
| `language/eval-code` | 275 | 0.97% |
| `built-ins/Date` | 259 | 0.91% |
| `built-ins/Function` | 245 | 0.86% |
| `built-ins/DataView` | 230 | 0.81% |

More concentrated third-level hotspots:

| Subarea | Remaining failures | Share of failures |
| --- | --- | --- |
| `language/statements/class` | 2,619 | 9.21% |
| `language/expressions/class` | 2,399 | 8.44% |
| `language/statements/for-await-of` | 1,146 | 4.03% |
| `built-ins/Temporal/ZonedDateTime` | 899 | 3.16% |
| `built-ins/TypedArray/prototype` | 711 | 2.50% |
| `built-ins/Array/prototype` | 700 | 2.46% |
| `built-ins/Temporal/PlainDateTime` | 677 | 2.38% |
| `language/expressions/dynamic-import` | 628 | 2.21% |
| `intl402/Temporal/ZonedDateTime` | 583 | 2.05% |
| `built-ins/Temporal/PlainDate` | 578 | 2.03% |
| `language/expressions/object` | 548 | 1.93% |
| `language/expressions/async-generator` | 543 | 1.91% |
| `language/statements/for-of` | 527 | 1.85% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 1.79% |
| `intl402/Temporal/PlainDate` | 491 | 1.73% |
| `intl402/Temporal/PlainDateTime` | 483 | 1.70% |
| `built-ins/Temporal/Duration` | 454 | 1.60% |
| `built-ins/Temporal/PlainTime` | 431 | 1.52% |
| `built-ins/Temporal/Instant` | 414 | 1.46% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.15% |
| `language/statements/async-generator` | 267 | 0.94% |
| `built-ins/Iterator/prototype` | 251 | 0.88% |
| `language/eval-code/direct` | 248 | 0.87% |
| `language/module-code/top-level-await` | 244 | 0.86% |
| `language/expressions/assignment` | 237 | 0.83% |

The largest remaining buckets are still language syntax/static semantics, class semantics, Temporal, async/generator/for-await, Array/TypedArray, Object/Promise/Iterator, module-code, and RegExp semantics.

## Failure Classification

The following grouping is heuristic and based on failure messages plus path names. It is intended for engineering triage rather than as a formal ECMAScript taxonomy.

| Failure class | Count | Share of failures |
| --- | --- | --- |
| Temporal / Intl Temporal not implemented | 6,301 | 22.17% |
| frontend syntax / static semantics gap | 4,756 | 16.73% |
| assertion / runtime semantic mismatch | 3,047 | 10.72% |
| other runtime failure | 2,592 | 9.12% |
| missing builtin/object shape or undefined target | 2,575 | 9.06% |
| binding / environment record gap | 2,289 | 8.05% |
| yield / generator gap | 1,399 | 4.92% |
| property descriptor / builtin shape gap | 1,299 | 4.57% |
| module/import/export or TLA gap | 1,200 | 4.22% |
| unsupported feature gap | 1,193 | 4.20% |
| BigInt unsupported or incomplete | 983 | 3.46% |
| RegExp syntax/parser gap | 495 | 1.74% |
| host / cross-realm gap | 282 | 0.99% |
| Annex B HTML comment / dynamic source parsing gap | 14 | 0.05% |

## Reported Error Types

| Reported error type | Count | Share of failures |
| --- | --- | --- |
| TypeError | 8,197 | 28.84% |
| SyntaxError | 8,071 | 28.39% |
| Test262Error | 7,539 | 26.52% |
| ReferenceError | 2,506 | 8.82% |
| Unsupported | 1,317 | 4.63% |
| Error | 586 | 2.06% |
| RangeError | 115 | 0.40% |
| Other | 91 | 0.32% |
| EvalError | 3 | 0.01% |

## Feature-Term Hotspots

These counts are non-disjoint. One failing test may be counted under multiple terms.

| Feature term | Mentioned failures | Share of failures |
| --- | --- | --- |
| Temporal | 6,318 | 22.23% |
| async | 5,634 | 19.82% |
| class | 5,506 | 19.37% |
| Intl | 3,055 | 10.75% |
| private | 2,514 | 8.84% |
| Iterator | 1,895 | 6.67% |
| await | 1,798 | 6.33% |
| generator | 1,357 | 4.77% |
| TypedArray | 1,200 | 4.22% |
| BigInt | 1,164 | 4.09% |
| yield | 1,053 | 3.70% |
| Promise | 1,011 | 3.56% |
| RegExp | 930 | 3.27% |
| import | 886 | 3.12% |
| module | 648 | 2.28% |
| Proxy | 593 | 2.09% |
| ArrayBuffer | 404 | 1.42% |
| DataView | 237 | 0.83% |
| ShadowRealm | 64 | 0.23% |

## Representative Failures by Class

### Temporal / Intl Temporal not implemented

Representative files:
- `test262\test\built-ins\Date\prototype\toTemporalInstant\name.js`
- `test262\test\built-ins\Date\prototype\toTemporalInstant\length.js`
- `test262\test\built-ins\Date\prototype\toTemporalInstant\this-value-invalid-date.js`

### frontend syntax / static semantics gap

Representative files:
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-a-eval-func-skip-early-err-try.js`

### assertion / runtime semantic mismatch

Representative files:
- `test262\test\annexB\built-ins\Array\from\iterator-method-emulates-undefined.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\this-not-date.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\this-not-date.js`

### other runtime failure

Representative files:
- `test262\test\annexB\built-ins\Date\prototype\getYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-control-escape-russian-letter.js`

### missing builtin/object shape or undefined target

Representative files:
- `test262\test\annexB\built-ins\Date\prototype\getYear\B.2.4.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\name.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\length.js`

### binding / environment record gap

Representative files:
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-block-scoping.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-existing-block-fn-no-init.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-existing-block-fn-update.js`

### yield / generator gap

Representative files:
- `test262\test\annexB\language\expressions\yield\star-iterable-return-emulates-undefined-throws-when-called.js`
- `test262\test\annexB\language\expressions\yield\star-iterable-throw-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\AsyncGeneratorFunction\instance-await-expr-in-param.js`

### property descriptor / builtin shape gap

Representative files:
- `test262\test\annexB\built-ins\String\prototype\substr\length-to-int-err.js`
- `test262\test\annexB\built-ins\String\prototype\trimLeft\name.js`
- `test262\test\annexB\built-ins\String\prototype\trimRight\name.js`

### module/import/export or TLA gap

Representative files:
- `test262\test\built-ins\AbstractModuleSource\proto.js`
- `test262\test\built-ins\AbstractModuleSource\length.js`
- `test262\test\built-ins\AbstractModuleSource\name.js`

### unsupported feature gap

Representative files:
- `test262\test\annexB\language\statements\for-await-of\iterator-close-return-emulates-undefined-throws-when-called.j`
- `test262\test\annexB\language\statements\for-of\iterator-close-return-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\AsyncFromSyncIteratorPrototype\next\absent-value-not-passed.js`

### BigInt unsupported or incomplete

Representative files:
- `test262\test\built-ins\Array\fromAsync\asyncitems-bigint.js`
- `test262\test\built-ins\Array\prototype\entries\resizable-buffer-grow-mid-iteration.js`
- `test262\test\built-ins\Array\prototype\every\resizable-buffer-grow-mid-iteration.js`

### RegExp syntax/parser gap

Representative files:
- `test262\test\annexB\built-ins\RegExp\RegExp-decimal-escape-class-range.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-decimal-escape-not-capturing.js`
- `test262\test\annexB\built-ins\RegExp\incomplete_hex_unicode_escape.js`

### host / cross-realm gap

Representative files:
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\index\this-cross-realm-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\input\this-cross-realm-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\lastMatch\this-cross-realm-constructor.js`

### Annex B HTML comment / dynamic source parsing gap

Representative files:
- `test262\test\annexB\built-ins\Function\createdynfn-html-close-comment-body.js`
- `test262\test\annexB\built-ins\Function\createdynfn-html-close-comment-params.js`
- `test262\test\annexB\built-ins\Function\createdynfn-html-open-comment-body.js`

## Skipped Tests

The uploaded log records 2 explicit skips:

- `test262\test\built-ins\Atomics\wait\bigint\cannot-suspend-throws.js` — non-blocking agent tests are not enabled
- `test262\test\built-ins\Atomics\wait\cannot-suspend-throws.js` — non-blocking agent tests are not enabled

These are Atomics wait tests requiring host suspension behavior. They should remain explicit skips, not hidden passes.

## Interpretation

The latest fix round is a major improvement over the previous V11 fix baseline:

1. The pass count increased from 19,027 to 24,950.
2. Conformance increased from 35.65% to 46.74%.
3. The remaining gap to 60% is now 7,078 tests, down from about 13,001 tests after the previous V11 fix run.
4. The largest successful reductions came from `language/statements/class`, `language/expressions/class`, `TypedArray.prototype`, `Array.prototype`, object expressions, `Object.seal`, `Object.hasOwn`, DataView, RegExp prototype work, and several expression/statement operator families.
5. Remaining failures are still large enough that random one-off fixes will not reach 60%. The next milestone should target high-density families: class/private/static semantics, async/generator/iterator, Array/TypedArray/DataView, Object/Reflect descriptor precision, module/TLA parsing, and BigInt/TypedArray integration.
6. Temporal remains the largest single broad builtin family, but it is expensive. Unless a compact implementation strategy is available, it should remain a later milestone rather than the next short-term scoring focus.

## Suggested Follow-Up Order

1. Build a `fix` pinned regression gate from the files that changed from failure to pass in this run, especially class, object-expression, TypedArray, Array, Object, DataView, and RegExp files.
2. Continue the frontend/class track: `language/statements/class` and `language/expressions/class` still have more than 5,000 combined failures.
3. Continue the object-model track: descriptor precision, `Object`/`Reflect`, `Array.prototype`, `TypedArray.prototype`, `ArrayBuffer`, and `DataView` produced large gains and still contain dense remaining failures.
4. Continue the runtime protocol track: generator/yield, iterator protocol, async function, Promise job queue, and for-await-of.
5. Treat Temporal/Intl402, full module loading, ShadowRealm, and cross-realm `$262.createRealm` as larger future milestones unless the team explicitly shifts scope.
6. Keep full-suite scans diagnostic and separate from smaller merge gates. Failed, skipped, crashed, timed-out, or uncaptured tests must not be counted as passes.

## Quality Gates

Recommended commands before merging the next fix round:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features
```

Focused diagnostic scans:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress
```

Full diagnostic scan:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress *> reports\fix-test262-output.txt
```

If JSON output is supported for this runner mode, prefer producing a machine-readable summary beside the verbose log.
