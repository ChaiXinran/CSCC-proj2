# AgentBench 2.0

- Mode: `batch`
- Warmup: `3`
- Repeat: `15`
- Batch iterations per process: `5`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | agentjs RSS | boa RSS |
|:---|:---|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 1105.8ms | 656.1ms | 26.99MiB | 76.20MiB |
| pressure | json-parse-transform | 97.5ms | 71.0ms | 20.00MiB | 20.75MiB |
| general | json-record-aggregate | 94.6ms | 37.5ms | 19.68MiB | 13.62MiB |
| pressure | large-index-dense-array | 1799.5ms | 1226.3ms | 42.41MiB | 35.85MiB |
| general | object-property-hot-loop | 187.4ms | 83.4ms | 23.12MiB | 14.63MiB |
| general | rule-filter-dense-window | 1866.9ms | 470.0ms | 112.07MiB | 40.52MiB |
| pressure | short-lived-object-churn | 148.4ms | 49.1ms | 20.40MiB | 14.51MiB |
| general | startup-noop | 19.2ms | 15.5ms | 7.06MiB | 11.28MiB |
| pressure | string-ascii-index-scan | 1596.6ms | 103.4ms | 8.38MiB | 12.67MiB |
| general | string-cleanup-replace-window | 358.6ms | 5475.8ms | 9.77MiB | 14.03MiB |
| general | string-log-token-slice | 1231.0ms | 2783.0ms | 10.30MiB | 13.54MiB |
| general | tool-result-schema-filter | 120.0ms | 48.7ms | 28.00MiB | 17.10MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `boa`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=0.619x, general=0.906x, pressure=0.363x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=0.979x, general=0.839x, pressure=1.216x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10891776 bytes
- `boa`: 29693440 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
