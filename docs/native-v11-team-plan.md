# Native V11 Team Plan

V11 is a three-track semantic precision and RegExp batch. The three groups work
in parallel on different feature families, then merge into one V11 integration
pass.

Shared contracts in `native-v11-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/v11-contracts
feat/v11-a-regexp-static
feat/v11-b-object-descriptor-precision
feat/v11-c-regexp-annexb-descriptor-builtins
test/v11-integration
```

Recommended merge order:

```text
V11 contracts
  -> V11-A RegExp parser/static-error work
  -> V11-B object model and descriptor precision
  -> V11-C RegExp/Annex B/builtin descriptor sweep
  -> V11 integration reports and docs
```

## 2. A Group — RegExp Parser / Static Errors

Owned files:

```text
src/lexer/
src/parser/
src/ast/
tests/frontend_v11.rs
```

Tasks:

- RegExp property escapes token/static checks;
- regexp literal error-kind precision;
- unicode escape residuals in RegExp literals;
- early-error residuals for named groups/backreferences.

A must not implement RegExp runtime matching or builtin behavior.

Independent validation:

```sh
cargo test --no-default-features frontend_v11
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json
```

Required report:

- `reports/v11-partA-report.md`

## 3. B Group — Object Model / Descriptor Precision

Owned files:

```text
src/runtime/
src/vm/
src/contracts.rs
tests/native_v11_runtime.rs
```

Tasks:

- descriptor exactness;
- receiver handling;
- getter/setter order;
- property lookup and own-key order; ✅ first boundary fix landed for
  `4294967295` ordinary string-key ordering
- expected error ordering in shared object/runtime paths.

B must not implement RegExp builtin algorithms or Annex B globals directly; C
owns JS-visible builtins.

Independent validation:

```sh
cargo test --no-default-features native_v11_runtime
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-b-object-summary.json
```

Required report:

- `reports/v11-partB-report.md`

## 4. C Group — RegExp / Annex B / Descriptor Builtins

Owned files:

```text
src/builtins/
tests/native_v11_builtins.rs
```

Tasks:

- RegExp prototype/static builtin semantics;
- String methods that dispatch to RegExp;
- Annex B legacy behavior;
- descriptor sweep for Object / Function / Array / String / Iterator.

C must coordinate with B before changing shared descriptor/property-order
helpers.

Independent validation:

```sh
cargo test --no-default-features native_v11_builtins
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress --json reports/native-v11-c-regexp-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB --jobs 4 --progress --json reports/native-v11-c-annexb-summary.json
```

Required report:

- `reports/v11-partC-report.md`

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `src/lexer/`, `src/parser/`, `src/ast/` | A | Preserve V10-A work; B/C request syntax changes through interface docs |
| `src/runtime/` | B | C descriptor sweeps must use shared object-model helpers |
| `src/vm/` | B | Runtime error ordering and receiver behavior may require VM review |
| `src/builtins/` | C | RegExp/Annex B/descriptor sweep work lives here |
| `src/test262.rs` | shared | Keep V11 scan selector stable |
| `docs/native-v11-*.md` | all groups | Contract updates before shared-file changes |

## 6. Integration Gate

Before V11 is considered complete:

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Initial V11 scan status: selector installed and manifest locked; first local
setup attempt exceeded 300s and did not produce a JSON summary.

Focused summaries should also be current:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-b-object-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress --json reports/native-v11-c-regexp-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB --jobs 4 --progress --json reports/native-v11-c-annexb-summary.json
```

Reports and docs to update after integration:

- `reports/v11-partA-report.md`
- `reports/v11-partB-report.md`
- `reports/v11-partC-report.md`
- `reports/native-v11-scan-summary.json`
- `docs/status.md`
- `AGENTS.md`
- `readme.md`
- `thoughts/newplan.md`
