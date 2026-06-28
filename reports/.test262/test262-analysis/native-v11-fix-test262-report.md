# Native V11 Fix Test262 Report

## Scope

This report records the Native backend Test262 result after the V11 fixbug pass.

It follows the structure of `reports/native-v7-test262-report.md`, but the run here is a much broader diagnostic scan: it uses the full `test262/test` suite through the self-developed Native lexer, parser, bytecode compiler, VM, runtime, builtin layer, object model, and Test262 host skeleton. Boa is not used as a fallback for the measured Native backend.

This is a diagnostic full-suite baseline, not a zero-regression acceptance gate.

## Test Command

Captured command:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

Build/run evidence from the log:

- `agentjs v0.1.0` was compiled from `D:\00_OS\CSCC`.
- The release build completed successfully in `33.87s`.
- The runner executed `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`.

## Result

The uploaded log ends at `[53377/53379 100.0%] pass=19027 fail=34348 skip=2`. The runner denominator is 53,379, while the last captured progress line accounts for 53,377 processed outcomes. Therefore, this report treats the following numbers as the last captured state rather than inventing a missing final summary.

| Total selected | Last captured processed | Passed | Failed | Skipped | Conformance |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |

Skipped tests are not counted as passes. The conformance column is computed as `passed / total selected`, matching the V7 report convention.

## Change from the Previous V11 Full-Suite Baseline

The previous captured V11 full-suite run ended at `[53377/53379] pass=18050 fail=35325 skip=2`. The fixbug run improves the captured result by 977 passing tests.

| Run | Total selected | Last captured processed | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Previous V11 full-suite baseline | 53,379 | 53,377 | 18,050 | 35,325 | 2 | 33.81% |
| V11 fixbug run | 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |
| Delta | 0 | 0 | +977 | -977 | +0 | +1.83 pp |

By failure-record comparison, 1,151 old failures disappeared and 174 failure records newly appeared, giving a net reduction of 977 failures.

## Fix Impact by Area

The largest absolute failure reductions are concentrated in language syntax/semantics and RegExp property-escape coverage.

| Area | Failure-count delta |
| --- | ---: |
| `language/expressions` | -409 |
| `built-ins/RegExp` | -261 |
| `language/statements` | -253 |
| `language/literals` | -13 |
| `language/destructuring` | -12 |
| `built-ins/Reflect` | -9 |
| `staging/sm` | -8 |
| `built-ins/Temporal` | -7 |
| `built-ins/TypedArrayConstructors` | -2 |
| `annexB/built-ins` | -2 |

The largest three-level reductions are:

| Sub-area | Failure-count delta |
| --- | ---: |
| `built-ins/RegExp/property-escapes` | -261 |
| `language/expressions/class` | -210 |
| `language/statements/function` | -100 |
| `language/expressions/function` | -92 |
| `language/expressions/object` | -63 |
| `language/statements/for-of` | -45 |
| `language/expressions/arrow-function` | -36 |
| `language/statements/variable` | -33 |
| `language/statements/const` | -32 |
| `language/statements/let` | -32 |
| `language/literals/regexp` | -13 |
| `language/destructuring/binding` | -12 |

Newly appearing failure areas are much smaller:

| Area | Failure-count delta |
| --- | ---: |
| `built-ins/Object` | +3 |
| `built-ins/Error` | +2 |
| `language/module-code` | +1 |

Interpretation:

1. The strongest visible gain is in `built-ins/RegExp/property-escapes`, which accounts for 261 fewer failures.
2. The Native frontend also improved many expression and statement cases, especially class/function/object-expression related failures.
3. The small new-failure set is concentrated in syntax early-error precision and a few object/error builtin semantics. This does not erase the net improvement.

## Top Remaining Failure Areas

The table below groups remaining failures by the first two path segments under `test262/test`.

| Area | Remaining failures | Share of failures |
| --- | ---: | ---: |
| `language/expressions` | 7,458 | 21.7% |
| `language/statements` | 7,378 | 21.5% |
| `built-ins/Temporal` | 4,214 | 12.3% |
| `intl402/Temporal` | 2,026 | 5.9% |
| `built-ins/TypedArray` | 1,155 | 3.4% |
| `built-ins/Array` | 1,109 | 3.2% |
| `staging/sm` | 1,003 | 2.9% |
| `built-ins/RegExp` | 817 | 2.4% |
| `built-ins/Object` | 711 | 2.1% |
| `built-ins/Promise` | 703 | 2.0% |
| `annexB/language` | 481 | 1.4% |
| `built-ins/TypedArrayConstructors` | 462 | 1.3% |
| `built-ins/Iterator` | 418 | 1.2% |
| `language/module-code` | 398 | 1.2% |
| `built-ins/Atomics` | 388 | 1.1% |
| `built-ins/DataView` | 359 | 1.0% |
| `built-ins/Proxy` | 311 | 0.9% |
| `language/eval-code` | 304 | 0.9% |
| `built-ins/Function` | 260 | 0.8% |
| `built-ins/Date` | 258 | 0.8% |

The most concentrated three-level buckets are:

| Sub-area | Remaining failures | Share of failures |
| --- | ---: | ---: |
| `language/statements/class` | 3,741 | 10.9% |
| `language/expressions/class` | 3,165 | 9.2% |
| `language/statements/for-await-of` | 1,149 | 3.3% |
| `built-ins/TypedArray/prototype` | 1,136 | 3.3% |
| `built-ins/Array/prototype` | 950 | 2.8% |
| `built-ins/Temporal/ZonedDateTime` | 900 | 2.6% |
| `language/expressions/object` | 758 | 2.2% |
| `built-ins/Temporal/PlainDateTime` | 683 | 2.0% |
| `language/expressions/dynamic-import` | 628 | 1.8% |
| `language/statements/for-of` | 595 | 1.7% |
| `intl402/Temporal/ZonedDateTime` | 583 | 1.7% |
| `built-ins/Temporal/PlainDate` | 579 | 1.7% |
| `language/expressions/async-generator` | 554 | 1.6% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 1.5% |
| `intl402/Temporal/PlainDate` | 491 | 1.4% |
| `intl402/Temporal/PlainDateTime` | 483 | 1.4% |
| `built-ins/Temporal/Duration` | 458 | 1.3% |
| `built-ins/Temporal/PlainTime` | 432 | 1.3% |
| `built-ins/Temporal/Instant` | 417 | 1.2% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.0% |

The dominant remaining areas are still broad language syntax/static-semantics, Temporal, TypedArray, Array, RegExp, Object, Promise, module-code, and staging tests.

## Failure Classification

The following categories are derived from failure message heuristics. They are diagnostic groupings, not a formal ECMAScript taxonomy.

| Failure class | Count | Share of failures |
| --- | ---: | ---: |
| Frontend syntax / static-semantics gap | 12,251 | 35.7% |
| Assertion / semantic mismatch | 6,734 | 19.6% |
| Binding / environment semantics gap | 4,482 | 13.0% |
| Missing builtin method / undefined target | 3,043 | 8.9% |
| Unsupported yield/generator | 2,013 | 5.9% |
| Unsupported feature | 1,944 | 5.7% |
| Unsupported BigInt | 1,494 | 4.3% |
| Module syntax / source-phase import gap | 1,030 | 3.0% |
| Property descriptor / builtin shape gap | 598 | 1.7% |
| RegExp syntax / parser gap | 500 | 1.5% |
| Missing host / cross-realm support | 222 | 0.6% |
| Other runtime failure | 24 | 0.1% |
| Annex B HTML comment syntax gap | 13 | 0.0% |

### Frontend syntax / static-semantics gap
Representative files:
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-a-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-b-eval-func-skip-early-err-try.js`

### Assertion / semantic mismatch
Representative files:
- `test262\test\annexB\built-ins\Array\from\iterator-method-emulates-undefined.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\this-not-date.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\this-not-date.js`

### Binding / environment semantics gap
Representative files:
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\index\this-subclass-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\input\this-subclass-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\lastMatch\this-subclass-constructor.js`

### Missing builtin method / undefined target
Representative files:
- `test262\test\annexB\built-ins\Date\prototype\getYear\nan.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\return-value.js`

### Unsupported yield/generator
Representative files:
- `test262\test\annexB\built-ins\RegExp\RegExp-control-escape-russian-letter.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-invalid-control-escape-character-class.js`
- `test262\test\annexB\language\expressions\yield\star-iterable-return-emulates-undefined-throws-when-called.js`

### Unsupported feature
Representative files:
- `test262\test\annexB\language\statements\for-of\iterator-close-return-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\Array\fromAsync\async-iterable-async-mapped-awaits-once.js`
- `test262\test\built-ins\Array\fromAsync\async-iterable-input-does-not-await-input.js`

### Unsupported BigInt
Representative files:
- `test262\test\annexB\built-ins\escape\argument_bigint.js`
- `test262\test\annexB\built-ins\unescape\argument_bigint.js`
- `test262\test\built-ins\Array\fromAsync\asyncitems-arraylike-length-accessor-throws.js`

### Module syntax / source-phase import gap
Representative files:
- `test262\test\built-ins\Proxy\preventExtensions\trap-is-undefined-target-is-proxy.js`
- `test262\test\language\comments\hashbang\module.js`
- `test262\test\language\eval-code\indirect\export.js`



## Skipped Tests

The log records two explicit skips:

- `test262\test\built-ins\Atomics\wait\bigint\cannot-suspend-throws.js`
- `test262\test\built-ins\Atomics\wait\cannot-suspend-throws.js`

Both are Atomics wait tests that require a suspend-capable host behavior. They remain explicit skips, not hidden passes.

## Interpretation

The V11 fixbug run is a clear improvement over the previous V11 diagnostic baseline:

1. Passing tests increased from 18,050 to 19,027.
2. Failures decreased from 35,325 to 34,348.
3. The net conformance gain is +1.83 pp on the full selected Test262 denominator.
4. The most visible fix effect is in RegExp property escapes and broad frontend syntax/static-semantics paths.
5. The largest remaining blockers are still outside a single builtin family: modern language syntax, class semantics, Temporal, module syntax, async/generator support, BigInt, cross-realm host support, and exact object/builtin descriptor behavior.

This result should be treated as a broad engineering baseline. It is useful for tracking direction, but it should not replace smaller pinned acceptance gates for each V11 fix.

## Suggested Follow-Up Order

1. Keep a pinned V11-fix regression gate containing the newly fixed files, especially the RegExp property-escape cases and the expression/statement cases that disappeared from the failure list.
2. Stabilize frontend static semantics next: class/function/object expression, strict early errors, destructuring, and for-of/for-await paths still dominate failure volume.
3. Implement or explicitly gate unsupported runtime families: BigInt, async functions, generators/yield, modules/import/export, Temporal, and cross-realm `$262.createRealm`.
4. Continue builtin-shape work for Date/String Annex B methods, Object/Reflect descriptors, RegExp legacy accessors, and exact `name` / `length` / property descriptor behavior.
5. Keep full-suite scans diagnostic and compare them against previous captured baselines; do not count skipped, failed, crashed, or timed-out tests as passes.
