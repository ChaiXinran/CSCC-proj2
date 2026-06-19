# JetStream 2 Performance Report

Test date: 2026-06-19 (UTC+08:00)

- JetStream branch: `JetStream2.0`
- JetStream revision: `60cdba17bef0dcdb3fca2263e3916c3c45bfb7c2`
- AgentJS: 0.1.0 release build, 22,277,120-byte binary
- Reference: Node.js 22.17.0 / V8
- Platform: Windows NT 10.0.26200.0, x86-64
- Iterations: official 120 for every selected test

JetStream 2 scores are computed by the official driver. Higher is better. For
most JavaScript tests, the individual score is the geometric mean of startup,
worst-case, and average scores.

## Results

| Test | AgentJS | Node/V8 | Node ÷ AgentJS |
| --- | ---: | ---: | ---: |
| richards | 2.804 | 920.943 | 328.44× |
| splay | 4.104 | 174.194 | 42.44× |
| navier-stokes | 6.765 | 1,046.808 | 154.74× |
| crypto | 4.228 | 1,507.437 | 356.54× |
| ai-astar | 2.691 | 760.567 | 282.63× |
| stanford-crypto-sha256 | 5.924 | 923.276 | 155.85× |
| **Selected-subset geometric mean** | **4.169** | **749.849** | **179.88×** |

## Interpretation

The largest gaps occur in `crypto`, `richards`, and `ai-astar`, pointing to
integer arithmetic, object/property access, arrays, and long-running loop
dispatch as the highest-priority optimization areas. `splay` has the smallest
relative gap, though AgentJS still trails the JIT reference substantially.

Several AgentJS tests slow down after startup: their average and worst-case
scores are lower than their first-iteration score. This suggests allocation,
garbage collection, or interpreter-state degradation during sustained loads.

## Scope and Caveats

This is a six-test, pure-JavaScript CLI subset, not the complete 64-subtest
browser JetStream 2 score. WebAssembly and Web Worker tests are excluded because
AgentJS does not expose those host capabilities. The adapter embeds official
test-plan files and supplies the shell functions required by JetStream's CLI
path. It renames only the driver's internal base class to avoid sharing its
global lexical name with benchmark payload classes; workload and scoring code
are otherwise unchanged.

Raw outputs are stored in `reports/jetstream2/` and
`reports/jetstream2-node/`.

