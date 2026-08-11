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
| n-body-SP | 1.37s | 387.0ms | 82.0ms | 1.13s |
| base64-SP | 1.37s | 337.0ms | 130.0ms | INCOMPLETE |
| crypto-sha1-SP | 4.50s | 588.0ms | 144.0ms | 4.41s |
| crypto-md5-SP | 4.86s | 567.0ms | 135.0ms | 4.05s |
| 3d-cube-SP | 1.87s | 464.0ms | 115.0ms | 1.09s |

## Correctness

- `agentjs`: 5/5 passed
- `boa`: 5/5 passed
- `quickjs`: 5/5 passed
- `oxide`: 4/5 passed

## Reference / AgentJS kernel-time ratio

- `boa`: 0.192x (>1 means AgentJS is faster)
- `quickjs`: 0.050x (>1 means AgentJS is faster)
- `oxide`: 0.793x (>1 means AgentJS is faster)

## Peak RSS

- `agentjs` maximum observed: 27.39 MiB
- `boa` maximum observed: 16.97 MiB
- `quickjs` maximum observed: 5.62 MiB
- `oxide` maximum observed: 1123.87 MiB

## Reproduction

See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.
