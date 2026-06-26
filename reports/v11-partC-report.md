# V11 Part C Report — RegExp / Annex B / Descriptor Builtins

Owner: C group
Scope: builtins / RegExp / Annex B / descriptor sweep

This report must be updated by any worker or AI agent that changes V11-C code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v11-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V11 hotspots.

| Metric | Baseline |
| --- | ---: |
| V11 scan total | 5,000 |
| V11 scan passed | 1,247 |
| V11 scan failed | 3,753 |
| V11 scan skipped | 0 |

Primary directories:

- `test/built-ins/RegExp`
- RegExp-facing `String.prototype` methods
- `test/annexB`
- Object/Function/Array/String/Iterator descriptor sweep cases

## Current Status

Status: basic V11-C functionality plus Object/Reflect descriptor bridge,
ArrayBuffer/DataView/TypedArray storage, JS-visible Array/TypedArray
iterator objects, and Array callback-method generic/hole semantics
implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-C report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partC-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-25 | Codex | Implemented basic RegExp refinement pass, Annex B legacy globals/accessors, and focused descriptor shape coverage for V11-C | `src/builtins/mod.rs`, `src/builtins/v11.rs`, `tests/native_v11_builtins.rs`, `reports/v11-partC-report.md` | `rustfmt --edition 2024 src/builtins/v11.rs tests/native_v11_builtins.rs`; `cargo test --no-default-features --test native_v11_builtins`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_test262`; `cargo test --no-default-features --test native_v8_builtins`; `cargo test --no-default-features --test native_v9_builtins`; `cargo test --no-default-features --test native_v10_builtins`; `cargo test --no-default-features --test native_v6_builtins string_search_methods` | focused local gates pass; V11 scan not rerun in this basic-functionality pass |
| 2026-06-25 | Codex / C group | Updated JS-visible `Object.defineProperty` and `Reflect.*` symbol/receiver behavior to reuse B-owned descriptor helpers instead of builtin-local patches | `src/builtins/object.rs`, `src/builtins/std_primitives.rs`, `src/runtime/context.rs`, `src/vm/interpreter.rs`, `tests/native_object_keys.rs`, `reports/native-v11-b-object-summary.json`, `reports/native-v11-b-reflect-summary.json`, `reports/v11-partC-report.md` | `cargo fmt --all -- --check`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_object_keys`; `cargo test --no-default-features --test native_regexp`; `cargo test --no-default-features --test native_test262`; Object/Reflect focused Test262 scans | `native_regexp`: 6/6 remains passing; Object scan: 2700/3411 pass; Reflect scan: 118/153 pass, up from 111/153 during this pass |
| 2026-06-26 | Codex / C group | Wired ArrayBuffer/DataView/TypedArray to runtime byte storage, added live typed-array indexed properties and many prototype methods, implemented basic resizable/transfer/immutable ArrayBuffer operations, fixed computed symbol method calls, and repaired `in`/`instanceof` precedence | `src/builtins/binary_data.rs`, `src/runtime/buffer.rs`, `src/runtime/context.rs`, `src/runtime/object.rs`, `src/builtins/object.rs`, `src/builtins/json.rs`, `src/builtins/annex_b.rs`, `src/builtins/std_primitives.rs`, `src/bytecode/compiler.rs`, `src/bytecode/opcode.rs`, `src/vm/interpreter.rs`, `src/parser/expression.rs`, `tests/native_typed_arrays.rs`, C focused summary JSON/log files | `rustfmt --edition 2024` on touched Rust files; `cargo check --no-default-features --all-targets`; C/native regression tests; focused ArrayBuffer/DataView/TypedArray Test262 scans | ArrayBuffer focused scan: 170/221 pass (76.92%), up from 126/221. DataView focused scan: 297/561 pass (52.94%), up from 296/561. TypedArray focused scan: 651/1446 pass (45.02%), up from 350/1446. Full V11 scan not rerun after this pass. |
| 2026-06-26 | Codex / C group | Replaced Array/TypedArray iterator snapshot arrays with JS-visible iterator objects, wired `Iterator.prototype.next` to runtime iterator records, and added length-tracking/OOB validation for resizable TypedArray views | `src/builtins/array.rs`, `src/builtins/binary_data.rs`, `src/builtins/collections.rs`, `src/runtime/buffer.rs`, `src/runtime/context.rs`, `src/runtime/iterator.rs`, `src/runtime/mod.rs`, `tests/native_typed_arrays.rs`, focused C summary JSON files | `rustfmt --edition 2024` on touched Rust files; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_typed_arrays`; `cargo test --no-default-features --test native_binary_data`; `cargo test --no-default-features --test native_collections`; `cargo test --no-default-features --test native_iteration`; focused Array/TypedArray iterator scans; focused ArrayBuffer/DataView/TypedArray scans | `Array/prototype/values`: 9/12 pass, up from 7/12. `TypedArray/prototype/values`: 15/21, up from 10/21. `keys`: 13/19, up from 9/19. `entries`: 13/19, up from 10/19. Full TypedArray focused scan: 706/1446 pass (48.82%), up from 651/1446. ArrayBuffer/DataView focused scans remained 170/221 and 297/561. |
| 2026-06-26 | Codex / C group | Updated Array callback methods to use ToObject/ToLength, skip holes with HasProperty, read inherited elements, preserve sparse map results, and support primitive string receivers | `src/builtins/array.rs`, `tests/native_array_methods.rs`, focused Array summary JSON files, `reports/v11-partC-report.md` | `rustfmt --edition 2024 src/builtins/array.rs tests/native_array_methods.rs`; `cargo check --no-default-features --all-targets`; `cargo test --no-default-features --test native_array_methods`; `cargo test --no-default-features --test native_typed_arrays`; `cargo test --no-default-features --test native_iteration`; `cargo test --no-default-features --test native_collections`; `cargo test --no-default-features --test native_object_keys`; `cargo test --no-default-features --test native_test262`; focused Array callback scans; focused Array scan | `Array/prototype/map`: 171/216 pass, failed count down from 66 to 45. `filter`: 206/242, down from 66 to 36. `reduce`: 228/260, down from 71 to 32. `reduceRight`: 226/260, down from 73 to 34. `forEach`: 166/190, down from 41 to 24. `every`: 194/218, down from 42 to 24. `some`: 196/219, down from 41 to 23. Full Array focused scan: 2169/3081 pass (70.40%), up from 1987/3081. |

## Implemented Functionality

- Added `v11` builtin installation after V6/V8/V9/V10 setup.
- Replaced the JS-visible `RegExp` constructor after V6 installation so V11-C
  can validate flags, preserve the existing `RegExp.prototype`, and install
  `RegExp.escape`.
- Refined `RegExp.prototype.exec`, `test`, `compile`, `toString`, and
  prototype getters for `source`, `flags`, `global`, `ignoreCase`,
  `multiline`, `dotAll`, `sticky`, `unicode`, `unicodeSets`, and
  `hasIndices`.
- Added basic `lastIndex` handling for global/sticky `exec`/`test` and
  post-V6 Symbol dispatch for `@@match`, `@@matchAll`, `@@search`, and
  `@@split`.
- Added Annex B legacy globals `escape` and `unescape`.
- Added Annex B `Object.prototype.__defineGetter__`,
  `__defineSetter__`, `__lookupGetter__`, `__lookupSetter__`, and
  `__proto__` accessor behavior.
- Added `String.prototype.trimLeft` and `trimRight` aliases.
- `Object.defineProperty` with symbol keys now uses the shared descriptor
  validation path, so invalid redefinitions throw instead of silently
  overwriting non-configurable symbol properties.
- `Reflect.defineProperty`, `Reflect.getOwnPropertyDescriptor`,
  `Reflect.deleteProperty`, and `Reflect.has` now expose symbol-key descriptor
  semantics from the B-owned object model helpers.
- `Reflect.get` and `Reflect.set` now honor the explicit receiver argument for
  string and symbol keys, including accessor `this` binding and receiver-side
  writes.
- ArrayBuffer objects now use runtime byte storage for `byteLength`,
  `detached`, `slice`, Test262 `$262.detachArrayBuffer`, basic constructor
  `maxByteLength` options, `resize`, `transfer`, `transferToFixedLength`,
  `sliceToImmutable`, and `transferToImmutable`.
- DataView objects now share ArrayBuffer storage and implement numeric
  get/set methods with endian handling. `new DataView(buffer, offset,
  undefined)` now uses the remaining buffer length.
- TypedArray objects now use live runtime views for indexed properties,
  construction from lengths, ArrayBuffers, array-like sources, and simple
  iterable sources. Common static/prototype operations are implemented for
  numeric typed arrays, including `from`, `of`, `at`, iterators, `join`,
  `fill`, search methods, callbacks, `map`, `filter`, reductions, `set`,
  `slice`, `subarray`, `sort`, `toReversed`, `toSorted`, and `with`.
- `Array.prototype.keys`, `values`, and `entries` now return JS-visible
  iterator objects instead of snapshot arrays. `TypedArray.prototype.keys`,
  `values`, and `entries` use the same runtime iterator records and share
  `Iterator.prototype.next` / `@@iterator` behavior with Array iterators.
- `Array.prototype.map`, `filter`, `forEach`, `every`, `some`, `reduce`,
  and `reduceRight` now use ToObject/ToLength array-like setup, HasProperty
  element checks, inherited element reads, primitive string receiver support,
  and sparse result preservation for `map`.
- Runtime iterator records now carry key/value/entry mode, can be consumed by
  JS `.next()` and `for...of`, and keep Array iteration live across mutations.
- Resizable ArrayBuffer-backed TypedArray views now track current length for
  length-tracking views, report `0` length/byteLength/byteOffset when fixed
  views are out of bounds, and validate detached/OOB views at TypedArray
  method entry.
- JSON/Object key enumeration now sees virtual TypedArray indices through
  `NativeContext::get_own_property_descriptor`.
- VM/bytecode now preserve `this` for computed method calls such as
  `array[Symbol.iterator]()`, and computed `delete` / `in` handle symbol keys.
- Parser keyword binary precedence for `in` and `instanceof` now matches the
  relational precedence table, fixing cases such as
  `value instanceof Type !== true`.

## Test Results and Delta Analysis

Initial V11 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Record focused C commands and future `--native-v11-scan` deltas here.

Current locked full V11 scan summary:

- `reports/native-v11-scan-summary.json`: 5,000 total, 1,247 passed, 3,753
  failed, 0 skipped, 24.94% conformance, 1,022,268 ms.

Setup validation:

- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- Initial `--native-v11-scan`: timed out after 300s in local tool execution;
  baseline pass/fail totals remain pending.

Focused local validation for this pass:

```text
rustfmt --edition 2024 src/builtins/v11.rs tests/native_v11_builtins.rs
cargo test --no-default-features --test native_v11_builtins
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test native_v8_builtins
cargo test --no-default-features --test native_v9_builtins
cargo test --no-default-features --test native_v10_builtins
cargo test --no-default-features --test native_v6_builtins string_search_methods
```

Results:

- `native_v11_builtins`: 6 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- `native_v8_builtins`: 10 passed, 0 failed.
- `native_v9_builtins`: 9 passed, 0 failed.
- `native_v10_builtins`: 7 passed, 0 failed.
- `native_v6_builtins string_search_methods`: 1 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.

`cargo test --no-default-features --test native_v6_builtins` was also tried;
it reported 25 passed and 1 failed. The failure is the existing BigInt literal
smoke case (`1n + 2`) and is outside V11-C RegExp/Annex B scope.

The focused Test262 RegExp/Annex B commands and `--native-v11-scan` were not
rerun in this step. This pass stops at the requested basic functionality point
instead of using large-suite failures to drive a broader semantic expansion.

Object/Reflect descriptor bridge validation:

```text
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_regexp
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-b-object-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --progress --json reports/native-v11-b-reflect-summary.json
```

Results:

- `cargo fmt --all -- --check`: passed.
- `cargo check --no-default-features --all-targets`: passed.
- `native_object_keys`: 7 passed, 0 failed.
- `native_regexp`: 6 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- Object focused scan: 3,411 total, 2,700 passed, 711 failed, 0 skipped,
  79.1557% conformance.
- Reflect focused scan: 153 total, 118 passed, 35 failed, 0 skipped,
  77.1242% conformance. The explicit receiver change accounts for a local
  improvement from 111/153 to 118/153 during this pass.
- Full `--native-v11-scan` was not rerun in this pass because prior local
  attempts against the locked manifest timed out before producing JSON.

ArrayBuffer/DataView/TypedArray storage pass validation:

```text
rustfmt --edition 2024 src/builtins/annex_b.rs src/builtins/binary_data.rs src/builtins/json.rs src/builtins/object.rs src/builtins/std_primitives.rs src/runtime/buffer.rs src/runtime/context.rs src/runtime/object.rs src/parser/expression.rs src/bytecode/opcode.rs src/bytecode/compiler.rs src/vm/interpreter.rs tests/native_typed_arrays.rs
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_binary_data
cargo test --no-default-features --test native_symbol
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test native_json
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_regexp
cargo test --no-default-features --test native_collections
cargo test --no-default-features parser::expression::tests::in_and_instanceof_bind_at_relational_precedence
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --json reports/native-v11-c-arraybuffer-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --json reports/native-v11-c-dataview-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --json reports/native-v11-c-typedarray-summary.json
```

Results:

- `cargo check --no-default-features --all-targets`: passed.
- `native_typed_arrays`: 16 passed, 0 failed.
- `native_binary_data`: 6 passed, 0 failed.
- `native_symbol`: 28 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- `native_json`: 7 passed, 0 failed.
- `native_object_keys`: 7 passed, 0 failed.
- `native_regexp`: 6 passed, 0 failed.
- `native_collections`: 9 passed, 0 failed.
- Parser precedence filter: 1 passed, 0 failed.
- ArrayBuffer focused scan: 221 total, 170 passed, 51 failed, 0 skipped,
  76.9231% conformance.
- DataView focused scan: 561 total, 297 passed, 264 failed, 0 skipped,
  52.9412% conformance.
- TypedArray focused scan: 1,446 total, 651 passed, 795 failed, 0 skipped,
  45.0207% conformance.
- Test262 focused commands exit with status 1 while failed cases remain; the
  JSON summaries above were still written successfully.
- Full `--native-v11-scan` was not rerun after this pass because the last
  locked full scan took about 17 minutes locally.

TypedArray iterator/RAB validation:

```text
rustfmt --edition 2024 src/runtime/buffer.rs src/runtime/context.rs src/builtins/collections.rs src/builtins/array.rs src/builtins/binary_data.rs tests/native_typed_arrays.rs
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_binary_data
cargo test --no-default-features --test native_collections
cargo test --no-default-features --test native_iteration
cargo check --no-default-features --all-targets
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/values --jobs 4 --json reports/native-v11-c-array-values-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray/prototype/values --jobs 4 --json reports/native-v11-c-typedarray-values-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray/prototype/keys --jobs 4 --json reports/native-v11-c-typedarray-keys-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray/prototype/entries --jobs 4 --json reports/native-v11-c-typedarray-entries-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --json reports/native-v11-c-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --json reports/native-v11-c-arraybuffer-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --json reports/native-v11-c-dataview-summary.json
```

Results:

- `native_typed_arrays`: 22 passed, 0 failed.
- `native_binary_data`: 6 passed, 0 failed.
- `native_collections`: 9 passed, 0 failed.
- `native_iteration`: 5 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- `Array/prototype/values`: 12 total, 9 passed, 3 failed, 0 skipped,
  75.0000% conformance.
- `TypedArray/prototype/values`: 21 total, 15 passed, 6 failed, 0 skipped,
  71.4286% conformance.
- `TypedArray/prototype/keys`: 19 total, 13 passed, 6 failed, 0 skipped,
  68.4211% conformance.
- `TypedArray/prototype/entries`: 19 total, 13 passed, 6 failed, 0 skipped,
  68.4211% conformance.
- TypedArray focused scan: 1,446 total, 706 passed, 740 failed, 0 skipped,
  48.8243% conformance.
- ArrayBuffer focused scan remained 221 total, 170 passed, 51 failed, 0
  skipped, 76.9231% conformance.
- DataView focused scan remained 561 total, 297 passed, 264 failed, 0
  skipped, 52.9412% conformance.
- Full `--native-v11-scan` was not rerun in this pass; the last locked full
  scan took about 17 minutes locally.

Array callback-method validation:

```text
rustfmt --edition 2024 src/builtins/array.rs tests/native_array_methods.rs
cargo test --no-default-features --test native_array_methods
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_iteration
cargo test --no-default-features --test native_collections
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/map --jobs 4 --json reports/native-v11-c-array-map-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/filter --jobs 4 --json reports/native-v11-c-array-filter-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/reduce --jobs 4 --json reports/native-v11-c-array-reduce-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/reduceRight --jobs 4 --json reports/native-v11-c-array-reduce-right-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/forEach --jobs 4 --json reports/native-v11-c-array-for-each-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/every --jobs 4 --json reports/native-v11-c-array-every-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype/some --jobs 4 --json reports/native-v11-c-array-some-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --json reports/native-v11-c-array-summary.json
```

Results:

- `native_array_methods`: 5 passed, 0 failed.
- `native_typed_arrays`: 22 passed, 0 failed.
- `native_iteration`: 5 passed, 0 failed.
- `native_collections`: 9 passed, 0 failed.
- `native_object_keys`: 7 passed, 0 failed.
- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- `Array/prototype/map`: 216 total, 171 passed, 45 failed, 0 skipped,
  79.1667% conformance.
- `Array/prototype/filter`: 242 total, 206 passed, 36 failed, 0 skipped,
  85.1240% conformance.
- `Array/prototype/reduce`: 260 total, 228 passed, 32 failed, 0 skipped,
  87.6923% conformance.
- `Array/prototype/reduceRight`: 260 total, 226 passed, 34 failed, 0
  skipped, 86.9231% conformance.
- `Array/prototype/forEach`: 190 total, 166 passed, 24 failed, 0 skipped,
  87.3684% conformance.
- `Array/prototype/every`: 218 total, 194 passed, 24 failed, 0 skipped,
  88.9908% conformance.
- `Array/prototype/some`: 219 total, 196 passed, 23 failed, 0 skipped,
  89.4977% conformance.
- Array focused scan: 3,081 total, 2,169 passed, 912 failed, 0 skipped,
  70.3992% conformance.
- Full `--native-v11-scan` was not rerun in this pass.

Array/Object/TypedArray/DataView/RegExp four-stage sprint:

Implemented in one C-part sprint:

- Stage 1 Object/Reflect:
  - Added an `[[Extensible]]` bit to native objects and wired it through
    `Object.preventExtensions`, `Object.seal`, `Object.freeze`,
    `Object.isExtensible`, `Object.isSealed`, `Reflect.preventExtensions`,
    `Reflect.isExtensible`, `Reflect.defineProperty`, property `Set`, and
    `Reflect.setPrototypeOf`.
  - Made array own-key enumeration expose non-enumerable `length`; made string
    wrapper objects synthesize indexed own properties and `length`.
  - Made `Object.keys`, `Object.values`, `Object.entries`,
    `Object.getOwnPropertyNames`, `Object.getOwnPropertyDescriptor`,
    `Object.getPrototypeOf`, and `Object.assign` perform ToObject where
    required; `Object.assign` now uses strict Set and copies enumerable symbols.
  - Installed `Object.hasOwn`, `Object.seal`, `Object.preventExtensions`, and
    `Object.isSealed`.
- Stage 2 Array:
  - Added `Array.prototype.findLast` and `findLastIndex`.
  - Reworked `find/findIndex/findLast/findLastIndex`, `slice`, `indexOf`,
    `lastIndexOf`, and `includes` around generic ToObject/LengthOfArrayLike
    semantics, primitive string receivers, sparse holes, inherited properties,
    SameValueZero, and correct fromIndex handling.
  - `slice` now creates own result data properties instead of being blocked by
    inherited non-writable Array.prototype indices.
- Stage 3 TypedArray/DataView:
  - `TypedArray.prototype.set` now ToObjects primitive array-like sources.
  - DataView `byteLength`/`byteOffset` accessors reject detached buffers, while
    `buffer` remains readable; DataView get/set now check detached buffers
    before range checks.
- Stage 4 RegExp:
  - `RegExp.prototype.flags` is now generic and reads flag properties in spec
    order (`hasIndices`, `global`, `ignoreCase`, `multiline`, `dotAll`,
    `unicode`, `unicodeSets`, `sticky`).
  - `RegExp.prototype.source` and boolean flag getters handle
    `RegExp.prototype` special cases.

Validation:

```text
rustfmt --edition 2024 src/builtins/array.rs src/builtins/object.rs src/builtins/binary_data.rs src/builtins/annex_b.rs src/builtins/std_primitives.rs src/runtime/object.rs src/runtime/context.rs tests/native_array_methods.rs tests/native_object_keys.rs tests/native_typed_arrays.rs tests/native_regexp.rs
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_array_methods
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_objects
cargo test --no-default-features --test native_objects_runtime
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_binary_data
cargo test --no-default-features --test native_regexp
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --json reports/native-v11-c-array-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --json reports/native-v11-c-object-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --json reports/native-v11-c-reflect-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --json reports/native-v11-c-typedarray-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --json reports/native-v11-c-arraybuffer-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --json reports/native-v11-c-dataview-summary.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/RegExp/prototype --jobs 4 --json reports/native-v11-c-regexp-prototype-summary.json
```

Results after this sprint:

- Array focused scan: 3,081 total, 2,217 passed, 864 failed, 0 skipped,
  71.9572% conformance. The peak local Array run during the slice pass was
  2,218 passed; the final full Array rerun was 2,217, while directly touched
  sub-suites stayed stable.
- Object focused scan: 3,411 total, 2,981 passed, 430 failed, 0 skipped,
  87.3937% conformance.
- Reflect focused scan: 153 total, 129 passed, 24 failed, 0 skipped,
  84.3137% conformance.
- TypedArray focused scan: 1,446 total, 707 passed, 739 failed, 0 skipped,
  48.8935% conformance.
- ArrayBuffer focused scan remained 221 total, 170 passed, 51 failed, 0
  skipped, 76.9231% conformance.
- DataView focused scan: 561 total, 328 passed, 233 failed, 0 skipped,
  58.4670% conformance.
- RegExp/prototype focused scan: 487 total, 268 passed, 219 failed, 0 skipped,
  55.0308% conformance.
- RegExp/prototype/flags: 16 total, 14 passed, 2 failed, 0 skipped,
  87.5000% conformance.
- Full `--native-v11-scan` was not rerun in this pass; the locked full scan is
  still treated as a long-running final confirmation step.

## Open Risks / Coordination Notes

- Coordinate with B before descriptor sweep changes depend on object-model
  behavior.
- Annex B coverage is intentionally partial and isolated to builtins that can
  be implemented without runtime object-model changes.
- RegExp matching still uses the existing Rust regex helper, so full ECMAScript
  RegExp semantics, named groups, indices arrays, unicode set behavior, and
  complete sticky/global edge cases remain future work.
- Descriptor sweep coverage is limited to newly installed/refined builtin
  entry points; broader Object/Function/Array/String/Iterator descriptor and
  property-order precision remains coordinated with V11-B.
- Object/Reflect descriptor builtins now depend on B-owned symbol and receiver
  helpers. Future C descriptor sweep work should extend those helpers when the
  missing behavior is shared, and only patch builtin-local metadata when the
  issue is genuinely JS-visible shape data.
- SharedArrayBuffer, BigInt typed arrays, BigInt DataView methods, full
  species-constructor behavior, cross-realm prototype selection, and complete
  resizable/length-tracking TypedArray semantics remain open.
- TypedArray iterator `next()` errors reached through bytecode `for...of`
  still surface as VM execution errors instead of JS-catchable exceptions in
  some shrink-mid-iteration Test262 cases. Fixing that likely belongs in the
  VM abrupt-completion path rather than in binary-data builtins alone.
- The current ArrayBuffer resize/transfer implementation is intentionally
  basic: it supports byte storage, max length, detach, and immutable marking,
  but not the full growable view invalidation semantics required by every RAB
  Test262 case.
