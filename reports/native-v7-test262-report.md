# Native V7 Test262 Report

## Scope

Native V7 is an engineering-hardening milestone. It focuses on resource
budgets, allocation guards, non-moving GC, native script caching, crash-safe
Test262 dashboards, and benchmark evidence. It does not add new JavaScript
syntax or a new builtin family.

All commands below run through the self-developed Native lexer, parser,
bytecode compiler, VM, runtime, builtin implementation, GC, and cache path. Boa
is not used as a fallback.

## Acceptance Gate

Command:

```powershell
cargo run --release --no-default-features -- test262 --native-v7 --jobs 1 --json reports/native-v7-gate-summary.json
```

Result:

| Total | Passed | Failed | Skipped | Conformance |
| ---: | ---: | ---: | ---: | ---: |
| 69 | 69 | 0 | 0 | 100.00% |

The V7 pinned gate aggregates the zero-failure, zero-skip Native V1-V6 Test262
files and verifies that they still pass after the V7 stability, GC, cache, and
reporting integration. This is the zero-regression merge gate, not a broad V7
conformance percentage.

## Diagnostic V7 Scan

Command:

```powershell
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/native-v7-frontend-summary.json
```

Verbose failure capture used for this analysis:

```powershell
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --verbose --json reports/native-v7-frontend-summary.json *> reports\native-v7-verbose.txt
```

Scanned directories:

- `test/language/literals`
- `test/language/types`
- `test/language/block-scope`
- `test/language/function-code`
- `test/language/global-code`
- `test/built-ins/Function`
- `test/built-ins/String`
- `test/built-ins/Symbol`
- `test/built-ins/Reflect`

Result:

| Total | Passed | Failed | Skipped | Conformance |
| ---: | ---: | ---: | ---: | ---: |
| 3,034 | 1,977 | 1,045 | 12 | 65.16% |

Skipped tests are not counted as passes. The diagnostic scan intentionally
covers several areas outside the V7 feature scope so that future conformance
work has a stable baseline.

B-line integration added dynamic `Function`, global `eval`,
`Function.prototype.apply`, `Function.prototype[Symbol.hasInstance]`, bound
function `instanceof`, catchable invalid-call / invalid-`instanceof`
TypeErrors, top-level `this`, `globalThis`, and sloppy/strict receiver
handling. Compared with the previous V7 scan baseline, this adds 206 passing
tests without changing the 12 explicit unsupported skips.

## Per-Directory Results

| Area | Total | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: | ---: |
| `test/language/literals` | 534 | 240 | 294 | 0 | 44.94% |
| `test/language/types` | 113 | 94 | 11 | 8 | 83.19% |
| `test/language/block-scope` | 145 | 110 | 35 | 0 | 75.86% |
| `test/language/function-code` | 217 | 161 | 55 | 1 | 74.19% |
| `test/language/global-code` | 42 | 11 | 29 | 2 | 26.19% |
| `test/built-ins/Function` | 509 | 242 | 267 | 0 | 47.54% |
| `test/built-ins/String` | 1,223 | 975 | 247 | 1 | 79.72% |
| `test/built-ins/Symbol` | 98 | 39 | 59 | 0 | 39.80% |
| `test/built-ins/Reflect` | 153 | 105 | 48 | 0 | 68.63% |

The largest remaining absolute failure bucket is now `language/literals`.
`Function` remains important, but B-line work reduced it by 124 failures.
`String` has many scanned files but a comparatively strong pass rate after the
V6 builtin work plus B-line call/apply/eval improvements.

## Failure Classification

The following categories are derived from `reports/native-v7-verbose.txt` by
grouping failure detail strings. They are diagnostic heuristics, not a formal
spec taxonomy.

| Failure class | Count | Share of failures |
| --- | ---: | ---: |
| Syntax, lexer, or static-negative mismatch | 424 | 40.6% |
| Assertion or semantic mismatch | 311 | 29.8% |
| Undefined property or call target | 93 | 8.9% |
| TypeError / runtime object-model gap | 68 | 6.5% |
| Missing global object or cross-realm host support | 55 | 5.2% |
| Other runtime or semantic failure | 48 | 4.6% |
| Reference binding or property gap | 46 | 4.4% |

### Syntax, lexer, and static-negative mismatch

This is the largest bucket. It includes:

- unsupported class syntax in Function internal tests;
- optional chaining / modern member syntax in some builtin tests;
- resizable-array-buffer helper syntax in harness includes;
- strict-mode string literal negative tests where legacy octal or non-octal
  escapes should be rejected but currently parse;
- Unicode line/paragraph separator string literal behavior.

Representative failures:

- `test/built-ins/Function/internals/Call/class-ctor.js`
- `test/built-ins/Function/prototype/bind/instance-construct-newtarget-boundtarget-bound.js`
- `test/language/literals/string/legacy-octal-escape-sequence-strict.js`
- `test/language/literals/string/line-separator.js`

Most of this is outside V7's explicit feature scope. It becomes important if
the next milestone targets broader language syntax and strict-mode literal
conformance.

### Dynamic `Function` and `eval`

The B-line integration now installs global `eval` and routes both
`Function(...)` and `new Function(...)` through the native lexer, parser,
compiler, VM, and runtime. Dynamic functions are created in the global
environment, expose `name`, `length`, and `prototype`, and surface syntax
errors as catchable `SyntaxError` values.

Remaining failures in this neighborhood are no longer caused by a missing
global `eval` or missing dynamic Function entry point. They mostly come from
source forms the current native frontend still rejects or only partially
models, especially strict-mode string literal early errors, Unicode
line/paragraph separator behavior, class syntax used by internal Function
tests, and advanced construct/newTarget cases.

Representative remaining failures:

- `test/built-ins/Function/internals/Call/class-ctor.js`
- `test/built-ins/Function/prototype/bind/instance-construct-newtarget-boundtarget-bound.js`
- `test/language/literals/string/line-separator-eval.js`

### Function object and object-model semantics

The main B-line call surface is now covered by focused native tests:
`Function.prototype.apply`, existing `call` / `bind` forwarding, dynamic
Function construction, global `eval`, `Function.prototype[Symbol.hasInstance]`,
bound function `instanceof`, top-level `this`, `globalThis`, sloppy/strict
receiver handling, and catchable invalid call / invalid `instanceof`
TypeErrors.

The `Function` directory still exposes incomplete details around:

- `Function.prototype` property descriptors;
- exact `name`, `length`, and native-source `toString` behavior for every
  builtin and bound function shape;
- construct/newTarget semantics for bound functions and classes;
- cross-realm receiver and constructor identity checks;
- Proxy/class paths that are outside the current Native V7 scope.

Representative failure messages now mostly include assertion mismatches,
unsupported class/proxy syntax, and object-model descriptor differences rather
than missing `apply`, missing `eval`, or missing top-level `this`.

These are real conformance gaps, but they are mostly V4/V6 object-model and
function-semantics follow-up work rather than V7 GC/cache/reporting issues.

### Cross-realm and missing global host objects

Some tests require Test262 host or ECMAScript objects not implemented in the
native runtime:

- `$262` realm helpers;
- `Proxy`;
- cross-realm constructor and prototype identity checks.

Representative failures:

- `test/built-ins/Function/call-bind-this-realm-undef.js`
- `test/built-ins/Function/internals/Construct/base-ctor-revoked-proxy.js`
- `test/language/types/reference/get-value-prop-base-primitive-realm.js`

These are intentionally not V7 goals.

### String and Symbol areas

`String` is the strongest broad directory in this scan:

| Area | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: |
| `test/built-ins/String` | 975 | 247 | 1 | 79.72% |

Remaining String failures mostly come from advanced method semantics,
RegExp-like replacement behavior, Unicode edge cases, and callback ordering.

`Symbol` is lower:

| Area | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: |
| `test/built-ins/Symbol` | 39 | 59 | 0 | 39.80% |

Symbol tests depend heavily on exact property descriptors, wrapper behavior,
well-known symbol protocol interactions, and `Object.prototype.toString`
tagging.

## Skipped Tests

The 12 skipped tests are unsupported compiler shapes, not hidden passes:

| Skip class | Count |
| --- | ---: |
| Unsupported `++` / `--` operand shape | 6 |
| Unsupported `delete` operand shape | 6 |

Representative skipped files:

- `test/built-ins/String/S15.5.5.1_A4_T1.js`
- `test/language/types/object/S8.6_A2_T1.js`
- `test/language/function-code/S10.1.6_A1_T1.js`
- `test/language/global-code/script-decl-lex-deletion.js`

## Interpretation

The pinned V7 gate remains stable while the broad diagnostic scan now also
reflects B-line Function/runtime improvements. The scan is still useful because
it proves the V7 runner, native runtime, GC/cache path, and JSON reporting can
survive a few thousand representative Test262 files and produce a repeatable
failure baseline.

The current data says:

1. The pinned V7 integration gate is healthy: 69/69, zero failures, zero skips.
2. The broad V7 scan is now 1,977/3,034, or 65.16%, with 12 explicit
   unsupported skips. B-line work accounts for +206 passing tests versus the
   previous baseline.
3. The largest remaining bucket is syntax/static-negative and literal
   behavior, followed by object-model assertion mismatches.
4. The best near-term conformance gains are likely not in GC/cache, but in
   frontend syntax/static-negative handling and Function/object-model semantics.

## Suggested Follow-Up Order

1. Fix strict-mode string literal early errors and Unicode line/paragraph
   separator behavior. This targets `language/literals` and should unlock many
   syntax/static-negative failures.
2. Improve residual Function descriptor and construct/newTarget edge cases.
   Coordinate with the B-line owners before changing `Function.prototype`,
   bound-function dispatch, or `instanceof`.
3. Continue C-line String/Symbol/Reflect precision work, especially descriptor
   exactness and well-known-symbol protocol interactions.
4. Fill the remaining small compiler gaps for `delete` and non-identifier
   `++` / `--` operands so skipped tests become real pass/fail signals.
5. Treat Proxy, class syntax, cross-realm `$262`, and resizable ArrayBuffer
   helper syntax as future larger milestones unless the scope changes.

## Quality Gates

The following commands pass on the tested Windows environment:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_function_bind
cargo test --no-default-features --test native_v7_runtime
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v7 --jobs 1 --json reports/native-v7-gate-summary.json
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/native-v7-frontend-summary.json
```

The dashboard and broad scan reports are diagnostic. They must not count
failed, skipped, crashed, or timed-out suites as passes.
