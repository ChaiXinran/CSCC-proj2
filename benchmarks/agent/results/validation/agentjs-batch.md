# AgentBench 2.0

- Mode: `batch`
- Warmup: `0`
- Repeat: `1`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | node P50 | agentjs RSS | node RSS |
|:---|:---|---:|---:|---:|---:|
| general | json-record-aggregate | - | - | - | - |
| general | object-property-hot-loop | 10.00s | 55.6ms | - | - |
| general | rule-filter-dense-window | 977.4ms | 70.1ms | - | - |
| general | string-cleanup-replace-window | 205.8ms | 80.1ms | - | - |
| general | string-log-token-slice | 787.3ms | 49.4ms | - | - |
| general | tool-result-schema-filter | 1177.8ms | 60.4ms | - | - |

## Correctness

- `agentjs`: 5/6 cases passed
- `node`: 5/6 cases passed

## Geometric-mean speedup versus AgentJS

- `node`: all=0.05491490089501197x, general=0.05491490089501197x, pressure=-x

## Reproduction

See `environment.json`/the JSON report for machine, compiler, command and binary fingerprints.
