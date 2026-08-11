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
