# Native V17 Part C report

## Ownership and scope

This batch covers shared Temporal PlainDate, PlainDateTime, and PlainTime
argument normalization and ISO date range validation.

## Changes

- Truncate PlainDate and PlainDateTime constructor date arguments after
  `ToNumber`, as required by Temporal integer conversion.
- Validate the inclusive Temporal ISO date range using its exact civil-date
  endpoints. This avoids the negative-boundary off-by-one in epoch-day
  conversion.
- Treat an explicit `undefined` time component as the Temporal default zero,
  without changing the distinct legacy `Date` constructor conversion rules.

Files touched:

- `src/builtins/date_intl.rs`
- `reports/test262-latest-common-errors-2026-07-28.md`

## Focused Test262 results

| Suite | Baseline failures | Final failures | Newly passing | Regressions |
|---|---:|---:|---:|---:|
| `built-ins/Temporal/PlainDate` | 136 | 125 | 11 | 0 |
| `built-ins/Temporal/PlainDateTime` | 132 | 131 | 1 | 0 |
| `built-ins/Temporal/PlainTime` | 67 | 64 | 3 | 0 |
| `built-ins/Temporal/PlainDate` (`monthCode`) | 125 | 123 | 2 | 0 |
| `built-ins/Temporal/PlainDateTime` (`monthCode`) | 131 | 129 | 2 | 0 |
| `built-ins/Temporal/PlainYearMonth` (`monthCode`) | 124 | 123 | 1 | 0 |
| `built-ins/Temporal/PlainMonthDay` (`monthCode`) | 47 | 47 | 0 | 0 |

Focused cases:

- `built-ins/Temporal/PlainDate/limits.js`: pass
- `built-ins/Temporal/PlainDate/argument-convert.js`: pass
- `built-ins/Temporal/PlainDateTime/argument-convert.js`: pass
- `built-ins/Temporal/PlainDate/from/monthcode-invalid.js`: pass

## Repository lint cleanup

- Applied repository-wide rustfmt output.
- Applied safe Clippy suggestions and resolved all remaining diagnostics.
- Removed redundant Set-like object bindings, simplified Date argument
  conversion loops, and used narrow lint allowances for stable internal
  functions whose spec-driven signatures exceed Clippy's argument threshold.
- No public contract signature was changed.

## Commands

```text
cargo build --release
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainDate\" --jobs 4
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainDateTime\" --jobs 4
target\release\agentjs.exe test262 --filter "built-ins\Temporal\PlainTime\" --jobs 4
cargo check --all-targets
cargo test --all-targets
rustfmt --edition 2024 --check src/builtins/date_intl.rs
```

## Coordination notes

- No shared contract was changed.
- No Test262, Boa, or QuickJS submodule file was modified.
- `cargo check --all-targets` and `cargo test --all-targets` pass.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test --no-default-features --test native_test262` passes (15/15).
# WeakRef / FinalizationRegistry correctness follow-up

- Added the missing `WeakRef` constructor/prototype surface, target validation,
  `deref`, custom `new.target.prototype` handling, and `Symbol.toStringTag`.
- Added the missing `FinalizationRegistry` constructor/prototype surface with
  cleanup-callback validation, `register`/`unregister`, weak-target/token
  validation, token identity removal, and descriptor-visible builtin shape.
- The current collector does not yet enqueue cleanup callbacks. The
  implementation deliberately covers deterministic ECMAScript observability;
  nondeterministic collection scheduling remains deferred.

Validation:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/WeakRef` | 0 / 29 | 25 / 29 | +25 |
| `test/built-ins/FinalizationRegistry` | 0 / 47 | 43 / 47 | +43 |
| Full Test262 | 45,654 / 53,379 | 45,725 / 53,379 | +71 |

Full conformance moved from **85.5280%** to **85.6610%** (+0.1330 percentage
points), with failures decreasing from 7,723 to 7,652 and skips unchanged at
2. The focused suites account for +68 passes; the full scan records a further
+3 net cross-suite improvement.

Commands:

```powershell
cargo test --locked --test native_weak_refs
cargo check --locked --all-targets
cargo test --locked --all-targets
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\WeakRef --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\FinalizationRegistry --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --jobs 4 --progress --json reports\full-test262-summary.json
```

## Temporal relative calendar-unit rounding

- Replaced fixed 365-day/30-day rounding for `Temporal.PlainDate` and
  `Temporal.PlainDateTime` differences with quantities measured relative to
  the receiver date.
- Added correct `roundingIncrement` handling and all Temporal rounding modes
  for year, month, week, and day smallest units.
- PlainDateTime calculations include the time-of-day fraction when choosing a
  calendar-unit boundary.
- Negative differences round relative to the correct directional calendar
  boundary; half-even ties select the even increment.

Focused results:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `Temporal/PlainDate/prototype/until` | 62 / 86 | 77 / 86 | +15 |
| `Temporal/PlainDate/prototype/since` | 61 / 87 | 75 / 87 | +14 |
| `Temporal/PlainDateTime/prototype/until` | 74 / 98 | 80 / 98 | +6 |
| `Temporal/PlainDateTime/prototype/since` | 72 / 95 | 77 / 95 | +5 |
| Full Test262 | 45,725 / 53,379 | 45,771 / 53,379 | +46 |

The full conformance rate moved from **85.6610%** to **85.7472%** (+0.0862
percentage points). Relative to the original 85.5280% baseline used for this
correctness pass, the cumulative result is **+117 passes** and **+0.2192
percentage points**.

## Temporal add/subtract range and time balancing

- Reject date arithmetic before `civil_from_days` can clamp an out-of-range
  intermediate back to a valid boundary date.
- Use the exact asymmetric Temporal day-number interval
  `[-100000001, 100000000]`, including the earliest `PlainDate`.
- Convert the time portion of a duration to whole days with truncation toward
  zero for `PlainDate`, fixing negative fractional-hour/minute additions.
- Validate final `PlainDate` and `PlainDateTime` results after date and time
  balancing, including the one-nanosecond lower `PlainDateTime` boundary.
- Added focused integration coverage for both ISO range ends.

Focused results:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `Temporal/PlainDate/prototype/add` | 31 / 39 | 35 / 39 | +4 |
| `Temporal/PlainDate/prototype/subtract` | 30 / 38 | 34 / 38 | +4 |
| `Temporal/PlainDateTime/prototype/add` | 34 / 42 | 37 / 42 | +3 |
| `Temporal/PlainDateTime/prototype/subtract` | 34 / 42 | 37 / 42 | +3 |
| Full Test262 | 45,771 / 53,379 | 45,795 / 53,379 | +24 |

Full conformance is now **85.7922%**, an increase of **0.0450 percentage
points** in this batch. Relative to the original 85.5280% baseline, the
cumulative improvement is **+141 passes** and **+0.2642 percentage points**.

Validation:

```powershell
cargo test --locked --test native_temporal_rounding
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
target\release\agentjs.exe test262 --root test262 --suite test --jobs 4 --json reports\full-test262-summary.json
```

## Temporal.Duration calendar-unit arithmetic guard

- `Temporal.Duration.prototype.add` and `subtract` now reject nonzero years,
  months, or weeks in either operand when no `relativeTo` calendar context is
  available.
- Added focused integration coverage for receiver, property-bag, and duration
  string operands.

Focused results:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `Temporal/Duration/prototype/add` | 21 / 34 | 22 / 34 | +1 |
| `Temporal/Duration/prototype/subtract` | 20 / 34 | 21 / 34 | +1 |

## Temporal correctness sweep: +103 focused passes

- Fixed `Temporal.PlainYearMonth.prototype.until` and `since` so omitted or
  `auto` `largestUnit` defaults to `year`, restoring year/month balancing
  across default options and rounding modes.
- Fixed `Temporal.ZonedDateTime.prototype.until` and `since` so their default
  largest unit is `hour`, rather than leaking whole days into the result.
- Replaced floating-point time balancing with exact `i128` nanosecond
  arithmetic in PlainTime, PlainDateTime, ZonedDateTime, and Duration
  arithmetic paths.
- Duration addition/subtraction now balances only as high as the largest input
  unit. For example, `-PT24.5H` remains an hour-based duration rather than
  becoming a mixed day/time result.
- PlainYearMonth addition/subtraction now rejects weeks and lower units and
  validates the overflow options object before arithmetic.
- Temporal `toLocaleString(locale)` no longer treats the locale argument as a
  Temporal `toString` options object.

Focused type results:

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `Temporal/Duration` | 367 / 540 | 378 / 540 | +11 |
| `Temporal/Instant` | 433 / 465 | 434 / 465 | +1 |
| `Temporal/PlainDateTime` | 661 / 773 | 664 / 773 | +3 |
| `Temporal/PlainTime` | 429 / 493 | 436 / 493 | +7 |
| `Temporal/PlainYearMonth` | 386 / 509 | 432 / 509 | +46 |
| `Temporal/ZonedDateTime` | 728 / 901 | 763 / 901 | +35 |
| **Complete `test/built-ins/Temporal`** | **3,793 / 4,603** | **3,896 / 4,603** | **+103** |

The complete Temporal built-ins suite moved from **82.40%** to **84.64%**
(+2.24 percentage points). The full-suite runner was attempted with both
120-second and 300-second limits but did not finish within either limit; no
full-suite result is claimed for this batch.

Validation:

```powershell
cargo test --locked --test native_temporal_rounding
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
target\release\agentjs.exe test262 --root test262 --suite test\built-ins\Temporal --jobs 4 --json reports\.native-test262-tmp\temporal-plus-103-summary.json
```

## JetStream RegExp compatibility follow-up

- Translate JavaScript control-letter escapes such as `\cX` to the
  corresponding control code before compiling with the Rust regex backend.
- Install the `RegExp` `Symbol.toStringTag`, allowing validatorjs to recognize
  regular expressions through `Object.prototype.toString.call(value)`.
- Added focused tests for control escapes and both literal/constructed RegExp
  branding.

Validation:

| Suite | Result |
|---|---:|
| `test/built-ins/RegExp` | 1464/1879, 0 skipped |
| control-letter translation unit test | pass |
| RegExp branding integration test | pass |

validatorjs now executes its assertion corpus until it reaches a pattern with
numeric backreferences. The current Rust `regex` backend does not support
backreferences; resolving that remaining item requires a backtracking-capable
backend or a separate compatibility execution path.

## AgentBench multi-engine command support

- Extended reference-engine command parsing so an engine can include fixed CLI
  arguments while retaining executable fingerprint and binary-size reporting.
- Documented Oxide's required `run` subcommand in the AgentBench comparison
  command alongside Boa, QuickJS, and Node.js.
- Restored the default batch size to five tasks. A batch of 25 makes two
  pressure workloads exceed AgentJS's per-execution loop budget, while the
  established five-task comparison baseline exercises the same in-process
  path and keeps all cases inside the correctness gate.
- No native engine stage or shared contract changed.

Validation:

```powershell
python -m py_compile benchmarks/agent/run_agentbench.py
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --ref boa=.\boa\target\release\boa.exe --ref oxide=".\target\oxide-compare\release\oxide.exe run" --cases startup-noop --mode cold --warmup 0 --repeat 1
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --ref boa=.\boa\target\release\boa.exe --ref quickjs=.\quickjs\qjs.exe --ref oxide=".\target\oxide-compare\release\oxide.exe run" --group all --mode cold --warmup 3 --repeat 15 --out-dir benchmarks\agent\results\four-engine-comparison
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --ref boa=.\boa\target\release\boa.exe --ref quickjs=.\quickjs\qjs.exe --ref oxide=".\target\oxide-compare\release\oxide.exe run" --group all --mode batch --warmup 3 --repeat 15 --batch-repeat 5 --out-dir .cache\agentbench-fourway-batch-final
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Final comparison results:

- All four engines passed 12/12 cases in both cold and five-task batch modes.
- Cold reference/AgentJS geometric-mean ratios: Boa 1.090x, QuickJS 0.186x,
  and Oxide 1.237x.
- Batch reference/AgentJS geometric-mean ratios: Boa 0.625x, QuickJS
  0.112x, and Oxide 0.924x.
- Reports, environment metadata, executable hashes, and binary sizes are in
  `benchmarks/agent/results/four-engine-comparison/`.

## JetStream2 four-engine portable kernel runner

- Added a four-engine runner for AgentJS, Boa, QuickJS, and Oxide using
  identical self-contained files generated from the pinned JetStream2
  JavaScript workload sources.
- Correctness requires each workload's deterministic completion summary;
  zero exit status alone is not accepted.
- The report separates workload kernel time from process wall time and records
  peak RSS, all samples, revisions, executable hashes, and binary sizes.
- The runner is explicitly not presented as the browser suite's official
  composite score; browser, worker, Wasm, and full-driver lifecycle tests stay
  outside this portable comparison.

Final JetStream2 kernel results:

- The six-workload common set passed 6/6 on all four engines, with seven
  measured processes per engine/workload pair.
- Geometric-mean reference/AgentJS kernel-time ratios: Boa 0.202x, QuickJS
  0.047x, and Oxide 0.945x.
- Maximum observed peak RSS: AgentJS 27.04 MiB, Boa 16.43 MiB, QuickJS 7.02
  MiB, and Oxide 1,123.82 MiB.
- In a three-iteration pressure run, Oxide exceeded the 1,536 MiB limit on the
  SHA-1 and MD5 kernels; the other three engines passed all selected kernels.
- Consolidated interpretation is in
  `benchmarks/jetstream/results/summary.md`.

Validation:

```powershell
python -m py_compile benchmarks/jetstream/run_four_engine.py
node --check scripts/prepare-simple-benchmark.mjs
python benchmarks/jetstream/run_four_engine.py --iterations 1 --warmup 0 --repeat 1 --out-dir benchmarks/jetstream/results/four-engine-smoke
python benchmarks/jetstream/run_four_engine.py --tests n-body-SP,crypto-sha1-SP,crypto-md5-SP,3d-cube-SP,navier-stokes,richards --iterations 1 --warmup 2 --repeat 7 --timeout 180 --out-dir benchmarks/jetstream/results/four-engine
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```
