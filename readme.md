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

Only an empty program is supported by the default Native pipeline at present.
This is an intentional scaffold, not a conformance claim.
The first end-to-end implementation target is defined in
[Native V1 expression milestone](docs/native-v1-scope.md).
The next collaborative milestone is defined in
[Native V2 control-flow milestone](docs/native-v2-scope.md), with shared
contracts in [Native V2 interface specification](docs/native-v2-interface.md).
[Native V3 control-flow milestone](docs/native-v3-scope.md), with shared
contracts in [Native V3 interface specification](docs/native-v3-interface.md).

## Parallel Development

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

Run the fixed Native V1 acceptance gate:

```sh
cargo run -- test262 --native-v1 --jobs 1 --verbose
cargo run -- test262 --native-v2 --jobs 1 --verbose
cargo test --test native_test262
```

This runs the six official expression tests listed in
`docs/native-v1-scope.md` through the self-developed lexer, parser, compiler,
VM, runtime, and minimal host-provided Test262 harness. Every file is checked
in default and strict mode; Boa is not used.

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
