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
| sync-fs | RUNTIME-ERROR | 558.0ms | 126.0ms | INCOMPLETE |
| js-tokens | RUNTIME-ERROR | 2.14s | 84.0ms | INCOMPLETE |

## Correctness

- `agentjs`: 0/2 passed
- `boa`: 2/2 passed
- `quickjs`: 2/2 passed
- `oxide`: 0/2 passed

## Reference / AgentJS kernel-time ratio

- `boa`: - (>1 means AgentJS is faster)
- `quickjs`: - (>1 means AgentJS is faster)
- `oxide`: - (>1 means AgentJS is faster)

## Peak RSS

- `agentjs` maximum observed: 117.17 MiB
- `boa` maximum observed: 20.96 MiB
- `quickjs` maximum observed: 8.94 MiB
- `oxide` maximum observed: 14.27 MiB

## Reproduction

See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.
