# AgentJS

AgentJS is a lightweight JavaScript engine written in Rust for short-lived,
high-frequency AI agent workloads. Its self-developed lexer, parser, bytecode
compiler, VM, runtime, and built-in library form the default native backend;
Boa remains available as an explicitly selected compatibility reference.

Boa is the executable baseline and behavior oracle, not the final engine.
QuickJS is a compact architecture and performance reference. Neither upstream
tree should receive AgentJS features. The final native execution path must not
depend on Boa or QuickJS internals.

Clone all pinned dependencies with:

```sh
git clone --recurse-submodules <repository-url>
# Existing checkout:
git submodule update --init --recursive
```

Pinned revisions are listed in
[docs/dependencies.md](docs/dependencies.md). Current limitations are recorded
in [docs/status.md](docs/status.md).

## Requirements

- Rust 1.91 or newer
- Linux, macOS, or Windows
- A C toolchain only when building the QuickJS reference

## Build and Run

```sh
cargo build
cargo run -- eval "1 + 2"
cargo run -- run examples/hello.js
cargo run -- repl
cargo build --release
```

These commands use `BackendKind::Native` by default. Unsupported native syntax
returns a categorized error and never falls back to Boa. To use Boa, enable its
optional build feature and select it explicitly at runtime:

```sh
cargo run --features boa-backend -- eval --backend boa "1 + 2"
```

Enabling `boa-backend` alone does not change the default: commands without
`--backend boa` continue to use native. The standard `cargo build` is already a
native-only build.

Rust callers can select a backend explicitly:

```rust
use agentjs::{BackendKind, Engine, ExecutionOptions, RuntimeConfig};

fn run_script() -> Result<(), agentjs::EvalFailure> {
    let engine = Engine::with_backend(BackendKind::Boa, RuntimeConfig::default());
    let result = engine.execute("6 * 7", ExecutionOptions::default())?;
    assert_eq!(result.value, "42");
    Ok(())
}
```

`BackendKind::Boa` is available only when the crate is built with the
`boa-backend` feature. `Engine::new`, `Engine::default`, and `Runtime::new`
always select native.

## Architecture

```text
CLI / Test262
      |
      v
Engine / Runtime
      |
      +-------------------+
      |                   |
      v                   v
 BoaRuntime          NativeRuntime
                          |
                          v
 source -> lexer -> parser/AST -> bytecode -> VM -> JsValue
                                             |
                                             v
                                  runtime + builtins + heap
```

Native implementation directories:

```text
src/
├── contracts.rs       # reviewed cross-team API
├── lexer/             # source -> Token
├── ast/               # shared syntax representation
├── parser/            # Token -> Program
├── bytecode/          # Program -> Chunk
├── vm/                # Chunk -> JsValue
├── runtime/           # values, objects, environments, heap, GC
├── builtins/          # Object, Function, Array, etc.
└── backend/
    ├── boa.rs          # compatibility baseline
    └── native.rs       # native pipeline assembly
```

Dependencies should flow in one direction:

```text
lexer -> parser/AST -> bytecode -> VM -> runtime
                                      -> builtins
```

`backend/native.rs` assembles modules but should not contain lexer, parser, VM,
or object-model implementations. See
[docs/architecture.md](docs/architecture.md) for the full boundary.

## Shared Interfaces

Cross-team code should import shared types and traits through
`src/contracts.rs`:

```rust
use agentjs::contracts::{
    ChunkExecutor, NativePipeline, ProgramCompiler, SourceParser,
};
```

The stable collaboration contracts are:

```text
SourceParser::parse_source        source  -> Program
ProgramCompiler::compile_program  Program -> Chunk
ChunkExecutor::execute_chunk      Chunk + NativeContext -> JsValue
NativePipeline::evaluate          source + NativeContext -> JsValue
```

`Token`, `Program`, `Chunk`, `Instruction`, `NativeContext`, and `JsValue` are
important shared data contracts. Changes to these types or the three traits
require team review.
Implementation details should remain inside their owning directory.
The normative ownership, input/output, error, and compatibility rules are in
[the module interface specification](docs/interface-spec.md).

The complete native pipeline can be called directly during development:

```rust
use agentjs::contracts::{JsValue, NativeContext, NativePipeline};

fn run_native_pipeline() -> Result<(), agentjs::NativeError> {
    let mut context = NativeContext::default();
    let value = NativePipeline::default().evaluate("", &mut context)?;
    assert_eq!(value, JsValue::Undefined);
    Ok(())
}
```

The Native pipeline currently implements the scoped V1-V3 milestones:

- [Native V1 expressions](docs/version/native-v1-scope.md)
- [Native V2 control flow](docs/version/native-v2-scope.md) and its
  [shared interface](docs/version/native-v2-interface.md)
- [Native V3 functions and compound values](docs/version/native-v3-scope.md) and its
  [shared interface](docs/version/native-v3-interface.md)

The active collaborative milestone is the expanded
[Native V4 object semantics](docs/version/native-v4-scope.md), with contracts frozen in
the [Native V4 shared interface](docs/version/native-v4-interface.md). The object model,
accessors, constructors, sparse-array core, and the minimal `Object`, `Array`,
and `Function` builtin/intrinsic layer are connected. Remaining V4 work focuses
on broadening standard builtins and reducing the diagnostic Test262 failure set.
File ownership, C0–C3 execution subgroups, branch suggestions, and merge order
are defined in the [Native V4 team plan](docs/version/native-v4-team-plan.md).

Native V5 now has an initial Native VM/runtime integration for structured
completion, `try/catch/finally`, `switch`, and lexical `let`/`const`
semantics. See the
[Native V5 scope](docs/version/native-v5-scope.md),
[shared interface](docs/version/native-v5-interface.md), and
[team plan](docs/version/native-v5-team-plan.md). The pinned `--native-v5` Test262
gate is intentionally small and zero-skip; broader V5 directories remain a
diagnostic scan.

Native V6 provides the core builtin and coercion milestone. It standardizes
primitive wrappers and object-aware conversions, and connects String, Number,
Boolean, Math, Error, and core JSON through independently owned modules. See
the [Native V6 scope](docs/version/native-v6-scope.md),
[shared interface](docs/version/native-v6-interface.md), and
[team plan](docs/version/native-v6-team-plan.md). Its pinned Test262 gate passes 7/7;
the six-directory diagnostic scan passes 1,489/2,199 after the Track A and B
merge. Map/Set, RegExp, Date,
Promise, advanced JSON callbacks, and new language syntax remain deferred.

Native V7 is the stability and performance-evidence milestone. It does not add
new JavaScript syntax; instead it freezes contracts for resource budgets,
large-allocation guards, non-moving mark-and-sweep GC, native script caching,
crash-safe Test262 dashboards, and benchmark reporting. See the
[Native V7 scope](docs/version/native-v7-scope.md),
[shared interface](docs/version/native-v7-interface.md), and
[team plan](docs/version/native-v7-team-plan.md).

Native V8 development has started as a three-track parallel feature batch:
[Native V8 scope](docs/version/native-v8-scope.md),
[shared interface](docs/version/native-v8-interface.md), and
[team plan](docs/version/native-v8-team-plan.md). V8 focuses on frontend unlockers,
module runner infrastructure, and first-batch builtin skeletons.

V8-B first-stage module runner infrastructure is now available: module-flagged
Test262 cases enter native `module mode`, module code is strict by default,
module top-level `this` is `undefined`, relative dependency loading and module
registry deduplication are implemented, and focused module-code coverage is
201/599 passed with 0 skipped. The standard V8 lightweight scan is now
205/5,000 passed, 4,795 failed, and 0 skipped.

Native V9 development has started. V9 covers async/generator/for-of lowering,
Promise/job queue/iterator runtime, and Map/Set/Iterator builtins. Planning and
ownership live in [Native V9 scope](docs/version/native-v9-scope.md),
[shared interface](docs/version/native-v9-interface.md), and
[team plan](docs/version/native-v9-team-plan.md). Worker progress is tracked in
`reports/.version-report/v9-partA-report.md`, `reports/.version-report/v9-partB-report.md`, and
`reports/.version-report/v9-partC-report.md`.

V9-B first runtime substrate is now available: minimal Promise records,
single-settle Promise state helpers, deterministic FIFO native job queue,
native backend job draining, and array/string iterator fallback helpers.
Focused runtime coverage is `cargo test --no-default-features --test
native_v9_runtime` (5/5 passed). JS-visible Promise and collection builtins are
still owned by the V9-C builtin track.

The standard V9 lightweight scan is:

```sh
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/.native-test262-tmp/native-v9-scan-summary.json
```

It runs the locked 5,000-case manifest in
`reports/.test262/test262-scan-failure/native-v9-scan-failures.txt`. Initial result: 0/5,000 passed, 5,000
failed, and 0 skipped.

Native V10 setup is also available while V9-A continues. V10 covers
BigInt/numeric/unicode syntax tail work, TypedArray/ArrayBuffer/DataView runtime
substrate, and Date/Intl/Temporal builtin semantics. Planning and ownership
live in [Native V10 scope](docs/version/native-v10-scope.md),
[shared interface](docs/version/native-v10-interface.md), and
[team plan](docs/version/native-v10-team-plan.md). Worker progress is tracked in
`reports/.version-report/v10-partA-report.md`, `reports/.version-report/v10-partB-report.md`, and
`reports/.version-report/v10-partC-report.md`.

The standard V10 lightweight scan is:

```sh
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/.native-test262-tmp/native-v10-scan-summary.json
```

It runs the locked 5,000-case manifest in
`reports/.test262/test262-scan-failure/native-v10-scan-failures.txt`. Initial result: 645/5,000 passed,
4,355 failed, and 0 skipped.

V10-B first runtime substrate is now available: shared `ArrayBuffer` byte
storage, typed-array view metadata, DataView metadata, detach/range checks,
Number-backed element load/store, `Uint8Clamped` rounding, and endian-aware
DataView helpers. Focused runtime coverage is
`cargo test --no-default-features --test native_v10_runtime` (6/6 passed).
JS-visible TypedArray/ArrayBuffer/DataView constructor migration is still owned
by the V10-C builtin track.

Native V11 setup is also available while V10-A may continue. V11 covers RegExp
parser/static errors, object-model/descriptor precision, and RegExp/Annex B/
descriptor builtin sweeps. Planning and ownership live in
[Native V11 scope](docs/version/native-v11-scope.md),
[shared interface](docs/version/native-v11-interface.md), and
[team plan](docs/version/native-v11-team-plan.md). Worker progress is tracked in
`reports/.version-report/v11-partA-report.md`, `reports/.version-report/v11-partB-report.md`, and
`reports/.version-report/v11-partC-report.md`.

The standard V11 lightweight scan is:

```sh
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/.native-test262-tmp/native-v11-scan-summary.json
```

It runs the locked 5,000-case manifest in
`reports/.test262/test262-scan-failure/native-v11-scan-failures.txt`. The selector is installed, but the first
local scan attempt exceeded the 300s tool timeout and did not produce
`reports/.native-test262-tmp/native-v11-scan-summary.json`; rerun with a longer timeout or refresh
long-running samples before recording a baseline.

V11-B first object-key ordering precision fix is now available: the runtime now
treats `4294967295` as an ordinary string key rather than an array index, so
Object/Reflect own-key helpers preserve spec-aligned ordering at that boundary.
Focused runtime coverage is `cargo test --no-default-features --test
native_v11_runtime` (3/3 passed).

Planning note: `thoughts/plan_1_version.md` is retained as the pre-V8 planning record.
The active post-V8 roadmap is `thoughts/plan_1_version.md`.

V8 worker progress is tracked in per-part report files. AI agents and human
contributors should update the relevant report in the same change as their
implementation, without waiting for a separate reminder:

- `reports/.version-report/v8-partA-report.md` for frontend unlockers.
- `reports/.version-report/v8-partB-report.md` for module runner infrastructure.
- `reports/.version-report/v8-partC-report.md` for builtin skeletons and Test262 host work.

The standard V8 lightweight scan is:

```sh
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/.native-test262-tmp/native-v8-scan-summary.json
```

It runs the locked 5,000-case manifest in
`reports/.test262/test262-scan-failure/native-v8-scan-failures.txt`, selected from cases that did not pass in
the 2026-06-24 full direct run. The initial summary is
`reports/.native-test262-tmp/native-v8-scan-summary.json`: initially 0/5,000 passed, 4,504 failed,
and 496 skipped; after the first V8-B module runner pass it is 205/5,000
passed, 4,795 failed, and 0 skipped.

Runnable integration coverage lives in
[`tests/native_v2.rs`](tests/native_v2.rs),
[`tests/frontend_bytecode_v3.rs`](tests/frontend_bytecode_v3.rs), and
[`tests/native_v3.rs`](tests/native_v3.rs), with V5 end-to-end coverage in
[`tests/native_v5.rs`](tests/native_v5.rs) and VM/runtime coverage in
[`tests/native_v5_runtime.rs`](tests/native_v5_runtime.rs). A compact V3 sample is available at
[`examples/v3.js`](examples/v3.js).

## Parallel Development

The repeatable milestone workflow, merge order, Test262 selection rules, and
completion checklist are documented in
[Native version development workflow](docs/version-development-workflow.md).
From V8 onward, that workflow requires per-track worker reports and a
version-specific 5,000-case failed-Test262 lightweight scan, exposed as
`--native-vN-scan`.

Suggested ownership:

- Front end: `lexer/`, `ast/`, and `parser/`
- Compiler: `bytecode/`
- Execution: `vm/`, `runtime/`, and `builtins/`
- Integration: `backend/`, Test262, benchmarks, and reports

Avoid editing another team's implementation directory. When an upstream or
downstream stage is unfinished, replace it in a unit test:

```rust
let mut pipeline =
    NativePipeline::from_stages(fake_parser, compiler_under_test, fake_vm);
let value = pipeline.evaluate("ignored", &mut native_context)?;
```

This permits:

- parser tests without a compiler or VM;
- compiler tests with hand-built ASTs;
- VM tests with hand-built bytecode;
- runtime tests using values and objects directly;
- end-to-end differential tests against Boa.

Keep commits scoped, for example `feat(lexer): tokenize numeric literals` or
`fix(vm): preserve operands across calls`. Merge shared-contract changes before
dependent implementation branches to reduce conflicts.

## Testing Strategy

Every module should contain normal, boundary, and error-path tests beside its
implementation.

```text
Lexer:     source -> expected token sequence
Parser:    tokens/source -> expected AST
Compiler:  hand-built AST -> expected instructions
VM:        hand-built Chunk -> expected JsValue/error
Runtime:   direct object, property, scope, and heap operations
Builtins:  direct calls in a controlled native runtime
```

Run the project gate before every merge:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```

Boa differential tests should compare values, output, and error categories, but
ECMAScript specification text and Test262 remain authoritative when behavior
differs.

## Test262

Run the fixed Native V1-V4 milestone acceptance gates and the V4/V5 diagnostic
area scans:

```sh
cargo run -- test262 --native-v1 --jobs 1 --verbose
cargo run -- test262 --native-v2 --jobs 1 --verbose
cargo run -- test262 --native-v3 --jobs 1 --verbose
cargo run -- test262 --native-v4 --jobs 1 --verbose
cargo run -- test262 --native-v4-scan --jobs 4 --progress
cargo run -- test262 --native-v5 --jobs 1 --verbose
cargo run -- test262 --native-v5-scan --jobs 4 --progress
cargo run -- test262 --native-v6 --jobs 1 --verbose
cargo run -- test262 --native-v6-scan --jobs 4 --progress
cargo run --release --no-default-features -- test262 --native-v7 --jobs 1 --verbose
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/.native-test262-tmp/native-v7-frontend-summary.json
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/.native-test262-tmp/native-v8-scan-summary.json
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/.native-test262-tmp/native-v9-scan-summary.json
cargo test --test native_test262
```

These commands run the pinned files listed in the milestone documents and the
V4/V5 diagnostic directories through the
self-developed lexer, parser, compiler, VM, runtime, and minimal host-provided
Test262 harness. Every file is checked in default and strict mode; Boa is not
used. Current Native V4 results are recorded in the
[Native V4 Test262 report](reports/.test262/test262-analysis/native-v4-test262-report.md). Native V5
results are recorded in the
[Native V5 Test262 report](reports/.test262/test262-analysis/native-v5-test262-report.md). Native V7
diagnostic results and failure classification are recorded in the
[Native V7 Test262 report](reports/.test262/test262-analysis/native-v7-test262-report.md).

`--native-v4` is the curated zero-failure, zero-skip gate for the pinned V4
object-model files. `--native-v4-scan` is the broader diagnostic scan for
object, array, delete, in, instanceof, and minimal Object/Array/Function
built-in directories.

`--native-v5` is the curated zero-failure, zero-skip gate. `--native-v5-scan`
is the broader diagnostic scan for try, switch, let, and const directories.

`--native-v6` is the curated core-builtin gate. `--native-v6-scan` scans the
String, Number, Math, Boolean, Error, and JSON directories diagnostically.

`--native-v7` is the curated V7 integration gate. Because V7 is a stability,
limits, GC, cache, reporting, and benchmark-evidence milestone rather than a
new JavaScript feature milestone, this gate aggregates the zero-failure,
zero-skip Native V1-V6 Test262 files and runs them through the native backend.

`--native-v7-scan` is a lightweight frontend/cache-safety diagnostic scan over
a few thousand representative Test262 files. It covers selected language
literal/type/scope/function/global-code directories plus Function, String,
Symbol, and Reflect builtin directories. It is intentionally diagnostic:
skipped and failed tests are reported separately and never count as passes.

`--native-v8-scan` is the standard V8 lightweight integration scan. It runs
5,000 concrete Test262 files from `reports/.test262/test262-scan-failure/native-v8-scan-failures.txt`, sampled
from cases that did not pass in the locked 2026-06-24 full direct run. Use it
after focused V8-A/B/C tests and record deltas in the relevant V8 part report.

`--native-v9-scan` is the standard V9 lightweight integration scan. It runs
5,000 V9 hotspot failures from `reports/.test262/test262-scan-failure/native-v9-scan-failures.txt`, covering
async/generator/for-of, Promise/job queue, iterator runtime, and Map/Set areas.
Use it after focused V9-A/B/C tests and record deltas in the relevant V9 part
report.

V7 dashboard tests are reporting tools rather than ordinary pass/fail gates.
They run child suites separately so parent reporting survives child OOM, stack
overflow, panic, non-zero process exit, or a child-suite timeout:

```powershell
$env:AGENTJS_TEST262_SUITE_TIMEOUT_SECS = "300"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/built-ins"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/language"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

Dashboard JSON reports include separate `crashed_suites` and
`timed_out_suites` counters. Timed-out, crashed, skipped, and unsupported
tests are visible diagnostics, not passes.

Start with the feature directory affected by a change:

```sh
cargo run --release -- test262 \
  --root test262 \
  --suite test/language/expressions \
  --limit 100 \
  --jobs 8 \
  --verbose
```

Windows users can run:

```powershell
.\scripts\test262-sample.ps1 -Suite test/language/expressions -Limit 100
```

Do not count skipped tests as passes. Pull requests that affect language
behavior should report newly passed, newly failed, skipped, and regressed
cases. Run the full suite only after focused suites pass.

The current full Test262 stress report is a native-backend direct run over all
of `test/`, including `test/staging`: 14,035 of 53,379 tests passed, 38,507
failed, and 837 were skipped. It is useful for feature planning, not directly
comparable with older non-staging sharded reports. See
[reports/.test262/test262-analysis/test262-report.md](reports/.test262/test262-analysis/test262-report.md) and
[reports/.test262/test262-analysis/test262-analysis.md](reports/.test262/test262-analysis/test262-analysis.md).

`reports/.test262/test262-analysis/test262-analysis.md` is locked as the 2026-06-24 baseline analysis.
Do not rewrite it for later runs; create a new dated or versioned analysis file
for future full-suite analysis so the project keeps a clean audit trail.

## Benchmarking

```sh
cargo run --release -- bench 1000
```

This uses native and compares cold isolates, warm uncached runtimes, and warm
cached runtimes. Run the Boa comparison explicitly with:

```sh
cargo run --release --features boa-backend -- bench --backend boa 1000
```

Native and Boa results must be reported separately.

### JetStream 2 CLI

The pinned JetStream 2 source tree is at `benchmarks/JetStream2`. Build the
native-only release binary, then generate and run one or more CLI-compatible
workloads from the repository root:

```powershell
cargo build --release
.\scripts\run-jetstream2.ps1 -Tests richards,splay -Iterations 5
```

Omit `-Iterations` to use each official test plan's iteration count (usually
120). A one-iteration run is useful only as a functionality probe; use at least
five iterations for a score. The native script uses
`target/release/agentjs.exe` and `scripts/prepare-jetstream2.mjs`.

Run the same generated workloads with Node/V8 as a control:

```powershell
.\scripts\run-jetstream2-node.ps1 -Tests richards,splay -Iterations 5
```

Generated runners are written to `benchmarks/generated/`. Native plan metadata
and output are written to `reports/jetstream2/<test>-plan.json` and
`reports/jetstream2/<test>.txt`; Node/V8 output uses
`reports/jetstream2-node/`. A generated native runner can also be invoked
directly:

```powershell
.\target\release\agentjs.exe jetstream .\benchmarks\generated\richards.js
```

This adapter excludes WebAssembly and Web Worker tests and does not represent a
complete browser JetStream 2 score. See [docs/benchmark.md](docs/benchmark.md)
for methodology and [reports/jetstream2-report.md](reports/jetstream2-report.md)
for the latest native compatibility and performance results.

## Contribution Checklist

Before requesting review:

- modify only the intended module and necessary shared contracts;
- add focused unit tests for new behavior;
- run formatting, checks, tests, and Clippy;
- record relevant Test262 or benchmark changes;
- document unsupported behavior instead of silently falling back to Boa;
- avoid committing generated files, build output, or local configuration.

The contest release gate is a self-developed Rust engine with more than 60%
Test262 conformance, complete documentation, and measurable performance—not a
wrapper around the compatibility backend.
