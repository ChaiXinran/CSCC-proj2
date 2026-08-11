# Optimization, Memory, Atomics, and Demo History

This document consolidates the former Atomics, JetStream, Phase 2/3/4, and
AgentJS Demo version reports.

| Track | Retained result |
| --- | --- |
| Atomics / modules | Atomics and module-related focused correctness work, including concurrency checks across one-job and four-job scans. |
| Phase 2 hot paths | Compact property storage, shared property-name backing, tombstone deletion, and hash-map measurement. |
| Phase 3 | Shape and monomorphic property inline-cache work plus integration validation. |
| Phase 4 memory | Runtime memory accounting, adaptive GC, stable-arena integration, and jsdom/D3 RegExp string-sharing. The protected jsdom/D3 run reduced peak RSS from 1606.8 MiB to 88.0 MiB. |
| JetStream 2 | Performance diagnosis and optimization against Boa and the competitor workloads; raw local reruns remain ignored. |
| AgentJS Demo | Integrated chat/demo application, three-engine comparison, persistent script-cache benchmark, and direct Test262 accuracy lookup. |

The Demo reads `Test262-final/full-test262-summary.json` first, searches the
project only when that file is absent, and reruns the full suite as the final
fallback. Its 37 orchestration tests passed and the executable was rebuilt
after this behavior was finalized.

## Retained measured evidence

Phase 2 local-slot lowering improved the five-workload JetStream median
geometric mean by approximately 21.6%. Representative median changes were
crypto -41.69%, navier-stokes -45.07%, splay -7.43%, richards -0.95%, and
raytrace +0.75%; no common passing workload regressed beyond 5%.

Phase 3 upvalue-slot work preserved the directly reproducible Test262 baseline
and improved a closure-dense 500k-call workload by 19.87%. Its protected
JetStream integration matrix established passing coverage for crypto,
gaussian-blur, hash-map, cdjs, intl, mobx, navier-stokes, and web-ssr while
identifying memory limits in ai-astar, jsdom/D3, threejs, and WSL workloads.

Phase 4 added side-registry lifetime management and adaptive GC. The later
shared-string RegExp fix reduced protected jsdom/D3 peak RSS from 1606.8 MiB
to 88.0 MiB, after which execution reached a separate semantic error instead
of the memory limit.

The Agent Host and desktop Demo use a constrained render-event contract rather
than HTML interpretation. The packaged Demo compares AgentJS, Boa, and the
local competitor runtime and includes the official Test262 summary.
