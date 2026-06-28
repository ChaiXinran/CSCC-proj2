# Native Full Test262 Analysis

Updated: 2026-06-24 (UTC+08:00)

## Scope

This document analyzes the current Native backend against the broad Test262
`test/` suite using the exact command requested for the full scan:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

The run completed and wrote:

- `reports/native-full-test262-summary.json`
- `reports/native-full-test262-output.txt`

Important: this command scans all of `test/`, including `test/staging`.
Therefore this report is a direct full stress/conformance scan, not the same
shape as older sharded non-staging reports.

## Observed Result

JSON summary:

```json
{
  "total": 53379,
  "passed": 14035,
  "failed": 38507,
  "skipped": 837,
  "conformance_percent": 26.2931,
  "elapsed_ms": 49161
}
```

Interpretation:

- The direct full run completed; no stack overflow was observed in this run.
- Pass rate for this exact run is `14,035 / 53,379 = 26.29%`.
- The process exit code was `1` because failures were present, not because the
  runner failed to write the summary.
- The captured output was written by PowerShell as UTF-16LE, so parsers should
  decode `reports/native-full-test262-output.txt` as UTF-16.

## Error-Type Counts

The captured output contains 38,507 failure detail lines.

| Reported error class | Count | Share of failures |
| --- | ---: | ---: |
| `SyntaxError` | 17,905 | 46.50% |
| `ReferenceError` | 7,201 | 18.70% |
| Harness-load / harness-thrown errors | 6,210 | 16.13% |
| `Test262Error` | 3,024 | 7.85% |
| Other / no parsed detail | 2,691 | 6.99% |
| `TypeError` | 1,419 | 3.68% |
| `RangeError` | 43 | 0.11% |
| Direct parser error lines | 14 | 0.04% |

## Root-Cause Classification

The table below is an exclusive heuristic classification of the 38,507 observed
failures. Each failure is assigned to the first matching category, so counts add
up to the observed failure total. This is diagnostic, not a formal
spec-taxonomy.

| Failure class | Count | Share | Main meaning |
| --- | ---: | ---: | --- |
| Parser syntax gap | 16,259 | 42.22% | Modern syntax or dynamic source forms not parsed/lowered yet |
| Missing global / builtin / harness helper | 9,219 | 23.94% | Large unimplemented surfaces or missing Test262 host helpers |
| Template literal substitutions unsupported | 5,307 | 13.78% | `` `${...}` `` still blocks harnesses and test bodies |
| Other / unclassified | 2,527 | 6.56% | Mostly failure lines without a following detail line or mixed staging cases |
| Assertion / semantic mismatch | 2,131 | 5.53% | Test runs but value, descriptor, ordering, or observable behavior differs |
| Runtime call/property/object-model gap | 789 | 2.05% | Missing method dispatch, property descriptor, prototype, or object-model edge |
| Expected-error / early-error mismatch | 789 | 2.05% | Wrong error kind or no expected early/runtime error |
| Lexer/static syntax gap | 530 | 1.38% | Tokenization, escapes, numeric literals, or static syntax handling |
| RegExp engine feature gap | 528 | 1.37% | Backreferences, property escapes, invalid-regex error kind, Annex B RegExp |
| Other TypeError runtime failure | 379 | 0.98% | TypeErrors not captured by object-model bucket |
| Other RangeError runtime failure | 43 | 0.11% | Mostly numeric/array length boundary behavior |
| Module runner / module syntax failures | 4 | 0.01% | Failed module-related files that were not skipped |
| Other ReferenceError runtime failure | 2 | 0.01% | Residual ReferenceError cases not matching missing globals |

Module is underrepresented in the failure table because most module cases are
reported as skipped:

| Skip reason | Count |
| --- | ---: |
| `module runner not implemented yet` | 821 |
| no detail | 14 |
| `non-blocking agent tests are not enabled` | 2 |

## Top-Level Failure Distribution

| Top-level area | Failures | Share |
| --- | ---: | ---: |
| `language` | 16,660 | 43.27% |
| `built-ins` | 16,482 | 42.80% |
| `intl402` | 3,331 | 8.65% |
| `staging` | 1,199 | 3.11% |
| `annexB` | 748 | 1.94% |
| `harness` | 87 | 0.23% |

## Missing Global / Builtin Hotspots

Top missing names observed in failure details:

| Missing name | Count |
| --- | ---: |
| `Float64Array` | 2,018 |
| `Temporal` | 1,950 |
| `Date` | 786 |
| `Intl` | 597 |
| `ArrayBuffer` | 586 |
| `Promise` | 424 |
| `Proxy` | 382 |
| `$262` | 230 |
| `Set` | 216 |
| `Iterator` | 193 |
| `Map` | 169 |
| `DataView` | 161 |
| `import` | 137 |
| `SharedArrayBuffer` | 122 |
| `Atomics` | 91 |
| `WeakMap` | 89 |
| `DisposableStack` | 87 |
| `BigInt` | 72 |
| `Uint8Array` | 64 |
| `AsyncDisposableStack` | 55 |
| `Int32Array` | 54 |
| `ShadowRealm` | 50 |
| `WeakSet` | 47 |
| `FinalizationRegistry` | 38 |
| `testLenientAndStrict` | 35 |
| `WeakRef` | 23 |
| `BigInt64Array` | 22 |

Not all names in this table represent true missing globals. Some lowercase
entries in the raw output are ordinary program bindings that failed because an
earlier parser/scope/runtime behavior was wrong. The high-signal entries are
the standard globals and Test262 host helpers.

## Directory Hotspots

Highest failure-density path buckets:

| Path bucket | Failures |
| --- | ---: |
| `language/statements/class` | 3,706 |
| `language/expressions/class` | 3,443 |
| `built-ins/TypedArray/prototype` | 1,404 |
| `language/statements/for-await-of` | 1,142 |
| `built-ins/Array/prototype` | 986 |
| `built-ins/Temporal/ZonedDateTime` | 901 |
| `language/expressions/object` | 819 |
| `built-ins/Temporal/PlainDateTime` | 773 |
| `language/expressions/dynamic-import` | 718 |
| `language/statements/for-of` | 656 |
| `built-ins/Temporal/PlainDate` | 652 |
| `built-ins/RegExp/property-escapes` | 609 |
| `intl402/Temporal/ZonedDateTime` | 583 |
| `built-ins/Temporal/Duration` | 540 |
| `language/expressions/async-generator` | 537 |
| `built-ins/Temporal/PlainYearMonth` | 509 |
| `built-ins/DataView/prototype` | 499 |
| `built-ins/Temporal/PlainTime` | 493 |
| `intl402/Temporal/PlainDate` | 493 |
| `built-ins/Date/prototype` | 485 |
| `intl402/Temporal/PlainDateTime` | 483 |
| `built-ins/Temporal/Instant` | 465 |
| `built-ins/RegExp/prototype` | 378 |
| `built-ins/Iterator/prototype` | 373 |
| `built-ins/Set/prototype` | 357 |
| `annexB/language/eval-code` | 330 |
| `intl402/Temporal/PlainYearMonth` | 327 |
| `language/expressions/assignment` | 322 |
| `language/expressions/compound-assignment` | 317 |
| `language/statements/for` | 301 |

## Representative Root Causes

### 1. Parser and modern syntax gaps

Largest class: 16,259 failures.

Observed hotspots:

- class declarations and class expressions;
- `for-await-of`;
- dynamic import;
- object literal advanced forms;
- async generator and generator syntax;
- assignment and compound assignment edge cases;
- Annex B dynamic Function source forms.

Typical symptoms:

- `parse error: expected ...`
- `dynamic Function source could not be compiled`
- `unexpected token`

Impact:

These failures occur before execution, so they mask many later runtime and
builtin semantics. This is the highest-leverage frontend/bytecode area.

### 2. Missing globals, builtin families, and Test262 host helpers

Second largest class: 9,219 failures.

High-signal missing surfaces:

- TypedArray constructors, especially `Float64Array`;
- `Temporal`;
- `Date`;
- `Intl`;
- `ArrayBuffer`;
- `Promise`;
- `Proxy`;
- `$262`;
- `Set`, `Map`, `Iterator`;
- `SharedArrayBuffer`, `Atomics`;
- `WeakMap`, `WeakSet`, `WeakRef`, `FinalizationRegistry`;
- `DisposableStack`, `AsyncDisposableStack`;
- `BigInt`.

Impact:

Large directories cannot produce useful semantic signals until the global
objects exist. For V8 planning, prefer constructor/prototype/descriptor skeletons
first, then fill method semantics by failure cluster.

### 3. Template literal substitution gap

Count: 5,307 failures.

Typical symptom:

```text
lex error: template substitutions are not supported
```

Impact:

This blocks both test bodies and shared helpers. It especially affects Temporal,
Promise, RegExp helper code, String raw tests, and many modern syntax tests.
Supporting `` `${expr}` `` should unlock a large number of currently masked
failures.

### 4. Assertion and semantic mismatches

Count: 2,131 failures.

Typical symptoms:

- `Expected SameValue(...) to be true`
- expected a specific error type but got another;
- descriptor/property shape mismatch;
- ordering mismatch.

Impact:

These are the most useful failures after syntax and missing-global blockers are
reduced. They usually point to real spec behavior gaps rather than simply
missing parser support.

### 5. Runtime object-model and method-dispatch gaps

Count: 789 failures.

Typical symptoms:

- `cannot read property on undefined`;
- `is not callable`;
- `property setter is undefined`;
- `cannot get own property descriptor on undefined`.

Impact:

Likely causes include incomplete prototype installation, missing builtin method
properties, descriptor exactness, receiver handling, and property lookup
edge-cases.

### 6. RegExp engine and Annex B RegExp gaps

Count: 528 failures.

Observed areas:

- `built-ins/RegExp/property-escapes`;
- backreferences;
- invalid regex error kind;
- Annex B RegExp legacy accessors and `compile`.

Impact:

RegExp is cross-cutting: lexer/parser, regex engine, RegExp object behavior, and
String method dispatch all interact. Treat it as its own focused milestone.

### 7. Module runner gap

Failures in module-related files are low because most module cases are skipped:

```text
module runner not implemented yet
```

Skipped module cases: 821.

Impact:

Adding a native module execution path is the clearest way to reduce skipped
tests. Minimal scope should include `flags: [module]`, module top-level strict
mode, relative loading, module registry, and import/export syntax.

## Recommended Next Steps

Suggested V8 order based on this run:

1. Frontend unblockers:
   - template literal substitutions;
   - class declaration/expression coverage;
   - spread/rest/destructuring;
   - `for-of` / `for-await-of`;
   - async/generator parser and lowering;
   - dynamic import syntax triage.
2. Module runner:
   - implement the native `flags: [module]` path;
   - reduce the 821 module skips;
   - report module failures separately from script failures.
3. Builtin/global skeletons:
   - TypedArray constructors and `ArrayBuffer`;
   - `Temporal`;
   - `Intl`;
   - `Date`;
   - `Promise`;
   - `Proxy`;
   - `Map` / `Set` / `Iterator`;
   - `$262` host helper coverage required by selected Test262 cases.
4. Focused semantic cleanup:
   - descriptor exactness;
   - prototype/method installation;
   - receiver and property lookup behavior;
   - expected error kind/order.
5. RegExp milestone:
   - property escapes;
   - backreferences;
   - Annex B legacy accessors;
   - invalid-regex error mapping.

## Artifacts

- Full JSON summary: `reports/native-full-test262-summary.json`
- Captured full output: `reports/native-full-test262-output.txt`
- Short conformance report: `reports/test262-report.md`
