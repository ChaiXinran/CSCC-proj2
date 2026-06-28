# Fixup8 Shared Interface

## 0. Authority

This document freezes the shared interfaces for the Fixup8 3-person sprint.

It supplements earlier Native interface documents. For Fixup8 work, this document is authoritative when new changes affect more than one track.

Fixup8 principle:

```text
One descriptor path.
One builtin installer.
One iterator path.
One Promise job queue.
One call / construct path.
One environment binding path.
One RegExp dispatch path.
```

Any duplicated implementation of these mechanisms is a merge blocker.

---

## 1. Track ownership

| Track                                   | Owner | Shared authority                                                                                                   |
| --------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------------------ |
| P1 Builtin Core + Temporal + Descriptor | P1    | builtin installer, descriptor helper, Temporal value skeleton                                                      |
| P2 RegExp + String Dispatch             | P2    | RegExp parser/runtime helper, String-RegExp dispatch                                                               |
| P3 Language + Module + Async Protocol   | P3    | parser/compiler language residuals, iterator helper, Promise job queue, call/construct helper, environment binding |

---

## 2. Shared file ownership

| File or area                            | Owner         | Rule                                                                        |
| --------------------------------------- | ------------- | --------------------------------------------------------------------------- |
| `src/builtins/date_intl.rs`             | P1            | Temporal/Date/Intl skeleton work only                                       |
| `src/builtins/array.rs`                 | P1            | Uses P3 iterator helper for iteration                                       |
| `src/builtins/object.rs`                | P1            | Descriptor/object shape                                                     |
| `src/builtins/function.rs`              | P1            | Function descriptor/call/apply/bind shape; call VM helper from P3 if needed |
| `src/builtins/regexp.rs`                | P2            | RegExp runtime/builtin behavior                                             |
| `src/builtins/string.rs`                | P2            | String-RegExp dispatch and String residuals                                 |
| `src/builtins/std_primitives.rs`        | P1/P2 shared  | P1 for generic primitive builtin shape; P2 for String-RegExp dispatch       |
| `src/builtins/promise.rs`               | P3            | Promise builtin behavior and job queue integration                          |
| `src/builtins/collections.rs`           | P3 by default | Iterator/Map/Set protocol; P1 may touch Set descriptor with coordination    |
| `src/runtime/property.rs`               | P1            | Descriptor helper and descriptor update semantics                           |
| `src/runtime/object.rs`                 | P1            | Object property model and key order                                         |
| `src/runtime/iterator.rs`               | P3            | IteratorRecord and iterator protocol                                        |
| `src/runtime/job.rs`                    | P3            | Promise jobs and host jobs                                                  |
| `src/runtime/module.rs`                 | P3            | Module registry and top-level await state                                   |
| `src/runtime/environment.rs`            | P3            | Scope/binding/eval/Annex B binding                                          |
| `src/runtime/context.rs`                | shared        | Only small helper additions; no large unrelated rewrites                    |
| `src/parser/`, `src/ast/`, `src/lexer/` | P3            | P2 may touch RegExp-specific parser only                                    |
| `src/bytecode/compiler.rs`              | P3            | P1/P2 do not edit without interface note                                    |
| `src/vm/`                               | P3            | Call execution, async, module and iterator execution                        |
| `src/test262.rs`, `src/main.rs`         | integration   | Fixup8 scan selector only                                                   |

---

## 3. Descriptor and property interface

P1 owns this interface.

### 3.1 Existing concepts

The project already uses a `PropertyDescriptor` and object property model. Fixup8 must centralize builtin descriptor behavior around it instead of setting flags by hand in each builtin.

### 3.2 Required helper shape

Names may be adapted to existing code, but semantics must remain:

```rust
pub struct BuiltinAttrs {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

pub fn define_data_property(
    ctx: &mut NativeContext,
    object: ObjectId,
    key: impl Into<PropertyKey>,
    value: JsValue,
    attrs: BuiltinAttrs,
) -> Result<(), VmError>;

pub fn define_accessor_property(
    ctx: &mut NativeContext,
    object: ObjectId,
    key: impl Into<PropertyKey>,
    get: Option<JsValue>,
    set: Option<JsValue>,
    enumerable: bool,
    configurable: bool,
) -> Result<(), VmError>;

pub fn get_own_property_descriptor(
    ctx: &NativeContext,
    object: ObjectId,
    key: &str,
) -> Option<PropertyDescriptor>;

pub fn ordinary_own_property_keys(
    ctx: &NativeContext,
    object: ObjectId,
) -> Vec<PropertyKey>;
```

### 3.3 Key order

`ordinary_own_property_keys` must use one ordering:

```text
1. array-index string keys in ascending numeric order
2. other string keys in insertion order
3. symbol keys in insertion order
```

No builtin may implement its own property key order.

### 3.4 Descriptor defaults

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

constructor.prototype:
  writable: false
  enumerable: false
  configurable: false

accessor property:
  enumerable: false
  configurable: true
```

Any exception must be documented in the relevant track report.

---

## 4. Builtin installer interface

P1 owns this interface.

### 4.1 Required helper shape

```rust
pub fn install_builtin_constructor(
    ctx: &mut NativeContext,
    global: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
    construct: Option<NativeConstruct>,
    prototype: ObjectId,
) -> Result<JsValue, VmError>;

pub fn install_builtin_function(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError>;

pub fn install_builtin_method(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError>;

pub fn install_builtin_accessor(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    getter: Option<NativeCall>,
    setter_name: Option<&'static str>,
    setter: Option<NativeCall>,
) -> Result<(), VmError>;

pub fn mark_not_constructor(
    ctx: &mut NativeContext,
    function: JsValue,
) -> Result<(), VmError>;
```

### 4.2 Required users

The following must use this installer rather than manual descriptor setup:

```text
Temporal
Date
Array
Object
Function
String
RegExp
Promise
Iterator
Map
Set
TypedArray
DataView
```

### 4.3 Forbidden behavior

```text
Do not manually set name/length descriptors in each builtin.
Do not manually create prototype.constructor differently in each builtin.
Do not install JS-visible methods with enumerable: true unless the spec requires it.
Do not silently ignore descriptor failure.
```

---

## 5. Iterator interface

P3 owns this interface.

### 5.1 Existing model

The project already has `IteratorMode` and `IteratorRecord` in `src/runtime/iterator.rs`.

Fixup8 should extend behavior around this model instead of creating another iterator record.

### 5.2 Required helper shape

```rust
pub enum IteratorHint {
    Sync,
    Async,
}

pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
}

pub struct IteratorStep {
    pub value: JsValue,
    pub done: bool,
}

pub fn get_iterator(
    ctx: &mut NativeContext,
    value: JsValue,
    hint: IteratorHint,
) -> Result<IteratorRecord, VmError>;

pub fn iterator_next(
    ctx: &mut NativeContext,
    record: &mut IteratorRecord,
    value: Option<JsValue>,
) -> Result<IteratorStep, VmError>;

pub fn iterator_value(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<JsValue, VmError>;

pub fn iterator_complete(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<bool, VmError>;

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

### 5.3 Required users

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
Iterator builtin methods
```

### 5.4 Forbidden behavior

```text
Do not manually create { value, done } objects in builtins.
Do not implement Array.from iteration separately.
Do not implement TypedArray.from iteration separately.
Do not skip IteratorClose on abrupt completion.
Do not treat non-callable return/throw as success.
```

---

## 6. Promise and job queue interface

P3 owns this interface.

### 6.1 Existing model

The project already has `PromiseId`, `PromiseRecord`, `PromiseState`, `PromiseThenReaction`, `PromiseJob`, `PromiseCallbackJob`, `NativeJob`, `Job`, and `JobQueue`.

Fixup8 must use this queue for every async path.

### 6.2 Required helper shape

```rust
pub struct PromiseCapability {
    pub promise: JsValue,
    pub resolve: JsValue,
    pub reject: JsValue,
}

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
    promise: PromiseId,
    value: JsValue,
) -> Result<(), VmError>;

pub fn reject_promise(
    ctx: &mut NativeContext,
    promise: PromiseId,
    reason: JsValue,
) -> Result<(), VmError>;

pub fn enqueue_promise_job(
    ctx: &mut NativeContext,
    job: Job,
);

pub fn drain_promise_jobs(
    ctx: &mut NativeContext,
) -> Result<(), VmError>;
```

### 6.3 Required users

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
async generator
for-await-of
dynamic import
top-level await
Test262 async completion
```

### 6.4 Rules

```text
All Promise callbacks go through one queue.
Dynamic import returns or rejects a Promise; it must not panic.
Top-level await uses the same queue.
Promise jobs are drained at Test262 async completion.
Unsupported module loading should reject or throw catchable error, not silently pass.
```

---

## 7. Call and construct interface

P3 owns the shared call/construct interface. P1 and P2 may call it.

### 7.1 Required helper shape

```rust
pub fn is_callable(
    ctx: &NativeContext,
    value: &JsValue,
) -> bool;

pub fn is_constructor(
    ctx: &NativeContext,
    value: &JsValue,
) -> bool;

pub fn call_function(
    ctx: &mut NativeContext,
    function: JsValue,
    this_value: JsValue,
    args: Vec<JsValue>,
) -> Result<JsValue, VmError>;

pub fn construct_function(
    ctx: &mut NativeContext,
    constructor: JsValue,
    args: Vec<JsValue>,
    new_target: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

### 7.2 Required users

```text
String.prototype.replace callback
String.prototype.match custom matcher
String.prototype.search custom searcher
String.prototype.split custom splitter
Array.prototype map/filter/reduce/find
Array.from mapfn
Promise then/catch/finally callbacks
Temporal from/toString helper calls if needed
RegExp custom dispatch
```

### 7.3 Forbidden behavior

```text
Do not call NativeCall function pointers directly from unrelated builtins.
Do not assume every BuiltinFunction is constructible.
Do not treat undefined as callable.
Do not swallow TypeError when a callback should be callable.
```

---

## 8. RegExp interface

P2 owns this interface.

### 8.1 RegExp literal metadata

If the parser exposes RegExp metadata, it should include at least:

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

If the existing AST uses a different shape, keep the existing shape but preserve these semantics.

### 8.2 Runtime helpers

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

### 8.3 String dispatch

String methods should call RegExp/custom-dispatch helpers:

```rust
pub fn string_match_dispatch(
    ctx: &mut NativeContext,
    this_value: JsValue,
    matcher: JsValue,
) -> Result<JsValue, VmError>;

pub fn string_replace_dispatch(
    ctx: &mut NativeContext,
    this_value: JsValue,
    search_value: JsValue,
    replace_value: JsValue,
) -> Result<JsValue, VmError>;

pub fn string_search_dispatch(
    ctx: &mut NativeContext,
    this_value: JsValue,
    searcher: JsValue,
) -> Result<JsValue, VmError>;

pub fn string_split_dispatch(
    ctx: &mut NativeContext,
    this_value: JsValue,
    splitter: JsValue,
    limit: Option<JsValue>,
) -> Result<JsValue, VmError>;
```

### 8.4 Rules

```text
Invalid RegExp syntax must throw SyntaxError, not panic.
Valid Annex B legacy patterns must not be rejected too early.
String dispatch must use call_function for custom matchers/replacers/searchers/splitters.
RegExp legacy accessor descriptors must use P1 descriptor helpers.
```

---

## 9. Temporal skeleton interface

P1 owns this interface.

### 9.1 Installation

```rust
pub fn install_temporal(
    ctx: &mut NativeContext,
) -> Result<(), VmError>;
```

If current code uses `install_temporal` inside `date_intl.rs`, keep that entry point and refine internals.

### 9.2 Required objects

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

### 9.3 Required object shape

Every Temporal constructor or namespace object should have:

```text
correct global property descriptor
correct name if function
correct length if function
prototype when constructor-like
prototype.constructor
method descriptors
valueOf throws TypeError where required
catchable TypeError / RangeError for unsupported deep semantics
```

### 9.4 Internal slots

Use hidden own data properties only as an implementation bridge:

```text
__agentjs_temporal_kind__
__agentjs_temporal_iso__
__agentjs_temporal_epoch_ns__
```

Do not make hidden slots enumerable.

### 9.5 Allowed minimal methods

```text
from simple ISO input
toString simple stored value
valueOf TypeError
compare trivial equal values if safe
```

### 9.6 Non-goals

```text
complete calendar protocol
complete timezone database
duration balancing
rounding
ZonedDateTime arithmetic
full Intl.DateTimeFormat integration
```

---

## 10. Environment and Annex B binding interface

P3 owns this interface.

### 10.1 Required helper shape

```rust
pub fn create_var_binding(
    ctx: &mut NativeContext,
    name: &str,
    value: JsValue,
) -> Result<(), VmError>;

pub fn create_lexical_binding(
    ctx: &mut NativeContext,
    name: &str,
    mutable: bool,
) -> Result<(), VmError>;

pub fn initialize_binding(
    ctx: &mut NativeContext,
    name: &str,
    value: JsValue,
) -> Result<(), VmError>;

pub fn set_mutable_binding(
    ctx: &mut NativeContext,
    name: &str,
    value: JsValue,
    strict: bool,
) -> Result<(), VmError>;

pub fn annex_b_instantiate_block_function(
    ctx: &mut NativeContext,
    name: String,
    function_value: JsValue,
) -> Result<(), VmError>;
```

### 10.2 Annex B rules

```text
Only sloppy mode uses Annex B block-level function declaration compatibility behavior.
Strict mode remains strict.
direct eval, indirect eval, global code, and function code must share the implementation.
Valid Annex B cases must not throw duplicate lexical declaration errors.
Invalid lexical collisions must still throw SyntaxError.
Binding updates must be observable after eval where required.
```

### 10.3 Required users

```text
annexB/language/eval-code
annexB/language/function-code
annexB/language/global-code
class name binding
function declarations in blocks
eval
global declarations
module lexical binding only where appropriate
```

---

## 11. Module and dynamic import interface

P3 owns this interface.

### 11.1 Existing module model

The project already has:

```text
ModuleId
ModuleRecord
ModuleImportBinding
ModuleExportBinding
ModuleStatus
ModuleEvaluationState
ModuleRegistry
```

Fixup8 should extend this model rather than creating a second module registry.

### 11.2 Dynamic import

```rust
pub fn dynamic_import(
    ctx: &mut NativeContext,
    specifier: JsValue,
) -> Result<JsValue, VmError>;
```

Rules:

```text
Return a Promise.
Unsupported loading should reject or throw catchable error.
Do not fake success if source is not actually evaluated.
Do not panic on unsupported specifier.
```

### 11.3 Top-level await

```rust
pub fn set_module_evaluation_state(
    ctx: &mut NativeContext,
    module_id: ModuleId,
    state: ModuleEvaluationState,
) -> Result<(), VmError>;
```

Rules:

```text
Top-level await uses the shared Promise queue.
Module failure is catchable.
Unsupported source-phase import must fail consistently.
```

---

## 12. Error convention

All tracks must preserve catchable JavaScript errors.

```text
Syntax problem -> SyntaxError
Invalid callable / constructor -> TypeError
Invalid range/date/numeric input -> RangeError
Unsupported deep feature -> honest failure, not pass
Runtime limit -> RuntimeLimit
No panic
No memory allocation failure
No skipped/crashed/timeout case counts as pass
```

When unsure, prefer a correct catchable TypeError over silent success.

---

## 13. Fixup8 scan interface

Add one selector for the 5,000-case scan.

### 13.1 Constants

Use one naming convention. Recommended:

```rust
pub const FIXUP8_SCAN_TESTS: &str = include_str!("../reports/fixup8-scan-failures.txt");
pub const FIXUP8_SCAN_TEST_COUNT: usize = 5_000;
```

Alternative project-style name:

```rust
pub const NATIVE_FIXUP8_SCAN_TESTS: &str = include_str!("../reports/fixup8-scan-failures.txt");
pub const NATIVE_FIXUP8_SCAN_TEST_COUNT: usize = 5_000;
```

### 13.2 Runner option

```rust
impl RunnerOptions {
    pub fn select_fixup8_scan(&mut self) {
        self.backend = BackendKind::Native;
        self.files = FIXUP8_SCAN_TESTS
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(PathBuf::from)
            .collect();
        self.suites.clear();
        self.skip_unsupported = false;
        debug_assert_eq!(self.files.len(), FIXUP8_SCAN_TEST_COUNT);
    }
}
```

### 13.3 CLI

Recommended:

```text
--fixup8-scan
```

Alternative:

```text
--native-fixup8-scan
```

Use one consistently.

### 13.4 Command

```powershell
cargo run --release --no-default-features -- test262 --fixup8-scan --jobs 4 --json reports/fixup8-scan-summary.json
```

---

## 14. Reporting contract

Required reports:

```text
reports/fixup8-p1-report.md
reports/fixup8-p2-report.md
reports/fixup8-p3-report.md
reports/fixup8-scan-summary.json
reports/fixup8-test262-analysis.md
```

Each report must include:

```text
owner and scope
locked fixRTLE baseline
commands run
before/after pass/fail deltas
newly exposed failures
regressions
cross-track dependencies
next action
```

If no tests were run:

```text
Tests not run: <reason>
Risk: <expected affected suites>
```

---

## 15. Interface change log

Append every shared-interface change here.

| Date       | Owner | Change                                             | Files affected                                                                                         | Reviewer | Tests                                |
| ---------- | ----- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | -------- | ------------------------------------ |
| 2026-06-28 | all   | Establish Fixup8 3-person shared interface         | `docs/fixup-interface.md`                                                                              | pending  | document-only                        |
| 2026-06-28 | P1    | Builtin installer / descriptor contract            | `src/runtime/property.rs`, `src/builtins/*`                                                            | P2/P3    | Object/Array/Function gates          |
| 2026-06-28 | P2    | RegExp + String dispatch contract                  | `src/builtins/regexp.rs`, `src/builtins/string.rs`                                                     | P1/P3    | RegExp/String gates                  |
| 2026-06-28 | P3    | Iterator / Promise / module / environment contract | `src/runtime/iterator.rs`, `src/runtime/job.rs`, `src/runtime/module.rs`, `src/runtime/environment.rs` | P1/P2    | Iterator/Promise/module/AnnexB gates |
| 2026-06-28 | P2    | Implement RegExp static validation, runtime exception flow, matchAll, legacy accessors, and String dispatch | `src/lexer/mod.rs`, `src/builtins/annex_b.rs`, `src/builtins/regexp.rs`, `src/builtins/std_primitives.rs` | pending | RegExp/AnnexB/String focused gates |

---

## 16. Merge blocker checklist

A PR is blocked if it:

```text
adds a second descriptor path
adds a second iterator path
adds a second Promise queue
adds a second call/construct path
adds a second Annex B binding path
manual-installs builtin descriptors inconsistently
counts skipped/timeout/crash as pass
modifies another track's core file without updating this interface
does not update the matching report
```

---

## 17. Final summary

Fixup8 has three owners:

```text
P1 = Builtin Core + Temporal + Descriptor
P2 = RegExp + String Dispatch
P3 = Language + Module + Async Protocol
```

The purpose of this interface is to allow cross-layer fixes without creating cross-track chaos.

Each track may cross layers inside its feature cluster, but shared mechanisms must remain single-owner and documented here before implementation.
