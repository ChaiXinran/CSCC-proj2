# JetStream2 four-engine kernel comparison

- Workload iterations per process: `3`
- Warmup processes: `1`
- Measured processes: `5`
- Timeout: `180s`
- Peak RSS limit: `1536 MiB`

This is a portable comparison of JetStream2 JavaScript workload kernels, not the browser suite's official composite score.
Only cases that print their deterministic completion summary are performance-eligible.

| Test | agentjs kernel P50 | boa kernel P50 | quickjs kernel P50 | oxide kernel P50 |
|:---|---:|---:|---:|---:|
| n-body-SP | 1.41s | 328.0ms | 66.0ms | 936.0ms |
| crypto-sha1-SP | 13.81s | 497.0ms | 120.0ms | MEMORY-LIMIT |
| crypto-md5-SP | 5.71s | 453.0ms | 117.0ms | MEMORY-LIMIT |
| 3d-cube-SP | 1.37s | 344.0ms | 94.0ms | 935.0ms |

## Correctness

- `agentjs`: 4/4 passed
- `boa`: 4/4 passed
- `quickjs`: 4/4 passed
- `oxide`: 2/4 passed

## Reference / AgentJS kernel-time ratio

- `boa`: 0.114x (>1 means AgentJS is faster)
- `quickjs`: 0.027x (>1 means AgentJS is faster)
- `oxide`: 0.673x (>1 means AgentJS is faster)

## Peak RSS

- `agentjs` maximum observed: 28.11 MiB
- `boa` maximum observed: 22.11 MiB
- `quickjs` maximum observed: 5.54 MiB
- `oxide` maximum observed: 1539.45 MiB

## Reproduction

See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.
