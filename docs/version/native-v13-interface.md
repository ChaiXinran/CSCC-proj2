# Native V13 Shared Interface

Native V13 freezes the contracts needed for the 80% Test262 score sprint. This
document supplements `docs/interface-spec.md` and the V1-V12 interface
documents. V13 changes may be Test262-oriented, but they must still use shared
interfaces instead of one-off per-test patches.

## 1. Score Sprint Contract

Rules:

- Every V13 feature track must report focused Test262 before/after numbers.
- Full-suite claims must come from `reports/full-test262-summary-v13.json`.
- Skipped cases are never counted as passes.
- It is acceptable to land intentionally partial implementations when the
  supported boundary is documented in the track report.
- It is not acceptable to silently change failure classification to pass without
  implementing the observable behavior required by the test.
- All new helpers must prefer shared abstract operations over local builtin
  special cases when multiple directories depend on the same behavior.

## 2. Temporal Core Contract

V13-A owns Temporal and intl402/Temporal score work.

Expected shared value shapes:

```rust
pub enum TemporalOverflow {
    Constrain,
    Reject,
}

pub enum TemporalRoundingMode {
    Trunc,
    HalfExpand,
    Floor,
    Ceil,
}

pub struct IsoDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

pub struct IsoTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub microsecond: u16,
    pub nanosecond: u16,
}

pub struct IsoDateTime {
    pub date: IsoDate,
    pub time: IsoTime,
}

pub struct TemporalDurationRecord {
    pub years: i64,
    pub months: i64,
    pub weeks: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub milliseconds: i64,
    pub microseconds: i64,
    pub nanoseconds: i64,
}
```

Expected helper shape:

```rust
pub fn regulate_iso_date(
    year: i32,
    month: i32,
    day: i32,
    overflow: TemporalOverflow,
) -> Result<IsoDate, VmError>;

pub fn balance_iso_date(year: i32, month: i32, day: i32) -> Result<IsoDate, VmError>;
pub fn balance_iso_time(time: IsoTime) -> Result<(i64, IsoTime), VmError>;
pub fn compare_iso_date(left: IsoDate, right: IsoDate) -> std::cmp::Ordering;
pub fn compare_iso_date_time(left: IsoDateTime, right: IsoDateTime) -> std::cmp::Ordering;
pub fn temporal_to_string(value: TemporalValue, options: TemporalStringOptions) -> String;
```

Rules:

- V13-A may support ISO calendar first and document non-ISO limitations.
- Constructors, `from`, `compare`, `equals`, `toString`, `with`, `add`, and
  `subtract` are higher priority than complete calendar/time-zone coverage.
- Getter access order and exception type must be preserved for implemented
  paths.
- `RangeError` vs `TypeError` timing must be recorded when intentionally
  partial.
- intl402/Temporal may use a minimal bridge if full ICU/CLDR behavior is out of
  scope, but constructor shape and supported formatting entries must be
  documented.
- A must not add generic object-model shortcuts in Temporal builtins when the
  behavior belongs in runtime helpers.

## 3. Dynamic Import and Module Contract

V13-B owns dynamic import and module execution score work.

Expected shape:

```rust
pub enum ModuleLoadState {
    New,
    Loading,
    Loaded,
    Evaluating,
    Evaluated,
    Failed,
}

pub struct DynamicImportRequest {
    pub specifier: String,
    pub referrer: Option<std::path::PathBuf>,
    pub attributes: Vec<(String, JsValue)>,
}

pub enum DynamicImportOutcome {
    Fulfilled(JsValue),
    Rejected(JsValue),
}
```

Expected runtime entry:

```rust
impl NativeRuntime {
    pub(crate) fn dynamic_import(
        &mut self,
        request: DynamicImportRequest,
    ) -> Result<JsValue, EvalFailure>;
}
```

Rules:

- `import()` must evaluate to a Promise-like result, not a synchronous plain
  value.
- Load, parse, compile, and evaluation failures must reject the returned
  promise for implemented paths.
- A local-path module resolver is acceptable for V13 if the host boundary is
  documented.
- Import attributes/options may be minimally accepted before complete semantic
  validation, but invalid forms must not panic.
- Module-cycle and live-binding limitations may remain documented non-goals for
  V13.
- B must coordinate with A/C before changing Promise or job-queue behavior used
  outside dynamic import.

## 4. Class Element Contract

V13-B owns class syntax/lowering/runtime score work.

Expected shared planning shape:

```rust
pub struct ClassElementPlan {
    pub name: PropertyKey,
    pub kind: ClassElementKind,
    pub is_static: bool,
    pub is_private: bool,
    pub initializer_index: Option<u16>,
}

pub enum ClassElementKind {
    Method,
    Getter,
    Setter,
    Field,
    StaticBlock,
}
```

Rules:

- Class field initializers run in source order.
- Static blocks run in source order among static elements.
- Computed class keys are evaluated exactly once and in spec order for
  implemented paths.
- Private fields and methods must use a shared private-name representation, not
  string-key hacks visible to ordinary property enumeration.
- Derived constructors must enforce `super()` before `this` access.
- Async/generator method combinations are lower priority than fields, private
  basics, static blocks, and derived constructors.
- Decorators are not a V13 target.

## 5. Array and TypedArray Abstract Operation Contract

V13-C owns Array, TypedArray, and shared low-cost builtin work.

Expected helper shape:

```rust
pub fn species_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: JsValue,
    default_constructor: JsValue,
) -> Result<JsValue, VmError>;

pub fn array_species_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    original_array: JsValue,
    length: u32,
) -> Result<JsValue, VmError>;

pub fn validate_typed_array(
    context: &NativeContext,
    value: JsValue,
) -> Result<TypedArrayViewId, VmError>;

pub fn require_not_detached(
    context: &NativeContext,
    view: TypedArrayViewId,
) -> Result<(), VmError>;

pub fn typed_array_species_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    exemplar: JsValue,
    length: usize,
) -> Result<JsValue, VmError>;
```

Rules:

- Species creation must use normal property access and propagate abrupt
  completions.
- IteratorClose must happen in implemented Array/TypedArray paths when mapping,
  constructing, or copying from iterables fails.
- TypedArray validation and detached-buffer checks must happen in the order
  expected by Test262 for implemented methods.
- Stable sort and comparator abrupt completion should be shared by Array and
  TypedArray where possible.
- BigInt typed-array work depends on the current BigInt representation being
  stable enough for storage and conversion.
- C must not add one-off fixes to single Array methods when a shared abstract
  operation covers multiple methods.

## 6. Missing Globals and Error-Like Constructor Contract

V13-C owns missing low-cost global constructors and URI globals.

Expected constructor helper:

```rust
pub fn install_error_like_constructor(
    context: &mut NativeContext,
    name: &'static str,
    length: u8,
    call: NativeCall,
    construct: NativeConstruct,
) -> Result<JsValue, VmError>;
```

Targets:

- `AggregateError`
- `SuppressedError`
- `encodeURI`
- `decodeURI`
- `encodeURIComponent`
- `decodeURIComponent`
- `DisposableStack`
- `AsyncDisposableStack`

Rules:

- Constructors must install `name`, `length`, `prototype`, and
  `prototype.constructor` descriptors matching existing project descriptor
  conventions.
- `AggregateError` must preserve iterable error collection order for
  implemented paths.
- `SuppressedError` must expose `error`, `suppressed`, and `message` shape for
  implemented paths.
- URI functions must use UTF-8 percent encoding/decoding and throw `URIError`
  for malformed escape sequences where implemented.
- `DisposableStack` may start as a synchronous resource-stack implementation.
- `AsyncDisposableStack` may start with Promise-shaped behavior and documented
  async limitations.

## 7. Merge Compatibility

Recommended order:

```text
V13 interface docs
  -> C missing globals and URI first batch
  -> B dynamic import minimal Promise path
  -> A Temporal PlainDate / Duration first batch
  -> C ArraySpeciesCreate / TypedArray validation
  -> B class fields / static blocks / private basics
  -> A Temporal Instant / PlainDateTime / PlainTime
  -> C DisposableStack / AsyncDisposableStack
  -> A ZonedDateTime / intl402 Temporal bridge
  -> full Test262 and regression repair
```

Shared files require coordination:

- `src/builtins/date_intl.rs` is A-owned for V13.
- `src/parser/`, `src/ast/`, `src/bytecode/compiler.rs`, and
  `src/runtime/module.rs` are B-owned for V13.
- `src/builtins/array.rs`, `src/builtins/binary_data.rs`, and
  `src/builtins/error.rs` are C-owned for V13.
- `src/runtime/context.rs` is shared. Every change must state whether it serves
  Temporal, module/class, or Array/TypedArray/Error work.
- `src/vm/interpreter.rs` is B-first. C may review or make isolated helper
  calls, but must not interleave large VM rewrites during class/module work.
