# AgentJS

AgentJS is a Rust-based lightweight JavaScript execution runtime for AI agent
workloads. It focuses on short-lived, high-frequency script calls, isolated
global state, restricted host capabilities, measurable startup cost, and
ECMAScript conformance through the bundled `test262/` suite.

The repository also contains `boa/` and `quickjs/` as implementation references.
The current bootstrap backend uses Boa's parser and VM; AgentJS adds its own
isolate lifecycle, resource policy, CLI, Test262 orchestration, and benchmark
workflow. See [implementation status](docs/status.md) for the exact boundary.

Clone with submodules, or run `git submodule update --init --recursive` after
cloning. Pinned revisions are listed in
[docs/dependencies.md](docs/dependencies.md).

## Requirements

- Rust 1.91 or newer
- A C toolchain only when building the QuickJS reference
- Linux, macOS, or Windows

## Build and Run

```sh
cargo build --release
cargo run -- eval "Array.from({length: 5}, (_, i) => i * i)"
cargo run -- run examples/hello.js
cargo run -- repl
```

The default `conformance` feature enables Intl, Temporal, and experimental
language support for the highest Test262 coverage. Build a smaller agent binary
with `cargo build --release --no-default-features`.

Independent `Engine::execute` calls receive fresh isolates. A persistent
`Runtime` is available for related calls and REPL use; it keeps a bounded LRU
of parsed and compiled scripts for high-frequency repeated calls. JavaScript
has no direct filesystem, process, or network API.

## Test

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Run a focused Test262 sample first:

```sh
cargo run --release -- test262 \
  --root test262 \
  --suite test/language/expressions \
  --limit 1000 \
  --jobs 8 \
  --json reports/test262-sample.json
```

Run the complete suite by changing `--suite` to `test` and removing `--limit`.
Module tests are currently reported as skipped, never as passed.

The pinned June 19, 2026 sharded run executed 47,516 non-staging tests and
passed 45,310 (95.36% of executed tests). Counting every remaining unexecuted
test as a failure still gives an 87.31% full non-staging lower bound. See
[reports/test262-report.md](reports/test262-report.md).

## Benchmark

```sh
cargo run --release -- bench 1000
```

This measures cold isolate creation and warm isolate reuse. The broader
comparison methodology is documented in [docs/benchmark.md](docs/benchmark.md).

A pinned JetStream 2.0 pure-JavaScript subset has also been run with the
official 120 iterations. AgentJS scored a 4.169 geometric mean across six
selected workloads; Node/V8 scored 749.849 on the same generated inputs. See
[reports/jetstream2-report.md](reports/jetstream2-report.md).

## Contest Targets

The project follows the 2026 OS Functional Challenge requirements:

- Rust implementation and native binaries for Linux, macOS, and Windows;
- more than 60% of ECMAScript Test262;
- complete technical documentation;
- competitive benchmark results;
- measurable innovation beyond simply wrapping another engine.

The 60% result is a release gate, not a current unverified claim. Architecture
and the native optimization roadmap are in [docs/architecture.md](docs/architecture.md).
