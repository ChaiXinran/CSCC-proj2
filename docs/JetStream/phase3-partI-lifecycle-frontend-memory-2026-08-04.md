# Phase 3 Part I: Run Lifecycle and Frontend Memory

Baseline: `main@6f44bfa`

## Implemented

- One `AbsoluteDeadline` and `RunControl` now cover runner read, runtime setup,
  prelude, staged resources, launch, execution, and job draining.
- `Runtime::set_run_control(Option<RunControl>)` preserves the same absolute
  deadline across repeated `eval` calls. Without run control, the existing
  per-evaluation `RuntimeConfig.wall_clock_limit` behavior remains unchanged.
- JetStream supports `--thread-stack-mib N`, defaults to 32 MiB, and rejects
  values outside `4..=256`.
- Lexer checkpoints run at approximately 4096-byte intervals. Parser
  checkpoints run at approximately 1024-token or 256-statement intervals.
- `TokenText::{SourceSlice, Cooked(JsString)}` covers identifiers, private
  names, BigInt, strings, templates, and template raw text. Escaped and
  normalized values retain cooked text; other production tokens resolve from
  their source span.
- Structured phase diagnostics report elapsed time, source/token/bytecode
  counts, heap statistics, and GC metrics. The external sampler records the
  phase associated with every RSS sample and the peak RSS phase.

## Validation

- `cargo fmt --all -- --check`: PASS
- `cargo check --locked --all-targets`: PASS
- `cargo test --locked --all-targets`: PASS
- `cargo clippy --locked --all-targets -- -D warnings`: PASS
- `cargo test --release --no-default-features --test native_test262`: 15/15 PASS
- `richards`, 1-second deadline: `RuntimeLimit` after approximately 1.14 s
- `crypto`, stack 8/16/32 MiB: PASS in all three configurations

## Memory Findings

The 1.5 GiB protected runs still classify all three large workloads as
`MEMORY_LIMIT`:

| Workload | Peak RSS | Peak phase |
| --- | ---: | --- |
| jsdom-d3-startup | 1580.3 MiB | job drain |
| WSL | 1613.5 MiB | job drain |
| threejs | 1729.2 MiB | job drain |

Parsing and compilation remain in the tens of MiB. The large increase starts
after launch while benchmark jobs execute, so these failures are not frontend
token/AST peaks. In particular, threejs parsed its 1.25 MiB library resource in
about 55 ms and entered launch with an estimated 22.3 MiB runtime heap, then
reached the protected RSS limit after about 142.4 seconds in job drain. The
sampler terminated the process and confirmed that no runner process remained.

The WSL GC threshold matrix further narrows the remaining issue:

| Threshold | Result | Peak RSS | Detail |
| ---: | --- | ---: | --- |
| 1,000,000 | MEMORY_LIMIT | 1613.5 MiB | job-drain growth |
| 100,000 | MEMORY_LIMIT | 1546.0 MiB | job-drain growth |
| 10,000 | ENGINE_FAILURE | 26.5 MiB | `missing object` after collection |

The default GC policy is therefore unchanged. Lowering it would trade the OOM
for incorrect reachability semantics.

## Integration Hooks

Part I does not edit `src/bytecode/compiler.rs`. After the G branch is merged,
the integrator should thread `FrontendControl` into the compiler and checkpoint
at the compiler entry plus every approximately 1024 emitted instructions or
256 visited AST nodes.

Dynamic `eval` and `Function` already execute under the shared VM deadline, but
their frontend parsing path cannot cooperatively stop until the same control is
threaded through the compiler-owned dynamic-source boundary. The 10k GC
`missing object` failure must be fixed in GC root ownership before adopting a
lower or adaptive JetStream threshold.
