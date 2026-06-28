# Benchmark Plan

Performance results must be reproducible and must compare like-for-like release
builds on the same machine.

## Local Microbenchmark

```sh
cargo build --release
target/release/agentjs bench 1000
```

The standard build is native-only. Build and select Boa explicitly when
collecting compatibility-backend comparison data:

```sh
cargo build --release --features boa-backend
target/release/agentjs bench --backend boa 1000
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

## Native V7 Evidence

Native V7 benchmark evidence must be recorded separately from Boa-backed
baselines. At minimum, capture:

- `cargo build --release --no-default-features` binary size;
- cold native isolate latency;
- warm native uncached latency;
- warm native cached latency with `script_cache_capacity > 0`;
- peak resident memory when available on the platform;
- top-level Test262 dashboard totals plus crashed- and timed-out-suite counts;
- `test/built-ins` and `test/language` child-dashboard summaries;
- same-machine reference results for Boa, QuickJS, and the JetStream Node/V8
  adapter where the workload is supported.

Recommended reporting commands:

```powershell
cargo run --release --no-default-features -- bench 1000

$env:AGENTJS_TEST262_SUITE_TIMEOUT_SECS = "300"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/built-ins"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture

$env:AGENTJS_TEST262_SUITE = "test/language"
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

The V7 dashboard percentages are diagnostic. Crashed, timed-out, and skipped
suites must be reported explicitly and must never be counted as passes.
