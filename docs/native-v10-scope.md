# Native V10 Scope: Numeric and Builtin Semantics Expansion

Native V10 starts while V9-A frontend work is still in progress, but V9-B and
V9-C are treated as available integration inputs. V10 is a three-track feature
batch focused on numeric syntax tail work, typed-array storage semantics, and
date/i18n/time-related builtin behavior.

Shared contracts are defined in
[Native V10 Shared Interface](native-v10-interface.md), and file ownership is
defined in [Native V10 Team Plan](native-v10-team-plan.md).

## 1. Baseline

V10 uses the locked V10 lightweight scan manifest:

```text
reports/native-v10-scan-failures.txt
```

The manifest contains 5,000 previously non-passing Test262 cases sampled from
the locked full direct output, prioritizing:

- `test/built-ins/TypedArray`
- `test/built-ins/ArrayBuffer`
- `test/built-ins/SharedArrayBuffer`
- `test/built-ins/DataView`
- `test/built-ins/Temporal`
- `test/intl402`
- `test/built-ins/Date` and Annex B Date tests
- BigInt/numeric/unicode syntax tail cases

Initial scan command:

```sh
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

Initial result:

```text
5,000 total, 645 passed, 4,355 failed, 0 skipped, 12.90%
```

## 2. V10 Tracks

### V10-A — BigInt / Numeric / Unicode Syntax Tail

Owner: A group.

Scope:

- BigInt literal edge cases;
- numeric separator and numeric literal residual cases;
- unicode identifier escapes and source-text residual cases;
- static syntax tail cases that block V10 focused builtins.

Expected effect:

- reduce lexer/static syntax failures in V10 scan;
- expose typed-array/date/i18n runtime failures instead of parse failures.

### V10-B — TypedArray / ArrayBuffer / DataView Runtime Substrate

Owner: B group.

Scope:

- ArrayBuffer byte storage;
- typed-array view metadata and element access helpers;
- DataView shared storage helpers;
- bounds checks and minimal detach model;
- numeric conversion helpers required by typed-array element stores.

Expected effect:

- support C group typed-array builtins without duplicating storage logic;
- convert missing runtime substrate failures into descriptor/algorithm failures.

### V10-C — Temporal / Intl / Date Builtin Semantics

Owner: C group.

Scope:

- Date constructor/prototype semantic expansion;
- deterministic Intl fallback behavior for high-signal tests;
- Temporal skeleton-to-semantics pass for selected core types;
- descriptor and error-shape fixes for implemented date/i18n builtins.

Expected effect:

- reduce `Date is not defined`, `Intl is not defined`, and Temporal semantic
  failures;
- improve full-suite builtin/global-family coverage.

## 3. Explicit Non-Goals

V10 does not include:

- full Temporal specification coverage;
- full locale-sensitive ICU-quality formatting;
- shared-memory Atomics semantics;
- spec-perfect detached buffer behavior across workers;
- RegExp property escapes/backreference implementation;
- module live-binding/cycle semantics;
- completing any remaining V9-A async/generator lowering.

## 4. Focused Commands

### V10-A

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals --jobs 4 --progress --json reports/native-v10-a-literals-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/identifiers --jobs 4 --progress --json reports/native-v10-a-identifiers-summary.json
```

### V10-B

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v10-b-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress --json reports/native-v10-b-dataview-summary.json
```

### V10-C

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Date --jobs 4 --progress --json reports/native-v10-c-date-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v10-c-intl402-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress --json reports/native-v10-c-temporal-summary.json
```

### V10 Integration

```sh
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

## 5. Completion Criteria

V10 is complete only when:

- all three tracks have merged;
- old V1-V9 regression gates remain green;
- focused A/B/C summaries are current;
- `--native-v10-scan` has a current JSON summary;
- each `reports/v10-part*-report.md` records changes and deltas;
- `AGENTS.md`, `readme.md`, `docs/status.md`, and `thoughts/newplan.md`
  reflect the V10 status;
- skipped tests are never counted as passes.
