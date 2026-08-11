# AgentBench 2.0

- Mode: `cold`
- Warmup: `0`
- Repeat: `1`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | node P50 | agentjs RSS | node RSS |
|:---|:---|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 454.2ms | 72.0ms | 12.86MiB | 46.24MiB |
| pressure | json-parse-transform | 230.2ms | 44.8ms | 12.32MiB | 34.66MiB |
| general | json-record-aggregate | 292.6ms | 55.6ms | 10.27MiB | 34.68MiB |
| pressure | large-index-dense-array | 632.6ms | 54.3ms | 13.99MiB | 39.23MiB |
| general | object-property-hot-loop | 1973.0ms | 57.0ms | 31.83MiB | 40.63MiB |
| general | rule-filter-dense-window | 469.4ms | 61.3ms | 101.67MiB | 52.62MiB |
| pressure | short-lived-object-churn | 58.29s | 56.0ms | 29.48MiB | 41.46MiB |
| general | startup-noop | 20.8ms | 46.9ms | 6.69MiB | 33.42MiB |
| pressure | string-ascii-index-scan | 383.2ms | 52.7ms | 7.39MiB | 37.37MiB |
| general | string-cleanup-replace-window | 155.2ms | 49.6ms | 7.61MiB | 38.24MiB |
| general | string-log-token-slice | 453.6ms | 50.5ms | 8.46MiB | 38.03MiB |
| general | tool-result-schema-filter | 417.0ms | 45.1ms | 19.38MiB | 36.42MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `node`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `node`: all=0.10669451011360397x, general=0.1804099804367014x, pressure=0.051141821690693215x (>1 means AgentJS is faster)

## Reproduction

See `environment.json`/the JSON report for machine, compiler, command and binary fingerprints.
