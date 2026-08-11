# JetStream2 four-engine kernel comparison

- Workload iterations per process: `1`
- Warmup processes: `0`
- Measured processes: `1`
- Timeout: `120s`
- Peak RSS limit: `1536 MiB`

This is a portable comparison of JetStream2 JavaScript workload kernels, not the browser suite's official composite score.
Only cases that print their deterministic completion summary are performance-eligible.

| Test | agentjs kernel P50 | boa kernel P50 | quickjs kernel P50 | oxide kernel P50 |
|:---|---:|---:|---:|---:|
| ai-astar | RUNTIME-ERROR | 419.0ms | 56.0ms | MEMORY-LIMIT |
| crypto | 1.14s | 373.0ms | 62.0ms | INCOMPLETE |
| gaussian-blur | 3.75s | 890.0ms | 273.0ms | INCOMPLETE |
| hash-map | 12.92s | 871.0ms | 223.0ms | INCOMPLETE |
| cdjs | RUNTIME-ERROR | 3.27s | 252.0ms | INCOMPLETE |
| navier-stokes | 604.0ms | 182.0ms | 35.0ms | 713.0ms |
| raytrace | 11.81s | 1.22s | 223.0ms | INCOMPLETE |
| richards | 3.58s | 485.0ms | 111.0ms | 2.99s |
| splay | RUNTIME-ERROR | 269.0ms | 73.0ms | 728.0ms |
| stanford-crypto-sha256 | 3.18s | 246.0ms | 59.0ms | INCOMPLETE |

## Correctness

- `agentjs`: 7/10 passed
- `boa`: 10/10 passed
- `quickjs`: 10/10 passed
- `oxide`: 3/10 passed

## Reference / AgentJS kernel-time ratio

- `boa`: 0.150x (>1 means AgentJS is faster)
- `quickjs`: 0.033x (>1 means AgentJS is faster)
- `oxide`: 0.994x (>1 means AgentJS is faster)

## Peak RSS

- `agentjs` maximum observed: 233.70 MiB
- `boa` maximum observed: 214.51 MiB
- `quickjs` maximum observed: 164.91 MiB
- `oxide` maximum observed: 1578.68 MiB

## Reproduction

See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.
