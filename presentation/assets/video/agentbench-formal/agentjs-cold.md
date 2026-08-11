# AgentBench 2.0

- Mode: `cold`
- Warmup: `3`
- Repeat: `15`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | agentjs RSS | boa RSS |
|:---|:---|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 427.2ms | 485.7ms | 13.04MiB | 25.54MiB |
| pressure | json-parse-transform | 25.2ms | 30.9ms | 12.46MiB | 18.84MiB |
| general | json-record-aggregate | 25.4ms | 20.4ms | 10.55MiB | 11.99MiB |
| pressure | large-index-dense-array | 629.9ms | 766.2ms | 14.13MiB | 17.86MiB |
| general | object-property-hot-loop | 64.0ms | 70.3ms | 14.51MiB | 12.43MiB |
| general | rule-filter-dense-window | 455.9ms | 339.6ms | 101.95MiB | 26.77MiB |
| pressure | short-lived-object-churn | 41.6ms | 42.4ms | 18.08MiB | 13.30MiB |
| general | startup-noop | 13.9ms | 20.0ms | 6.86MiB | 10.97MiB |
| pressure | string-ascii-index-scan | 362.3ms | 108.9ms | 7.56MiB | 12.79MiB |
| general | string-cleanup-replace-window | 138.9ms | 1075.0ms | 7.91MiB | 14.37MiB |
| general | string-log-token-slice | 424.4ms | 483.6ms | 8.59MiB | 13.66MiB |
| general | tool-result-schema-filter | 36.2ms | 25.4ms | 19.54MiB | 14.34MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `boa`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=1.097x, general=1.287x, pressure=0.877x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.123x, general=0.980x, pressure=1.360x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10891776 bytes
- `boa`: 29693440 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
