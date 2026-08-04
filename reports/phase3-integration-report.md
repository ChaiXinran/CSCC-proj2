# Phase 3 G/I Integration Report

Integration base: `main@dd69f5d` (G `89f9283`, I `dedbc4a`)

## Integration result

- Connected I's absolute run deadline to G's bytecode compiler.
- The compiler checks once at entry and then every 256 visited statement or
  expression nodes. This follows the shared interface without checking the
  clock for every emitted instruction.
- The bytecode layer stores only `Option<Instant>` and therefore does not add a
  reverse dependency on `engine::FrontendControl`.
- `NativeRuntime` installs the current absolute deadline before compilation,
  clears it afterwards, and classifies expiry as `FailureKind::RuntimeLimit`.
- The same path is used with and without the script cache.
- Added unit coverage for an expired entry deadline and the 256-node cadence.

## Cross-track validation

Richards was run from `benchmarks/generated/richards.js` with the external
resource root, a 32 MiB thread stack, diagnostics, and a one-second absolute
deadline. It parsed and compiled the prelude and resource, entered job drain,
and exited with:

```text
RuntimeLimit: execution error: wall-clock deadline exceeded
```

The diagnostic stream contained both I-track phase records and G-track name
resolution counters, including `load_upvalue_count`, confirming that the two
merged paths are active in the same protected run. The observed termination
was within the requested one-second budget plus normal process/reporting
overhead.

The merged Upvalue suite also passed all 8 tests, including dynamic-scope
deoptimization, async/generator capture, loop-created closures, fixed-hop
ancestor access, and the 70% eligible-access lowering threshold.

## Commands run

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --all-targets` | PASS |
| `cargo test --all-targets` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --no-default-features --test native_test262` | PASS, 15/15 |
| `cargo build --release --locked` | PASS |
| Richards, `--wall-clock-seconds 1 --thread-stack-mib 32 --diagnostics` | Expected `RuntimeLimit` |

## I-track memory evidence retained from the merged report

The Part I protected 1.5 GiB runs remain valid integration evidence:

| Workload | Result | Peak RSS | Peak phase |
| --- | --- | ---: | --- |
| `jsdom-d3-startup` | `MEMORY_LIMIT` | 1580.3 MiB | job drain |
| `WSL` | `MEMORY_LIMIT` | 1613.5 MiB | job drain |
| `threejs` | `MEMORY_LIMIT` | 1729.2 MiB | job drain |

These peaks occur after parsing and compilation. Compiler deadline integration
therefore closes the cancellation gap but does not claim to solve the remaining
runtime/GC growth.

## Follow-up: low-threshold GC root repair

The WSL 10k `missing object` failure was reproduced and fixed. Two native values
were previously invisible to tracing while allocations could trigger GC:

- a Promise job after it had been popped from the queue but before execution
  completed;
- the result arrays of callback-driven `map`, `filter`, and `flatMap` loops.

Active jobs now publish their carried values through temporary roots, and the
array results remain rooted for the full callback loop. Regression tests force
collections during Promise-job execution and allocating `map`/`filter`
callbacks.

The protected WSL rerun at threshold 10,000 completed 920 collections without
`missing object` and kept the reported engine heap near 16.6 MiB. It eventually
reached the unified 150-second runtime deadline. GC pause time was about 19.0
seconds, so 10k is now correct but remains too aggressive to become the default
without performance tuning.

The complete 19-workload, GC-threshold, and 8/16/32 MiB matrices were not
repeated in this integration change; Part I's protected measurements and stack
matrix are retained as the current evidence. No performance claim is made from
the one-second Richards deadline run.
