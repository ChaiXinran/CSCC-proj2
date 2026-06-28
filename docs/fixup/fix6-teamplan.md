# Native V12 70% Team Plan

## 0. Goal

Native V12 targets 70% full Test262 conformance.

Locked baseline:

```text
total = 53379
passed = 34640
failed = 18737
skipped = 2
conformance = 64.89%
```

70% target:

```text
ceil(53379 * 0.70) = 37366
remaining pass gap = 2726
```

Recommended safe target:

```text
passed >= 37600
```

V12 should aim for at least 70.4%, not just 70.0%, so that small regressions do not drop the project below the milestone.

---

## 1. Strategy

Fixbug6 proves that the previous class / iterator / Promise strategy worked. However, the remaining failures are now dominated by builtins and cross-cutting object-model semantics.

V12 should therefore shift from a language-feature sprint to a builtin-infrastructure sprint.

Main strategy:

```text
1. Preserve Fixbug6 as the new baseline.
2. Finish shared descriptor and builtin installation infrastructure.
3. Continue Iterator / Promise shared protocol work.
4. Harvest Array / Object / Function / String / Date / Set / TypedArray.
5. Run a focused RegExp sweep.
6. Continue Temporal skeleton but avoid full Temporal semantics.
7. Keep class/dstr residuals as a supporting track.
```

---

## 2. Track Overview

| Track | Name                                | Main responsibility                                                                           | Estimated gain |
| ----- | ----------------------------------- | --------------------------------------------------------------------------------------------- | -------------: |
| A     | Frontend and language unlockers     | parser, static semantics, class/dstr, Annex B function binding, Temporal harness blockers     |   +600 ~ +1000 |
| B     | Iterator / Promise / async protocol | IteratorClose, Promise queue, Array.from protocol, for-of/for-await-of, module async          |   +700 ~ +1200 |
| C     | Builtin harvest                     | descriptor, Object, Function, Array, String, Date, Set, RegExp, TypedArray, Temporal skeleton |  +1800 ~ +3000 |
| D     | Integration and reporting           | V12 scan, reports, regression gates, full Test262 tracking                                    | protects gains |

C is the main 70% pass-gain track. B provides the shared protocol C must use. A removes parser and language blockers that prevent C/B tests from executing.

---

## 3. Branch Plan

Recommended branches:

```text
docs/v12-70-contracts
feat/v12-a-frontend-language-unlockers
feat/v12-b-iterator-promise-module-protocol
feat/v12-c-builtin-harvest-regexp-temporal
test/v12-70-integration
```

Recommended merge order:

```text
docs/v12-70-contracts
  -> B minimal iterator/promise shared helpers
  -> C descriptor/builtin installer foundation
  -> A parser and class/dstr unlockers
  -> C Array/Object/Function/String/Date/Set harvest
  -> B Array.from/TypedArray.from/Promise combinator integration
  -> C RegExp focused sweep
  -> C Temporal skeleton continuation
  -> D V12 integration scan
  -> full Test262 70% report
```

Shared contracts must be merged before large feature branches.

---

## 4. File Ownership

| File or directory              | Owner        | Rule                                                              |
| ------------------------------ | ------------ | ----------------------------------------------------------------- |
| `src/lexer/`                   | A            | B/C must not directly change tokenization                         |
| `src/parser/`                  | A            | Parser changes require interface notes                            |
| `src/ast/`                     | A            | AST shape changes require interface update                        |
| `src/bytecode/opcode.rs`       | B            | New opcode must document stack effect                             |
| `src/bytecode/compiler.rs`     | A/B          | A owns class/dstr lowering; B owns iterator/async/module lowering |
| `src/vm/`                      | B            | B owns VM call path, job queue, iterator execution                |
| `src/runtime/`                 | B/C shared   | Object model, descriptor, iterator, Promise helpers live here     |
| `src/builtins/`                | C            | Builtins must use shared runtime helpers                          |
| `src/test262.rs`               | D            | V12 scan selector and manifest                                    |
| `reports/.version-report/v12-part*.md`         | track owners | Every code change updates the matching report                     |
| `docs/version/native-v12-interface.md` | all          | Interface authority                                               |

---

## 5. A Track: Frontend and Language Unlockers

### Scope

A owns syntax, AST, static semantics, class/destructuring lowering, and Annex B language behavior.

A should not implement builtin algorithms.

### Primary targets

```text
language/statements/class
language/expressions/class
language/expressions/object
language/statements/for-of
language/statements/for-await-of
language/eval-code
annexB/language/eval-code
annexB/language/function-code
language/module-code
dynamic-import syntax
top-level-await syntax
Temporal harness parser blockers
```

### Main tasks

#### A1. Annex B block-level function semantics

Many failures show:

```text
ReferenceError: f is not defined
value is not updated following evaluation
duplicate lexical declaration
```

These are concentrated in:

```text
annexB/language/eval-code
annexB/language/function-code
language/global-code
```

A should implement a limited Annex B block-level function declaration strategy for sloppy mode:

```text
1. Recognize function declarations inside blocks in sloppy mode.
2. Create or update corresponding var binding when Annex B requires it.
3. Avoid duplicate lexical declaration false positives.
4. Preserve strict-mode early errors.
5. Ensure eval/global/function-code paths share one implementation.
```

#### A2. class/elements and class/dstr residuals

Remaining class directories are still large:

```text
language/statements/class
language/expressions/class
```

Focus:

```text
class fields
private fields
private methods
private accessors
computed property name order
static elements
derived constructor / super()
method parameter destructuring
```

#### A3. dynamic import / top-level await parser blockers

A does not need to implement full module loading. It should first make parser behavior correct:

```text
dynamic import expression parse shape
top-level await syntax recognition
source phase import syntax rejection or skeleton acceptance
correct SyntaxError constructor
```

#### A4. Temporal harness and generated helper blockers

If Temporal tests fail before reaching Temporal builtins because of parser/lexer/harness errors, A owns the fix.

### A validation commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/eval-code --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/function-code --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress
```

---

## 6. B Track: Iterator / Promise / Async / Module Protocol

### Scope

B owns execution protocols used by both language and builtins.

B does not own Array algorithm details, RegExp matching, or Temporal semantics.

### Primary targets

```text
built-ins/Iterator
built-ins/Promise
built-ins/Array/from
built-ins/TypedArrayConstructors/from
language/statements/for-of
language/statements/for-await-of
language/expressions/yield
language/expressions/async-generator
language/module-code/top-level-await
```

### Main tasks

#### B1. IteratorClose correctness

Focus failures:

```text
return method exists but is undefined
return method exists but not callable
return method returns non-object
abrupt completion inside mapping
abrupt completion inside destructuring
yield* return/throw delegation
```

All iterator users must call one shared helper.

#### B2. Promise job queue

Focus:

```text
Promise.resolve
Promise.reject
Promise.prototype.then
Promise.prototype.catch
Promise.prototype.finally
thenable assimilation
microtask ordering
asyncTest completion
```

All async paths must drain the same queue:

```text
await
async function
async generator
Promise combinators
Test262 async completion
```

#### B3. Array.from / TypedArray.from protocol bridge

B owns the iteration protocol. C owns actual Array/TypedArray construction.

B should expose a shared sequence collection helper:

```text
collect_from_iterable
IteratorClose on abrupt completion
mapfn call order
thisArg handling
```

#### B4. Module async skeleton

B should not implement a full module loader unless already available. It should first handle:

```text
top-level await completion state
module async job scheduling
correct parse/execution error shape
dynamic import Promise result skeleton
```

### B validation commands

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

### Scope

C owns builtin families and JS-visible property shape.

C must not duplicate iterator, Promise, or descriptor logic.

### Primary targets

```text
built-ins/Object
built-ins/Function
built-ins/Array
built-ins/String
built-ins/Date
built-ins/Set
built-ins/Map
built-ins/TypedArray
built-ins/TypedArrayConstructors
built-ins/DataView
built-ins/RegExp
built-ins/Temporal
```

### Main tasks

#### C1. Builtin installer and descriptor foundation

Unify installation for:

```text
name
length
prototype
constructor
writable
enumerable
configurable
accessor get/set
not-a-constructor behavior
native function toString
```

This should be the first C task because it affects almost every builtin family.

#### C2. Object / Function / Array harvest

Focus:

```text
Object.getOwnPropertyDescriptor
Object.defineProperty
Object.getOwnPropertyNames
Object.keys / values / entries
Reflect.ownKeys / get / set if available
Function.prototype.name/length/toString
Array.from
Array.of
Array.prototype.values/keys/entries
Array length defineProperty behavior
Array holes
Array species simplified path
```

#### C3. String / Date / Set / Map low-risk sweep

Focus:

```text
String.prototype.match/search/replace/split dispatch
String.prototype.trimLeft / trimStart alias
String.prototype.trimRight / trimEnd alias
Date.prototype.getYear / setYear
Date constructor and prototype descriptor
Set constructor/prototype shape
Set.prototype.add/delete/has/clear/forEach
Set iterator
Map constructor/prototype shape if low risk
```

#### C4. RegExp focused sweep

RegExp remains a high-value clustered area.

Focus:

```text
Unicode property escapes
RegExp legacy accessors
RegExp.prototype.compile
RegExp.prototype.exec/test
RegExp.prototype[@@split]
named groups
backreferences
String-RegExp dispatch
RegExp descriptor exactness
```

#### C5. Temporal skeleton continuation

Do not implement full Temporal.

Implement or refine:

```text
Temporal global object
Temporal.Now
Temporal.Duration
Temporal.Instant
Temporal.PlainDate
Temporal.PlainDateTime
Temporal.PlainTime
Temporal.PlainYearMonth
Temporal.PlainMonthDay
Temporal.ZonedDateTime
constructor name/length
prototype.constructor
valueOf throws TypeError
simple from/toString for simple ISO cases
```

Non-goals:

```text
complete timezone arithmetic
complete calendar protocol
duration balancing
rounding
full Intl integration
```

### C validation commands

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

### Scope

D owns reproducibility, reports, and Test262 gates.

### Required files

```text
docs/version/native-v12-team-plan.md
docs/version/native-v12-interface.md
reports/.version-report/v12-partA-report.md
reports/.version-report/v12-partB-report.md
reports/.version-report/v12-partC-report.md
reports/.test262/test262-scan-failure/native-v12-scan-failures.txt
reports/.native-test262-tmp/native-v12-scan-summary.json
reports/.test262/test262-analysis/fixbug6-test262-analysis.md
docs/status.md
```

### V12 scan manifest

Create a 5,000-case manifest from fixbug6 failures.

Suggested composition:

| Bucket                                     | Cases |
| ------------------------------------------ | ----: |
| Temporal / Intl Temporal skeleton          |   900 |
| Object / Function / descriptor             |   700 |
| Array / TypedArray / DataView              |   700 |
| Iterator / Promise / for-of / for-await-of |   800 |
| RegExp / String-RegExp                     |   700 |
| class / dstr / Annex B function semantics  |   700 |
| module / dynamic import / top-level await  |   500 |

Total:

```text
5000
```

### D validation commands

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --native-v12-scan --jobs 4 --json reports/.native-test262-tmp/native-v12-scan-summary.json
```

Full scan:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

---

## 9. Merge Gates

Each PR must run:

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
```

If feasible:

```powershell
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

Every PR must update the matching report:

```text
A changes -> reports/.version-report/v12-partA-report.md
B changes -> reports/.version-report/v12-partB-report.md
C changes -> reports/.version-report/v12-partC-report.md
D changes -> reports/.native-test262-tmp/native-v12-scan-summary.json or integration report
```

---

## 10. V12 Acceptance

V12 is complete only when:

```text
passed >= 37366
conformance >= 70.00%
skipped are not counted as passed
no panic
no runner crash
no memory allocation failure
no regression below fixbug6 baseline without explanation
```

Recommended acceptance:

```text
passed >= 37600
conformance >= 70.44%
```

---

## 11. Non-goals

Do not spend major V12 time on:

```text
complete Temporal semantics
complete Intl402
Atomics wait/wake host behavior
full ShadowRealm
large VM rewrite
switching regex engine wholesale
using Boa as fallback
counting skipped/crashed/timeout tests as pass
```

---

## 12. Summary

The fastest route from 64.89% to 70% is:

```text
C: builtin descriptor + Array/Object/RegExp/Temporal skeleton
B: iterator/promise/async protocol
A: class/dstr + Annex B function semantics + parser unlockers
D: V12 scan and regression gates
```

V12 should be treated as a builtin infrastructure sprint, not a random builtin implementation sprint.
