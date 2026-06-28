# Fixup8 Team Plan

## 0. Goal

Fixup8 is the 3-person sprint after the fixRTLE full Test262 run.

Locked baseline:

```text
total = 53379
passed = 35472
failed = 17905
skipped = 2
conformance = 66.45%
```

70% target:

```text
ceil(53379 * 0.70) = 37366
remaining pass gap = 37366 - 35472 = 1894
```

Recommended safety target:

```text
passed >= 37600
remaining safe gap = 37600 - 35472 = 2128
```

Fixup8 should not attempt a large VM rewrite or complete Temporal/Intl implementation. The goal is to gain roughly 2,000 reliable passes by harvesting already-visible clusters:

```text
Temporal skeleton / prototype shape
RegExp + String-RegExp dispatch
Array / Object / Function descriptor precision
class / destructuring residuals
Annex B function binding
dynamic import / module / Promise / Iterator residuals
```

---

## 1. New 3-person strategy

The old split:

```text
A = frontend
B = compiler/runtime
C = builtins
```

is no longer appropriate because most remaining high-value failures cross layers.

Fixup8 uses functional ownership instead:

| Person | Track                                | Main target                                                                               | Main conflict surface                                                                                                                                                                                                   |
| ------ | ------------------------------------ | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1     | Builtin Core + Temporal + Descriptor | Temporal skeleton, Array/Object/Function descriptor, builtin installer                    | `src/builtins/date_intl.rs`, `src/builtins/array.rs`, `src/builtins/object.rs`, `src/builtins/function.rs`, `src/runtime/property.rs`, selected `src/runtime/context.rs` helpers                                        |
| P2     | RegExp + String Dispatch             | RegExp property escapes, legacy accessors, String match/search/replace/split dispatch     | `src/builtins/regexp.rs`, `src/builtins/string.rs`, `src/builtins/std_primitives.rs`, RegExp parser/runtime helpers                                                                                                     |
| P3     | Language + Module + Async Protocol   | class/dstr, Annex B function binding, dynamic import, module, Promise, Iterator residuals | `src/parser/`, `src/ast/`, `src/bytecode/compiler.rs`, `src/runtime/environment.rs`, `src/runtime/module.rs`, `src/runtime/job.rs`, `src/runtime/iterator.rs`, `src/builtins/promise.rs`, `src/builtins/collections.rs` |

This split allows each person to cross layers inside their feature cluster, but avoids three people editing the same files at the same time.

---

## 2. Expected gain

Conservative gain target:

| Track                                    | Expected gain |
| ---------------------------------------- | ------------: |
| P1: Builtin Core + Temporal + Descriptor |  +800 ~ +1400 |
| P2: RegExp + String Dispatch             |   +300 ~ +600 |
| P3: Language + Module + Async Protocol   |  +600 ~ +1000 |
| Total                                    | +1700 ~ +3000 |

Fixup8 should aim for:

```text
passed >= 37600
```

not just barely `passed >= 37366`.

---

## 3. Branches

Recommended branches:

```text
docs/fixup8-contracts
fixup8-p1-builtin-core-temporal
fixup8-p2-regexp-string
fixup8-p3-language-module-async
test/fixup8-integration
```

Merge order:

```text
docs/fixup8-contracts
  -> P1 minimal shared builtin installer / descriptor helper contract
  -> P3 minimal Iterator / Promise / module helper contract
  -> P2 RegExp focused fixes
  -> P1 Temporal skeleton + Array/Object/Function descriptor sweep
  -> P3 class/dstr + Annex B + dynamic import/module fixes
  -> test/fixup8-integration
  -> full Test262 scan
```

Why P1 and P3 contract work must merge first:

```text
Temporal, RegExp, Array, Promise, Iterator, class destructuring all need shared helpers.
If every track writes its own descriptor / iterator / call / environment path, later merge will be painful and regressions will be hard to isolate.
```

---

## 4. Shared baseline and acceptance

Fixup8 starts from fixRTLE:

```text
passed >= 35472
failed <= 17905
conformance >= 66.45%
```

70% acceptance:

```text
passed >= 37366
conformance >= 70.00%
```

Recommended acceptance:

```text
passed >= 37600
conformance >= 70.44%
```

Do not count skipped, crashed, timeout, or unsupported cases as passed.

---

## 5. P1 Track: Builtin Core + Temporal + Descriptor

### 5.1 Scope

P1 owns the JS-visible builtin object shape and the Temporal skeleton.

This track is responsible for making builtins look correct to Test262:

```text
constructor exists
prototype exists
prototype.constructor is correct
name / length are correct
property descriptor flags are correct
method exists with correct callability
not-a-constructor behavior is correct
basic TypeError / RangeError behavior is catchable
```

P1 should not implement complete Temporal semantics.

### 5.2 Main test targets

```text
test/built-ins/Temporal
test/built-ins/Temporal/ZonedDateTime/prototype
test/built-ins/Temporal/PlainDateTime/prototype
test/built-ins/Temporal/PlainDate/prototype
test/built-ins/Temporal/Duration/prototype
test/built-ins/Temporal/Instant/prototype
test/built-ins/Array
test/built-ins/Array/prototype
test/built-ins/Object
test/built-ins/Function
test/built-ins/Reflect
test/built-ins/Date
test/built-ins/Set
```

### 5.3 Primary files

P1 may modify:

```text
src/builtins/date_intl.rs
src/builtins/array.rs
src/builtins/object.rs
src/builtins/function.rs
src/builtins/std_primitives.rs
src/builtins/collections.rs
src/builtins/mod.rs
src/runtime/property.rs
src/runtime/object.rs
src/runtime/context.rs
tests/native_array.rs
tests/native_object.rs
tests/native_temporal.rs
reports/fixup8-p1-report.md
```

P1 should avoid:

```text
src/builtins/regexp.rs
src/parser/
src/ast/
src/bytecode/compiler.rs
src/runtime/module.rs
src/runtime/job.rs
src/runtime/iterator.rs
```

If P1 needs one of those files, write the requested interface change in `docs/fixup/fix8-interface.md` first and ask the owner to land the contract.

### 5.4 Required work

#### P1-A. Shared builtin installer

Unify repeated builtin installation patterns.

Targets:

```text
name
length
prototype
prototype.constructor
writable / enumerable / configurable flags
accessor descriptors
not-a-constructor marking
native function toString marker if available
```

The installer should serve:

```text
Object
Function
Array
String
Date
Set
Map
Iterator
Promise
RegExp
Temporal
TypedArray
DataView
```

P1 owns the implementation; other tracks call the interface.

#### P1-B. Temporal skeleton

Target objects:

```text
Temporal
Temporal.Now
Temporal.Duration
Temporal.Instant
Temporal.PlainDate
Temporal.PlainDateTime
Temporal.PlainTime
Temporal.PlainYearMonth
Temporal.PlainMonthDay
Temporal.ZonedDateTime
```

Minimum behavior:

```text
constructor name / length
prototype object
prototype.constructor
correct descriptor flags
valueOf throws TypeError where required
toString for simple stored values
from for simple ISO strings when low-risk
not-a-constructor behavior where required
catchable TypeError / RangeError for unsupported deep cases
```

Non-goals:

```text
complete calendar protocol
complete timezone arithmetic
duration balancing
rounding
full Intl integration
complete ZonedDateTime arithmetic
```

#### P1-C. Array / Object / Function descriptor sweep

Targets:

```text
Array.prototype methods
Array length descriptor
Array holes
Array.from / Array.of descriptor and callability
Object.defineProperty
Object.getOwnPropertyDescriptor
Object.getOwnPropertyNames
Object.keys / values / entries
Reflect.ownKeys
Function.prototype.name / length / toString
Function.prototype.call / apply / bind residuals
```

P1 should use P3 iterator helpers for `Array.from`.

### 5.5 P1 commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/ZonedDateTime/prototype --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainDateTime/prototype --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Function --jobs 4 --progress
```

### 5.6 P1 success target

```text
built-ins/Temporal: meaningful reduction, especially prototype directories
built-ins/Array: 722 -> below 500
built-ins/Object/Function: no regression; descriptor-related failures reduced
estimated gain: +800 ~ +1400
```

---

## 6. P2 Track: RegExp + String Dispatch

### 6.1 Scope

P2 owns RegExp parsing/runtime behavior and the String methods that dispatch to RegExp or custom matchers.

This track is isolated enough to run in parallel with P1 and P3.

### 6.2 Main test targets

```text
test/built-ins/RegExp
test/built-ins/RegExp/property-escapes
test/built-ins/RegExp/prototype
test/annexB/built-ins/RegExp
test/built-ins/String/prototype/match
test/built-ins/String/prototype/matchAll
test/built-ins/String/prototype/replace
test/built-ins/String/prototype/replaceAll
test/built-ins/String/prototype/search
test/built-ins/String/prototype/split
test/annexB/built-ins/String/prototype/match
test/annexB/built-ins/String/prototype/replace
test/annexB/built-ins/String/prototype/search
test/annexB/built-ins/String/prototype/split
```

### 6.3 Primary files

P2 may modify:

```text
src/builtins/regexp.rs
src/builtins/string.rs
src/builtins/std_primitives.rs
src/builtins/annex_b.rs
src/parser/regexp.rs
src/lexer/regexp.rs
src/runtime/context.rs only for isolated RegExp helpers
tests/native_regexp.rs
tests/native_string.rs
reports/.test262/test262-analysis/fixup8-p2-report.md
```

P2 should avoid:

```text
src/builtins/array.rs
src/builtins/date_intl.rs
src/builtins/object.rs
src/builtins/function.rs
src/runtime/job.rs
src/runtime/module.rs
src/runtime/environment.rs
src/bytecode/compiler.rs
```

### 6.4 Required work

#### P2-A. RegExp property escapes

Continue the successful property escape work.

Targets:

```text
General_Category
Script
Script_Extensions
ASCII
Any
Assigned
ID_Start
ID_Continue
White_Space
Emoji if low-risk
remaining generated property-escapes cases
```

Policy:

```text
accept valid patterns
throw SyntaxError for truly invalid patterns
do not panic
do not over-accept invalid cases when the test expects SyntaxError
```

#### P2-B. RegExp prototype and compile

Targets:

```text
RegExp.prototype.compile
RegExp.prototype.exec
RegExp.prototype.test
RegExp.prototype[@@split]
named groups
backreferences where feasible
duplicate named group edge cases
lastIndex handling
```

#### P2-C. Annex B RegExp legacy accessors

Targets:

```text
RegExp.input
RegExp.$_
RegExp.lastMatch
RegExp.$&
RegExp.lastParen
RegExp.$+
RegExp.leftContext
RegExp.$`
RegExp.rightContext
RegExp.$'
RegExp.$1 ... RegExp.$9
```

Important receiver behavior:

```text
cross-realm receiver
subclass constructor receiver
non-RegExp constructor receiver
property descriptor get/set shape
```

#### P2-D. String-RegExp dispatch

Targets:

```text
String.prototype.match
String.prototype.matchAll
String.prototype.replace
String.prototype.replaceAll
String.prototype.search
String.prototype.split
custom matcher / replacer / searcher / splitter
emulates-undefined Annex B behavior
```

P2 should use P1 builtin descriptor helpers and P3 call helpers. Do not create a second call path.

### 6.5 P2 commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp/property-escapes --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp/prototype --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/built-ins/RegExp --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String/prototype/match --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String/prototype/split --jobs 4 --progress
```

### 6.6 P2 success target

```text
built-ins/RegExp: 610 -> below 350
built-ins/RegExp/property-escapes: 213 -> below 100
String-RegExp dispatch failures reduced without breaking String basics
estimated gain: +300 ~ +600
```

---

## 7. P3 Track: Language + Module + Async Protocol

### 7.1 Scope

P3 owns the remaining language/runtime protocol work:

```text
class / destructuring
Annex B block-level function binding
eval/global/function environment binding
dynamic import
module runtime
Promise job queue
Iterator residuals
for-of / for-await-of / yield*
```

This is the widest track, but it avoids most builtin algorithm files.

### 7.2 Main test targets

```text
test/language/statements/class
test/language/expressions/class
test/language/statements/class/dstr
test/language/expressions/class/dstr
test/language/expressions/object/dstr
test/language/statements/for-of/dstr
test/annexB/language/eval-code
test/annexB/language/function-code
test/annexB/language/global-code
test/language/expressions/dynamic-import
test/language/module-code
test/language/module-code/top-level-await
test/built-ins/Promise
test/built-ins/Iterator
test/language/statements/for-of
test/language/statements/for-await-of
test/language/expressions/yield
test/language/expressions/async-generator
```

### 7.3 Primary files

P3 may modify:

```text
src/lexer/
src/parser/
src/ast/
src/bytecode/opcode.rs
src/bytecode/compiler.rs
src/vm/
src/runtime/environment.rs
src/runtime/module.rs
src/runtime/job.rs
src/runtime/iterator.rs
src/runtime/context.rs for environment/job/iterator/module helpers
src/builtins/promise.rs
src/builtins/collections.rs
tests/native_class.rs
tests/native_destructuring.rs
tests/native_promise.rs
tests/native_iterator.rs
tests/native_module.rs
reports/fixup8-p3-report.md
```

P3 should avoid:

```text
src/builtins/date_intl.rs
src/builtins/regexp.rs
src/builtins/string.rs except through agreed call helper
src/builtins/array.rs except wiring to shared iterator helper
src/builtins/object.rs
src/builtins/function.rs
```

### 7.4 Required work

#### P3-A. Class and destructuring residuals

Targets:

```text
class fields
private fields
private methods
private accessors
static fields
static blocks
computed property name order
derived constructor and super()
class method parameter destructuring
object destructuring
array destructuring
rest pattern
for-of destructuring
async-generator destructuring
```

Rule:

```text
There must be one destructuring lowering path.
Do not duplicate destructuring logic separately for let/const, assignment, parameters, class methods, for-of, and async paths.
```

#### P3-B. Annex B function binding

Targets:

```text
sloppy-mode block-level function declarations
direct eval
indirect eval
global code
function code
switch/if/block function declaration cases
skip-early-error behavior
duplicate lexical declaration behavior
```

Typical remaining symptoms:

```text
ReferenceError: f is not defined
value is not updated following evaluation
duplicate binding
expected SyntaxError but no exception
unexpected SyntaxError when Annex B should allow sloppy-mode form
```

#### P3-C. Dynamic import and module residuals

Targets:

```text
dynamic import parse shape
dynamic import Promise skeleton
import.meta
top-level await
module evaluation state
source-phase import syntax behavior
unsupported module loading rejects or throws consistently
```

Policy:

```text
Do not fake successful module loading if the module is not evaluated.
Unsupported dynamic import should reject or throw a catchable error, not panic.
Top-level await must use the shared Promise job queue.
```

#### P3-D. Promise and Iterator residuals

Targets:

```text
Promise.resolve
Promise.reject
Promise.prototype.then
Promise.prototype.catch
Promise.prototype.finally
Promise combinators
thenable assimilation
microtask ordering
IteratorClose
yield*
for-of / for-await-of residuals
Iterator.prototype remaining methods
```

P3 owns the shared helper used by P1 Array/TypedArray and P2 String custom matcher dispatch where needed.

### 7.5 P3 commands

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/eval-code --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/function-code --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/dynamic-import --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress
```

### 7.6 P3 success target

```text
class + dstr failures reduced by 400+
Annex B eval/function/global-code failures reduced by 150+
dynamic import regression contained or reduced
Promise/Iterator residual failures reduced by 100+
estimated gain: +600 ~ +1000
```

---

## 8. Cross-track ownership

### 8.1 Shared helper ownership

| Helper family                         | Owner | Users                                                                        |
| ------------------------------------- | ----- | ---------------------------------------------------------------------------- |
| Builtin installer / descriptor helper | P1    | P1, P2, P3                                                                   |
| Iterator helper                       | P3    | P1 Array/TypedArray, P2 String/RegExp when needed, P3                        |
| Promise job queue                     | P3    | P3 dynamic import / top-level await / Promise, P1 Array.fromAsync if touched |
| call / construct helper               | P3    | P1 Array methods, P2 String custom dispatch, P3 Promise callbacks            |
| Environment binding helper            | P3    | P3 Annex B, eval, class name binding, module binding                         |
| RegExp runtime helper                 | P2    | P2 String-RegExp dispatch                                                    |
| Temporal value helper                 | P1    | P1 Temporal only                                                             |

### 8.2 Shared files requiring coordination

Before modifying these files, update `docs/fixup/fix8-interface.md` or notify the owner:

```text
src/runtime/context.rs
src/runtime/property.rs
src/runtime/object.rs
src/runtime/iterator.rs
src/runtime/job.rs
src/runtime/environment.rs
src/bytecode/compiler.rs
src/builtins/mod.rs
src/test262.rs
src/main.rs
```

### 8.3 Forbidden duplicate implementations

```text
No second descriptor path.
No second builtin installer.
No second iterator path.
No second Promise job queue.
No second call/construct path.
No second Annex B binding path.
No second RegExp dispatch path.
```

---

## 9. Fixup8 scan

Create a 5,000-case scan from current fixRTLE failures:

```text
reports/.test262/test262-scan-failure/fixup8-scan-failures.txt
reports/.native-test262-tmp/fixup8-scan-summary.json
```

Suggested composition:

| Bucket                                       | Cases |
| -------------------------------------------- | ----: |
| Temporal / Intl Temporal skeleton            |  1000 |
| Array / Object / Function descriptor         |   700 |
| RegExp / String-RegExp dispatch              |   700 |
| class / dstr                                 |   700 |
| Annex B binding / eval / function-code       |   600 |
| dynamic import / module / top-level await    |   600 |
| Promise / Iterator / for-of / for-await-of   |   500 |
| TypedArray / DataView / Date / Set residuals |   200 |
| Total                                        |  5000 |

Required command after selector is implemented:

```powershell
cargo run --release --no-default-features -- test262 --fixup8-scan --jobs 4 --json reports/.native-test262-tmp/fixup8-scan-summary.json
```

If the project prefers the previous naming convention, use:

```powershell
cargo run --release --no-default-features -- test262 --native-fixup8-scan --jobs 4 --json reports/.native-test262-tmp/fixup8-scan-summary.json
```

Pick one name and keep it consistent in `src/test262.rs`, `src/main.rs`, `readme.md`, `AGENTS.md`, and reports.

---

## 10. Reports

Each person must maintain one report:

```text
reports/fixup8-p1-report.md
reports/.test262/test262-analysis/fixup8-p2-report.md
reports/fixup8-p3-report.md
```

Each report must contain:

```text
owner and scope
locked fixRTLE baseline
focused commands run
before/after deltas
newly exposed failures
regressions
cross-track dependencies
next action
```

If tests were not run, write:

```text
Tests not run: <reason>
Risk: <expected affected suites>
```

---

## 11. Merge gates

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

Before merging to integration branch, run the relevant focused Test262 suites.

After merging to integration branch, run:

```powershell
cargo run --release --no-default-features -- test262 --fixup8-scan --jobs 4 --json reports/.native-test262-tmp/fixup8-scan-summary.json
```

At daily end or after major merges, run full scan:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

---

## 12. Suggested implementation order

### Phase 1: Contracts and low-conflict setup

```text
1. Add docs/fixup8-teamplan.md
2. Add docs/fixup/fix8-interface.md
3. Add reports/fixup8-p1-report.md
4. Add reports/.test262/test262-analysis/fixup8-p2-report.md
5. Add reports/fixup8-p3-report.md
6. Generate reports/.test262/test262-scan-failure/fixup8-scan-failures.txt
7. Add fixup8 scan selector
```

### Phase 2: Shared infrastructure

```text
P1 lands builtin installer / descriptor helper
P3 lands iterator / Promise / call helper contract
P2 lands focused RegExp regression tests and no broad runtime changes
```

### Phase 3: Feature harvest

```text
P1: Temporal skeleton + Array/Object/Function descriptors
P2: RegExp property escapes + String dispatch + legacy accessors
P3: class/dstr + Annex B binding + dynamic import/module residuals
```

### Phase 4: Integration

```text
Run focused gates
Run fixup8 scan
Run full Test262
Compare against fixRTLE baseline
Update docs/status.md and final report
```

---

## 13. Non-goals

Do not spend Fixup8 time on:

```text
complete Temporal implementation
complete Intl402 implementation
complete Atomics wait/wake host behavior
full ShadowRealm
large VM rewrite
switching regex engine wholesale
using Boa as fallback
counting skipped/crashed/timeout as pass
```

---

## 14. Final rule

Fixup8 should be a 3-person functional sprint:

```text
P1 = Builtin Core + Temporal + Descriptor
P2 = RegExp + String Dispatch
P3 = Language + Module + Async Protocol
```

Each person may cross layers inside their functional cluster, but shared mechanisms must remain single-owner.

The goal is not to make every area perfect. The goal is to gain roughly 2,000 reliable passes with minimal merge conflict and minimal regression risk.
