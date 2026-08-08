# AgentJS Phase 4：收口阶段三人并行修复方案与共享接口

> Repository: `ChaiXinran/CSCC-proj2`  
> Baseline: `main@b6168e86a3a91ac7ad43293e806c070663a488ad`  
> Date: 2026-08-08  
> Previous phase: G / H / I integrated  
> Current goal: close the remaining JetStream failures instead of adding broad new optimizations.

---

## 1. Current baseline

Phase 3 has already landed the following structural work:

- SharedChunk / bytecode sharing
- Shared runtime strings (`JsString(Arc<str>)`)
- Compact `PropertyMap` with stable property slots and tombstone deletion
- Local Slot
- Upvalue Slot for statically resolvable function activation bindings
- Ordinary-object Shape table
- Monomorphic own-data-property Get/Set IC
- staged JetStream external resources
- configurable JetStream thread stack, default 32 MiB
- one absolute run deadline across runner read / frontend / execution / jobs
- frontend TokenText sharing
- previously found Promise-job and array callback GC-root holes

The latest protected JetStream matrix is still:

```text
12 / 19 PASS
```

Remaining failures:

```text
MEMORY_LIMIT:
- ai-astar
- jsdom-d3-startup
- threejs
- WSL

CALL_ERROR / semantic:
- validatorjs

ENGINE_FAILURE / staged-script semantics:
- regexp
```

`web-ssr` is now PASS after direct-eval Upvalue deoptimization.

The most important new evidence is:

1. WSL / jsdom / threejs peak in runtime job-drain rather than parsing or compilation.
2. WSL at GC threshold 10k no longer has `missing object`, but becomes GC-bound.
3. 100k and 1m thresholds still exceed the 1.5 GiB protection limit.
4. Engine `Heap::estimated_bytes` does not represent the full process memory footprint.
5. `NativeContext` owns several long-lived registries outside `Heap`, including promises,
   ArrayBuffers, TypedArray/DataView metadata, jobs, module/realm/side tables and caches.
6. H's Set IC currently reports zero hits and high invalidations, so expanding IC is not
   the current priority.
7. `regexp` still requires persistent host-script scope semantics.
8. `validatorjs` is a two-file staged workload and should be re-tested after host-script
   semantics are repaired before treating it as an independent builtin bug.

---

# 2. Phase 4 priorities

This phase uses three tracks:

| Track | Owner | Main problem |
|---|---|---|
| J | Memory accounting + adaptive GC | Current GC threshold is count-based and memory-unaware |
| K | Runtime side-registry lifetime | Large memory is not fully represented or reclaimed by `Heap` GC |
| L | Persistent staged Host-script session | `regexp` semantics and possibly `validatorjs` |

Do **not** start the following unless profiling after J/K/L proves they are required:

- polymorphic property IC
- prototype-chain IC
- Block Slot
- module slot optimization
- JIT
- full moving/generational collector
- AST-wide atom interning
- reverting to a concatenated workload `new Function`

The objective is:

```text
remove MEMORY_LIMIT failures
+
remove staged-script semantic failures
+
produce a stable final integration baseline
```

---

# 3. Pre-flight baseline

All three branches start from the same SHA:

```text
b6168e86a3a91ac7ad43293e806c070663a488ad
```

Before branching, record:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Preserve the current Phase 3 matrix as the comparison baseline:

| workload | result | peak RSS |
|---|---|---:|
| ai-astar | MEMORY_LIMIT | ~1537 MiB |
| jsdom-d3-startup | MEMORY_LIMIT | ~1579 MiB |
| threejs | MEMORY_LIMIT | ~1537 MiB |
| WSL | MEMORY_LIMIT | ~1667 MiB |
| validatorjs | CALL_ERROR | ~29 MiB |
| regexp | ENGINE_FAILURE | ~128 MiB |

Branches:

```text
phase4/adaptive-gc-accounting
phase4/runtime-side-registry-gc
phase4/persistent-host-script
integration/phase4
```

---

# 4. Shared memory contract

J and K both work on memory but must not independently invent accounting APIs.

The integrator freezes the following interfaces first.

## 4.1 RuntimeMemoryStats

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeMemoryStats {
    pub heap_estimated_bytes: usize,
    pub heap_object_slots: usize,
    pub heap_live_objects: usize,
    pub heap_live_environments: usize,
    pub heap_live_functions: usize,

    pub object_arena_capacity_bytes: usize,
    pub environment_arena_capacity_bytes: usize,
    pub function_arena_capacity_bytes: usize,

    pub promise_records: usize,
    pub promise_capacity: usize,
    pub promise_reaction_capacity: usize,

    pub job_queue_len: usize,
    pub job_queue_capacity: usize,

    pub array_buffer_records: usize,
    pub array_buffer_capacity: usize,
    pub array_buffer_payload_bytes: usize,

    pub typed_array_views: usize,
    pub typed_array_view_capacity: usize,

    pub data_views: usize,
    pub data_view_capacity: usize,

    pub private_slot_entries: usize,
    pub function_object_entries: usize,
    pub object_value_entries: usize,
    pub module_records: usize,
    pub realm_records: usize,

    pub shape_count: usize,
    pub property_ic_entries: usize,
    pub regexp_cache_entries: usize,

    pub tracked_runtime_bytes: usize,
    pub charged_bytes_since_gc: usize,
}
```

This is diagnostic/accounting information only and must not alter JavaScript-visible behavior.

## 4.2 Runtime memory classes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryClass {
    HeapObject,
    HeapEnvironment,
    HeapFunction,
    ObjectMutation,
    PromiseRegistry,
    JobQueue,
    ArrayBuffer,
    TypedArrayMetadata,
    DataViewMetadata,
    ModuleRegistry,
    RealmRegistry,
    RuntimeCache,
    OtherNative,
}
```

## 4.3 Pressure accounting

```rust
pub struct AllocationPressure {
    pub allocations_since_gc: usize,
    pub charged_bytes_since_gc: usize,
}

impl NativeContext {
    pub(crate) fn note_runtime_growth(
        &mut self,
        class: MemoryClass,
        bytes: usize,
    );

    pub fn runtime_memory_stats(&self) -> RuntimeMemoryStats;
}
```

Important:

```text
tracked bytes != exact RSS
```

The purpose is to explain memory growth and drive GC policy, not to claim equality with OS RSS.

---

# 5. Track J — Memory accounting + adaptive GC

## 5.1 Problem

The current collector is non-moving mark-and-sweep.

The collection trigger is effectively:

```text
allocations_since_collection >= fixed_threshold
```

This is too coarse:

- 1,000,000: WSL exceeds memory limit;
- 100,000: still exceeds memory limit;
- 10,000: memory is controlled but GC dominates runtime.

Also, mutation growth inside already-allocated objects and NativeContext-owned registries is not naturally represented by the current object-allocation counter.

## 5.2 Goal

Replace the single fixed allocation-count trigger with a low-risk adaptive controller.

Do **not** implement a moving or generational collector in this phase.

```rust
#[derive(Debug, Clone, Copy)]
pub struct GcPolicy {
    pub min_allocations: usize,
    pub min_pressure_bytes: usize,
    pub growth_factor_num: usize,
    pub growth_factor_den: usize,
    pub max_allocations: usize,
    pub min_reclaim_percent: u8,
}
```

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct GcControllerState {
    pub last_live_bytes: usize,
    pub last_tracked_runtime_bytes: usize,
    pub allocations_since_gc: usize,
    pub pressure_bytes_since_gc: usize,
    pub last_reclaim_percent: u8,
}
```

Candidate trigger:

```text
collect if ANY:
1. allocations_since_gc >= max_allocations
2. pressure_bytes_since_gc >= min_pressure_bytes
3. tracked_runtime_bytes > last_post_gc_bytes * growth_factor
```

Initial conservative values can be benchmark parameters, e.g.:

```text
min_allocations = 20_000
min_pressure_bytes = 16 MiB
growth_factor = 1.5x
max_allocations = 250_000
```

Tune only from actual workload data.

## 5.3 Post-GC adaptation

Compute reclaim ratio after each collection:

```text
reclaim_ratio =
(before_tracked - after_tracked) / max(before_tracked, 1)
```

Interpretation:

```text
high reclaim ratio -> collect sooner under repeated growth
low reclaim ratio  -> increase interval to avoid GC thrash
```

Do not dynamically pin the system at 10k.

## 5.4 Mutation-aware pressure

Track important capacity growth:

- Array elements / dense segments
- PropertyMap capacity
- Environment slots/bindings
- Promise reactions
- Job queue
- ArrayBuffer payload
- major runtime registries

J defines the API; K instruments most side registries.

## 5.5 Diagnostics

```text
gc_policy:
reason=<allocation|bytes|growth|manual>
tracked_before=<bytes>
tracked_after=<bytes>
heap_before=<bytes>
heap_after=<bytes>
pressure_since_last=<bytes>
reclaim_percent=<n>
next_soft_limit=<bytes>
pause_ns=<n>
```

## 5.6 Files owned by J

```text
src/runtime/gc.rs
src/runtime/heap.rs
src/runtime/memory.rs
tests/native_gc.rs
tests/runtime_memory.rs
```

Shared `src/runtime/context.rs` region:

```text
GC configuration
memory snapshot methods
GC trigger/controller
GC diagnostics
```

J must not modify Promise semantics or Host-script compilation.

## 5.7 Acceptance

Correctness:

```text
cargo gates PASS
native_gc PASS
adaptive/low-threshold collection has no missing-object error
```

Resource target:

```text
WSL: no MEMORY_LIMIT before deadline
jsdom: no MEMORY_LIMIT before deadline
```

Compare:

```text
10k
100k
1m
adaptive
```

Record:

```text
result
wall
peak RSS
collection count
GC pause
tracked runtime bytes
post-GC live bytes
```

---

# 6. Track K — Runtime side-registry lifetime and reclaim

## 6.1 Problem

`Heap` owns and sweeps:

```text
JsObject
Environment
JsFunction
```

But `NativeContext` also owns long-lived structures such as:

```text
Vec<PromiseRecord>
Vec<ArrayBufferRecord>
Vec<TypedArrayView>
Vec<DataViewRecord>
JobQueue
multiple HashMap side tables
```

JavaScript objects hold stable IDs into these structures:

```text
ObjectKind::Promise { promise: PromiseId }
ObjectKind::ArrayBuffer { buffer: ArrayBufferId }
ObjectKind::TypedArray { view: TypedArrayViewId, ... }
ObjectKind::DataView { view: DataViewId }
```

If those registries only grow and never recycle entries when their owning JS graph dies,
normal object GC cannot recover their payload/capacity.

This is a prime suspect for:

```text
small Heap::estimated_bytes
+
very large process RSS
```

## 6.2 Goal

Make side registries lifecycle-aware and reclaimable without changing JS-visible identity.

## 6.3 Stable arena contract

```rust
pub struct StableArena<T, I> {
    slots: Vec<Option<T>>,
    free: Vec<u32>,
    _id: PhantomData<I>,
}
```

Required operations:

```rust
pub fn allocate(&mut self, value: T) -> Option<I>;
pub fn get(&self, id: I) -> Option<&T>;
pub fn get_mut(&mut self, id: I) -> Option<&mut T>;
pub fn sweep_unmarked(&mut self, marks: &[bool]) -> SideSweepStats;
pub fn capacity_bytes(&self) -> usize;
```

Do not compact by renumbering live IDs.

Equivalent per-registry `Vec<Option<T>> + free` implementations are acceptable.

## 6.4 Side marks

```rust
#[derive(Debug, Default)]
pub struct SideMarks {
    pub promises: Vec<bool>,
    pub array_buffers: Vec<bool>,
    pub typed_array_views: Vec<bool>,
    pub data_views: Vec<bool>,
}
```

Tracing direction:

```text
live JsObject
    Promise     -> mark PromiseId
    ArrayBuffer -> mark ArrayBufferId
    TypedArray  -> mark TypedArrayViewId -> mark backing ArrayBufferId
    DataView    -> mark DataViewId -> mark backing ArrayBufferId

JobQueue / active jobs
    -> mark Promise IDs they carry

PromiseRecord
    -> mark result promises reachable through reactions
    -> trace callback/resolve/reject JsValues
```

The side graph must participate in the same collection boundary as normal heap GC.

## 6.5 Collection stats

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct SideCollectionStats {
    pub promises_before: usize,
    pub promises_after: usize,
    pub array_buffers_before: usize,
    pub array_buffers_after: usize,
    pub array_buffer_bytes_before: usize,
    pub array_buffer_bytes_after: usize,
    pub typed_array_views_before: usize,
    pub typed_array_views_after: usize,
    pub data_views_before: usize,
    pub data_views_after: usize,
}
```

J consumes these stats; K owns the sweep.

## 6.6 Capacity cleanup

After large reclamation:

```text
if capacity >= 4 * live_len
and retained capacity is materially large
then shrink to a bounded reasonable capacity
```

Do not `shrink_to_fit()` on every collection.

## 6.7 Promise reaction cleanup

Audit:

```text
PromiseRecord.reactions
PromiseCallbackJob
PromiseResolveThenableJob
```

Rules:

```text
queued/active reactions stay rooted
scheduled reactions are removed from the source Promise
completed jobs do not remain retained
```

## 6.8 ArrayBuffer / TypedArray accounting

Track:

```text
logical byte length
Vec capacity
number of views
retained payload bytes
```

RSS diagnostics should report payload capacity, not only logical length.

## 6.9 Files owned by K

```text
src/runtime/job.rs
src/runtime/object.rs            # side-ID trace hooks only
src/runtime/*buffer*             # actual binary-data runtime file(s)
src/builtins/promise.rs
src/builtins/binary_data.rs
tests/native_gc.rs
tests/runtime_side_gc.rs
```

Shared `context.rs` region:

```text
Promise registry accessors
ArrayBuffer/TypedArray/DataView registries
JobQueue lifecycle
side sweep hooks
side memory snapshot provider
```

K must not modify J's GC policy.

## 6.10 Acceptance

Required:

```text
dead Promise records recycled
dead ArrayBuffer payloads released
TypedArray/DataView metadata recycled
active jobs preserve required records
no use-after-free
low-threshold GC remains semantically correct
side memory visible in diagnostics
```

Target:

```text
WSL peak RSS reduction >= 25% or PASS below 1.5 GiB
jsdom peak RSS reduction >= 25% or PASS below 1.5 GiB
```

For threejs, at minimum produce before/after monotonic memory curves.

---

# 7. Track L — Persistent staged Host-script session

## 7.1 Problem

JetStream resources are intentionally staged:

```text
prelude eval
resource1 eval
resource2 eval
...
launch eval
```

This avoids source concatenation but does not fully reproduce the original shell's shared
function-like script scope.

`regexp` currently fails because of this mismatch.

`validatorjs` also has two staged files:

```text
validatorjs/dist/bundle.es6.min.js
validatorjs/benchmark.js
```

So validatorjs must be re-tested after Host-script semantics are fixed.

## 7.2 Goal

Provide one persistent Host-script session without concatenating workload source.

Preserve:

```text
shared var/function declarations
shared top-level lexical names as required
file execution order
later declaration visibility when combined-script hoisting requires it
direct eval correctness
```

## 7.3 New source mode

```rust
pub enum SourceKind {
    Script,
    Module,
    HostScriptFragment,
}
```

Only JetStream host-session code should use `HostScriptFragment`.

## 7.4 HostScriptSession

```rust
pub struct HostScriptSession {
    environment: EnvironmentId,
    fragments: Vec<PreparedHostFragment>,
    instantiated: bool,
}
```

```rust
pub struct PreparedHostFragment {
    pub path: String,
    pub chunk: SharedChunk,
    pub declarations: HostFragmentDeclarations,
}
```

```rust
#[derive(Debug, Default)]
pub struct HostFragmentDeclarations {
    pub var_names: Vec<JsString>,
    pub function_names: Vec<JsString>,
    pub lexical_names: Vec<JsString>,
}
```

## 7.5 Two-phase execution

```text
Phase A: prepare
    read each resource
    parse separately
    compile separately
    collect declaration metadata

Phase B: instantiate session
    create one persistent Host-script environment
    validate cross-fragment declaration conflicts
    instantiate var/function bindings for the shared scope

Phase C: execute
    execute prepared chunks in original file order
    all chunks use the same Host-script session
```

Do not concatenate resource text.

## 7.6 Hoisting requirement

Must handle cases like:

```js
// file A
foo();

// file B
function foo() {}
```

if the original combined function-body model would make `foo` visible before file B executes.

Therefore declaration instantiation must occur before fragment A execution.

## 7.7 Lexical conflicts

Detect at least:

```text
duplicate incompatible lexical declaration
var vs lexical conflict
function vs lexical conflict
```

Fail before workload execution when the equivalent combined scope would be invalid.

## 7.8 Upvalue / eval interaction

Conservative phase-4 rule:

```text
do not create cross-fragment Upvalue descriptors
```

Nested functions entirely inside one fragment may use normal Local/Upvalue optimization when safe.

Direct eval must retain the Phase 3 deoptimization rule.

## 7.9 Runtime API

```rust
impl Runtime {
    pub fn prepare_host_fragment(
        &mut self,
        source: &str,
        path: &str,
    ) -> Result<PreparedHostFragment, EvalFailure>;

    pub fn start_host_script_session(
        &mut self,
        fragments: &[PreparedHostFragment],
    ) -> Result<HostScriptSession, EvalFailure>;

    pub fn eval_host_fragment(
        &mut self,
        session: &mut HostScriptSession,
        fragment: &PreparedHostFragment,
    ) -> Result<ExecutionReport, EvalFailure>;
}
```

Exact ownership may differ, but the prepare / instantiate / execute phases must remain explicit.

## 7.10 Files owned by L

```text
src/main.rs
src/backend/mod.rs
src/bytecode/compiler.rs
src/engine.rs
tests/host_script_session.rs
scripts/prepare-jetstream2.mjs   # only if manifest metadata is needed
```

Minimal shared environment changes allowed in:

```text
src/runtime/environment.rs
src/runtime/context.rs
```

L must not modify GC policy or side-registry sweeping.

## 7.11 Acceptance

Tests:

```text
cross-file var visibility
cross-file function visibility
later function hoisting
cross-file assignment
lexical conflict rejection
direct eval
nested closure
multiple files with same var
session teardown
normal Runtime::eval unchanged
```

JetStream:

```text
regexp: ENGINE_FAILURE -> PASS
validatorjs: rerun after session fix
```

If validatorjs still fails:

1. capture exact exception/phase;
2. minimize to standalone JS;
3. classify the responsible builtin;
4. fix only after classification.

---

# 8. File ownership matrix

| File / area | J | K | L | Integrator |
|---|---:|---:|---:|---:|
| `runtime/gc.rs` | OWNER | side hook only | no | merge |
| `runtime/heap.rs` | OWNER | no | no | review |
| `runtime/memory.rs` | OWNER | provider hooks | no | exports |
| `runtime/job.rs` | no | OWNER | no | review |
| Promise registry | no | OWNER | no | review |
| ArrayBuffer/View registries | no | OWNER | no | review |
| `runtime/object.rs` | no | side-ID tracing only | no | merge |
| `runtime/context.rs` GC section | OWNER | no | no | merge |
| `runtime/context.rs` registry section | no | OWNER | no | merge |
| `runtime/context.rs` host-env section | no | no | OWNER | merge |
| `bytecode/compiler.rs` | no | no | OWNER | merge |
| `backend/mod.rs` | diagnostics read only | no | OWNER | merge |
| `engine.rs` | no | no | OWNER | merge |
| `main.rs` | no | no | OWNER | merge |
| `builtins/promise.rs` | no | OWNER | validator-only after classification | merge |
| `builtins/binary_data.rs` | no | OWNER | no | review |
| diagnostics PowerShell | memory fields | side fields | host fields | final owner |

Do not reformat entire shared files.

---

# 9. Shared diagnostics format

Memory:

```text
runtime_memory:
heap_estimated_bytes=...
tracked_runtime_bytes=...
promise_records=...
promise_reactions=...
job_queue_len=...
array_buffer_payload_bytes=...
typed_array_views=...
data_views=...
shape_count=...
property_ic_entries=...
```

GC:

```text
gc_policy:
reason=...
before=...
after=...
reclaim_percent=...
pressure_bytes=...
pause_ns=...
```

Host session:

```text
host_script:
phase=prepare|instantiate|execute
fragment=<index>
path=<path>
var_count=...
function_count=...
lexical_count=...
```

---

# 10. Merge order

Recommended final merge order:

```text
1. shared diagnostic/interface freeze
2. K runtime side-registry lifetime
3. J adaptive GC/accounting
4. L persistent Host-script session
5. integration/phase4
```

K and J still develop in parallel; K merges first because J's adaptive policy is more meaningful
once side memory is reclaimable.

After each merge:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

---

# 11. Required final integration matrix

## 11.1 JetStream 19 workloads

Configuration:

```text
release
1 iteration
32 MiB thread stack
1.5 GiB working-set protection
150 s external timeout
adaptive GC
```

Record:

```text
status
wall time
peak RSS
peak phase
GC collections
GC pause
tracked runtime bytes
side-registry stats
property-cache metrics
name/upvalue metrics
```

Intermediate gate:

```text
>= 15/19 PASS
```

Phase target:

```text
>= 17/19 PASS
```

Desired closeout:

```text
no MEMORY_LIMIT caused by known reclaimable runtime growth
regexp PASS
validatorjs classified and preferably PASS
```

## 11.2 GC matrix

For WSL + jsdom + threejs + ai-astar:

```text
10k
100k
1m
adaptive
```

## 11.3 Test262

Phase 3 final integration intentionally did not run full Test262.

Phase 4 should restore it.

Freeze:

```text
Test262 revision
command line
jobs
features
skip manifest
failure-path set
```

Compare path-level diffs, not only aggregate counts:

```text
baseline failures
Phase4 failures
new failures
new passes
unchanged failures
```

## 11.4 Stack/deadline smoke

```text
crypto: 8 / 16 / 32 MiB
richards: 1-second absolute deadline
```

---

# 12. Secondary backlog after Phase 4

## Set IC

Current evidence:

```text
Get hit rate strong
Set hits = 0
invalidations high
```

Before polymorphic IC:

```text
classify Set misses
classify invalidations
identify top structural mutation sites
```

Do not expand IC blindly.

## Block Slot

Re-profile:

```text
LoadName/StoreName
environment hops
block lexical frequency
```

Only implement if measurable.

## Generational GC

Only consider if:

```text
adaptive mark/sweep still spends excessive time
and
most allocations demonstrably die young
and
side registries are reclaimable
```

---

# 13. Definition of Done

## J

- [ ] runtime-wide memory stats
- [ ] byte-pressure accounting
- [ ] adaptive GC trigger
- [ ] trigger-reason diagnostics
- [ ] WSL adaptive run semantically correct
- [ ] materially less pause than fixed 10k
- [ ] no GC root regression

## K

- [ ] Promise records reclaim/reuse
- [ ] reactions released after scheduling
- [ ] ArrayBuffer payload reclaim
- [ ] TypedArray/DataView metadata reclaim
- [ ] active jobs preserve side records
- [ ] side memory visible in diagnostics
- [ ] WSL/jsdom RSS materially reduced

## L

- [ ] external staged files retained
- [ ] no source concatenation
- [ ] persistent host session
- [ ] cross-file declarations correct
- [ ] declaration conflicts checked
- [ ] later function hoisting covered
- [ ] direct eval remains safe
- [ ] regexp PASS
- [ ] validatorjs rerun and classified

## Integration

- [ ] all cargo gates PASS
- [ ] final full Test262 path-level comparison
- [ ] complete 19-workload matrix
- [ ] adaptive GC comparison
- [ ] no orphan processes
- [ ] final report committed
- [ ] every remaining failure has a concrete root cause

---

## Bottom line

The project is now in a closeout stage.

The next three tracks should focus on:

```text
J: make GC memory-aware
K: make runtime side memory reclaimable
L: make staged multi-file execution semantically faithful
```

Only after those are done should the team consider polymorphic IC, Block Slot, or a true
generational collector.
