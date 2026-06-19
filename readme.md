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

These CLI commands currently use `BackendKind::Boa`. The Native backend remains
explicitly unsupported at the public runtime boundary until its complete
source-to-value pipeline is ready.

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
ChunkExecutor::execute_chunk      Chunk   -> JsValue
NativePipeline::evaluate          source  -> JsValue
```

`Token`, `Program`, `Chunk`, `Instruction`, and `JsValue` are important shared
data contracts. Changes to these types or the three traits require team review.
Implementation details should remain inside their owning directory.

The complete native pipeline can be called directly during development:

```rust
use agentjs::contracts::{JsValue, NativePipeline};

fn run_native_pipeline() -> Result<(), agentjs::NativeError> {
    let value = NativePipeline::default().evaluate("")?;
    assert_eq!(value, JsValue::Undefined);
    Ok(())
}
```

Only an empty program is supported by the default Native pipeline at present.
This is an intentional scaffold, not a conformance claim.

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
let value = pipeline.evaluate("ignored")?;
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
