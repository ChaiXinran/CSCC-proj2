# Native JetStream 2 CLI Report

Date: 2026-06-28  
Commit: `8015046` (dirty worktree)  
Platform: Windows NT 10.0.26200.0  
Toolchain: `rustc 1.91.0`, `cargo 1.91.0`  
Binary: `target/release/agentjs.exe`, 7,056,896 bytes

## Method

The native release binary was built with:

```powershell
cargo build --release --no-default-features
```

The six pinned CLI candidates were generated from the official JetStream plan
and run through `scripts/run-jetstream2.ps1`. One iteration is still a complete
workload iteration, but it is only a functionality probe and is not a valid
JetStream score. The normal minimum probe uses five iterations.

## Results

| Workload | Iterations | Result | Evidence |
| --- | ---: | --- | --- |
| richards | 5 | Timeout | Exceeded both 120 s and 600 s; a 1-iteration retry also exceeded 300 s |
| splay | 1 | Timeout | Exceeded 120 s |
| navier-stokes | 1 | Timeout | Exceeded 120 s |
| crypto | 1 | Crash | Rust main-thread stack overflow; exit `-1073741571` after about 0.65 s |
| ai-astar | 1 | Compile failure | Dynamic function constant pool exceeded the `u16` index range |
| stanford-crypto-sha256 | 1 | Compile failure | Native compiler rejected a `break` statement as outside a loop or switch |

No native workload produced a valid score. Timed-out, crashed, and rejected
workloads are not counted as passes.

## Control

The generated `richards` runner was executed with Node/V8 for five iterations.
It completed successfully with score `945.522` and reported benchmark wall time
of 27 ms. This confirms that resource embedding, iteration selection, execution,
and scoring remain valid in the generated runner.

## Adapter Findings

- Native global properties do not automatically provide same-named lexical
  bindings required by the upstream CLI driver.
- Explicit derived-class `super()` calls currently fail in native, so the CLI
  adapter links the small driver wrapper hierarchy through prototypes.
- The native JetStream command now installs the existing minimal host output,
  and the adapter executes embedded payloads in one dynamic function scope.
- The PowerShell runner now records stderr and rejects non-zero native exits.
- `command_jetstream` also treats a printed `JetStream2 failed:` line as an
  error so caught Promise failures cannot become false passes.

## Verification Notes

`node --check` passed for both adapter scripts, and the Node/V8 Richards control
passed. A final native rebuild was blocked after unrelated worktree changes
removed the tracked V8-V11 scan manifests required by `include_str!`; therefore
the new printed-failure exit handling in `src/main.rs` is not yet verified in a
fresh binary. Full `cargo fmt --all -- --check` is also blocked by unrelated
formatting differences in files outside this JetStream change.
