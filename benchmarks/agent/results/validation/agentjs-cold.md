# AgentBench 2.0

- Mode: `cold`
- Warmup: `0`
- Repeat: `1`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | node P50 | agentjs RSS | node RSS |
|:---|:---|---:|---:|---:|---:|
| general | json-record-aggregate | - | - | - | - |
| general | object-property-hot-loop | 1965.4ms | 69.9ms | - | - |
| general | rule-filter-dense-window | 462.1ms | 64.3ms | - | - |
| general | string-cleanup-replace-window | 145.9ms | 48.8ms | - | - |
| general | string-log-token-slice | 413.2ms | 52.7ms | - | - |
| general | tool-result-schema-filter | 404.5ms | 51.9ms | - | - |

## Correctness

- `agentjs`: 5/6 cases passed
- `node`: 5/6 cases passed

## Geometric-mean speedup versus AgentJS

- `node`: all=0.12205427027955242x, general=0.12205427027955242x, pressure=-x

## Reproduction

See `environment.json`/the JSON report for machine, compiler, command and binary fingerprints.
