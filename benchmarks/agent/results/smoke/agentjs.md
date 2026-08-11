# AgentBench 2.0

- Mode: `cold`
- Warmup: `0`
- Repeat: `1`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | node P50 | agentjs RSS | boa RSS | node RSS |
|:---|:---|---:|---:|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 462.2ms | 1331.8ms | 91.8ms | 12.95MiB | 39.19MiB | 46.44MiB |
| pressure | json-parse-transform | 247.5ms | 40.5ms | 60.5ms | 12.37MiB | 18.84MiB | 34.92MiB |
| general | json-record-aggregate | 306.1ms | 29.0ms | 51.0ms | 10.35MiB | 11.99MiB | 34.74MiB |
| pressure | large-index-dense-array | 645.6ms | 636.3ms | 56.7ms | 14.03MiB | 17.86MiB | 39.42MiB |
| general | object-property-hot-loop | 721.3ms | 55.9ms | 56.4ms | 14.43MiB | 12.44MiB | 37.71MiB |
| general | rule-filter-dense-window | 455.0ms | 279.0ms | 59.5ms | 101.74MiB | 26.79MiB | 52.84MiB |
| pressure | short-lived-object-churn | 1385.1ms | 36.6ms | 51.8ms | 17.84MiB | 13.29MiB | 35.66MiB |
| general | startup-noop | 16.5ms | 16.4ms | 45.6ms | 6.75MiB | 10.96MiB | 33.50MiB |
| pressure | string-ascii-index-scan | 355.7ms | 88.1ms | 55.5ms | 7.44MiB | 12.72MiB | 37.45MiB |
| general | string-cleanup-replace-window | 146.1ms | 1086.8ms | 63.9ms | 7.84MiB | 14.29MiB | 38.29MiB |
| general | string-log-token-slice | 453.7ms | 491.9ms | 52.0ms | 8.50MiB | 13.66MiB | 37.89MiB |
| general | tool-result-schema-filter | 399.6ms | 51.1ms | 52.4ms | 19.45MiB | 14.35MiB | 36.45MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `boa`: 12/12 cases passed
- `node`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=0.394x, general=0.464x, pressure=0.314x (>1 means AgentJS is faster)
- `node`: all=0.174x, general=0.227x, pressure=0.120x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.175x, general=0.989x, pressure=1.495x (>1 means AgentJS uses less memory)
- `node`: all=2.830x, general=2.652x, pressure=3.100x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10859520 bytes
- `boa`: 29693440 bytes
- `node`: 92279112 bytes

## Reproduction

See `environment.json`/the JSON report for machine, compiler, command and binary fingerprints.
