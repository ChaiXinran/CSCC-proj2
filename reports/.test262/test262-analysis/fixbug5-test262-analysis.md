# Native Fixbug5 Test262 Analysis Report

## Scope

This report analyzes `test262-fixbug5-output.txt`, a full Native backend Test262 run over `test262/test`.

The command captured in the log is:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

This report follows the structure of `reports/native-v7-test262-report.md`: scope, diagnostic result, per-directory failure distribution, failure classification, interpretation, suggested follow-up order, and quality gates.

The run goes through the self-developed Native path and does not count skipped tests as passes.

---

## Overall Result

| Total | Passed | Failed | Skipped | Conformance | Elapsed |
| --- | ---: | ---: | ---: | ---: | ---: |
| 53,379 | 32,097 | 21,280 | 2 | 60.13% | 1377.58s |

This run completed the full `53,379`-case scan and produced a final summary.

```text
60% target = ceil(53379 * 0.6) = 32028
current passed = 32097
margin above 60% = 32097 - 32028 = 69
```

Fixbug5 has crossed the contest target, but only by **69 passing tests**. This means the result is valid and important, but still fragile: a small regression in class, async, iterator, or builtins may drop the project back below 60%.

---

## Compared with Fixbug4

| Run | Total | Passed | Failed | Skipped | Conformance | Δ Passed | Δ Failed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| fixbug4 | 53,379 | 28,468 | 24,909 | 2 | 53.33% | - | - |
| fixbug5 | 53,379 | 32,097 | 21,280 | 2 | 60.13% | +3,629 | -3,629 |

Net change from fixbug4 to fixbug5:

```text
passed +3629
failed -3629
conformance +6.80 pp
```

The gain is large enough to push the project over the 60% line. Most of the improvement comes from `language/statements`, `language/expressions`, `for-await-of`, `class`, and async/generator-related paths.

---

## Per-Directory Failure Distribution

### Top-level areas

| Area | Failed | Share of all failures |
| --- | ---: | ---: |
| `built-ins` | 9651 | 45.4% |
| `language` | 7337 | 34.5% |
| `intl402` | 3036 | 14.3% |
| `staging` | 847 | 4.0% |
| `annexB` | 377 | 1.8% |
| `harness` | 32 | 0.2% |

The remaining failures are still concentrated in:

```text
built-ins: 9651
language:  7337
intl402:   3036
```

`built-ins` and `language` together account for 79.8% of all remaining failures.

---

### Two-level directories

| Area | Failed | Share |
| --- | ---: | ---: |
| `built-ins/Temporal` | 4187 | 19.7% |
| `language/expressions` | 3447 | 16.2% |
| `language/statements` | 2856 | 13.4% |
| `intl402/Temporal` | 2020 | 9.5% |
| `built-ins/RegExp` | 1070 | 5.0% |
| `staging/sm` | 774 | 3.6% |
| `built-ins/Array` | 722 | 3.4% |
| `built-ins/Promise` | 471 | 2.2% |
| `language/module-code` | 398 | 1.9% |
| `built-ins/Atomics` | 388 | 1.8% |
| `built-ins/TypedArray` | 361 | 1.7% |
| `annexB/language` | 326 | 1.5% |
| `built-ins/Iterator` | 323 | 1.5% |
| `built-ins/Object` | 309 | 1.5% |
| `language/eval-code` | 235 | 1.1% |
| `intl402/NumberFormat` | 205 | 1.0% |
| `intl402/DateTimeFormat` | 187 | 0.9% |
| `built-ins/TypedArrayConstructors` | 184 | 0.9% |
| `built-ins/Set` | 167 | 0.8% |
| `intl402/Locale` | 125 | 0.6% |
| `built-ins/String` | 120 | 0.6% |
| `language/import` | 117 | 0.5% |
| `intl402/DurationFormat` | 110 | 0.5% |
| `built-ins/AsyncDisposableStack` | 104 | 0.5% |
| `built-ins/DisposableStack` | 93 | 0.4% |

Key observations:

- `built-ins/Temporal` and `intl402/Temporal` remain the largest residual family.
- `language/statements` and `language/expressions` dropped substantially compared with fixbug4.
- `built-ins/Promise` also improved, but still has 471 failures.
- `built-ins/Iterator` remains almost unchanged, which suggests that iterator prototype/builtin shape is still incomplete even though `for-await-of` improved.
- `built-ins/RegExp` is worse than fixbug4 and should be watched as a regression-risk area.

---

### Three-level directories

| Area | Failed | Share |
| --- | ---: | ---: |
| `language/statements/class` | 1408 | 6.6% |
| `language/expressions/class` | 1233 | 5.8% |
| `built-ins/Temporal/ZonedDateTime` | 896 | 4.2% |
| `built-ins/Temporal/PlainDateTime` | 677 | 3.2% |
| `language/expressions/dynamic-import` | 628 | 3.0% |
| `built-ins/Array/prototype` | 601 | 2.8% |
| `intl402/Temporal/ZonedDateTime` | 583 | 2.7% |
| `built-ins/Temporal/PlainDate` | 577 | 2.7% |
| `built-ins/RegExp/property-escapes` | 573 | 2.7% |
| `built-ins/Temporal/PlainYearMonth` | 507 | 2.4% |
| `intl402/Temporal/PlainDate` | 489 | 2.3% |
| `intl402/Temporal/PlainDateTime` | 481 | 2.3% |
| `built-ins/Temporal/Duration` | 452 | 2.1% |
| `built-ins/Temporal/PlainTime` | 431 | 2.0% |
| `built-ins/Temporal/Instant` | 413 | 1.9% |
| `built-ins/TypedArray/prototype` | 345 | 1.6% |
| `language/expressions/object` | 338 | 1.6% |
| `intl402/Temporal/PlainYearMonth` | 326 | 1.5% |
| `language/expressions/async-generator` | 274 | 1.3% |
| `language/statements/for-await-of` | 251 | 1.2% |
| `language/module-code/top-level-await` | 244 | 1.1% |
| `built-ins/Iterator/prototype` | 239 | 1.1% |
| `language/statements/for-of` | 228 | 1.1% |
| `language/eval-code/direct` | 209 | 1.0% |
| `built-ins/Temporal/PlainMonthDay` | 197 | 0.9% |

Class and async areas are still visible, but they are much smaller than in fixbug4:

```text
language/statements/class:    2051 -> 1408
language/expressions/class:   1864 -> 1233
language/statements/for-await-of: 1146 -> 251
language/expressions/async-generator: 542 -> 274
```

This confirms that the latest implementation made real progress in the exact areas that were identified as the fastest route to 60%.

---

### Four-level hotspots

| Area | Failed | Share |
| --- | ---: | ---: |
| `built-ins/Temporal/ZonedDateTime/prototype` | 740 | 3.5% |
| `language/statements/class/elements` | 651 | 3.1% |
| `language/expressions/class/elements` | 573 | 2.7% |
| `built-ins/Temporal/PlainDateTime/prototype` | 558 | 2.6% |
| `intl402/Temporal/ZonedDateTime/prototype` | 516 | 2.4% |
| `built-ins/Temporal/PlainDate/prototype` | 473 | 2.2% |
| `built-ins/RegExp/property-escapes/generated` | 469 | 2.2% |
| `language/expressions/class/dstr` | 464 | 2.2% |
| `language/statements/class/dstr` | 464 | 2.2% |
| `intl402/Temporal/PlainDate/prototype` | 449 | 2.1% |
| `intl402/Temporal/PlainDateTime/prototype` | 449 | 2.1% |
| `built-ins/Temporal/PlainYearMonth/prototype` | 384 | 1.8% |
| `built-ins/Temporal/Duration/prototype` | 356 | 1.7% |
| `built-ins/Temporal/Instant/prototype` | 348 | 1.6% |
| `built-ins/Temporal/PlainTime/prototype` | 346 | 1.6% |
| `intl402/Temporal/PlainYearMonth/prototype` | 287 | 1.3% |
| `language/module-code/top-level-await/syntax` | 206 | 1.0% |
| `language/expressions/dynamic-import/syntax` | 192 | 0.9% |
| `language/expressions/object/dstr` | 181 | 0.9% |
| `language/expressions/dynamic-import/catch` | 176 | 0.8% |
| `language/statements/for-of/dstr` | 160 | 0.8% |
| `language/expressions/async-generator/dstr` | 140 | 0.7% |
| `annexB/language/eval-code/direct` | 137 | 0.6% |
| `language/expressions/object/method-definition` | 119 | 0.6% |
| `built-ins/Temporal/PlainMonthDay/prototype` | 118 | 0.6% |

The largest remaining sub-hotspots are now:

```text
Temporal prototypes
class/elements
class/dstr
RegExp/property-escapes/generated
dynamic-import syntax/catch
top-level-await syntax
object/dstr
for-of/dstr
```

---

## Failure Reduction vs Fixbug4

### Biggest two-level improvements

| Area | fixbug4 failed | fixbug5 failed | Change |
| --- | ---: | ---: | ---: |
| `language/statements` | 4612 | 2856 | -1756 |
| `language/expressions` | 4568 | 3447 | -1121 |
| `built-ins/Proxy` | 211 | 10 | -201 |
| `built-ins/Date` | 246 | 71 | -175 |
| `built-ins/Promise` | 591 | 471 | -120 |
| `built-ins/Object` | 356 | 309 | -47 |
| `language/eval-code` | 276 | 235 | -41 |
| `staging/sm` | 810 | 774 | -36 |
| `built-ins/TypedArrayConstructors` | 195 | 184 | -11 |
| `built-ins/Array` | 729 | 722 | -7 |
| `built-ins/Iterator` | 326 | 323 | -3 |
| `intl402/DateTimeFormat` | 189 | 187 | -2 |
| `language/module-code` | 398 | 398 | +0 |
| `intl402/Temporal` | 2020 | 2020 | +0 |
| `built-ins/Temporal` | 4187 | 4187 | +0 |
| `built-ins/TypedArray` | 361 | 361 | +0 |
| `built-ins/Atomics` | 388 | 388 | +0 |
| `annexB/language` | 326 | 326 | +0 |

### Biggest three-level improvements

| Area | fixbug4 failed | fixbug5 failed | Change |
| --- | ---: | ---: | ---: |
| `language/statements/for-await-of` | 1146 | 251 | -895 |
| `language/statements/class` | 2051 | 1408 | -643 |
| `language/expressions/class` | 1864 | 1233 | -631 |
| `language/expressions/async-generator` | 542 | 274 | -268 |
| `language/expressions/object` | 492 | 338 | -154 |
| `built-ins/Array/prototype` | 607 | 601 | -6 |
| `built-ins/Temporal/PlainTime` | 431 | 431 | +0 |
| `built-ins/Temporal/PlainDateTime` | 677 | 677 | +0 |
| `built-ins/Temporal/ZonedDateTime` | 896 | 896 | +0 |
| `intl402/Temporal/ZonedDateTime` | 583 | 583 | +0 |
| `built-ins/Temporal/PlainYearMonth` | 507 | 507 | +0 |
| `built-ins/TypedArray/prototype` | 345 | 345 | +0 |
| `intl402/Temporal/PlainDate` | 489 | 489 | +0 |
| `language/expressions/dynamic-import` | 628 | 628 | +0 |
| `built-ins/Temporal/Instant` | 413 | 413 | +0 |
| `built-ins/Temporal/PlainDate` | 577 | 577 | +0 |
| `intl402/Temporal/PlainDateTime` | 481 | 481 | +0 |
| `built-ins/Temporal/Duration` | 452 | 452 | +0 |
| `intl402/Temporal/PlainYearMonth` | 326 | 326 | +0 |
| `intl402/Locale/constructor-options-numeric-undefined.js` | 0 | 1 | +1 |

The most important improvements are:

- `language/statements` reduced by 1,756 failures.
- `language/expressions` reduced by 1,121 failures.
- `language/statements/for-await-of` reduced by 895 failures.
- `language/statements/class` reduced by 643 failures.
- `language/expressions/class` reduced by 631 failures.
- `language/expressions/async-generator` reduced by 268 failures.
- `built-ins/Promise` reduced by 120 failures.

This matches the expected Fix4 strategy: prioritize async/generator/Promise/Iterator plus class/destructuring.

---

## Newly Exposed or Worsened Areas

Some areas increased in failure count compared with the fixbug4 baseline.

### Two-level increases

| Area | fixbug4 failed | fixbug5 failed | Change |
| --- | ---: | ---: | ---: |
| `built-ins/RegExp` | 842 | 1070 | +228 |
| `built-ins/Set` | 0 | 167 | +167 |
| `intl402/Locale` | 0 | 125 | +125 |
| `built-ins/String` | 0 | 120 | +120 |
| `language/import` | 0 | 117 | +117 |
| `intl402/DurationFormat` | 0 | 110 | +110 |
| `built-ins/AsyncDisposableStack` | 0 | 104 | +104 |
| `built-ins/DisposableStack` | 0 | 93 | +93 |
| `built-ins/Function` | 0 | 93 | +93 |
| `intl402/Segmenter` | 0 | 78 | +78 |

### Three-level increases

| Area | fixbug4 failed | fixbug5 failed | Change |
| --- | ---: | ---: | ---: |
| `language/module-code/top-level-await` | 0 | 244 | +244 |
| `built-ins/Iterator/prototype` | 0 | 239 | +239 |
| `language/statements/for-of` | 0 | 228 | +228 |
| `built-ins/RegExp/property-escapes` | 345 | 573 | +228 |
| `language/eval-code/direct` | 0 | 209 | +209 |
| `built-ins/Temporal/PlainMonthDay` | 0 | 197 | +197 |
| `annexB/language/eval-code` | 0 | 177 | +177 |
| `built-ins/RegExp/prototype` | 0 | 176 | +176 |
| `language/statements/with` | 0 | 160 | +160 |
| `built-ins/Set/prototype` | 0 | 155 | +155 |

The clearest warning sign is `built-ins/RegExp`:

```text
built-ins/RegExp: 842 -> 1070 (+228)
built-ins/RegExp/property-escapes: 345 -> 573 (+228)
```

This may mean one of two things:

1. previous failures were hidden behind earlier parser/runtime failures and are now exposed;
2. recent changes regressed RegExp property escapes or RegExp builtin behavior.

This area should be tested before final submission, but it should not replace the current 60% stabilization work unless the regression drops the full run below 60%.

---

## Failure Classification

The following categories are derived from failure detail strings. They are diagnostic heuristics, not a formal ECMAScript taxonomy.

| Failure class | Count | Share |
| --- | ---: | ---: |
| Temporal / Intl Temporal incomplete | 6302 | 29.6% |
| Assertion / runtime semantic mismatch | 4052 | 19.0% |
| Frontend syntax / static semantics mismatch | 1517 | 7.1% |
| Generator / yield / async iterator gaps | 1357 | 6.4% |
| RegExp parsing / legacy semantics | 1244 | 5.8% |
| Module syntax / source-phase import / top-level await gaps | 1145 | 5.4% |
| Binding / environment record semantics | 1133 | 5.3% |
| Missing builtin / undefined call target | 910 | 4.3% |
| Property descriptor / builtin object shape gaps | 867 | 4.1% |
| Promise / async / job queue gaps | 795 | 3.7% |
| TypedArray / ArrayBuffer / DataView semantics | 760 | 3.6% |
| Other runtime or semantic failure | 615 | 2.9% |
| Host object / cross-Realm / Proxy / weak collection gaps | 308 | 1.4% |
| Runtime limit / performance guard | 275 | 1.3% |

Interpretation:

- `Temporal / Intl Temporal` remains large, but it is not the best short-term target because full Temporal semantics are expensive.
- Assertion and semantic mismatch is the second largest category, meaning many remaining tests now execute but produce subtly wrong results.
- Promise/async and generator/yield failures are still significant, but much smaller than before.
- RegExp is now a visible risk area.
- Descriptor, builtin shape, and undefined call targets still appear across many builtins.

---

## Representative Failure Samples

### Temporal / Intl Temporal incomplete

- `test/built-ins/Array/fromAsync/asyncitems-arraylike-promise.js` — default mode: harness `temporalHelpers.js` failed: SyntaxError: lex error: unterminated string literal at bytes 1123. .1131
- `test/built-ins/Array/fromAsync/asyncitems-asynciterator-exists.js` — default mode: harness `temporalHelpers.js` failed: SyntaxError: lex error: unterminated string literal at bytes 1123. .1131
- `test/built-ins/Array/fromAsync/asyncitems-asynciterator-null.js` — default mode: harness `temporalHelpers.js` failed: SyntaxError: lex error: unterminated string literal at bytes 1123. .1131

### Assertion / runtime semantic mismatch

- `test/annexB/built-ins/Array/from/iterator-method-emulates-undefined.js` — default mode: unexpected Test262Error: execution error: Expected a TypeError to be thrown but no exception was thrown at all in `test262\test\annexB\built-ins\Array\from\iterator-method-emulates-undefined.js`
- `test/annexB/language/eval-code/direct/func-block-decl-eval-func-existing-var-update.js` — default mode: unexpected Test262Error: execution error: Expected SameValue(芦"undefined"禄, 芦"function"禄) to be true in `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-existing-var-update.js`
- `test/annexB/language/eval-code/direct/func-block-decl-eval-func-skip-early-err.js` — default mode: unexpected Test262Error: execution error: value is not updated following evaluation Expected SameValue( 芦function f() { [native code] }禄, 芦123禄) to be true in `test262\test\annexB\language\eval-code\direct\

### Frontend syntax / static semantics mismatch

- `test/annexB/built-ins/Function/createdynfn-html-close-comment-body.js` — default mode: unexpected SyntaxError: execution error: dynamic Function source could not be compiled: unexpected `>` at bytes 27..28 in `test262\test\annexB\built-ins\Function\createdynfn-html-close-comment-body.js`
- `test/annexB/built-ins/Function/createdynfn-html-close-comment-params.js` — default mode: unexpected SyntaxError: execution error: dynamic Function source could not be compiled: expected identi fier but found `--` at bytes 21..23 in `test262\test\annexB\built-ins\Function\createdynfn-html-close-
- `test/annexB/built-ins/Function/createdynfn-html-open-comment-body.js` — default mode: unexpected SyntaxError: execution error: dynamic Function source could not be compiled: unexpected `<` at bytes 24..25 in `test262\test\annexB\built-ins\Function\createdynfn-html-open-comment-body.js`

### Generator / yield / async iterator gaps

- `test/annexB/language/expressions/yield/star-iterable-return-emulates-undefined-throws-when-called.js` — default mode: unexpected Test262Error: execution error: Expected a TypeError to be thrown but no exception was thrown at all in `test262\test\annexB\language\expressions\yield\star-iterable-return-emulates-undefined-thro
- `test/annexB/language/expressions/yield/star-iterable-throw-emulates-undefined-throws-when-called.js` — default mode: unexpected Test262Error: execution error: Expected SameValue(芦1禄, 芦0禄) to be true in `test262\test\anne xB\language\expressions\yield\star-iterable-throw-emulates-undefined-throws-when-called.js`
- `test/annexB/language/statements/for-await-of/iterator-close-return-emulates-undefined-throws-when-called.j` — s default mode: TypeError: execution error: cannot test property on expected TypeError because `iter[Symbol.asyncIterat or]() returned a non-object: shouldn't touch Symbol.iterator Expected SameValue(芦function Test262Err

### RegExp parsing / legacy semantics

- `test/annexB/built-ins/RegExp/RegExp-decimal-escape-class-range.js` — default mode: unexpected SyntaxError: execution error: invalid regular expression: regex parse error: [\d][\12-\14]{1,}[^\d] ^^ error: backreferences are not supported in `test262\test\annexB\built-ins\RegExp\RegExp-deci
- `test/annexB/built-ins/RegExp/RegExp-control-escape-russian-letter.js` — default mode: unexpected SyntaxError: execution error: invalid regular expression: regex parse error: \c袗 ^^ error: unrecognized escape sequence in `test262\test\annexB\built-ins\RegExp\RegExp-control-escape-russian-lett
- `test/annexB/built-ins/RegExp/RegExp-decimal-escape-not-capturing.js` — default mode: unexpected SyntaxError: execution error: invalid regular expression: regex parse error: \b(\w+) \2\b ^^ error: backreferences are not supported in `test262\test\annexB\built-ins\RegExp\RegExp-decimal-escape

### Module syntax / source-phase import / top-level await gaps

- `test/built-ins/AbstractModuleSource/length.js` — module mode: unexpected Test262Error: execution error: Expected SameValue(芦"undefined"禄, 芦"function"禄) to be true in `test262\test\built-ins\AbstractModuleSource\length.js`
- `test/built-ins/AbstractModuleSource/name.js` — module mode: unexpected Test262Error: execution error: Expected SameValue(芦"undefined"禄, 芦"function"禄) to be true in `test262\test\built-ins\AbstractModuleSource\name.js`
- `test/built-ins/AbstractModuleSource/proto.js` — module mode: unexpected Test262Error: execution error: Expected SameValue(芦"undefined"禄, 芦"function"禄) to be true in `test262\test\built-ins\AbstractModuleSource\proto.js`

### Binding / environment record semantics

- `test/annexB/language/eval-code/direct/func-block-decl-eval-func-block-scoping.js` — default mode: unexpected ReferenceError: execution error: f is not defined at instruction 1 in `test262\test\annexB\l anguage\eval-code\direct\func-block-decl-eval-func-block-scoping.js`
- `test/annexB/language/eval-code/direct/func-block-decl-eval-func-existing-block-fn-no-init.js` — default mode: unexpected ReferenceError: execution error: f is not defined at instruction 0 in `test262\test\annexB\l anguage\eval-code\direct\func-block-decl-eval-func-existing-block-fn-no-init.js`
- `test/annexB/language/eval-code/direct/func-block-decl-eval-func-existing-block-fn-update.js` — default mode: unexpected ReferenceError: execution error: f is not defined at instruction 2 in `test262\test\annexB\l anguage\eval-code\direct\func-block-decl-eval-func-existing-block-fn-update.js`

### Missing builtin / undefined call target

- `test/annexB/built-ins/Object/is/emulates-undefined.js` — default mode: unexpected TypeError: execution error: undefined is not callable in `test262\test\annexB\built-ins\Obje ct\is\emulates-undefined.js`
- `test/annexB/built-ins/String/prototype/match/custom-matcher-emulates-undefined.js` — default mode: unexpected TypeError: execution error: cannot define property on undefined in `test262\test\annexB\buil t-ins\String\prototype\match\custom-matcher-emulates-undefined.js`
- `test/annexB/built-ins/String/prototype/matchAll/custom-matcher-emulates-undefined.js` — default mode: unexpected TypeError: execution error: cannot define property on undefined in `test262\test\annexB\buil t-ins\String\prototype\matchAll\custom-matcher-emulates-undefined.js`


---

## Interpretation

### 1. Fixbug5 reaches the 60% target

The most important result is:

```text
32097 / 53379 = 60.13%
```

This exceeds the 60% target by 69 tests. The project has therefore reached the required conformance threshold in this captured run.

However, the margin is small. A regression of only 70 tests would put the project below 60%. The next work should be stabilization and regression protection, not broad risky rewrites.

---

### 2. The previous strategy was correct

The fixbug4 report recommended focusing on:

```text
async / generator / iterator / Promise
class elements + destructuring
TypedArray / DataView and descriptor cleanup
```

The fixbug5 data confirms this was the correct route:

```text
for-await-of:       -895 failures
class statements:   -643 failures
class expressions:  -631 failures
async-generator:    -268 failures
Promise:            -120 failures
```

This is the main reason the pass count crossed 60%.

---

### 3. Temporal remains too large but not worth full implementation now

Temporal-related failures still total roughly:

```text
built-ins/Temporal: 4187
intl402/Temporal:   2020
combined:           6207
```

This is the largest single family, but full implementation is too expensive for a final sprint. Only a small Temporal skeleton is worth considering, and only if it is low risk.

Recommended Temporal policy:

```text
do not implement full Temporal
only add descriptor/name/length/prototype skeletons if they are isolated
do not touch date/time arithmetic in final stabilization
```

---

### 4. Remaining class work is still valuable

Class-related failures are still:

```text
language/statements/class:  1408
language/expressions/class: 1233
combined:                   2641
```

Within class, the biggest residual groups are:

```text
language/statements/class/elements:  651
language/expressions/class/elements: 573
language/statements/class/dstr:      464
language/expressions/class/dstr:     464
```

Class is still worth fixing, but changes here must be regression-tested because it was one of the main sources of the 60% gain.

---

### 5. Iterator builtin shape is still weak

Although `for-await-of` improved greatly, `built-ins/Iterator` barely moved:

```text
built-ins/Iterator: 326 -> 323
```

This suggests the execution path improved, but JS-visible `Iterator` builtin/prototype shape remains incomplete.

Recommended focus:

```text
Iterator constructor/prototype descriptor
Iterator.prototype methods
IteratorResult object shape
IteratorClose behavior
Array.from / TypedArray.from shared helper
```

---

### 6. RegExp now needs regression control

`built-ins/RegExp` increased from 842 to 1070. The largest visible bucket is:

```text
built-ins/RegExp/property-escapes: 573
```

This should become a focused regression gate. Do not necessarily make RegExp the top implementation priority, but make sure future changes do not worsen it further.

---

## Suggested Follow-Up Order

### Priority 0: Lock the current 60% result

Before adding risky features, immediately preserve the current state:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/fixbug5-full-test262-summary.json
```

Create or update:

```text
reports/fixbug5-test262-analysis.md
reports/fixbug5-full-test262-summary.json
docs/status.md
readme.md
AGENTS.md
```

The goal is to make the 60.13% result reproducible.

---

### Priority 1: Build a 60% regression gate

Create a smaller gate that covers the directories responsible for the latest gain:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/async-generator --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress
```

This should be run before every merge.

---

### Priority 2: Stabilize async/generator/Promise/Iterator

Remaining targets:

```text
built-ins/Promise:                 471
built-ins/Iterator:                323
language/expressions/async-generator: 274
language/statements/for-await-of:  251
```

Recommended work:

```text
Promise job queue ordering
then/catch/finally callback scheduling
Iterator.prototype descriptor
IteratorClose on abrupt completion
yield* return/throw edge cases
async generator dstr cases
```

This is still the best place to gain a few hundred safety tests.

---

### Priority 3: Finish class/elements and class/dstr

Remaining targets:

```text
language/statements/class/elements:  651
language/expressions/class/elements: 573
language/statements/class/dstr:      464
language/expressions/class/dstr:     464
```

Recommended work:

```text
class field initialization order
private method / private getter / private setter
static elements
computed property name order
class method parameter destructuring
derived constructor / super()
```

---

### Priority 4: RegExp focused regression sweep

Run:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp/property-escapes --jobs 4 --progress
```

Recommended work:

```text
property escapes generated cases
legacy accessors
RegExp.prototype.compile
named groups / backreferences
RegExp.prototype[@@split]
```

---

### Priority 5: Descriptor / builtin shape sweep

Remaining low-to-medium risk targets:

```text
built-ins/Object:    309
built-ins/Array:     722
built-ins/String:    120
built-ins/Date:      71
```

Recommended work:

```text
name / length
prototype / constructor
writable / enumerable / configurable
not-a-constructor behavior
own property checks
String trimLeft/trimStart aliases
Date Annex B getYear/setYear
```

---

## Quality Gates

Before claiming contest readiness, require:

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
```

If feasible:

```powershell
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

Full Test262 final gate:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

Required final acceptance:

```text
passed >= 32028
conformance >= 60.00%
skipped not counted as passed
no panic
no runner crash
no memory allocation failure
```

---

## Final Conclusion

Fixbug5 is the first captured run in this sequence that crosses the 60% target:

```text
passed = 32097
target = 32028
margin = +69
conformance = 60.13%
```

The current implementation should be treated as a contest-ready but fragile milestone. The immediate next step is not a large new feature. The immediate next step is to lock this state with reports, JSON summaries, and regression gates.

Recommended final sprint order:

```text
1. Lock and preserve the 60.13% result.
2. Add a small regression gate for class + async/generator + Promise/Iterator.
3. Stabilize Iterator and Promise shape for extra safety margin.
4. Fix remaining class/elements and class/dstr cases.
5. Run a RegExp focused regression sweep.
6. Only consider Temporal skeletons if they are isolated and low-risk.
```

The project has crossed the formal threshold, but it should aim for at least 60.5% before final submission to survive small regressions.
