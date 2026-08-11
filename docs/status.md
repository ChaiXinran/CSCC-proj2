# Current Project Status

AgentJS is a native Rust JavaScript runtime with its own lexer, parser, AST,
bytecode compiler, VM, runtime, object model, GC, and standard builtins. Boa
and QuickJS are reference engines only and are not fallback execution paths.

## Current correctness

The authoritative full Test262 result is
`Test262-final/full-test262-summary.json`:

| Total | Passed | Failed | Skipped | Conformance | Elapsed |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 53,379 | 48,557 | 4,820 | 2 | 90.9665% | 452.354 s |

Historical per-version and 85%-target evidence is consolidated under
`reports/`. Fixed V8-V12 diagnostic manifests that remain executable CLI
inputs live in `src/test262_manifests/`; they are not reports.

## Current performance and product surfaces

- JetStream 2 and focused workloads are run through the scripts documented in
  `docs/benchmark.md`; raw local measurements are ignored under `reports/`.
- Runtime optimizations include shared chunks, compact property storage,
  local/upvalue slots, shapes and property ICs, adaptive GC, and side-registry
  reclamation.
- `dist/AgentJS-Demo.exe` provides the integrated chat/demo and three-engine
  comparison surface. Test262 accuracy questions read the official summary
  before searching or rerunning the suite.

## Known remaining work

- Complete the remaining Intl/Temporal, RegExp, module, and host-capability
  conformance gaps represented by the 4,820 failures.
- Continue reducing memory and runtime cost on complex JetStream workloads.
- Keep the canonical documents and consolidated reports synchronized with
  behavior changes.

## Documentation authority

- Architecture: `docs/architecture.md`
- Shared interfaces: `docs/interface-spec.md`
- Correctness policy: `docs/correctness_delivery_gate.md`
- Benchmark methodology: `docs/benchmark.md`
- Development workflow: `docs/version-development-workflow.md`
- Historical design decisions: `docs/runtime-evolution.md`
- Measured results and delivery history: `reports/`
