# Native V12 Team Plan: 70% Builtins Sprint

## 0. Goal

Native V12 targets the next conformance milestone after Fixbug5.

Current locked baseline:

```text
total = 53379
passed = 32097
failed = 21280
skipped = 2
conformance = 60.13%
```

70% target:

```text
ceil(53379 * 0.70) = 37366
required additional passes = 37366 - 32097 = 5269
```

Recommended safety target:

```text
passed >= 37600
conformance >= 70.44%
```

V12 is therefore not a small stabilization sprint. It must add roughly 5.3k net passing tests while preserving the current 60% milestone.

The remaining failures are concentrated in:

```text
built-ins: 9651
language:  7337
intl402:   3036
```

The primary V12 target is `built-ins`, but the plan must still include parser/harness blockers and language residuals that unlock builtin tests.

---

## 1. Strategy

V12 should not attempt to implement all JavaScript builtins completely. The goal is to maximize pass gain per implementation cost.

Recommended strategy:

```text
1. Preserve the 60.13% baseline.
2. Fix shared descriptor and builtin object shape infrastructure.
3. Stabilize Iterator / Promise / Array.from / TypedArray.from shared protocols.
4. Harvest Object / Function / Array / String / Date / Set / TypedArray failures.
5. Repair RegExp property-escapes and RegExp prototype regressions.
6. Add a low-risk Temporal skeleton after fixing temporalHelpers parser blockers.
7. Continue class / destructuring residuals only where they unlock many tests.
```

Expected gain envelope:

| Track                                                                 |                       Estimated gain |
| --------------------------------------------------------------------- | -----------------------------------: |
| A: Frontend blockers + class/dstr residuals                           |                        +1000 ~ +1800 |
| B: Iterator / Promise / async shared protocol                         |                        +1000 ~ +1800 |
| C: Builtin object model, descriptor, Array, RegExp, Temporal skeleton |                        +2500 ~ +4000 |
| D: Integration / regression protection                                | no direct gain, protects 60%+ result |

Target combined gain:

```text
A: +1200
B: +1200
C: +3000
----------------
total: +5400
```

This is enough to cross the 70% line if regressions are controlled.

---

## 2. Recommended Branches

```text
docs/v12-70-contracts
feat/v12-a-frontend-unlockers
feat/v12-b-iterator-promise-protocol
feat/v12-c-builtin-shape-regexp-temporal
test/v12-70-integration
```

Recommended merge order:

```text
V12 contracts
  -> B shared Iterator / Promise protocol skeleton
  -> C descriptor / builtin installer foundation
  -> A parser and class/dstr unlockers
  -> B Array.from / TypedArray.from / for-of / for-await-of integration
  -> C Array / Object / Function / String / Date / Set sweep
  -> C RegExp focused sweep
  -> C Temporal skeleton after A temporalHelpers blocker
  -> V12 70% integration scan
```

The first merge must be the contract branch. No group should start large feature changes before `docs/version/native-v12-interface.md` is accepted.

---

## 3. Track Overview

| Track | Owner                         | Main target                                                                                   | High-value directories                                                                                                                     |
| ----- | ----------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| A     | Frontend / language unlockers | Parser, AST, class/dstr, dynamic Function source parsing, temporalHelpers blocker             | `language/statements/class`, `language/expressions/class`, `language/*/dstr`, `built-ins/Temporal` harness failures                        |
| B     | Runtime protocol              | Iterator, Promise, async, job queue, `for-of`, `for-await-of`, `Array.from` protocol          | `built-ins/Iterator`, `built-ins/Promise`, `language/statements/for-of`, `language/statements/for-await-of`, `TypedArrayConstructors/from` |
| C     | Builtin harvest               | Descriptor, Object, Function, Array, String, Date, Set, TypedArray, RegExp, Temporal skeleton | `built-ins/Object`, `Function`, `Array`, `String`, `Date`, `Set`, `TypedArray`, `RegExp`, `Temporal`                                       |
| D     | Integration                   | Test262 scan, reports, CI, regression gates, 70% tracking                                     | `src/test262.rs`, `reports/`, `docs/status.md`, `readme.md`, `AGENTS.md`                                                                   |

---

## 4. Shared File Ownership

| File or area                   | Owner      | Rules                                                                               |
| ------------------------------ | ---------- | ----------------------------------------------------------------------------------- |
| `src/lexer/`                   | A          | B/C must request new lexical behavior through interface notes                       |
| `src/parser/`                  | A          | No builtin track should patch parser directly                                       |
| `src/ast/`                     | A          | AST changes require interface update first                                          |
| `src/bytecode/opcode.rs`       | B          | New opcode must document stack effect and error propagation                         |
| `src/bytecode/chunk.rs`        | B          | Any metadata change needs a contract test                                           |
| `src/bytecode/compiler.rs`     | A/B shared | A owns class/dstr lowering; B owns iterator/async/generator lowering                |
| `src/vm/`                      | B          | B owns suspend/resume, async, job queue, iterator execution                         |
| `src/runtime/`                 | B/C shared | Object model helpers, iterator helpers, Promise queue, descriptor helpers live here |
| `src/builtins/`                | C          | Builtins must use shared runtime helpers, not duplicate object/iterator logic       |
| `src/test262.rs`               | D          | V12 scan selector and locked manifest stay stable                                   |
| `tests/`                       | all groups | Each group adds focused tests for its contract                                      |
| `reports/.version-report/v12-part*.md`         | each group | Every change updates the matching report                                            |
| `docs/version/native-v12-interface.md` | all groups | Shared contracts are authoritative                                                  |

---

## 5. A Track: Frontend Unlockers and Language Residuals

### 5.1 Scope

A owns source-to-AST correctness and language-level blockers that currently prevent builtin tests from executing.

Primary goals:

```text
1. Fix temporalHelpers.js parser blocker.
2. Support dynamic Function HTML comment parsing.
3. Continue class/elements and class/dstr residuals.
4. Reduce object/dstr and for-of/dstr failures.
5. Patch small top-level-await / dynamic-import syntax blockers only when low risk.
```

A must not implement builtin algorithms. If a failure is caused by `Temporal.Duration.from` behavior, it belongs to C. If a failure is caused by `temporalHelpers.js` failing to parse before Temporal code runs, it belongs to A.

### 5.2 High-value targets

```text
test/language/statements/class
test/language/expressions/class
test/language/statements/class/dstr
test/language/expressions/class/dstr
test/language/expressions/object/dstr
test/language/statements/for-of/dstr
test/annexB/built-ins/Function/createdynfn-html-*.js
test/annexB/language/comments
Temporal harness include failures caused by parser/lexer issues
```

### 5.3 Tasks

#### A1. temporalHelpers.js parser blocker

Target symptoms:

```text
harness temporalHelpers.js failed
SyntaxError: lex error: unterminated string literal
```

Expected outcome:

```text
temporalHelpers.js can be included without parser panic or lexer failure.
Temporal tests then fail for real Temporal semantics instead of harness parse errors.
```

Non-goal:

```text
Do not implement full Temporal arithmetic in A.
```

#### A2. Dynamic Function HTML comments

Target symptoms:

```text
dynamic Function source could not be compiled: unexpected `<`
dynamic Function source could not be compiled: unexpected `>`
expected identifier but found `--`
```

Expected behavior:

```text
Annex B HTML comment forms are accepted or rejected with correct SyntaxError behavior.
Dynamic Function source parsing uses the same frontend path as normal scripts.
```

#### A3. class/elements and class/dstr residuals

Prioritize:

```text
class fields
static fields
private fields
private methods
private accessors
computed property name ordering
method parameter destructuring
derived constructor and super()
class name binding
```

#### A4. object/dstr and for-of/dstr

A must lower destructuring through shared runtime/iterator helpers.

Do not implement separate destructuring logic for:

```text
let/const binding
assignment pattern
function parameters
class method parameters
for-of
for-await-of
```

All must use one lowering path.

### 5.4 A validation commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class/dstr --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class/dstr --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/built-ins/Function --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/comments --jobs 4 --progress
```

---

## 6. B Track: Iterator, Promise, and Async Protocol

### 6.1 Scope

B owns all execution protocols that connect language syntax and builtin algorithms.

Primary goals:

```text
1. Freeze and implement shared Iterator helpers.
2. Freeze and implement shared Promise job queue helpers.
3. Make Array.from / TypedArray.from / Promise combinators use the same iterator path.
4. Reduce built-ins/Iterator and built-ins/Promise.
5. Reduce for-of, for-await-of, yield*, async-generator residuals.
```

B must not implement every Array, TypedArray, or RegExp builtin. B provides the protocol layer that C uses.

### 6.2 High-value targets

```text
test/built-ins/Iterator
test/built-ins/Promise
test/built-ins/Array/from
test/built-ins/TypedArrayConstructors/from
test/language/statements/for-of
test/language/statements/for-await-of
test/language/expressions/yield
test/language/expressions/async-generator
```

### 6.3 Tasks

#### B1. Iterator protocol

Implement or stabilize:

```text
GetIterator
IteratorNext
IteratorComplete
IteratorValue
IteratorClose
CreateIterResultObject
GetMethod
Call with correct receiver
```

All of the following must share this path:

```text
for-of
for-await-of
yield*
array destructuring
object/array rest where iterator is involved
Array.from
TypedArray.from
Promise.all
Promise.race
Promise.any
Promise.allSettled
```

#### B2. Iterator builtin shape

Coordinate with C for descriptors, but B owns behavior.

Required visible shape:

```text
Iterator
Iterator.prototype
Iterator.prototype.constructor
Iterator.prototype[Symbol.iterator]
Iterator.prototype.map / filter / take / drop if implemented
correct not-a-constructor behavior
correct name / length via C installer
```

If a method is not implemented, it should fail consistently, not panic or produce `undefined is not callable` from accidental missing installation.

#### B3. Promise job queue

Stabilize:

```text
Promise.resolve
Promise.reject
Promise.prototype.then
Promise.prototype.catch
Promise.prototype.finally
thenable assimilation
microtask/job queue ordering
drain_promise_jobs after async Test262 completion
```

All async paths must use one queue:

```text
await
async function return
async generator
Promise builtin callbacks
Test262 async completion
```

#### B4. for-of / for-await-of residuals

Target remaining edge cases:

```text
IteratorClose on break/throw/return
return method exists but is not callable
return method returns non-object
async iterator fallback from sync iterator
abrupt completion during destructuring
yield* return/throw delegation
```

### 6.4 B validation commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/from --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors/from --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress
```

---

## 7. C Track: Builtin Harvest

### 7.1 Scope

C owns JS-visible builtin families and descriptor exactness.

Primary goals:

```text
1. Build one shared builtin installer.
2. Improve Object / Function / Array / String / Date / Set / TypedArray.
3. Repair RegExp property-escapes and prototype behavior.
4. Add low-risk Temporal skeleton after A fixes parser blockers.
5. Avoid full Intl402 and full Temporal in V12.
```

C must not implement a second iterator protocol inside builtins. If a builtin needs iteration, it must call B helpers.

### 7.2 High-value targets

```text
test/built-ins/Object
test/built-ins/Function
test/built-ins/Array
test/built-ins/String
test/built-ins/Date
test/built-ins/Set
test/built-ins/TypedArray
test/built-ins/TypedArrayConstructors
test/built-ins/DataView
test/built-ins/RegExp
test/built-ins/Temporal
```

### 7.3 Tasks

#### C1. Builtin descriptor installer

Create shared installation helpers for:

```text
function object name
function object length
method writable/enumerable/configurable
constructor prototype
prototype constructor
accessor get/set descriptors
not-a-constructor behavior
native function toString marker
```

Default descriptor policy:

```text
builtin method:
  writable: true
  enumerable: false
  configurable: true

prototype.constructor:
  writable: true
  enumerable: false
  configurable: true

length/name:
  writable: false
  enumerable: false
  configurable: true

well-known symbol methods:
  writable: true
  enumerable: false
  configurable: true
```

#### C2. Object / Function foundation

Prioritize:

```text
Object.getOwnPropertyDescriptor
Object.defineProperty
Object.getOwnPropertyNames
Object.keys
Object.values
Object.entries
Reflect.ownKeys
Reflect.get
Reflect.set
Function.prototype.call/apply/bind residuals
Function.prototype.toString
bound function name/length
constructor/newTarget edge cases only if low risk
```

#### C3. Array / TypedArray / DataView

Prioritize:

```text
Array.from through B iterator helper
Array.of
Array.prototype.values / keys / entries
Array.prototype[Symbol.iterator]
Array holes
Array species simplified path
TypedArray.from through B iterator helper
TypedArray constructor range checks
TypedArray.prototype descriptor and methods
DataView get/set and range checks
ArrayBuffer byteLength / byteOffset
```

#### C4. String / Date / Set

Prioritize low-risk builtin harvest:

```text
String.prototype.trimLeft === trimStart
String.prototype.trimRight === trimEnd
String.prototype.match/search/replace/split dispatch to RegExp
Date.prototype.getYear / setYear
Date constructor descriptor
Set constructor/prototype shape
Set.prototype.add/delete/has/clear/forEach
Set iterator methods
```

#### C5. RegExp sweep

Priority:

```text
RegExp property escapes generated cases
RegExp legacy accessors
RegExp.prototype.compile
RegExp.prototype.exec
RegExp.prototype.test
RegExp.prototype[@@split]
named groups / backreferences where feasible
String-RegExp dispatch
RegExp descriptor/name/length
```

RegExp changes must run both focused and whole-RegExp gates because this area worsened in the Fixbug5 analysis.

#### C6. Temporal skeleton

Only after A fixes parser blockers.

Implement skeleton for object shape and simple behavior:

```text
Temporal object
Temporal.Now
Temporal.Duration
Temporal.Instant
Temporal.PlainDate
Temporal.PlainDateTime
Temporal.PlainTime
Temporal.PlainYearMonth
Temporal.PlainMonthDay
Temporal.ZonedDateTime
constructor name / length
prototype
prototype.constructor
not-a-constructor errors
basic from/toString for simple ISO cases if low risk
```

Non-goals:

```text
complete calendar protocol
complete timezone semantics
duration balancing
rounding
ZonedDateTime arithmetic
deep Intl.DateTimeFormat integration
```

### 7.4 C validation commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Function --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Date --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Set --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp/property-escapes --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress
```

---

## 8. D Track: Integration and Reporting

### 8.1 Scope

D ensures the 70% result is reproducible and not a one-off local run.

Tasks:

```text
1. Create V12 scan manifest from Fixbug5 failed cases.
2. Add --native-v12-scan selector.
3. Maintain per-track reports.
4. Run focused gates after each PR.
5. Run full 53k scan after major merges and final integration.
6. Record pass/fail deltas against Fixbug5.
7. Prevent skipped/crashed/timeout cases from being counted as pass.
```

### 8.2 Required files

```text
docs/version/native-v12-team-plan.md
docs/version/native-v12-interface.md
reports/.version-report/v12-partA-report.md
reports/.version-report/v12-partB-report.md
reports/.version-report/v12-partC-report.md
reports/.test262/test262-scan-failure/native-v12-scan-failures.txt
reports/.native-test262-tmp/native-v12-scan-summary.json
reports/.test262/test262-analysis/native-v12-test262-analysis.md
```

### 8.3 V12 scan manifest

Create a 5,000-case scan from Fixbug5 failures.

Suggested composition:

| Bucket                                                       | Cases |
| ------------------------------------------------------------ | ----: |
| Object / Function / descriptor                               |   700 |
| Array / TypedArray / DataView                                |   700 |
| Iterator / Promise / for-of / for-await-of                   |   900 |
| RegExp / String-RegExp                                       |   700 |
| Temporal skeleton and temporalHelpers                        |   900 |
| class/dstr residuals                                         |   700 |
| module / dynamic import / top-level await low-risk residuals |   400 |

Total:

```text
5000
```

### 8.4 Full scan policy

Full 53k scan should run:

```text
1. after B shared protocol integration;
2. after C descriptor + Object/Array integration;
3. after RegExp sweep;
4. after Temporal skeleton;
5. final submission candidate.
```

Full scan command:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

If JSON is supported:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/.native-test262-tmp/native-v12-full-test262-summary.json
```

---

## 9. Common Merge Gates

Every PR must run:

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
```

If feasible:

```powershell
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

Before merge, also run the track-specific focused commands.

After merge to integration branch:

```powershell
cargo run --release --no-default-features -- test262 --native-v12-scan --jobs 4 --json reports/.native-test262-tmp/native-v12-scan-summary.json
```

---

## 10. 70% Acceptance Gate

V12 is considered complete only when:

```text
passed >= 37366
conformance >= 70.00%
skipped are not counted as passed
no runner crash
no panic
no memory allocation failure
no regression below Fixbug5 baseline in high-value gates
```

Recommended safer acceptance:

```text
passed >= 37600
conformance >= 70.44%
```

Required documentation updates:

```text
reports/.version-report/v12-partA-report.md
reports/.version-report/v12-partB-report.md
reports/.version-report/v12-partC-report.md
reports/.test262/test262-analysis/native-v12-test262-analysis.md
docs/status.md
readme.md
AGENTS.md
thoughts/plan_1_version.md
```

---

## 11. Non-goals

V12 should not spend major effort on:

```text
complete Temporal semantics
complete Intl402
Atomics wait/wake host behavior
large VM rewrite
changing regex engine wholesale
replacing Native implementation with Boa behavior
counting skipped/crashed/timeout cases as pass
```

Temporal skeleton is allowed only as an object-shape and simple-constructor pass-gain strategy.

---

## 12. Final Summary

V12 should move from “language feature sprint” to “builtin infrastructure sprint”.

The highest-return plan is:

```text
A: frontend unlockers + class/dstr residuals
B: Iterator / Promise / async shared protocol
C: descriptor + builtin harvest + RegExp + Temporal skeleton
D: scan, reports, and regression protection
```

The most important architectural rule:

```text
Builtins must not duplicate runtime object, descriptor, iterator, or Promise logic.
Everything cross-cutting must go through shared Runtime helpers.
```

This is the only practical way to gain 5,269+ net passes without creating unmergeable branches or breaking the current 60.13% milestone.
