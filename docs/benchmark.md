# Benchmark Plan

Performance results must be reproducible and must compare like-for-like release
builds on the same machine.

## Local Microbenchmark

```sh
cargo build --release
target/release/agentjs bench 1000
```

Also record a minimal build:

```sh
cargo build --release --no-default-features
```

The command reports:

- cold execution: create an isolate and run one short script;
- warm uncached execution: reuse one isolate but parse each call;
- warm cached execution: reuse one isolate and its bounded script cache.

Record CPU, OS, Rust version, commit, binary size, and peak resident memory with
every result.

## Reference Engines

Build reference engines in release mode:

```sh
cargo build --release --manifest-path boa/Cargo.toml -p boa_cli
make -C quickjs
```

Use identical JavaScript inputs for AgentJS, Boa, and QuickJS. Run at least
five samples after one warm-up and report median plus p95. Do not compare debug
builds or silently exclude failures.

## JetStream 2

The pinned official source is under `benchmarks/JetStream2`. Generate and run a
single CLI-compatible JavaScript subtest with:

```powershell
scripts/run-jetstream2.ps1 -Tests richards -Iterations 5
```

Omit `-Iterations` to use the official count (usually 120). The adapter embeds
only the files declared by the official test plan and leaves benchmark source
and scoring unchanged. WebAssembly and Web Worker tests are excluded until the
runtime implements those host capabilities. Scores from this CLI subset must
not be presented as the complete browser JetStream 2 score.

Run the same generated workloads on Node/V8 as a reference:

```powershell
scripts/run-jetstream2-node.ps1 -Tests richards,splay -Iterations 5
```

## Conformance

```sh
target/release/agentjs test262 --root test262 --suite test \
  --jobs 8 --json reports/test262.json
```

Pin the Test262 commit in the report. Skipped tests are not counted as passed;
the displayed conformance rate is `passed / total`.
