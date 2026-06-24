# Native V10 Team Plan

V10 is a three-track numeric and builtin semantics batch. The three groups work
in parallel on different feature families, then merge into one V10 integration
pass.

Shared contracts in `native-v10-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v10-contracts
feat/v10-a-numeric-bigint-unicode
feat/v10-b-typedarray-buffer-runtime
feat/v10-c-date-intl-temporal-builtins
test/v10-integration
```

Recommended merge order:

```text
V10 contracts
  -> V10-A numeric/source-text frontend
  -> V10-B buffer and typed-array runtime substrate
  -> V10-C Date/Intl/Temporal builtin semantics
  -> V10 integration reports and docs
```

## 2. A Group — BigInt / Numeric / Unicode Syntax Tail

Owned files:

```text
src/lexer/
src/parser/
src/ast/
src/bytecode/compiler.rs
tests/frontend_v10.rs
```

Tasks:

- BigInt literal edge cases;
- numeric separator and numeric literal residuals;
- unicode identifier/source-text residuals;
- early errors for invalid numeric forms.

A must not implement typed-array storage, Date/Intl/Temporal algorithms, or
ArrayBuffer object state.

Independent validation:

```sh
cargo test --no-default-features frontend_v10
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals --jobs 4 --progress --json reports/native-v10-a-literals-summary.json
```

Required report:

- `reports/v10-partA-report.md`

## 3. B Group — TypedArray / ArrayBuffer / DataView Runtime

Owned files:

```text
src/runtime/
src/vm/
src/contracts.rs
tests/native_v10_runtime.rs
```

Tasks:

- shared ArrayBuffer byte storage; ✅ first runtime substrate implemented
- typed-array view records; ✅ `TypedArrayViewId` registry implemented
- DataView shared storage helpers; ✅ shared `ArrayBufferRecord` storage
- bounds checks and minimal detach flag; ✅ range and detached checks in runtime APIs
- numeric conversion helpers for element load/store. ✅ Number-backed non-BigInt
  paths implemented

B must not install JS-visible typed-array constructors directly; C owns builtin
installation and descriptor shape.

Current B status: first runtime substrate pass is complete. JS-visible
ArrayBuffer/TypedArray/DataView constructor migration remains C-owned; BigInt
typed-array element semantics remain future A/C integration work.

Independent validation:

```sh
cargo test --no-default-features native_v10_runtime
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v10-b-typedarray-summary.json
```

Required report:

- `reports/v10-partB-report.md`

## 4. C Group — Date / Intl / Temporal Builtins

Owned files:

```text
src/builtins/
tests/native_v10_builtins.rs
```

Tasks:

- Date constructor/prototype semantic expansion;
- deterministic Intl fallback behavior;
- selected Temporal core type semantics;
- descriptor and error-shape fixes for implemented builtins.

C must coordinate with B before typed-array constructors expose B runtime
storage through JS-visible objects.

Independent validation:

```sh
cargo test --no-default-features native_v10_builtins
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Date --jobs 4 --progress --json reports/native-v10-c-date-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v10-c-intl402-summary.json
```

Required report:

- `reports/v10-partC-report.md`

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `src/lexer/`, `src/parser/`, `src/ast/` | A | Preserve V9-A work; B/C request syntax changes through interface docs |
| `src/bytecode/compiler.rs` | A, with B review | Numeric literal lowering and BigInt placeholders require B review |
| `src/runtime/` | B | C typed-array builtins must use B storage helpers |
| `src/vm/` | B | A/C request opcode support through interface docs |
| `src/builtins/` | C | Date/Intl/Temporal and JS-visible TypedArray constructors live here |
| `src/test262.rs` | shared | Keep V10 scan selector stable |
| `docs/native-v10-*.md` | all groups | Contract updates before shared-file changes |

## 6. Integration Gate

Before V10 is considered complete:

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

Initial V10 scan baseline: 645/5,000 passed, 4,355 failed, 0 skipped.

Focused summaries should also be current:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals --jobs 4 --progress --json reports/native-v10-a-literals-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v10-b-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Date --jobs 4 --progress --json reports/native-v10-c-date-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v10-c-intl402-summary.json
```

Reports and docs to update after integration:

- `reports/v10-partA-report.md`
- `reports/v10-partB-report.md`
- `reports/v10-partC-report.md`
- `reports/native-v10-scan-summary.json`
- `docs/status.md`
- `AGENTS.md`
- `readme.md`
- `thoughts/newplan.md`
