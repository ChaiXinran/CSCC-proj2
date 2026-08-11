# JetStream2 four-engine kernel comparison

- Workload iterations per process: `1`
- Warmup processes: `2`
- Measured processes: `7`
- Timeout: `180s`
- Peak RSS limit: `1536 MiB`

This is a portable comparison of JetStream2 JavaScript workload kernels, not the browser suite's official composite score.
Only cases that print their deterministic completion summary are performance-eligible.

| Test | agentjs kernel P50 | boa kernel P50 | quickjs kernel P50 | oxide kernel P50 |
|:---|---:|---:|---:|---:|
| n-body-SP | 1.22s | 347.0ms | 74.0ms | 970.0ms |
| crypto-sha1-SP | 4.03s | 527.0ms | 200.0ms | 3.90s |
| crypto-md5-SP | 4.14s | 483.0ms | 129.0ms | 4.02s |
| 3d-cube-SP | 1.45s | 364.0ms | 97.0ms | 1.08s |
| navier-stokes | 594.0ms | 218.0ms | 34.0ms | 800.0ms |
| richards | 3.72s | 625.0ms | 116.0ms | 3.54s |

## Correctness

- `agentjs`: 6/6 passed
- `boa`: 6/6 passed
- `quickjs`: 6/6 passed
- `oxide`: 6/6 passed

## Reference / AgentJS kernel-time ratio

- `boa`: 0.202x (>1 means AgentJS is faster)
- `quickjs`: 0.047x (>1 means AgentJS is faster)
- `oxide`: 0.945x (>1 means AgentJS is faster)

## Peak RSS

- `agentjs` maximum observed: 27.04 MiB
- `boa` maximum observed: 16.43 MiB
- `quickjs` maximum observed: 7.02 MiB
- `oxide` maximum observed: 1123.82 MiB

## Reproduction

See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.
