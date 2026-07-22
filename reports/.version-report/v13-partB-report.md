# Native V13 Part B report

## Scope

Implemented only the V13-B-owned dynamic-import and class integration surface.
No Temporal, Array/TypedArray helper, URI, error-constructor, or scheduler files
were changed.

## Delivered

- Added `DynamicImportRequest`, `DynamicImportOutcome`, and `ModuleLoadState`
  to the native module contract.
- Added `Instruction::DynamicImport`; its compiler lowering evaluates the
  specifier and optional attributes expression exactly once and always returns a
  native `Promise`.
- The Test262 bridge supplies the current source path to the B local resolver.
  Dynamic import resolves relative fixture paths, compiles and links dependency
  modules, and settles through the normal Promise job queue. Each linked module
  now owns a persistent, GC-rooted `EnvironmentId` and stable namespace object;
  imports are initialized in the module environment rather than copied onto the
  global object. Namespace placeholders terminate dependency cycles.
- Added the shared `PrivateBrandId` / `PrivateSlot` representation and
  GC-rooted per-object private-slot storage to `NativeContext`. The existing
  class bytecode adapter remains in place until private accessors receive their
  dedicated opcode lowering; an attempted blanket redirect was rejected after
  focused scanning showed a 101-pass class regression.
- Retained and regression-tested class field source order, static field/static
  block order, and exactly-once computed field-key evaluation in B-owned class
  compiler/VM code.
- Derived constructors now track whether `super()` initialized `this`; an
  implicit primitive return without initialization throws `ReferenceError`.
  Synthesized default derived constructors forward all arguments through
  `super(...args)` before running instance field initializers.
- Class private-name early errors now use one class-wide namespace: static and
  instance declarations conflict correctly, and only one getter/setter pair may
  share a private name.
- Private field initialization has a distinct compiler marker, while ordinary
  private reads/writes validate that the receiver chain contains the private
  member before dispatching existing method/accessor descriptors.
- `super()` now performs a real Construct with the active `new.target` and uses
  the returned object to initialize derived `this`. This restores internal-slot
  construction for Array, Error, collection, buffer, and other builtin
  subclasses.
- A derived constructor that explicitly returns a non-object primitive now
  throws `TypeError`; an implicit/undefined return before `super()` continues to
  throw `ReferenceError`.
- Class heritage now treats `extends null` as a null prototype parent without
  reading `null.prototype`, validates that non-null heritage is constructable,
  and rejects non-object superclass prototypes.
- Synthesized default derived constructors forward their internal rest array
  directly, avoiding an observable `@@iterator` call.
- Async arrow concise bodies retain async parsing context until their expression
  is parsed, so `async () => await import(...)` lowers correctly.
- `await` is parsed at the UnaryExpression layer, enabling nested top-level
  forms such as `void await x`, `typeof await x`, and binary operands. Await now
  also enters the existing PromiseResolve path for non-Promise thenables.
- Replaced private-field hidden-property identity with a fresh runtime brand
  allocated for every evaluation of a class. Instance and static private fields
  are stored in exact-receiver private slots; equal `#x` spellings in distinct
  classes (including repeated evaluation of one class expression) no longer
  alias.
- Split the local dynamic-module path into graph loading, graph-wide
  declaration instantiation, and dependency-ordered evaluation. Registry state
  now distinguishes `Linking` and `Evaluating`, publishes environments and
  namespace identities before following cycles, and deduplicates dynamic
  self-import without recursive host-stack growth.
- Imported names are indirect environment-cell links rather than copied
  `JsValue`s. Export updates are therefore observed by importers, and cyclic
  graphs see TDZ cells created during instantiation. Circular indirect-binding
  and re-export resolution has explicit cycle detection.
- Module `var` declaration/load/store bytecode now targets the active module
  environment instead of the global environment. Default export expressions
  receive a stable synthetic module binding, restoring namespace values while
  preserving single evaluation.

## Focused Test262 scoreboard

Baseline and current results:

| Suite | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| `test/language/expressions/dynamic-import` | 336 / 1004 | 549 / 1004 | +213 |
| `test/language/module-code` | 351 / 599 | 392 / 599 | +41 |
| `test/language/statements/class` | 3622 / 4367 | 3794 / 4367 | +172 |
| `test/language/expressions/class` | 3473 / 4059 | 3562 / 4059 | +89 |
| `test/language/expressions/super` | 36 / 94 | 67 / 94 | +31 |

Focused private-member delete early-error subcluster:

| Suite | Current |
| --- | ---: |
| `test/language/expressions/class/elements/syntax/early-errors/delete` | 96 / 96 |

## Verification

```text
cargo fmt --all -- --check
cargo check --lib
cargo test --no-default-features --test native_modules
cargo test --no-default-features --test native_classes
cargo test --no-default-features --test parser_classes
cargo check --no-default-features --lib
cargo run --release --no-default-features -- test262 --root test262 --suite test/language/expressions/dynamic-import --jobs 4 --json reports/.native-test262-tmp/v13-b-dynamic-import-runtime-structures.json
cargo run --release --no-default-features -- test262 --root test262 --suite test/language/module-code --jobs 4 --json reports/.native-test262-tmp/v13-b-module-code-runtime-structures.json
cargo run --release --no-default-features -- test262 --root test262 --suite test/language/statements/class --jobs 4 --json reports/.native-test262-tmp/v13-b-statements-class-runtime-structures.json
cargo run --release --no-default-features -- test262 --root test262 --suite test/language/expressions/class --jobs 4 --json reports/.native-test262-tmp/v13-b-expressions-class-runtime-structures.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class/elements/syntax/early-errors/delete --jobs 4 --progress --json reports/native-v13-b-class-private-delete.json
```

All listed checks passed.

## Known V13-B limits

- Namespace properties are stable snapshots; complete live-binding propagation
  through namespace-property descriptors and non-relative host resolution
  remain deferred. Named imports themselves are live indirect cells.
- Private fields now use runtime class brands. Private methods/accessors still
  use the legacy NUL-prefixed adapter and need dedicated initialization opcodes
  before all private-element kinds share the same slot representation.
- Top-level await uses dependency-ordered synchronous job draining; fully
  suspended async module execution (including an async parent continuation
  graph) remains a later scheduler change.

## 85% correctness sprint follow-up (B track)

The sprint was rebased on a fresh full Test262 run rather than the stale
38,530-pass report. The before baseline is 41,069 / 53,379 (76.9385%) in
`reports/85-target/full-before.json`; `reports/full-test262-summary.json` holds
the current 41,253-pass result. Per-suite before/after summaries are in
`reports/85-target/`.

Delivered in this increment:

- Restored subclass construction for the abstract `Iterator` constructor while
  preserving the direct `new Iterator()` TypeError. The fallback prototype is
  selected from the new target's realm.
- Dynamic import rejection now preserves the original JavaScript thrown value
  from specifier coercion and module loading/evaluation instead of replacing it
  with the VM's callback wrapper error.
- Module evaluation failures are recorded as `Failed` / `Rejected` and are no
  longer silently converted into a fulfilled namespace.
- Module namespace objects now have null prototype, are non-extensible, expose
  `Symbol.toStringTag` as `"Module"`, and use the namespace export descriptor
  shape (`writable: true`, `enumerable: true`, `configurable: false`).
- Promise IsCallable checks now use the runtime callable protocol, including
  callable Proxy objects.

Focused score changes from this increment:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/Iterator` | 366 / 514 | 479 / 514 | +113 |
| `test/language/expressions/dynamic-import` | 549 / 1004 | 582 / 1004 | +33 |
| `test/language/module-code` | 392 / 599 | 392 / 599 | 0 |
| `test/built-ins/Promise` | 517 / 703 | 517 / 703 | 0 |
| `test/language/statements/for-await-of` | 1188 / 1234 | 1188 / 1234 | 0 |

The confirmed focused net increase is +146. The full fixed-version scan moved
from 41,069 to 41,253 passes (+184), with failures falling from 12,308 to
12,124 and skipped unchanged at 2. Iterator is back at the required 35-failure
ceiling. The remaining B target is not claimed as complete: full
async continuation scheduling, namespace exotic internal methods/live export
reads, and the dynamic import `source`/`defer` syntax owned by A remain open.

### Promise protocol follow-up

- Promise combinators now consume iterators incrementally instead of collecting
  the entire iterable before invoking `C.resolve`. Abrupt `resolve`/`then`
  completions close the original iterator, preventing infinite-iterator
  RuntimeLimit failures.
- Promise aggregate arrays use CreateDataProperty semantics and therefore do
  not invoke indexed setters inherited from `Array.prototype`.
- Abrupt result-capability resolution is routed through the capability reject
  function, and Promise resolve-element functions expose the required empty
  name.
- The native Test262 host now supplies `Test262Error.thrower`, which `sta.js`
  normally installs. The runner intentionally skips `sta.js`, so the missing
  host property had been turning valid Promise reject functions into
  `undefined`.

| Suite | Before follow-up | After follow-up | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/Promise` | 517 / 703 | 568 / 703 | +51 |
| `test/built-ins/Promise/all` | 82 / 98 | 98 / 98 | +16 |
| Full Test262 | 41,253 / 53,379 | 41,301 / 53,379 | +48 |

The cumulative full-suite result for this B sprint is now 41,069 -> 41,301
(+232), with 12,076 failures and 2 skipped tests.

### B0-B2 protocol-first follow-up

This pass reused 41,301 / 53,379 as its before baseline; no redundant full
before scan was run.

- `Test262Error` raised by a native host builtin now enters JavaScript as a
  catchable throw. Iterator getters and helper callbacks can therefore preserve
  abrupt completions instead of terminating the VM as a harness failure.
- Eager Iterator methods validate their receiver before inspecting callbacks or
  `next`, matching GetIteratorDirect ordering.
- Added shared `Promise.allKeyed` / `Promise.allSettledKeyed` machinery using
  OwnPropertyKeys, enumerable descriptors, symbol keys, null-prototype result
  objects, Promise capabilities, and shared reaction counters.
- Promise combinator iterator failures normalize native Type/Range/Reference/
  Syntax errors into real ECMAScript Error objects before rejection.
- `Promise.any` now constructs a real `AggregateError` with its ordered
  `errors` list rather than rejecting with an opaque native error value.
- Module export writes refresh the already-published namespace identity, so
  dynamic import consumers observe subsequent updates without replacing or
  recreating the namespace object.

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/Iterator` | 479 / 514 | 497 / 514 | +18 |
| `test/built-ins/Promise` | 568 / 703 | 679 / 703 | +111 |
| `test/language/expressions/dynamic-import` | 582 / 1004 | 600 / 1004 | +18 |
| Full Test262 | 41,301 / 53,379 | 41,476 / 53,379 | +175 |

Iterator now has 17 failures (target `<20`) and Promise has 24 failures
(target `<50`). Full failures are 11,901 with 2 skipped. The cumulative B
sprint result is 41,069 -> 41,476 (+407); relative to the original 40,854
analysis baseline it is +622.

### Shared async-entry follow-up

This pass reused the prior 41,476 / 53,379 JSON result as its before baseline;
no redundant full before scan was run. The JSON summary remains the source of
truth when the textual log differs by one result.

- Implemented `Array.fromAsync` as a Promise-returning async builtin with
  async-iterator, sync-iterator, and array-like entry paths. Abrupt completion
  after Promise creation rejects the returned Promise, mapper results are
  awaited, and iterable versus array-like constructor arity is preserved.
- Added the shared `%AsyncIteratorPrototype%` with conforming
  `Symbol.asyncIterator` and `Symbol.asyncDispose` methods. Async disposal gets
  and invokes `return` through the callable protocol, awaits its result, and
  preserves thrown/rejected reasons through the returned Promise.
- Corrected async-generator prototype layering so each generator prototype
  inherits through an async-generator prototype object to the shared
  `%AsyncIteratorPrototype%`.

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/Array/fromAsync` | 0 / 95 | 81 / 95 | +81 |
| `test/built-ins/AsyncIteratorPrototype` | 0 / 13 | 13 / 13 | +13 |
| `test/built-ins/Iterator` | 497 / 514 | 498 / 514 | +1 |
| `test/built-ins/Promise` | 679 / 703 | 679 / 703 | 0 |
| `test/language/expressions/dynamic-import` | 600 / 1004 | 600 / 1004 | 0 |
| `test/built-ins/AsyncDisposableStack` | 99 / 104 | 99 / 104 | 0 |
| Full Test262 | 41,476 / 53,379 | 41,565 / 53,379 | +89 |

The focused additions total +95, while the full fixed-version scan nets +89;
the six-result cross-suite difference is recorded as unresolved regression or
scan interaction rather than being offset by the focused gains. Full failures
are now 11,812 and skipped remains 2. Required sentinels stayed green:
Promise.allKeyed 32/32, Promise.allSettledKeyed 31/31, and Promise.allSettled
104/104.

The full after command was run exactly as requested:

```powershell
cargo run --release -- test262 --jobs 4 --progress --json reports/full-test262-summary.json 2>&1 | Out-File -FilePath output.txt -Encoding utf8
```

`cargo build --release --no-default-features` passed. `cargo check
--all-targets` remains blocked by pre-existing test-only initializers missing
`is_derived_constructor` and a token test passing `String` to a static-string
operator token; the release library and Test262 runner compile successfully.

### 85% foundation sprint, stage 1

This sprint uses the completed A-track full baseline as its only reference:
42,983 / 53,379 passed, 10,394 failed, 2 skipped (80.5242%). A fresh full scan
was completed before B changes; focused before summaries are stored under
`reports/85-next-b/`.

Delivered shared B mechanisms in this stage:

- Added an explicit `ToPropertyKey` bytecode operation and applied it to
  computed instance field names at class evaluation time. Key coercion and its
  abrupt completion no longer wait until construction, and happen exactly once.
- Public instance fields now use CreateDataProperty semantics through the Proxy
  internal define-own-property protocol. Inherited setters are not invoked,
  while proxy `defineProperty` observability and descriptor attributes remain
  intact.
- Static field initializers execute with the class constructor as `this` and as
  the initializer function's `[[HomeObject]]`, restoring lexical-arrow `this`
  capture and `super` lookup.
- Async function return completion now resolves/adopts returned native Promises
  and thenables instead of fulfilling with a Promise object. This fixes the
  shared nested-async path used by public and private class async methods.
- An attempted static-module reroute through the dynamic module graph loader was
  rejected and reverted after the complete module-code suite regressed from
  405 to 392 passes. No gain in another directory was used to offset it.

| Suite | Before | Stage 1 | Delta |
| --- | ---: | ---: | ---: |
| `test/language/statements/class` | 3883 / 4367 | 3905 / 4367 | +22 |
| `test/language/expressions/class` | 3616 / 4059 | 3639 / 4059 | +23 |
| `test/language/expressions/dynamic-import` | 600 / 1004 | 601 / 1004 | +1 |
| `test/language/module-code` | 405 / 599 | 405 / 599 | 0 |
| `test/language/eval-code` | 119 / 347 | 119 / 347 | 0 |

The confirmed focused net is +46 with no tracked B-suite regression. This is
below the +300 checkpoint, so no redundant post-change full scan was run yet.
The next stage remains private method/accessor brand installation followed by
module declaration instantiation and eval environment records; the +1,100 B
target is not claimed complete at this checkpoint.

### 85% foundation sprint, final B checkpoint

The user stopped further feature work at this checkpoint. The fixed baseline
was 42,983 / 53,379 passed, 10,394 failed, and 2 skipped.

- Completed class field initialization ordering, CreateDataProperty behavior,
  static initializer `this`/`super`, derived-constructor primitive-return
  validation, transparent parenthesized anonymous-function name inference, and
  the first branded private-method slot path.
- Added async-generator `yield*` support for async-from-sync and native async
  iterators, including awaited next/return/throw results and values.
- Split dynamic-module loading, instantiation, and evaluation; retained module
  environments, import indirections, namespace identity, cycles, and original
  evaluation exceptions. ImportCall now covers options trailing commas,
  `new import()` early errors, and source/defer phase syntax.
- Separated script/function-body declaration conflicts from block/module
  lexical conflicts, and separated function display names from the inner name
  binding used only by named function expressions. This restored default-export
  live updates; `dynamic-import/usage` is 108 / 108.
- The static-module reroute experiment remained reverted because it regressed
  the complete module-code suite. No focused gain was accepted as compensation.

| Suite | Before | Final | Delta |
| --- | ---: | ---: | ---: |
| `test/language/statements/class` | 3883 / 4367 | 4123 / 4367 | +240 |
| `test/language/expressions/class` | 3616 / 4059 | 3856 / 4059 | +240 |
| `test/language/expressions/dynamic-import` | 600 / 1004 | 867 / 1004 | +267 |
| `test/language/module-code` | 405 / 599 | 405 / 599 | 0 |
| `test/language/eval-code` | 119 / 347 | 120 / 347 | +1 |
| `test/built-ins/Promise` | 679 / 703 | 679 / 703 | 0 |
| Full Test262 | 42,983 / 53,379 | 43,961 / 53,379 | +978 |

Final full result: 43,961 passed, 9,416 failed, 2 skipped, 82.3564%.
The requested B stretch target of +1,100 was not claimed: this checkpoint is
+978, short by 122, because the user requested that implementation stop and
move to closeout. The JSON and textual output are stored in
`reports/full-test262-summary.json` and `output.txt`.
