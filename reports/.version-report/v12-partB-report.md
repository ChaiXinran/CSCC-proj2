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
