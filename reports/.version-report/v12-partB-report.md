# Native V12 Part B Report

## Scope and Baseline

B owns IteratorClose, generator/async execution, Promise capabilities and job
queue behavior, iterable collection protocol, and module async execution. A
owns syntax/class/destructuring; C owns JS-visible builtin algorithms and
property shape.

Locked Fixbug6 full baseline: 34,640/53,379 passed, 18,737 failed, 2 skipped
(64.89%). This change did not run a new full suite, so it claims no full-suite
delta.

## Fix6 B Work Completed (2026-06-27)

### B1: Iterator and generator completion

- Terminal Iterator helpers cache `next` and share the VM getter/call path.
- Iterator records cache the iterator's `next` method at acquisition time.
- Generator `return`/`throw` completions re-enter suspended bytecode through
  existing exception handlers, including `yield*` delegate completion.
- Missing delegate `throw` errors now run surrounding `catch`/`finally` logic.

### B2: Promise queue and resolution

- `then` creates its result through the receiver's species constructor.
- `catch` and `finally` are generic and invoke the receiver's `then` method.
- `finally` waits for its callback result and preserves the original outcome.
- Promise callback jobs now use the shared resolution procedure, including
  thenable assimilation, native Promise adoption, and self-resolution checks.

### B3: Iterable collection bridge

- Added one VM sequence collector for already-created iterators; Promise and
  TypedArray paths now reuse VM iterator stepping instead of local loops.
- `TypedArray.from` validates `mapfn` before touching the source, caches `next`,
  constructs the result before mapping, preserves `thisArg`, and reports a
  short custom result as `TypeError`.
- `Array.from` remains unchanged because its focused gate is already 47/47.

### B4: Module async state skeleton

- Added `ModuleEvaluationState::{Pending, Fulfilled, Rejected}` to the module
  registry and exported it through runtime contracts.
- Native module evaluation records pending state, drains jobs before success,
  and records rejected state on execution or job-queue failure.
- JS-visible top-level `await` and dynamic `import()` remain blocked on A-owned
  parser/AST nodes; no duplicate parser workaround was added.

### B5: High-yield protocol continuation

- Replaced eager `Iterator.concat` with a lazy helper that validates iterator
  methods once, opens sources in order, forwards `return`, preserves thrown
  getter values, and rejects reentrant `next`/`return` calls.
- Added one lazy row state machine for `Iterator.zip` and `zipKeyed`, covering
  shortest/longest/strict modes, iterable/keyed padding, reverse IteratorClose,
  proxy-aware symbol access, symbol keys, and fresh result containers.
- Promise combinators no longer require a custom constructor capability to
  return a native Promise. Aggregate state calls the captured capability
  `resolve`/`reject` functions and shares per-element already-called guards.
- Split for-of natural exhaustion from `break` lowering. Break now reaches an
  empty-stack exit, performs IteratorClose, and then joins lexical cleanup.

## Files Changed

```text
src/vm/interpreter.rs
src/builtins/{mod,promise,collections,binary_data}.rs
src/bytecode/compiler.rs
src/runtime/{mod,module}.rs
src/backend/native.rs
src/contracts.rs
tests/{parser_iteration,native_promise,native_collections}.rs
tests/{native_typed_arrays,native_modules}.rs
reports/v12-partB-report.md
```

## Focused Test262 Results

```text
Iterator                              457 / 514  (88.91%)
Promise                               497 / 703  (70.70%)
Array/from                             47 / 47   (100.00%)
TypedArrayConstructors/from            52 / 59   (88.14%)
language/statements/for-of            597 / 751  (79.49%)
language/statements/for-await-of     1129 / 1234 (91.49%)
language/expressions/yield             51 / 63   (80.95%)
language/expressions/async-generator  373 / 623  (59.87%)
language/module-code                  199 / 599  (33.22%)
```

Direct deltas measured during this B continuation:

```text
Iterator                    361 -> 457 / 514 (+96)
Promise                     440 -> 497 / 703 (+57)
TypedArrayConstructors/from  46 ->  52 / 59  (+6)
yield                        33 ->  51 / 63  (+18)
for-of                      586 -> 597 / 751 (+11)
for-await-of               1127 ->1129 / 1234 (+2)
```

## Validation

Passed:

```text
cargo check --all-targets --no-default-features
cargo test --no-default-features --test parser_iteration     # 34/34
cargo test --no-default-features --test native_promise       # 19/19
cargo test --no-default-features --test native_collections   # 20/20
cargo test --no-default-features --test native_typed_arrays  # 37/37
cargo test --no-default-features --test native_modules       # 7/7
all focused release Test262 commands listed in the Fix6 B plan
git diff --check
```

`cargo test --release --no-default-features --all-targets` reached an unrelated
A-owned failure in
`parser_control_flow::compiles_multiple_var_declarators_from_frontend_contract`
(expected 2, actual 4). B-focused release tests passed before that failure.
Repository-wide formatting still includes merged A/C differences; B-touched
files were formatted directly with `rustfmt` to avoid widening this patch.
`cargo clippy --all-targets --no-default-features -- -D warnings` is also
blocked by 28 repository-wide warnings across frontend, builtins, compiler,
and VM code; the new B-specific collapsible-if warnings were cleaned up.

## Remaining Failures and Coordination

- Promise failures now cluster around iterator-close timing, Test262Error
  thrower/harness shape, subclass lowering, and exact async ordering. Generic
  custom-constructor capabilities are supported.
- Iterator sequencing is now nearly green: release totals include concat
  31/32, with zip/zipKeyed residuals dominated by basic stress files hitting
  project allocation/deadline limits. Remaining Iterator failures are mainly
  prototype descriptors, Iterator.from edges, and resource ceilings.
- Remaining for-of/for-await-of failures are primarily A-owned destructuring,
  parameter binding, and nested try/finally control-flow lowering. B-owned
  break closure and stack joins are installed.
- The seven remaining TypedArray.from cases are mostly C-owned conversion,
  immutable-buffer validation, and abrupt error-shape behavior.
- Remaining yield failures include A-owned grammar/template/parameter cases.
- Module-code remains parser/linker limited. B's state and queue substrate is
  ready for A's top-level-await and dynamic-import nodes.
- `--native-v12-scan` is documented but is not wired in `src/` yet; the local
  manifest alone is insufficient, so no 5,000-case scan result is claimed.

## Arena Free-List Allocation Optimization (2026-06-28)

### Problem and design

The native heap stored objects, lexical environments, and functions in three
`Vec<Option<T>>` arenas. Every allocation first counted all live arena entries
and then linearly searched the target arena for a vacant slot. This made both
arena growth with no holes and reuse after collection linear in the arena high
water mark, with quadratic cumulative scanning under sustained allocation.

This change adds one explicit `Vec<u32>` free-slot stack per arena. Sweep adds
each newly reclaimed index exactly once; allocation pops a vacant index in
constant time or appends directly when the free stack is empty. Live counts are
now derived as `arena.len() - free_slots.len()`, so the allocation-limit check
also avoids a hidden full-arena traversal. Stable `ObjectId`, `EnvironmentId`,
and `FunctionId` semantics and the public V12 contracts are unchanged.

### Correctness coverage

- All three arena kinds recycle reclaimed identifiers.
- Repeated sweeps do not duplicate free-list entries.
- Marked live slots are never registered as free.
- A live-allocation limit permits reuse after collection but still rejects a
  second simultaneous live allocation.
- The context-level GC test verifies that ID-keyed raw-JSON metadata is pruned
  before a reclaimed `ObjectId` is reused and that the replacement object does
  not retain the old object's properties.

### Release microbenchmark

Same-machine PowerShell `Measure-Command`, native release build, process-level
timings. The baseline used the immediately preceding binary and three samples;
the optimized build used five samples. Medians:

```text
workload                         baseline    free-list   speedup
100,000 numeric loop iterations   44.58 ms     44.69 ms    1.00x
100,000 two-property objects      695.12 ms    112.75 ms    6.17x
100,000 three-element arrays      657.72 ms     79.41 ms    8.28x
```

Both allocation scripts returned `99999`. The unchanged numeric control
supports attributing the delta to allocation rather than general VM or process
startup variation. These are focused microbenchmarks, not new SunSpider totals.

### Files changed

```text
src/runtime/heap.rs
tests/native_gc.rs
reports/.version-report/v12-partB-report.md
```

### Validation

Passed:

```text
rustfmt --edition 2024 src/runtime/heap.rs tests/native_gc.rs
cargo test --no-default-features runtime::heap::tests --lib       # 5/5
cargo test --no-default-features --test native_gc                 # 4/4
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262            # 15/15
cargo build --release --no-default-features
```

Repository-wide `cargo fmt --all -- --check` remains blocked by a pre-existing
formatting difference in `tests/native_functions.rs`. Repository-wide clippy
remains blocked by existing warnings across AST, backend, builtins, compiler,
parser, runtime object, and VM files; neither command reported a diagnostic in
the heap or GC files changed here.

No Test262 or SunSpider pass-rate delta is claimed. This optimization targets
allocation complexity; numeric/property-heavy timeouts such as nbody still
require separate VM fast-path work.

## Number and Bitwise Fast Paths (2026-06-28)

### Problem and design

The native VM previously sent ordinary `Number` operands through the generic
ECMAScript coercion path for every arithmetic, bitwise, shift, and relational
operation. That path is necessary for objects, strings, booleans, and `BigInt`,
but it adds repeated type dispatch, cloning, and conversion work to numeric
loops where both stack operands are already `JsValue::Number`.

This change adds direct `Number`/`Number` execution paths for arithmetic,
bitwise and shift instructions, relational comparisons, unary plus/negation,
and increment/decrement. Every non-number pair continues through the existing
generic coercion and `BigInt` path, preserving observable conversion order and
mixed-number errors. A shared slow-path helper removes duplication without
adding work to the common numeric branch.

While validating the bitwise path, the previous `ToInt32`/`ToUint32`
implementation was found to rely on saturating Rust float-to-integer casts.
It returned incorrect results for large finite values, for example `1e30 | 0`
was `-1` instead of `0`. The conversion now truncates and applies the
ECMAScript modulo-2^32 rule before signed reinterpretation.

### Correctness coverage

- IEEE-754 arithmetic behavior, including `NaN` and negative zero.
- Modulo-2^32 conversion at signed/unsigned boundaries and for `1e30`.
- Shift-count masking and unsigned right-shift results.
- Relational comparisons involving `NaN`, signed zero, and ordinary numbers.
- String addition/comparison, boolean coercion, and object `valueOf` fallback.
- `BigInt` arithmetic/bitwise behavior and mixed `Number`/`BigInt` errors.

### Release microbenchmark

Same-machine PowerShell `Measure-Command`, native release build, process-level
timings over 1,000,000 loop iterations. The baseline used the immediately
preceding binary and three samples; the optimized build used five samples.
Medians are shown below (one cold-start optimized addition sample was excluded
by the median naturally):

```text
workload                       baseline    fast path   speedup
numeric addition loop          358.86 ms    325.96 ms    1.10x
addition plus bitwise OR       418.98 ms    357.23 ms    1.17x
shift, XOR, and AND loop       475.28 ms    399.78 ms    1.19x
```

These results demonstrate a focused throughput gain in number-heavy loops;
they are not presented as new SunSpider totals.

### Focused conformance sampling

The release native runner completed the relevant Test262 operator directories
without crashes or timeouts:

```text
addition                 42/48
bitwise-and              29/30
bitwise-or               29/30
bitwise-xor              29/30
bitwise-not              13/16
left-shift               44/45
right-shift              36/37
unsigned-right-shift     44/45
less-than                38/45
```

No pass-count improvement is claimed because these focused runs do not have a
locked pre-change baseline; remaining failures include previously unsupported
language behavior outside the fast-path change.

### Files changed

```text
src/vm/interpreter.rs
tests/native_numeric_fast_paths.rs
reports/.version-report/v12-partB-report.md
```

### Validation

Passed:

```text
rustfmt --edition 2024 src/vm/interpreter.rs tests/native_numeric_fast_paths.rs
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262        # 15/15
cargo test --no-default-features --test native_numeric_fast_paths # 5/5
cargo test --no-default-features --test native_primitives     # 30/30
cargo test --no-default-features --test native_stdlib         # 32/32
cargo test --no-default-features --test native_compound_assignment # 5/5
cargo build --release --no-default-features
```

Repository-wide `cargo fmt --all -- --check` remains blocked only by the
pre-existing formatting difference in `tests/native_functions.rs`.
Repository-wide `cargo clippy --all-targets -- -D warnings` remains blocked by
existing warnings across AST, backend, builtins, compiler, parser, runtime
object, and VM code. The new fast-path helper and integration test introduced
no additional clippy diagnostic.
