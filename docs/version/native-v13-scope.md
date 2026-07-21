# Native V13 Scope: 80 Percent Test262 Score Sprint

Native V13 is a score sprint targeting Test262 80%+. It is intentionally more
aggressive than the previous repair batches: the goal is a large measurable
pass-count increase, not a slow completeness sweep.

Baseline from `full-test262-analysis-zh.md`:

```text
total=53,379
passed=38,530
failed=14,847
skipped=2
conformance=72.18%
elapsed=454.22s
```

80% requires at least:

```text
ceil(53,379 * 0.80) = 42,704 passed
42,704 - 38,530 = 4,174 additional net passes
```

V13 target:

```text
hard target:        >= 42,704 / 53,379 (>= 80.00%)
preferred target:   >= 43,300 / 53,379 (>= 81.11%)
engineering target: +4,800 focused passes to absorb regressions
```

Shared contracts are defined in
[Native V13 Shared Interface](native-v13-interface.md), and file ownership is
defined in [Native V13 Team Plan](native-v13-team-plan.md).

## 1. Strategy

V13 prioritizes high-yield clusters over perfect long-term layering.

The largest failure pools from the full analysis are:

| Cluster | Current failures | Theoretical gain |
| --- | ---: | ---: |
| Temporal built-ins + intl402/Temporal | 4,575 | +8.57 pp |
| Class statements + expressions | 1,331 | +2.49 pp |
| Array / TypedArray family | 1,114 | +2.09 pp |
| Dynamic import | 668 | +1.25 pp |
| RegExp related | 507 | +0.95 pp |
| module-code + import | 365 | +0.68 pp |
| Annex B | 238 | +0.45 pp |
| Explicit resource management constructors | 219 | +0.41 pp |

V13 does not try to clear every cluster. It assigns the three developers to
the clusters with the best score potential for a 10-day sprint:

| Track | Owner | Target gain |
| --- | --- | ---: |
| V13-A | Temporal / Intl Temporal | +2,400 to +3,200 |
| V13-B | Dynamic import / module / class | +1,000 to +1,500 |
| V13-C | Array / TypedArray / missing globals | +800 to +1,200 |

The minimum planned contribution is:

```text
A +2,400
B +1,000
C   +800
-------
  +4,200 net focused passes
```

The preferred contribution is:

```text
A +3,000
B +1,300
C +1,000
-------
  +5,300 gross focused passes
```

## 2. V13 Tracks

### V13-A - Temporal / Intl Temporal

Owner: A group.

Scope:

- `Temporal.PlainDate`
- `Temporal.PlainDateTime`
- `Temporal.Duration`
- `Temporal.Instant`
- `Temporal.PlainTime`
- `Temporal.PlainYearMonth`
- `Temporal.PlainMonthDay`
- `Temporal.ZonedDateTime`
- `intl402/Temporal` minimal bridge

V13-A is Test262-oriented ISO Temporal core. Full Temporal/Intl correctness is
not required for V13 completion.

Expected effect:

- convert a large fraction of Temporal assertion and TypeError failures into
  passes;
- preserve object shape, descriptor, getter order, RangeError, and TypeError
  behavior for high-frequency Temporal paths;
- make the preferred 80%+ goal feasible.

### V13-B - Dynamic Import / Module / Class

Owner: B group.

Scope:

- dynamic `import()` syntax and bytecode lowering;
- dynamic import Promise result and rejection behavior;
- local module load/evaluate integration;
- module failure timing and error shape;
- class fields;
- private fields and private methods;
- static blocks;
- derived constructor `super()` / `this` initialization;
- computed class key evaluation order.

Expected effect:

- remove the high-frequency `dynamic import execution unsupported` failure;
- reduce language/class SyntaxError, Unsupported, TypeError, and assertion
  mismatches;
- push module-code and class directories into execution instead of early
  unsupported exits.

### V13-C - Array / TypedArray / Missing Globals

Owner: C group.

Scope:

- `AggregateError`;
- `SuppressedError`;
- URI encode/decode globals;
- `DisposableStack`;
- `AsyncDisposableStack`;
- `ArraySpeciesCreate`;
- typed-array validation and detached-buffer checks;
- BigInt typed-array variants where the BigInt representation is already
  usable;
- stable sort and comparator abrupt completion;
- sparse arrays, non-writable `length`, and descriptor edges.

Expected effect:

- remove low-cost `ReferenceError: ... is not defined` failures;
- improve Array and TypedArray clusters through shared abstract operations;
- add a reliable medium-size score contribution while A/B handle riskier work.

## 3. Explicit Non-Goals

V13 does not include:

- complete Temporal / Intl402 conformance;
- full ICU/CLDR-backed internationalization;
- Atomics agent and shared-memory Test262 harness;
- WeakRef / FinalizationRegistry GC semantics;
- ShadowRealm isolation;
- complete RegExp backend replacement;
- full Annex B declaration-instantiation rewrite;
- broad style-only formatting churn;
- counting skipped tests as passes;
- changes that make full Test262 fail to complete.

RegExp, Atomics, WeakRef, FinalizationRegistry, ShadowRealm, and complete Intl402
remain valid future work, but they are not V13 score-sprint targets.

## 4. Focused Score Commands

All commands below are intended to be run from the repository root.

### V13-A - Temporal

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress --json reports/native-v13-a-temporal-builtins.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402/Temporal --jobs 4 --progress --json reports/native-v13-a-temporal-intl402.json
```

Focused subclusters:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainDate --jobs 4 --progress --json reports/native-v13-a-plain-date.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainDateTime --jobs 4 --progress --json reports/native-v13-a-plain-date-time.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/Duration --jobs 4 --progress --json reports/native-v13-a-duration.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/Instant --jobs 4 --progress --json reports/native-v13-a-instant.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainTime --jobs 4 --progress --json reports/native-v13-a-plain-time.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainYearMonth --jobs 4 --progress --json reports/native-v13-a-plain-year-month.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/PlainMonthDay --jobs 4 --progress --json reports/native-v13-a-plain-month-day.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal/ZonedDateTime --jobs 4 --progress --json reports/native-v13-a-zoned-date-time.json
```

### V13-B - Dynamic Import / Module / Class

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/dynamic-import --jobs 4 --progress --json reports/native-v13-b-dynamic-import.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v13-b-module-code.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress --json reports/native-v13-b-statements-class.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress --json reports/native-v13-b-expressions-class.json
```

Focused supporting directories:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/import.meta --jobs 4 --progress --json reports/native-v13-b-import-meta.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/super --jobs 4 --progress --json reports/native-v13-b-super.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress --json reports/native-v13-b-for-of.json
```

### V13-C - Missing Globals / Array / TypedArray

Missing globals:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/AggregateError --jobs 4 --progress --json reports/native-v13-c-aggregate-error.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/SuppressedError --jobs 4 --progress --json reports/native-v13-c-suppressed-error.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/encodeURI --jobs 4 --progress --json reports/native-v13-c-encode-uri.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/decodeURI --jobs 4 --progress --json reports/native-v13-c-decode-uri.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/encodeURIComponent --jobs 4 --progress --json reports/native-v13-c-encode-uri-component.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/decodeURIComponent --jobs 4 --progress --json reports/native-v13-c-decode-uri-component.json
```

Resource management:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DisposableStack --jobs 4 --progress --json reports/native-v13-c-disposable-stack.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/AsyncDisposableStack --jobs 4 --progress --json reports/native-v13-c-async-disposable-stack.json
```

Array and TypedArray:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress --json reports/native-v13-c-array.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v13-c-typed-array.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors --jobs 4 --progress --json reports/native-v13-c-typed-array-constructors.json
```

Focused subclusters:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array/prototype --jobs 4 --progress --json reports/native-v13-c-array-prototype.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray/prototype --jobs 4 --progress --json reports/native-v13-c-typedarray-prototype.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors/ctors-bigint --jobs 4 --progress --json reports/native-v13-c-typedarray-ctors-bigint.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors/from --jobs 4 --progress --json reports/native-v13-c-typedarray-from.json
```

## 5. Integration Commands

Full score command:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/full-test262-summary-v13.json
```

Project gates:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo test --no-default-features --test native_test262
```

If V13 touches native-only behavior, also run:

```sh
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

## 6. Daily Scoreboard

Each group must record baseline and current scores in its V13 report:

```text
cluster
baseline total/passed/failed/skipped
current total/passed/failed/skipped
delta passed
delta failed
new regressions
commands run
```

Minimum expected contribution:

```text
V13-A: +2,400
V13-B: +1,000
V13-C:   +800
```

Preferred contribution:

```text
V13-A: +3,000
V13-B: +1,300
V13-C: +1,000
```

## 7. Completion Criteria

V13 is complete only when:

- full Test262 reaches at least 80.00%, or every group has documented why the
  target was missed;
- `reports/full-test262-summary-v13.json` exists and is current;
- all three focused V13 reports include baseline, current score, deltas, and
  commands;
- the full test run completes without runner crash;
- skipped tests are not counted as passes;
- old native smoke gates remain green or have documented pre-existing blockers;
- README/report material includes the new pass count only after a full run.
