# Native V12 Shared Interface: 70% Builtins Sprint

## 0. Authority

This document freezes the shared contracts for the Native V12 70% sprint.

It supplements earlier Native interface documents. For newly added V12 behavior, this document is authoritative.

Purpose:

```text
1. allow A/B/C/D to work in parallel;
2. prevent duplicate descriptor, iterator, Promise, or object-model helpers;
3. define ownership before touching shared files;
4. keep the current 60.13% result from regressing;
5. make 70% pass count reproducible.
```

---

## 1. Shared File Rules

| File or area               | Owner              | Rule                                                                 |
| -------------------------- | ------------------ | -------------------------------------------------------------------- |
| `src/lexer/`               | A                  | Only A changes lexical behavior                                      |
| `src/parser/`              | A                  | Syntax changes must be documented here first                         |
| `src/ast/`                 | A                  | AST shape changes require interface update                           |
| `src/bytecode/opcode.rs`   | B                  | New opcode must document stack effect                                |
| `src/bytecode/compiler.rs` | A/B                | No whole-function replacement without coordination                   |
| `src/vm/`                  | B                  | VM-level iterator, async, generator, Promise behavior belongs here   |
| `src/runtime/`             | B/C                | Shared object model, descriptor, iterator, Promise helpers live here |
| `src/builtins/`            | C                  | Builtins must call shared runtime helpers                            |
| `src/test262.rs`           | D                  | V12 selector and manifest are integration-owned                      |
| `reports/`                 | D and track owners | Every functional change updates matching report                      |

Rule:

```text
If a change affects two groups, update this document first.
If a change creates a new shared helper, document the helper signature here first.
If a builtin needs iteration, descriptors, object keys, or Promise jobs, it must use the shared helper.
```

---

## 2. Property and Descriptor Interface

V12 must centralize object property behavior.

### 2.1 Shared types

Suggested shape:

```rust
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}

pub struct PropertyDescriptor {
    pub value: Option<JsValue>,
    pub get: Option<JsValue>,
    pub set: Option<JsValue>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

pub struct BuiltinAttrs {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}
```

If existing project names differ, use existing names but preserve the same semantics.

### 2.2 Required runtime helpers

```rust
pub fn define_own_property(
    ctx: &mut NativeContext,
    object: JsValue,
    key: PropertyKey,
    descriptor: PropertyDescriptor,
) -> Result<bool, VmError>;

pub fn get_own_property(
    ctx: &mut NativeContext,
    object: JsValue,
    key: PropertyKey,
) -> Result<Option<PropertyDescriptor>, VmError>;

pub fn get_property(
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<JsValue, VmError>;

pub fn set_property(
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
    value: JsValue,
) -> Result<bool, VmError>;

pub fn has_property(
    ctx: &mut NativeContext,
    object: JsValue,
    key: PropertyKey,
) -> Result<bool, VmError>;

pub fn own_property_keys(
    ctx: &mut NativeContext,
    object: JsValue,
) -> Result<Vec<PropertyKey>, VmError>;

pub fn ordinary_to_property_descriptor(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyDescriptor, VmError>;

pub fn from_property_descriptor(
    ctx: &mut NativeContext,
    descriptor: PropertyDescriptor,
) -> Result<JsValue, VmError>;
```

### 2.3 Object key order

`own_property_keys` must centralize order:

```text
1. array index keys in ascending numeric order;
2. ordinary string keys in insertion order;
3. symbol keys in insertion order.
```

Do not implement separate key-order logic in Object, Array, TypedArray, RegExp, or Temporal builtins.

### 2.4 Descriptor exactness

Builtin descriptor defaults:

```text
method:
  writable: true
  enumerable: false
  configurable: true

constructor property on prototype:
  writable: true
  enumerable: false
  configurable: true

name:
  writable: false
  enumerable: false
  configurable: true

length:
  writable: false
  enumerable: false
  configurable: true

accessor:
  enumerable: false
  configurable: true
```

Any exception must be documented in the builtin file and the corresponding track report.

---

## 3. Builtin Installer Interface

C owns JS-visible builtin installation, but the helper must be shared and stable.

### 3.1 Required installer helpers

```rust
pub fn install_builtin_constructor(
    ctx: &mut NativeContext,
    global: JsValue,
    name: &str,
    length: usize,
    constructor: NativeFunction,
    prototype: JsValue,
    attrs: BuiltinAttrs,
) -> Result<JsValue, VmError>;

pub fn install_builtin_function(
    ctx: &mut NativeContext,
    target: JsValue,
    name: &str,
    length: usize,
    function: NativeFunction,
    attrs: BuiltinAttrs,
) -> Result<JsValue, VmError>;

pub fn install_builtin_method(
    ctx: &mut NativeContext,
    prototype: JsValue,
    name: &str,
    length: usize,
    function: NativeFunction,
) -> Result<(), VmError>;

pub fn install_builtin_accessor(
    ctx: &mut NativeContext,
    target: JsValue,
    name: &str,
    get: Option<NativeFunction>,
    set: Option<NativeFunction>,
    attrs: BuiltinAttrs,
) -> Result<(), VmError>;

pub fn mark_not_constructor(
    ctx: &mut NativeContext,
    function: JsValue,
) -> Result<(), VmError>;
```

### 3.2 Rules

```text
1. No builtin may manually set name/length flags in a one-off way.
2. No builtin may bypass define_own_property for visible properties.
3. If a function is not constructible, mark_not_constructor must be used.
4. Constructor.prototype and prototype.constructor must be installed by helper.
5. Builtin toString behavior must be centralized.
```

### 3.3 First users

The installer must first migrate:

```text
Object
Function
Array
Iterator
Promise
RegExp
String
Date
Set
TypedArray
Temporal skeleton
```

---

## 4. Iterator Interface

B owns the shared iterator protocol.

### 4.1 Shared types

```rust
pub enum IteratorHint {
    Sync,
    Async,
}

pub struct IteratorRecord {
    pub iterator: JsValue,
    pub next_method: JsValue,
    pub done: bool,
}

pub struct IteratorResult {
    pub value: JsValue,
    pub done: bool,
}

pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
}
```

### 4.2 Required helpers

```rust
pub fn get_iterator(
    ctx: &mut NativeContext,
    value: JsValue,
    hint: IteratorHint,
) -> Result<IteratorRecord, VmError>;

pub fn iterator_next(
    ctx: &mut NativeContext,
    record: &mut IteratorRecord,
    value: Option<JsValue>,
) -> Result<IteratorResult, VmError>;

pub fn iterator_complete(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<bool, VmError>;

pub fn iterator_value(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<JsValue, VmError>;

pub fn iterator_close(
    ctx: &mut NativeContext,
    record: &mut IteratorRecord,
    completion: Completion,
) -> Result<Completion, VmError>;

pub fn create_iterator_result_object(
    ctx: &mut NativeContext,
    value: JsValue,
    done: bool,
) -> Result<JsValue, VmError>;
```

### 4.3 Required users

These features must call the shared helpers:

```text
for-of
for-await-of
yield*
array destructuring
rest destructuring
Array.from
TypedArray.from
Promise.all
Promise.race
Promise.any
Promise.allSettled
Set constructor
Map constructor if implemented later
```

### 4.4 Forbidden behavior

```text
Do not create { value, done } objects by hand in builtins.
Do not implement Array.from iteration separately.
Do not implement TypedArray.from iteration separately.
Do not skip IteratorClose on abrupt completion.
```

---

## 5. Promise and Job Queue Interface

B owns the shared job queue and Promise execution protocol. C owns JS-visible Promise builtin installation using B helpers.

### 5.1 Shared types

```rust
pub enum PromiseState {
    Pending,
    Fulfilled(JsValue),
    Rejected(JsValue),
}

pub struct PromiseCapability {
    pub promise: JsValue,
    pub resolve: JsValue,
    pub reject: JsValue,
}

pub type PromiseJob = Box<dyn FnOnce(&mut NativeContext) -> Result<(), VmError>>;
```

### 5.2 Required helpers

```rust
pub fn new_promise_capability(
    ctx: &mut NativeContext,
    constructor: JsValue,
) -> Result<PromiseCapability, VmError>;

pub fn promise_resolve(
    ctx: &mut NativeContext,
    constructor: JsValue,
    value: JsValue,
) -> Result<JsValue, VmError>;

pub fn fulfill_promise(
    ctx: &mut NativeContext,
    promise: JsValue,
    value: JsValue,
) -> Result<(), VmError>;

pub fn reject_promise(
    ctx: &mut NativeContext,
    promise: JsValue,
    reason: JsValue,
) -> Result<(), VmError>;

pub fn enqueue_promise_job(
    ctx: &mut NativeContext,
    job: PromiseJob,
);

pub fn drain_promise_jobs(
    ctx: &mut NativeContext,
) -> Result<(), VmError>;
```

### 5.3 Required users

```text
Promise.resolve
Promise.reject
Promise.prototype.then
Promise.prototype.catch
Promise.prototype.finally
async function return
await
for-await-of
async generator
Test262 async completion
Promise combinators
```

### 5.4 Rules

```text
1. Promise callbacks must run through one queue.
2. Async Test262 completion must drain the same queue.
3. then/catch/finally must preserve callback order.
4. thenable assimilation must be centralized.
5. C may install Promise methods but must not own the job queue.
```

---

## 6. Array and TypedArray Interface

C owns Array and TypedArray builtins. B owns iteration protocol used by them.

### 6.1 Array.from contract

```rust
pub fn array_from_iterable(
    ctx: &mut NativeContext,
    constructor: JsValue,
    items: JsValue,
    mapfn: Option<JsValue>,
    this_arg: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

Rules:

```text
1. Use get_iterator when @@iterator exists.
2. Use array-like path only when iterator path is not used.
3. Call IteratorClose on abrupt completion.
4. Map function must be called with correct thisArg.
5. Constructor species behavior may be partial but must be documented.
```

### 6.2 TypedArray.from contract

```rust
pub fn typed_array_from(
    ctx: &mut NativeContext,
    constructor: JsValue,
    source: JsValue,
    mapfn: Option<JsValue>,
    this_arg: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

Rules:

```text
1. Use B iterator helpers.
2. Preserve typed-array element conversion path.
3. Range errors must be catchable.
4. BigInt typed arrays may be partial but must not masquerade as Number typed arrays.
5. Do not treat TypedArray as ordinary Array internally.
```

### 6.3 ArrayBuffer/DataView contract

```rust
pub fn get_array_buffer_byte_length(
    ctx: &mut NativeContext,
    buffer: JsValue,
) -> Result<usize, VmError>;

pub fn validate_typed_array(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<TypedArrayRecord, VmError>;

pub fn validate_data_view(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<DataViewRecord, VmError>;
```

Rules:

```text
1. Range checks must happen before byte access.
2. Detached buffer behavior may be partial but must be centralized.
3. DataView and TypedArray must share byte storage abstractions.
```

---

## 7. RegExp Interface

A owns RegExp literal parsing/static checks. C owns RegExp runtime and builtins.

### 7.1 Frontend contract

A must expose parsed metadata:

```rust
pub struct RegExpLiteral {
    pub pattern: String,
    pub flags: String,
    pub groups: RegExpGroupMeta,
}

pub struct RegExpGroupMeta {
    pub named_groups: Vec<String>,
    pub backreferences: Vec<String>,
    pub property_escapes: Vec<String>,
}
```

Rules:

```text
1. Invalid literal syntax must become SyntaxError, not panic.
2. Parser must not implement RegExp matching.
3. Duplicate named group policy must be documented when partial.
4. Unicode property escape tokenization must not regress existing accepted patterns.
```

### 7.2 Builtin contract

C must provide:

```rust
pub fn regexp_compile(
    ctx: &mut NativeContext,
    this_value: JsValue,
    pattern: JsValue,
    flags: JsValue,
) -> Result<JsValue, VmError>;

pub fn regexp_exec(
    ctx: &mut NativeContext,
    regexp: JsValue,
    input: JsValue,
) -> Result<JsValue, VmError>;

pub fn regexp_test(
    ctx: &mut NativeContext,
    regexp: JsValue,
    input: JsValue,
) -> Result<JsValue, VmError>;

pub fn regexp_symbol_split(
    ctx: &mut NativeContext,
    regexp: JsValue,
    input: JsValue,
    limit: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

### 7.3 Property escapes policy

Priority property escapes:

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
Emoji if low risk
```

If full Unicode property data is not available, implement a documented subset that converts many generated tests without breaking valid SyntaxError behavior.

### 7.4 Legacy accessors

RegExp legacy accessors must be installed through descriptor helpers:

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

Cross-realm and subclass receiver behavior may be partial but must fail with correct TypeError shape when required.

---

## 8. Temporal Skeleton Interface

C owns Temporal skeleton. A owns parser blockers that prevent Temporal tests from running.

### 8.1 Installation

```rust
pub fn install_temporal_skeleton(
    ctx: &mut NativeContext,
    global: JsValue,
) -> Result<(), VmError>;
```

Required objects:

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

### 8.2 Constructor/prototype shape

Each Temporal constructor must have:

```text
correct name
correct length if known
prototype object
prototype.constructor
not-a-constructor behavior when required
basic descriptor flags
```

### 8.3 Minimal method policy

Allowed low-risk methods:

```text
from for simple ISO strings
toString for stored simple value
valueOf throws TypeError where required
compare only for trivial equal values if safe
```

Forbidden in V12 unless isolated:

```text
complete timezone arithmetic
complete calendar protocol
rounding
duration balancing
Intl DateTimeFormat integration
ZonedDateTime arithmetic
```

### 8.4 Error policy

```text
Unsupported deep Temporal semantics should throw catchable TypeError or RangeError.
Do not panic.
Do not return nonsense values that create false semantic passes.
```

---

## 9. Frontend Harness Unlocker Interface

A owns parser unlockers that allow builtins tests to execute.

### 9.1 temporalHelpers.js blocker

A must add focused parser tests for the failing `temporalHelpers.js` constructs.

Required behavior:

```text
No lexer panic.
No unterminated string literal when the source is valid.
No incorrect recovery that corrupts subsequent tokens.
```

### 9.2 Dynamic Function source parser

Dynamic Function must share parser behavior with normal script code, but source position and error reporting may be simplified.

Required behavior:

```text
Function("...") compiles through Native frontend.
new Function("...") compiles through Native frontend.
HTML comment Annex B source forms are handled consistently.
Syntax errors are catchable SyntaxError values.
```

---

## 10. Error Convention

All groups must preserve catchable JS errors.

Rules:

```text
1. Syntax problems -> SyntaxError.
2. Invalid callable / constructor use -> TypeError.
3. Invalid numeric/date/range input -> RangeError where appropriate.
4. Unsupported feature may use Unsupported only if the runner treats it honestly.
5. No panic for Test262 inputs.
6. No memory allocation failure.
7. No skipped/crashed/timeout result may be counted as pass.
```

When unsure, prefer a correct catchable TypeError over panic or silent success.

---

## 11. Reporting Contract

Every track report must contain:

```text
owner and scope
locked Fixbug5 baseline
focused commands run
before/after pass/fail deltas
newly exposed failures
regressions
cross-group dependencies
next action
```

Required files:

```text
reports/.version-report/v12-partA-report.md
reports/.version-report/v12-partB-report.md
reports/.version-report/v12-partC-report.md
reports/.native-test262-tmp/native-v12-scan-summary.json
reports/.test262/test262-analysis/native-v12-test262-analysis.md
```

If a contributor changes code but does not run tests, the report must say:

```text
Tests not run: <reason>
Risk: <expected affected suites>
```

---

## 12. Integration Contract

D owns V12 scan selector.

Required constants:

```rust
pub const NATIVE_V12_SCAN_TESTS: &[&str] = &[/* 5000 locked failed cases */];
pub const NATIVE_V12_SCAN_TEST_COUNT: usize = 5000;
```

Required CLI:

```text
--native-v12-scan
```

Required command:

```powershell
cargo run --release --no-default-features -- test262 --native-v12-scan --jobs 4 --json reports/.native-test262-tmp/native-v12-scan-summary.json
```

The V12 scan is not the formal conformance number. It is a regression and planning artifact.

Formal conformance remains the full 53,379-case scan.

---

## 13. Interface Change Log

Append every shared-interface change here.

| Date       | Owner | Change                                    | Files affected                             | Reviewer | Tests                                       |
| ---------- | ----- | ----------------------------------------- | ------------------------------------------ | -------- | ------------------------------------------- |
| 2026-06-27 | all   | Establish V12 70% shared interface        | `docs/version/native-v12-interface.md`             | pending  | document-only                               |
| 2026-06-27 | C     | Add builtin installer contract            | `src/runtime/`, `src/builtins/`            | B        | Object/Function focused tests               |
| 2026-06-27 | B     | Add Iterator and Promise shared contracts | `src/runtime/`, `src/vm/`, `src/bytecode/` | A/C      | Iterator/Promise focused tests              |
| 2026-06-27 | A     | Add frontend unlocker contract            | `src/lexer/`, `src/parser/`, `src/ast/`    | B/C      | class/dstr and temporalHelpers parser tests |
| 2026-06-27 | C     | Add Temporal skeleton contract            | `src/builtins/temporal*`                   | A/B      | Temporal focused tests                      |

---

## 14. Final Rule

V12 should maximize shared infrastructure reuse.

```text
One descriptor path.
One iterator path.
One Promise job queue.
One builtin installer.
One object key ordering path.
One Test262 reporting path.
```

Any second implementation of these mechanisms is considered a merge blocker.
