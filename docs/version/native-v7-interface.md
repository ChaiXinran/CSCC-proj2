# Native V7 Shared Interface

Native V7 freezes the engineering contracts needed to make the native backend
safe under long Test262 scans and benchmark workloads. V7 is not a syntax
milestone. Its shared interfaces cover resource budgets, garbage collection,
native script caching, crash-safe reporting, and performance measurements.

This document supplements `interface-spec.md` and the V2-V6 interface
documents. If a V7 engineering contract conflicts with an older placeholder,
the V7 contract is authoritative for the new behavior.

## 1. Runtime Budget Contract

`RuntimeConfig` remains the public configuration boundary for one isolate. V7
adds byte, deadline, and GC thresholds without changing the meaning of existing
limits:

```rust
pub struct RuntimeConfig {
    pub loop_limit: u64,
    pub recursion_limit: usize,
    pub stack_limit: usize,
    pub backtrace_limit: usize,
    pub script_cache_capacity: usize,
    pub install_test262_host: bool,
    pub heap_object_limit: usize,

    pub heap_byte_limit: usize,
    pub wall_clock_limit: Option<std::time::Duration>,
    pub gc_allocation_threshold: usize,
}
```

Rules:

- loop, recursion, VM stack, heap object, heap byte, string allocation, and
  wall-clock exhaustion report `VmErrorKind::RuntimeLimit`;
- resource-limit failures must never abort the Rust process;
- limit checks must be cheap enough to remain enabled in release builds;
- default limits are conservative for agent workloads, while Test262 scans may
  opt into larger values explicitly;
- limits are per isolate and reset at the start of each top-level evaluation
  unless the limit represents persistent heap capacity.

## 2. Execution Budget Contract

The VM and runtime share one execution-budget object:

```rust
pub struct ExecutionBudget {
    pub loop_remaining: u64,
    pub call_depth_limit: u64,
    pub stack_limit: usize,
    pub deadline: Option<std::time::Instant>,
}

impl ExecutionBudget {
    pub fn check_loop(&mut self) -> Result<(), VmError>;
    pub fn check_call_depth(&self, depth: u64) -> Result<(), VmError>;
    pub fn check_stack_depth(&self, depth: usize) -> Result<(), VmError>;
    pub fn check_deadline(&self) -> Result<(), VmError>;
}
```

The VM must call the budget at these boundaries:

- loop backedges and explicit iterator advancement;
- function, constructor, getter, setter, and builtin callback entry;
- operand-stack growth;
- large builtin loops such as string repeat, array flattening, JSON walking,
  and property-key enumeration.

The deadline check is cooperative. V7 does not require preemptive thread
interruption, but long-running native loops must call `check_deadline`.

## 3. Heap Accounting Contract

The V7 heap tracks both object count and estimated bytes:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub object_slots: usize,
    pub live_objects: usize,
    pub live_environments: usize,
    pub live_functions: usize,
    pub estimated_bytes: usize,
    pub allocation_count: u64,
    pub collection_count: u64,
}
```

Allocation APIs return `VmError` instead of silently converting exhaustion into
generic runtime errors:

```rust
impl NativeContext {
    pub fn heap_stats(&self) -> HeapStats;
    pub fn ensure_heap_capacity(&mut self, additional_bytes: usize) -> Result<(), VmError>;
    pub fn maybe_collect_garbage(&mut self, roots: &RootSet) -> Result<CollectionStats, VmError>;
}
```

Byte accounting is allowed to be approximate, but it must be monotonic between
collections and conservative enough to reject obviously dangerous allocations
before the host process attempts them.

## 4. Garbage Collection Contract

V7 upgrades the placeholder collector into a non-moving mark-and-sweep
collector. Object, function, and environment IDs remain stable for the lifetime
of each allocation slot. Sweeping may turn unreachable slots into `None`, but
must not compact arenas or reuse IDs during the same collection pass.

Root discovery is explicit:

```rust
pub struct RootSet {
    pub global_environment: EnvironmentId,
    pub current_environment: EnvironmentId,
    pub environment_stack: Vec<EnvironmentId>,
    pub call_frames: Vec<CallFrameRoots>,
    pub operand_stack: Vec<JsValue>,
    pub pending_exception: Option<JsValue>,
}

pub struct CallFrameRoots {
    pub function: Option<FunctionId>,
    pub this_value: JsValue,
    pub environment: EnvironmentId,
    pub stack_base: usize,
}
```

Every heap-owned structure that can reference another heap allocation must
participate in tracing:

```rust
pub trait Trace {
    fn trace(&self, tracer: &mut Tracer);
}

pub struct Tracer<'a> {
    pub heap: &'a Heap,
}
```

Required trace participants:

- `JsValue`;
- `JsObject`, including string and symbol property descriptors;
- `PropertyDescriptor`;
- `Environment` bindings and outer environment links;
- `JsFunction`, closures, bound functions, and function templates that retain
  heap values.

`CollectionStats` expands to include all arena kinds:

```rust
pub struct CollectionStats {
    pub objects_before: usize,
    pub objects_after: usize,
    pub environments_before: usize,
    pub environments_after: usize,
    pub functions_before: usize,
    pub functions_after: usize,
    pub bytes_before: usize,
    pub bytes_after: usize,
}
```

## 5. Large Allocation Guard Contract

Builtins that derive allocation size from JavaScript values must use shared
guards before allocating:

```rust
pub fn checked_string_repeat_len(unit_len: usize, count: usize) -> Result<usize, VmError>;
pub fn checked_array_length(length: f64) -> Result<usize, VmError>;
pub fn checked_utf16_allocation(units: usize) -> Result<(), VmError>;
```

Rules:

- `RangeError` remains the ECMAScript error for invalid user-visible lengths;
- `RuntimeLimit` is used when a valid ECMAScript request exceeds engine
  resource budgets;
- no V7 builtin may call `Vec::with_capacity`, `String::with_capacity`, or
  recursive helper functions with untrusted size/depth without a guard.

## 6. Native Script Cache Contract

V7 implements native parsed/compiled script caching behind the existing
`script_cache_capacity` setting. Cache entries are immutable and isolate-local:

```rust
pub struct NativeScriptCacheKey {
    pub source_hash: u64,
    pub strict: bool,
}

pub struct NativeScriptCacheEntry {
    pub program: Program,
    pub chunk: Chunk,
    pub max_stack_depth: usize,
}
```

Rules:

- cached chunks must not contain `ObjectId`, `EnvironmentId`, or any
  context-local runtime identity;
- cache hits must execute through the same VM path as uncached chunks;
- capacity `0` disables native caching;
- eviction is LRU;
- cache statistics are reported by benchmarks but are not observable from
  JavaScript.

## 7. Crash-Safe Test262 Reporting Contract

The Test262 runner must distinguish JavaScript failures from host failures:

```rust
pub enum SuiteStatus {
    Completed,
    TimedOut,
    Crashed,
}

pub struct SuiteSummary {
    pub suite: PathBuf,
    pub status: SuiteStatus,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub conformance_percent: f64,
    pub detail: String,
}
```

Rules:

- skipped tests never count as passed;
- a crashed or timed-out child suite contributes to the report as a failed
  suite, not as hidden missing data;
- parent reporting tools must survive child OOM, stack overflow, panic, and
  non-zero process exit;
- failure samples are capped by configuration and never store every failing
  case by default.

## 8. Benchmark Evidence Contract

V7 benchmark reports must include:

- build profile, feature set, commit, OS, CPU, and Rust version;
- binary size;
- cold isolate latency;
- warm uncached latency;
- warm cached latency;
- peak resident memory where the platform can provide it;
- Test262 dashboard totals and crashed-suite counts;
- JetStream 2 CLI subset results compared with the same-machine reference
  runs.

Benchmarks must compare release builds only. Boa and QuickJS remain references,
not Native implementation dependencies.

## 9. Compatibility Rules

- V7 must preserve V1-V6 pinned gates.
- V7 must not add parser special cases for benchmark or Test262 names.
- V7 must not introduce JIT, WebAssembly, modules, async functions, or a moving
  collector.
- V7 must not treat unsupported tests as passes.
- V7 must prefer returning categorized `RuntimeLimit` or `Unsupported` failures
  over aborting the process.
