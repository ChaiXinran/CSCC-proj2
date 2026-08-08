# Phase 3 G/H/I Integration Report

Date: 2026-08-08

Integration base: `main@9ec36e0`

Tracks: G Upvalue Slot, H Shape / Property IC, I Lifecycle / Frontend Memory

## Outcome

The three tracks are integrated. One cross-track correctness regression was
found and fixed during the merged JetStream run: source introduced by direct
`eval` was compiled as if its nested closures had a fully static environment.
The web-ssr React bundle therefore loaded a function through an invalid upvalue
slot and later reported `undefined is not callable`.

`Compiler::compile_eval_program` now deoptimizes nested upvalues to name lookup
for eval-introduced source. This implements the dynamic-scope fallback required
by the shared interface while retaining Upvalue Slot for ordinary statically
compiled functions. The focused web-ssr rerun changed from `CALL_ERROR` to
`PASS` (`JETSTREAM_RUN_COMPLETE`). A structural compiler test verifies that
eval chunks contain no `LoadUpvalue` / `StoreUpvalue` instructions or residual
upvalue layouts.

I's unified diagnostics now also publish H's per-evaluation property-cache
metrics. The JetStream diagnostics script aggregates Get/Set hits and misses,
shape transitions, dictionary objects, and invalidations in its JSON output.

## Validation

Test262 was not run, per explicit user direction.

| Command / suite | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --locked --all-targets` | PASS |
| `cargo test --locked --lib` | PASS, 280/280 before the eval regression test; focused new test PASS afterwards |
| `cargo test --locked --test upvalue_slots` | PASS, 8/8 |
| `cargo test --locked --test native_gc` | PASS, 14/14 |
| `cargo test --locked --test native_classes` | PASS, 20/20 |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| `cargo build --release --locked` | PASS |
| `git diff --check` | PASS |

## JetStream 19-workload protected matrix

Configuration: release build, one generated staged runner at a time, 150-second
external timeout, 1536 MiB working-set limit, 32 MiB thread stack, GC threshold
1,000,000. The initial full matrix found the web-ssr integration regression;
the table below substitutes its post-fix focused rerun and is the final
consolidated result.

| Workload | Result | Wall time | Peak RSS |
| --- | --- | ---: | ---: |
| ai-astar | MEMORY_LIMIT | 12.214 s | 1537.4 MiB |
| crypto | PASS | 1.450 s | 164.7 MiB |
| gaussian-blur | PASS | 5.896 s | 259.6 MiB |
| hash-map | PASS | 9.491 s | 1234.9 MiB |
| cdjs | PASS | 15.073 s | 1072.0 MiB |
| intl | PASS | 1.199 s | 177.0 MiB |
| jsdom-d3-startup | MEMORY_LIMIT | 2.055 s | 1578.9 MiB |
| mobx | PASS | 6.423 s | 686.0 MiB |
| threejs | MEMORY_LIMIT | 103.982 s | 1537.1 MiB |
| validatorjs | CALL_ERROR | 0.535 s | 28.8 MiB |
| web-ssr | PASS (post-fix) | 7.609 s | 1336.3 MiB |
| WSL | MEMORY_LIMIT | 5.377 s | 1666.6 MiB |
| navier-stokes | PASS | 0.831 s | 29.5 MiB |
| raytrace | PASS | 13.054 s | 979.9 MiB |
| regexp | ENGINE_FAILURE | 0.764 s | 128.3 MiB |
| richards | PASS | 4.097 s | 361.4 MiB |
| splay | PASS | 4.435 s | 1339.3 MiB |
| stanford-crypto-sha256 | PASS | 6.712 s | 173.7 MiB |
| test-cdjs | PASS | 14.648 s | 1069.8 MiB |

Final classification: 12/19 PASS, 4 MEMORY_LIMIT, 2 CALL_ERROR, and 1
ENGINE_FAILURE. There were no orphaned runner processes. Compared with the
pre-Phase-3 staged matrix, splay moved from MEMORY_LIMIT to PASS. The remaining
regexp failure is the known staged Host-script environment mismatch; restoring
a concatenated `new Function` runner is not an acceptable workaround.

Raw full-matrix data is under `reports/phase3-integration-jetstream19/`; the
post-fix web-ssr data is under `reports/phase3-integration-web-ssr-fixed/`.

## H cache evidence

The post-integration hash-map rerun passed in 9.334 seconds at 1235.4 MiB peak
RSS. Aggregated diagnostics reported:

| Metric | Value |
| --- | ---: |
| Get hits / misses | 1431 / 220 |
| Set hits / misses | 0 / 2280 |
| Shape transitions | 84 |
| Dictionary objects | 22 |
| Invalidations | 8742 |

The Get hit rate among eligible observations was 86.67% across the complete
runner, including prelude and launch traffic. H's isolated five-run report
remains the performance comparison: hash-map median improved 12.78% versus its
frozen baseline. The zero Set hits and high invalidation count show that Set IC
specialization needs profiling before expanding to polymorphic IC.

## Stack and deadline matrices

Crypto passed with 8, 16, and 32 MiB thread stacks:

| Stack | Result | Wall time | Peak RSS |
| ---: | --- | ---: | ---: |
| 8 MiB | PASS | 1.355 s | 165.1 MiB |
| 16 MiB | PASS | 1.368 s | 159.7 MiB |
| 32 MiB | PASS | 1.451 s | 164.4 MiB |

Richards with a single one-second absolute deadline reached job drain and
returned `FailureKind::RuntimeLimit` after approximately one second. The same
deadline covered runner/resource parsing, compilation, execution, and jobs.
Compiler entry and 256-node cooperative checkpoints remain enabled.

## GC threshold matrix

WSL was sampled with the same 32 MiB stack and 1536 MiB protection:

| Threshold | Result | Limit | Peak RSS | Interpretation |
| ---: | --- | ---: | ---: | --- |
| 10,000 | TIMEOUT | 20 s | 423.6 MiB | no `missing object`; correct but GC-bound |
| 100,000 | MEMORY_LIMIT | 30 s | 1622.5 MiB | insufficient collection pressure |
| 1,000,000 | MEMORY_LIMIT | 30 s | 1561.3 MiB | default remains memory-heavy |

The prior GC-root corruption is closed: active Promise jobs and callback-built
array results are rooted, and the 10k run no longer fails semantically. A fixed
10k default is still unsuitable because collection throughput prevents WSL from
finishing within its budget. The next GC work should be adaptive rather than a
blind threshold reduction.

## Remaining issues and next phase

No undocumented shared-interface conflict was required for this integration.
The eval fallback is an implementation of the interface's existing direct-eval
rule, not a contract change.

Recommended next order:

1. Adaptive/generational GC work and allocation-pressure profiling for WSL,
   jsdom, threejs, and ai-astar.
2. Persistent Host-script environment semantics for staged regexp and other
   cross-file top-level declarations.
3. Diagnose validatorjs before classifying it as runner or builtin work.
4. Profile Set IC invalidations; only then consider prototype or polymorphic
   property IC.
5. Re-profile block locals before deciding whether Block Slot is worth the
   additional environment/deoptimization complexity.

The data does not support jumping directly to polymorphic IC: memory/GC and
Host-script semantics are still the larger integration blockers.
