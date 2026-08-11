# JetStream2 four-engine kernel comparison

`run_four_engine.py` compares AgentJS, Boa, QuickJS, and Oxide using identical
self-contained JavaScript files generated from the pinned JetStream2 workload
sources. It calls each workload's `Benchmark.runIteration()` directly and
requires the deterministic completion summary before accepting timing data.

This is a portable JavaScript-engine comparison, not the browser JetStream
suite's official composite score. Browser APIs, WebAssembly workloads, worker
tests, and full-driver Promise lifecycle behavior are outside this runner.

Compatibility scan:

```powershell
python benchmarks/jetstream/run_four_engine.py `
  --iterations 1 --warmup 0 --repeat 1 `
  --out-dir benchmarks/jetstream/results/four-engine-smoke
```

After identifying the four-engine common passing set, run multiple processes:

```powershell
python benchmarks/jetstream/run_four_engine.py `
  --tests n-body-SP,crypto-sha1-SP,crypto-md5-SP,3d-cube-SP,navier-stokes,richards `
  --iterations 1 --warmup 2 --repeat 7 `
  --out-dir benchmarks/jetstream/results/four-engine
```

The report records kernel time, process wall time, peak RSS, correctness
status, all raw samples, pinned revisions, executable sizes, and SHA-256
fingerprints. Ratios use only cases passed by AgentJS and the reference engine.
