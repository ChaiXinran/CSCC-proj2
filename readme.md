# AgentJS

AgentJS is a lightweight JavaScript engine written in Rust for short-lived,
high-frequency AI agent workloads. The repository currently provides a stable
Boa-backed compatibility runtime while a self-developed lexer, parser,
bytecode compiler, VM, runtime, and built-in library are developed in parallel.

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

These general CLI commands still default to `BackendKind::Boa`. The Native
backend now executes the V1 expression subset and can be selected explicitly;
unsupported syntax returns a categorized error instead of falling back to Boa.

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

- [Native V1 expressions](docs/native-v1-scope.md)
- [Native V2 control flow](docs/native-v2-scope.md) and its
  [shared interface](docs/native-v2-interface.md)
- [Native V3 functions and compound values](docs/native-v3-scope.md) and its
  [shared interface](docs/native-v3-interface.md)

The active collaborative milestone is the expanded
[Native V4 object semantics](docs/native-v4-scope.md), with contracts frozen in
the [Native V4 shared interface](docs/native-v4-interface.md). The object model,
accessors, constructors, sparse-array core, and the minimal `Object`, `Array`,
and `Function` builtin/intrinsic layer are connected. Remaining V4 work focuses
on broadening standard builtins and reducing the diagnostic Test262 failure set.
File ownership, C0–C3 execution subgroups, branch suggestions, and merge order
are defined in the [Native V4 team plan](docs/native-v4-team-plan.md).

Native V5 now has an initial Native VM/runtime integration for structured
completion, `try/catch/finally`, `switch`, and lexical `let`/`const`
semantics. See the
[Native V5 scope](docs/native-v5-scope.md),
[shared interface](docs/native-v5-interface.md), and
[team plan](docs/native-v5-team-plan.md). The pinned `--native-v5` Test262
gate is intentionally small and zero-skip; broader V5 directories remain a
diagnostic scan.

Native V6 provides the core builtin and coercion milestone. It standardizes
primitive wrappers and object-aware conversions, and connects String, Number,
Boolean, Math, Error, and core JSON through independently owned modules. See
the [Native V6 scope](docs/native-v6-scope.md),
[shared interface](docs/native-v6-interface.md), and
[team plan](docs/native-v6-team-plan.md). Its pinned Test262 gate passes 7/7;
the six-directory diagnostic scan passes 1,489/2,199 after the Track A and B
merge. Map/Set, RegExp, Date,
Promise, advanced JSON callbacks, and new language syntax remain deferred.

Native V7 is the stability and performance-evidence milestone. It does not add
new JavaScript syntax; instead it freezes contracts for resource budgets,
large-allocation guards, non-moving mark-and-sweep GC, native script caching,
crash-safe Test262 dashboards, and benchmark reporting. See the
[Native V7 scope](docs/native-v7-scope.md),
[shared interface](docs/native-v7-interface.md), and
[team plan](docs/native-v7-team-plan.md).

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
cargo run --release --no-default-features -- test262 --native-v7-scan --jobs 4 --json reports/native-v7-frontend-summary.json
cargo test --test native_test262
```

These commands run the pinned files listed in the milestone documents and the
V4/V5 diagnostic directories through the
self-developed lexer, parser, compiler, VM, runtime, and minimal host-provided
Test262 harness. Every file is checked in default and strict mode; Boa is not
used. Current Native V4 results are recorded in the
[Native V4 Test262 report](reports/native-v4-test262-report.md). Native V5
results are recorded in the
[Native V5 Test262 report](reports/native-v5-test262-report.md).

`--native-v4` is the curated zero-failure, zero-skip gate for the pinned V4
object-model files. `--native-v4-scan` is the broader diagnostic scan for
object, array, delete, in, instanceof, and minimal Object/Array/Function
built-in directories.

`--native-v5` is the curated zero-failure, zero-skip gate. `--native-v5-scan`
is the broader diagnostic scan for try, switch, let, and const directories.

`--native-v6` is the curated core-builtin gate. `--native-v6-scan` scans the
String, Number, Math, Boolean, Error, and JSON directories diagnostically.

`--native-v7-scan` is a lightweight frontend/cache-safety diagnostic scan over
a few thousand representative Test262 files. It covers selected language
literal/type/scope/function/global-code directories plus Function, String,
Symbol, and Reflect builtin directories. It is intentionally diagnostic:
skipped and failed tests are reported separately and never count as passes.

V7 dashboard tests are reporting tools rather than ordinary pass/fail gates.
They run child suites separately so parent reporting survives child OOM, stack
overflow, panic, or non-zero process exit:

```powershell
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/built-ins"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/language"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

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

The existing Test262 report is a **Boa-backed baseline**: 45,310 of 47,516
executed non-staging tests passed. It does not measure native engine
conformance. See [reports/test262-report.md](reports/test262-report.md).

## Benchmarking

```sh
cargo run --release -- bench 1000
```

This compares cold isolates, warm uncached runtimes, and warm cached runtimes.
JetStream methodology and results are documented in
[docs/benchmark.md](docs/benchmark.md) and
[reports/jetstream2-report.md](reports/jetstream2-report.md). Existing benchmark
numbers are also Boa-backed baselines; native results must be reported
separately.

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
