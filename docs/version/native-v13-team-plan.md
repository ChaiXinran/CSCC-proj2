# Native V13 Team Plan

V13 is a three-person score sprint. The goal is to reach 80%+ Test262
conformance from the current 72.18% baseline by attacking the largest practical
failure clusters in parallel.

Shared contracts in `native-v13-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v13-score-contracts
codex/v13-a-temporal-score
codex/v13-b-module-class-score
codex/v13-c-array-typedarray-globals-score
codex/v13-integration
```

Recommended merge order:

```text
V13 contracts
  -> C missing globals first batch
  -> B dynamic import minimal path
  -> A Temporal PlainDate / Duration
  -> C ArraySpeciesCreate / TypedArray validation
  -> B class fields / static blocks / private basics
  -> A Instant / PlainDateTime / PlainTime
  -> C DisposableStack / AsyncDisposableStack
  -> A ZonedDateTime / intl402 Temporal bridge
  -> full Test262
  -> regression repair and reports
```

Reports:

- A updates `reports/version-history-v13-v18.md`.
- B updates `reports/version-history-v13-v18.md`.
- C updates `reports/version-history-v13-v18.md`.

Each report must include baseline score, current score, pass delta, failure
delta, commands run, known regressions, and intentionally partial behavior.

## 2. A Group - Temporal / Intl Temporal

Owned files:

```text
src/builtins/date_intl.rs
src/builtins/mod.rs
src/runtime/context.rs
src/runtime/object.rs
tests/native_temporal.rs
reports/version-history-v13-v18.md
```

Primary tasks:

- Implement or tighten ISO `Temporal.PlainDate`.
- Implement or tighten ISO `Temporal.Duration`.
- Implement or tighten `Temporal.Instant`.
- Implement or tighten `Temporal.PlainDateTime`.
- Implement or tighten `Temporal.PlainTime`.
- Implement or tighten `Temporal.PlainYearMonth`.
- Implement or tighten `Temporal.PlainMonthDay`.
- Add high-yield `Temporal.ZonedDateTime` paths.
- Add minimal intl402/Temporal bridge behavior for high-frequency tests.
- Preserve object shape, descriptors, RangeError/TypeError timing, and getter
  order for implemented paths.

V13-A may intentionally defer:

- full non-ISO calendar behavior;
- complete IANA time-zone semantics;
- full ICU/CLDR formatting;
- rare rounding modes not needed by focused score work.

A must not:

- make broad Intl402 changes outside Temporal;
- add object-model shortcuts that bypass shared runtime helpers;
- modify class/module lowering.

Focused commands:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress --json reports/native-v13-a-temporal-builtins.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402/Temporal --jobs 4 --progress --json reports/native-v13-a-temporal-intl402.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainDate --jobs 4 --progress --json reports/native-v13-a-plain-date.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainDateTime --jobs 4 --progress --json reports/native-v13-a-plain-date-time.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/Duration --jobs 4 --progress --json reports/native-v13-a-duration.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/Instant --jobs 4 --progress --json reports/native-v13-a-instant.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/ZonedDateTime --jobs 4 --progress --json reports/native-v13-a-zoned-date-time.json
```

Score target:

```text
minimum:   +2,400
preferred: +3,000
```

Required report:

- `reports/version-history-v13-v18.md`

## 3. B Group - Dynamic Import / Module / Class

Owned files:

```text
src/ast/
src/parser/
src/bytecode/compiler.rs
src/bytecode/opcode.rs
src/runtime/module.rs
src/backend/native.rs
src/vm/interpreter.rs
tests/native_modules.rs
tests/parser_iteration.rs
tests/native_classes.rs
tests/parser_classes.rs
reports/version-history-v13-v18.md
```

Primary tasks:

- Lower `import()` without unsupported compiler failure.
- Return a Promise-like result from dynamic import.
- Reject the Promise on load, parse, compile, or evaluation failure.
- Accept common import options/attributes enough to reduce early failures.
- Implement class fields in source order.
- Implement static blocks in source order.
- Implement private field and private method basics.
- Enforce derived constructor `super()` / `this` initialization timing.
- Preserve computed class key evaluation order.

V13-B may intentionally defer:

- complete host module resolution;
- complete module cycles and live-binding edge cases;
- decorators;
- advanced class async/generator combinations;
- cross-Realm class edge cases.

B must not:

- change Temporal builtins;
- change Array/TypedArray helpers except through agreed shared runtime helpers;
- interleave a broad VM scheduler rewrite with class/module work.

Focused commands:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/dynamic-import --jobs 4 --progress --json reports/native-v13-b-dynamic-import.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v13-b-module-code.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress --json reports/native-v13-b-statements-class.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress --json reports/native-v13-b-expressions-class.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/import.meta --jobs 4 --progress --json reports/native-v13-b-import-meta.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/super --jobs 4 --progress --json reports/native-v13-b-super.json
```

Score target:

```text
minimum:   +1,000
preferred: +1,300
```

Required report:

- `reports/version-history-v13-v18.md`

## 4. C Group - Array / TypedArray / Missing Globals

Owned files:

```text
src/builtins/array.rs
src/builtins/binary_data.rs
src/builtins/error.rs
src/builtins/std_primitives.rs
src/builtins/object.rs
src/runtime/context.rs
src/runtime/object.rs
src/runtime/property.rs
tests/native_array_methods.rs
tests/native_typed_arrays.rs
tests/native_errors.rs
tests/native_stdlib.rs
reports/version-history-v13-v18.md
```

Primary tasks:

- Add `AggregateError`.
- Add `SuppressedError`.
- Add URI encode/decode globals.
- Add minimal `DisposableStack`.
- Add minimal `AsyncDisposableStack`.
- Add shared `ArraySpeciesCreate`.
- Add shared typed-array validation and detached-buffer check helpers.
- Improve BigInt typed-array constructor paths when BigInt storage supports it.
- Improve stable sort and comparator abrupt completion.
- Improve sparse array, non-writable length, and descriptor behavior.

V13-C may intentionally defer:

- full explicit resource management edge behavior;
- complete async-disposal scheduling;
- SharedArrayBuffer/Atomics agent behavior;
- every Array/TypedArray method edge if the shared helper is not ready.

C must not:

- modify Temporal builtins;
- modify class/module lowering;
- add single-method Array fixes when a shared helper is required.

Focused commands:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/AggregateError --jobs 4 --progress --json reports/native-v13-c-aggregate-error.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/SuppressedError --jobs 4 --progress --json reports/native-v13-c-suppressed-error.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/encodeURI --jobs 4 --progress --json reports/native-v13-c-encode-uri.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/decodeURI --jobs 4 --progress --json reports/native-v13-c-decode-uri.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/encodeURIComponent --jobs 4 --progress --json reports/native-v13-c-encode-uri-component.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/decodeURIComponent --jobs 4 --progress --json reports/native-v13-c-decode-uri-component.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DisposableStack --jobs 4 --progress --json reports/native-v13-c-disposable-stack.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/AsyncDisposableStack --jobs 4 --progress --json reports/native-v13-c-async-disposable-stack.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress --json reports/native-v13-c-array.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v13-c-typed-array.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors --jobs 4 --progress --json reports/native-v13-c-typed-array-constructors.json
```

Score target:

```text
minimum:   +800
preferred: +1,000
```

Required report:

- `reports/version-history-v13-v18.md`

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `src/builtins/date_intl.rs` | A | V13 Temporal work only |
| `src/parser/`, `src/ast/` | B | Class/module syntax and early errors |
| `src/bytecode/compiler.rs` | B | Class/module lowering; A/C request changes through B |
| `src/runtime/module.rs` | B | Dynamic import and module state |
| `src/vm/interpreter.rs` | B first | C avoids broad VM edits during class/module work |
| `src/builtins/array.rs` | C | Array helpers and method wiring |
| `src/builtins/binary_data.rs` | C | TypedArray/DataView helpers |
| `src/builtins/error.rs` | C | Error-like constructors |
| `src/builtins/std_primitives.rs` | C with A review | URI/resource globals; do not edit Temporal here |
| `src/runtime/context.rs` | shared | Declare whether change serves A, B, or C |
| `reports/version-history-v13-v18.md` | A | A score report |
| `reports/version-history-v13-v18.md` | B | B score report |
| `reports/version-history-v13-v18.md` | C | C score report |

## 6. Ten-Day Sprint Schedule

Day 1:

- merge V13 docs;
- run focused baseline JSON for every group;
- each group classifies top 50 failures.

Days 2-4:

- A: PlainDate / Duration;
- B: dynamic import minimal Promise path;
- C: missing globals / URI;
- target cumulative gain: +1,500.

Day 5:

- focused rerun;
- regression repair;
- update V13 reports.

Days 6-8:

- A: Instant / PlainDateTime / ZonedDateTime;
- B: class fields / private basics / static blocks;
- C: ArraySpeciesCreate / TypedArray validation / DisposableStack;
- target cumulative gain: +3,800.

Day 9:

- full Test262 run;
- generate `reports/full-test262-summary-v13.json`;
- identify regressions.

Day 10:

- regression repair;
- update reports, README, and final evidence;
- target cumulative gain: +4,800 gross and >= 80.00% net.

## 7. Integration Gate

Before claiming V13 score:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/full-test262-summary-v13.json
```

If any gate is blocked by pre-existing repository-wide issues, record the exact
blocker in the relevant V13 report. Do not claim the full score from focused
suite totals alone.
