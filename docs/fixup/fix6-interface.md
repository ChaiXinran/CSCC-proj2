# Native V12 Shared Interface

## 0. Authority

This document freezes shared interfaces for the Native V12 70% sprint.

The goal is to prevent duplicated implementations of:

```text
descriptor handling
builtin installation
iterator protocol
Promise job queue
object key ordering
Array.from / TypedArray.from iteration
RegExp literal/runtime split
Temporal skeleton shape
```

If implementation conflicts with this document, update this document first, then modify code.

---

## 1. Shared Ownership Rules

| File or area               | Owner | Rule                                                            |
| -------------------------- | ----- | --------------------------------------------------------------- |
| `src/lexer/`               | A     | Only A changes tokenization                                     |
| `src/parser/`              | A     | Syntax changes must be documented                               |
| `src/ast/`                 | A     | AST shape changes require interface update                      |
| `src/bytecode/opcode.rs`   | B     | New opcode requires stack-effect documentation                  |
| `src/bytecode/compiler.rs` | A/B   | A owns language lowering; B owns iterator/async/module lowering |
| `src/vm/`                  | B     | VM call path, async, job queue, iterator execution              |
| `src/runtime/`             | B/C   | Object model, descriptor, Promise, iterator helpers             |
| `src/builtins/`            | C     | Builtins must call runtime helpers                              |
| `src/test262.rs`           | D     | V12 scan selector                                               |
| `reports/`                 | all   | Matching report updates are mandatory                           |

Global rule:

```text
Do not create a second descriptor path.
Do not create a second iterator path.
Do not create a second Promise queue.
Do not manually install builtin name/length/prototype flags in each builtin.
```

---

## 2. Descriptor and Object Property Interface

### 2.1 PropertyKey

```rust
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}
```

If the project already has an equivalent type, use the existing type.

### 2.2 PropertyDescriptor

```rust
pub struct PropertyDescriptor {
    pub value: Option<JsValue>,
    pub get: Option<JsValue>,
    pub set: Option<JsValue>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}
```

### 2.3 BuiltinAttrs

```rust
pub struct BuiltinAttrs {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}
```

### 2.4 Required runtime helpers

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

### 2.5 Key order

`own_property_keys` must centralize key ordering:

```text
1. array-index string keys in ascending numeric order
2. other string keys in insertion order
3. symbols in insertion order
```

No builtin may implement separate key ordering.

---

## 3. Builtin Installer Interface

C owns builtin installation. The installer must be shared across builtin families.

### 3.1 Required helpers

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

### 3.2 Default descriptor policy

```text
builtin method:
  writable: true
  enumerable: false
  configurable: true

prototype.constructor:
  writable: true
  enumerable: false
  configurable: true

function name:
  writable: false
  enumerable: false
  configurable: true

function length:
  writable: false
  enumerable: false
  configurable: true

accessor property:
  enumerable: false
  configurable: true
```

### 3.3 Required users

The following must migrate to the shared installer:

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
TypedArray
DataView
Temporal skeleton
```

---

## 4. Iterator Interface

B owns iterator protocol.

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

These features must use the shared iterator helpers:

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
Map constructor
```

### 4.4 Forbidden behavior

```text
Do not manually create { value, done } objects in builtins.
Do not implement Array.from iteration separately.
Do not implement TypedArray.from iteration separately.
Do not skip IteratorClose on abrupt completion.
Do not silently ignore non-callable return/throw methods.
```

---

## 5. Promise and Job Queue Interface

B owns Promise execution protocol. C owns Promise builtin installation.

### 5.1 Types

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
Promise.all
Promise.race
Promise.any
Promise.allSettled
async function
await
for-await-of
async generator
dynamic import
top-level await
Test262 async completion
```

---

## 6. Array and TypedArray Interface

C owns Array/TypedArray algorithms. B owns iteration protocol used by them.

### 6.1 Array.from

```rust
pub fn array_from(
    ctx: &mut NativeContext,
    constructor: JsValue,
    items: JsValue,
    mapfn: Option<JsValue>,
    this_arg: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

Rules:

```text
1. Use get_iterator when @@iterator is present.
2. Fall back to array-like only when iterator path is not used.
3. Use IteratorClose on abrupt completion.
4. mapfn must be called with correct thisArg.
5. Constructor/species behavior may be partial but must be documented.
```

### 6.2 TypedArray.from

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
2. Preserve typed-array element conversion.
3. RangeError must be catchable.
4. BigInt typed arrays may be partial but must not masquerade as Number typed arrays.
5. TypedArray must not be implemented as ordinary Array.
```

### 6.3 ArrayBuffer / DataView

```rust
pub struct TypedArrayRecord {
    pub buffer: JsValue,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub element_length: usize,
}

pub struct DataViewRecord {
    pub buffer: JsValue,
    pub byte_offset: usize,
    pub byte_length: usize,
}

pub fn validate_typed_array(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<TypedArrayRecord, VmError>;

pub fn validate_data_view(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<DataViewRecord, VmError>;
```

---

## 7. RegExp Interface

A owns RegExp literal parsing. C owns RegExp object and builtin behavior.

### 7.1 RegExp literal metadata

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

A must ensure invalid literals produce catchable SyntaxError, not panic.

### 7.2 RegExp runtime helpers

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

### 7.3 Priority features

```text
Unicode property escapes
legacy accessors
RegExp.prototype.compile
RegExp.prototype.exec
RegExp.prototype.test
RegExp.prototype[@@split]
String.prototype.match/search/replace/split dispatch
named groups
backreferences
descriptor exactness
```

---

## 8. Temporal Skeleton Interface

C owns Temporal skeleton. A owns parser blockers.

### 8.1 Installation

```rust
pub fn install_temporal_skeleton(
    ctx: &mut NativeContext,
    global: JsValue,
) -> Result<(), VmError>;
```

### 8.2 Required objects

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

### 8.3 Required shape

Each constructor should provide:

```text
name
length
prototype
prototype.constructor
correct descriptor flags
not-a-constructor behavior where required
```

### 8.4 Allowed minimal methods

```text
valueOf throws TypeError
toString for simple stored values
from for simple ISO inputs
compare for trivial equal values if safe
```

### 8.5 Non-goals

```text
complete timezone arithmetic
complete calendar protocol
duration balancing
rounding
full Intl integration
ZonedDateTime arithmetic
```

---

## 9. Annex B Function Binding Interface

A owns Annex B function binding behavior. Runtime environment APIs must expose enough hooks.

### 9.1 Required behavior

In sloppy mode, block-level function declarations may need to create/update var-style bindings depending on Annex B rules.

Required helper shape:

```rust
pub fn annex_b_instantiate_block_function(
    ctx: &mut NativeContext,
    name: String,
    function_value: JsValue,
    scope: ScopeId,
) -> Result<(), VmError>;
```

Rules:

```text
1. Only sloppy-mode paths use Annex B behavior.
2. Strict mode remains strict.
3. eval-code, function-code, and global-code must share the implementation.
4. Duplicate lexical declaration errors must not be thrown for valid Annex B cases.
5. Function binding updates must be observable after eval where required.
```

---

## 10. Module / Dynamic Import / Top-Level Await Interface

A owns syntax. B owns async execution. D owns Test262 module harness integration.

### 10.1 Minimal dynamic import contract

```rust
pub fn dynamic_import(
    ctx: &mut NativeContext,
    specifier: JsValue,
) -> Result<JsValue, VmError>;
```

Rules:

```text
1. Return a Promise.
2. Unsupported module loading should reject, not panic.
3. Syntax parse shape must be correct.
4. Do not fake successful module loading if module source is not evaluated.
```

### 10.2 Top-level await contract

```rust
pub enum ModuleEvaluationState {
    Pending,
    Fulfilled,
    Rejected(JsValue),
}
```

Rules:

```text
1. Top-level await must use the shared Promise job queue.
2. Module execution failure must be catchable.
3. Unsupported source-phase import may throw SyntaxError or reject consistently.
```

---

## 11. Error Convention

All groups must preserve catchable JavaScript errors.

```text
Syntax problem -> SyntaxError
Invalid callable/constructor -> TypeError
Invalid range/date/numeric input -> RangeError
Unsupported deep feature -> Unsupported only if honestly counted as failure
Runtime limit -> RuntimeLimit, not pass
No panic
No memory allocation failure
No skipped/crashed/timeout case may count as pass
```

When unsure, prefer a correct catchable TypeError over silent success.

---

## 12. Reporting Contract

Every track report must include:

```text
owner and scope
locked fixbug6 baseline
commands run
before/after deltas
newly exposed failures
regressions
cross-group dependencies
next action
```

Required reports:

```text
reports/.version-report/v12-partA-report.md
reports/.version-report/v12-partB-report.md
reports/.version-report/v12-partC-report.md
reports/.native-test262-tmp/native-v12-scan-summary.json
reports/.test262/test262-analysis/fixbug6-test262-analysis.md
reports/.test262/test262-analysis/native-v12-test262-analysis.md
```

If tests were not run, write:

```text
Tests not run: <reason>
Risk: <expected affected suites>
```

---

## 13. V12 Scan Contract

D owns the V12 scan selector.

Required constants:

```rust
pub const NATIVE_V12_SCAN_TESTS: &[&str] = &[/* 5000 locked fixbug6 failures */];
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

The V12 scan is a planning and regression artifact, not the formal conformance number. The formal number remains the full 53,379-case scan.

---

## 14. Interface Change Log

| Date       | Owner | Change                                                    | Files affected                                   | Reviewer | Tests                       |
| ---------- | ----- | --------------------------------------------------------- | ------------------------------------------------ | -------- | --------------------------- |
| 2026-06-27 | all   | Establish V12 70% shared interface                        | `docs/version/native-v12-interface.md`                   | pending  | document-only               |
| 2026-06-27 | C     | Add builtin installer and descriptor contract             | `src/runtime/`, `src/builtins/`                  | B        | Object/Function/Array gates |
| 2026-06-27 | B     | Add Iterator and Promise protocol contract                | `src/runtime/`, `src/vm/`, `src/bytecode/`       | A/C      | Iterator/Promise gates      |
| 2026-06-27 | A     | Add Annex B function binding and parser unlocker contract | `src/parser/`, `src/ast/`, `src/runtime/`        | B/C      | AnnexB language gates       |
| 2026-06-27 | C     | Add RegExp and Temporal skeleton contract                 | `src/builtins/regexp*`, `src/builtins/temporal*` | A/B      | RegExp/Temporal gates       |

---

## 15. Final Rule

V12 must have:

```text
one descriptor path
one builtin installer
one iterator path
one Promise job queue
one object key ordering path
one Test262 reporting path
```

Any duplicated implementation of these mechanisms is a merge blocker.
