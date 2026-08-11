# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | node P50 | agentjs RSS | boa RSS | node RSS |
|:---|:---|---:|---:|---:|---:|---:|---:|
| general | startup-noop | 14.3ms | 21.0ms | 47.8ms | 7.00MiB | 11.23MiB | 34.02MiB |
| pressure | descriptor-side-table-array | 431.2ms | 470.0ms | 75.2ms | 13.46MiB | 26.23MiB | 47.39MiB |
| pressure | large-index-dense-array | 622.2ms | 614.7ms | 52.8ms | 14.39MiB | 18.38MiB | 39.82MiB |
| general | string-cleanup-replace-window | 141.6ms | 1057.4ms | 46.8ms | 8.14MiB | 14.76MiB | 38.81MiB |
| general | string-log-token-slice | 472.4ms | 479.4ms | 46.9ms | 8.82MiB | 14.04MiB | 38.50MiB |

## Correctness

- `agentjs`: 5/5 cases passed
- `boa`: 5/5 cases passed
- `node`: 5/5 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=1.643x, general=2.233x, pressure=1.038x (>1 means AgentJS is faster)
- `node`: all=0.277x, general=0.479x, pressure=0.122x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.631x, general=1.667x, pressure=1.578x (>1 means AgentJS uses less memory)
- `node`: all=3.969x, general=4.659x, pressure=3.121x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10859520 bytes
- `boa`: 29693440 bytes
- `node`: 92279112 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
