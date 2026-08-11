# AgentBench 2.0

- Mode: `batch`
- Warmup: `3`
- Repeat: `15`
- Batch iterations per process: `5`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | quickjs P50 | oxide P50 | agentjs RSS | boa RSS | quickjs RSS | oxide RSS |
|:---|:---|---:|---:|---:|---:|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 1297.1ms | 852.3ms | 95.4ms | 287.4ms | 27.16MiB | 76.51MiB | 11.92MiB | 33.28MiB |
| pressure | json-parse-transform | 108.1ms | 84.4ms | 30.5ms | 64.7ms | 20.08MiB | 21.09MiB | 5.33MiB | 15.19MiB |
| general | json-record-aggregate | 120.8ms | 46.9ms | 23.9ms | 86.3ms | 19.32MiB | 13.96MiB | 4.97MiB | 13.65MiB |
| pressure | large-index-dense-array | 1874.7ms | 1321.0ms | 168.7ms | 1959.3ms | 42.59MiB | 36.37MiB | 12.36MiB | 40.16MiB |
| general | object-property-hot-loop | 209.6ms | 76.6ms | 29.9ms | 237.8ms | 23.23MiB | 14.96MiB | 5.70MiB | 14.98MiB |
| general | rule-filter-dense-window | 1802.4ms | 492.8ms | 133.7ms | 1236.0ms | 112.29MiB | 40.78MiB | 18.39MiB | 105.70MiB |
| pressure | short-lived-object-churn | 153.0ms | 59.8ms | 25.7ms | 102.0ms | 20.39MiB | 14.89MiB | 4.95MiB | 19.92MiB |
| general | startup-noop | 21.2ms | 25.2ms | 18.8ms | 24.4ms | 6.56MiB | 11.64MiB | 4.36MiB | 8.21MiB |
| pressure | string-ascii-index-scan | 2426.6ms | 114.2ms | 47.0ms | 737.7ms | 8.43MiB | 12.96MiB | 4.72MiB | 71.04MiB |
| general | string-cleanup-replace-window | 366.0ms | 5503.1ms | 26.5ms | 1144.3ms | 10.03MiB | 14.46MiB | 5.39MiB | 1494.00MiB |
| general | string-log-token-slice | 1255.0ms | 2353.4ms | 29.8ms | 11.40s | 10.26MiB | 13.83MiB | 5.07MiB | 4993.32MiB |
| general | tool-result-schema-filter | 142.6ms | 58.4ms | 29.5ms | 107.7ms | 27.59MiB | 17.46MiB | 6.39MiB | 22.24MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `boa`: 12/12 cases passed
- `quickjs`: 12/12 cases passed
- `oxide`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=0.625x, general=0.914x, pressure=0.367x (>1 means AgentJS is faster)
- `quickjs`: all=0.112x, general=0.131x, pressure=0.091x (>1 means AgentJS is faster)
- `oxide`: all=0.924x, general=1.454x, pressure=0.489x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.004x, general=0.867x, pressure=1.232x (>1 means AgentJS uses less memory)
- `quickjs`: all=0.334x, general=0.330x, pressure=0.341x (>1 means AgentJS uses less memory)
- `oxide`: all=2.793x, general=4.389x, pressure=1.484x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10785280 bytes
- `boa`: 29936640 bytes
- `quickjs`: 1142784 bytes
- `oxide`: 5715968 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
