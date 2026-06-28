# Native V8 Team Plan

V8 is a three-track feature unlock batch. The three groups work in parallel on
different feature families, then merge into one V8 integration pass.

Shared contracts in `native-v8-interface.md` merge first.

## 1. Execution Model

Recommended branches:

```text
docs/binary_data-contracts
feat/binary_data-a-parser_basics-unlockers
feat/binary_data-b-module-runner
feat/binary_data-c-builtin-skeletons
test/binary_data-integration
```

Recommended merge order:

```text
V8 contracts
  -> shared AST/source-kind/runtime helper connector patches
  -> V8-A parser_basics unlockers
  -> V8-B module runner infrastructure
  -> V8-C builtin skeletons
  -> V8 integration reports and docs
```

The three feature branches may develop in parallel after the contracts merge.
If a branch needs a shared interface not covered here, update
`native-v8-interface.md` before continuing.

## 2. A Group — Frontend Unlockers

Owned files:

```text
src/lexer/
src/parser/
src/ast/
src/bytecode/compiler.rs
tests/frontend_v8.rs
```

Tasks:

- implement template literal substitutions for untagged templates;
- add class declaration/expression parsing and first-stage lowering;
- add first-stage spread/rest/destructuring support;
- keep failures deterministic for unsupported advanced syntax.

A must not implement TypedArray, Intl, `$262`, or module loader behavior except
for syntax/AST contracts needed by B and C.

Independent validation:

```sh
cargo test --no-default-features frontend_v8
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-a-language-summary.json
```

Required report:

- `reports/.version-report/v8-partA-report.md`

Every A-track implementation change must update this report with the change
summary, touched files, tests run, result deltas, and open coordination notes.

## 3. B Group — Module Runner Infrastructure

Owned files:

```text
src/backend/
src/runtime/
src/vm/
src/contracts.rs
src/test262.rs
tests/native_modules.rs
```

Tasks:

- add explicit script/module source-kind handling; **done first-stage**;
- add module strict-mode execution entry; **done first-stage**;
- add module record, registry, and acyclic dependency loading; **registry and
  relative loader done, AST graph wiring pending A-group import/export support**;
- define module environment and import/export binding storage;
- classify module failures separately in runner output; **module-mode labels
  done**.

B must not implement large builtin skeletons or rewrite A-owned parser behavior.
If parser support is missing, B may land runtime scaffolding with unit tests and
document the connector needed from A.

Independent validation:

```sh
cargo test --no-default-features native_modules
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-b-module-summary.json
```

Required report:

- `reports/.version-report/v8-partB-report.md`

Every B-track implementation change must update this report with the change
summary, touched files, tests run, module skip delta, and open coordination
notes.

Current B status (2026-06-24): first-stage module infrastructure is in place.
`cargo test --no-default-features --test native_modules` passes 5/5, and
`cargo test --no-default-features --test native_test262` passes 12/12. The
focused module-code command reports 201/599 passed, 398 failed, 0 skipped
(`reports/.native-test262-tmp/native-v8-b-module-summary.json`). The standard V8 scan reports
205/5,000 passed, 4,795 failed, 0 skipped
(`reports/.native-test262-tmp/native-v8-scan-summary.json`). Remaining failures are dominated by
import/export parser and linking semantics that require A/B connector work.

## 4. C Group — Builtin Skeletons and Test262 Host

Owned files:

```text
src/builtins/
src/test262.rs
reports/
docs/status.md
readme.md
tests/native_typed_arrays.rs
```

Tasks:

- install first-batch TypedArray / ArrayBuffer skeletons;
- install first-batch Intl skeletons;
- add `$262` host helpers required by V8 focused suites;
- maintain V8 focused Test262 commands and reports.

C must not bypass runtime object-model APIs to make descriptors appear to pass.
If object-model support is missing, C coordinates with B for a shared helper
instead of hard-coding behavior in one builtin.

Independent validation:

```sh
cargo test --no-default-features native_typed_arrays
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-c-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-c-intl402-summary.json
```

Required report:

- `reports/.version-report/v8-partC-report.md`

Every C-track implementation change must update this report with the change
summary, touched files, tests run, missing-global delta, and newly exposed
descriptor/semantic failures.

## 5. Shared-File Lock

| File or area | Owner | Notes |
| --- | --- | --- |
| `src/lexer/`, `src/parser/`, `src/ast/` | A | B/C request syntax changes through interface docs |
| `src/bytecode/compiler.rs` | A, with B review | VM stack/helper changes require B review |
| `src/runtime/context.rs` | B | C requests builtin helper APIs through B |
| `src/vm/interpreter.rs` | B | A may request opcode support through interface docs |
| `src/builtins/` | C | Must use runtime object-model APIs |
| `src/test262.rs` | B/C shared | B owns module runner behavior; C owns reports/host helpers |
| `docs/version/native-v8-*.md` | all groups | contract updates before shared-file changes |

## 6. Integration Gate

Before V8 is considered complete:

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-a-language-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-b-module-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/.native-test262-tmp/native-v8-c-intl402-summary.json
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/.native-test262-tmp/native-v8-scan-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/.native-test262-tmp/native-full-test262-summary.json
```

`--native-v8-scan` is the standard lightweight V8 integration check. It runs
the locked 5,000-case manifest in `reports/.test262/test262-scan-failure/native-v8-scan-failures.txt`, sampled
from cases that did not pass in the 2026-06-24 full direct run. The initial
initial summary was `reports/.native-test262-tmp/native-v8-scan-summary.json`: 0/5,000 passed,
4,504 failed, and 496 skipped. After the first V8-B module runner pass, the
current summary is 205/5,000 passed, 4,795 failed, and 0 skipped.

Reports and docs to update after the integration run:

- `reports/.test262/test262-analysis/test262-report.md`;
- a new dated or versioned analysis file, not the locked
  `reports/.test262/test262-analysis/test262-analysis.md`;
- `reports/.version-report/v8-partA-report.md`;
- `reports/.version-report/v8-partB-report.md`;
- `reports/.version-report/v8-partC-report.md`;
- `reports/.native-test262-tmp/native-v8-scan-summary.json` if generated and intentionally kept for
  the integration pass;
- `docs/status.md`;
- `AGENTS.md`;
- `thoughts/plan_1_version.md`.

`reports/.test262/test262-analysis/test262-analysis.md` is the locked 2026-06-24 baseline. For later
full-suite analysis, create a new dated or versioned analysis file instead of
rewriting the baseline.
