# AgentBench 2.0

- Mode: `cold`
- Warmup: `3`
- Repeat: `15`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | quickjs P50 | oxide P50 | agentjs RSS | boa RSS | quickjs RSS | oxide RSS |
|:---|:---|---:|---:|---:|---:|---:|---:|---:|---:|
| pressure | descriptor-side-table-array | 424.3ms | 569.2ms | 40.0ms | 343.0ms | 13.32MiB | 25.93MiB | 12.23MiB | 33.24MiB |
| pressure | json-parse-transform | 28.6ms | 40.2ms | 17.2ms | 35.1ms | 11.72MiB | 19.20MiB | 5.37MiB | 10.52MiB |
| general | json-record-aggregate | 29.2ms | 29.5ms | 18.1ms | 39.6ms | 10.63MiB | 11.97MiB | 4.94MiB | 9.72MiB |
| pressure | large-index-dense-array | 638.5ms | 715.9ms | 52.2ms | 484.7ms | 14.29MiB | 17.85MiB | 10.35MiB | 32.41MiB |
| general | object-property-hot-loop | 72.6ms | 67.4ms | 17.6ms | 79.4ms | 14.82MiB | 12.77MiB | 5.69MiB | 17.35MiB |
| general | rule-filter-dense-window | 443.2ms | 295.4ms | 51.3ms | 318.1ms | 102.07MiB | 26.83MiB | 18.40MiB | 36.73MiB |
| pressure | short-lived-object-churn | 44.9ms | 46.2ms | 18.4ms | 40.5ms | 16.92MiB | 13.67MiB | 4.89MiB | 12.63MiB |
| general | startup-noop | 18.5ms | 23.4ms | 12.6ms | 24.8ms | 7.02MiB | 11.33MiB | 4.33MiB | 8.12MiB |
| pressure | string-ascii-index-scan | 470.1ms | 114.1ms | 23.2ms | 787.5ms | 7.75MiB | 13.12MiB | 4.64MiB | 70.88MiB |
| general | string-cleanup-replace-window | 151.5ms | 1195.0ms | 17.8ms | 262.8ms | 8.08MiB | 14.32MiB | 5.20MiB | 305.76MiB |
| general | string-log-token-slice | 509.2ms | 555.4ms | 17.5ms | 2271.2ms | 8.77MiB | 13.99MiB | 5.22MiB | 1005.75MiB |
| general | tool-result-schema-filter | 45.2ms | 35.2ms | 17.8ms | 46.3ms | 19.68MiB | 14.70MiB | 5.96MiB | 13.74MiB |

## Correctness

- `agentjs`: 12/12 cases passed
- `boa`: 12/12 cases passed
- `quickjs`: 12/12 cases passed
- `oxide`: 12/12 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=1.090x, general=1.269x, pressure=0.880x (>1 means AgentJS is faster)
- `quickjs`: all=0.186x, general=0.211x, pressure=0.156x (>1 means AgentJS is faster)
- `oxide`: all=1.237x, general=1.414x, pressure=1.026x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.138x, general=0.980x, pressure=1.404x (>1 means AgentJS uses less memory)
- `quickjs`: all=0.472x, general=0.420x, pressure=0.555x (>1 means AgentJS uses less memory)
- `oxide`: all=2.450x, general=2.800x, pressure=2.032x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10785280 bytes
- `boa`: 29936640 bytes
- `quickjs`: 1142784 bytes
- `oxide`: 5715968 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
