# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `5`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | boa P50 | node P50 | agentjs RSS | boa RSS | node RSS |
|:---|:---|---:|---:|---:|---:|---:|---:|
| general | startup-noop | 16.3ms | 22.1ms | 47.9ms | 7.25MiB | 11.61MiB | 34.02MiB |
| pressure | descriptor-side-table-array | 1016.7ms | 571.0ms | 173.5ms | 27.25MiB | 76.73MiB | 63.32MiB |
| pressure | large-index-dense-array | 1633.5ms | 894.0ms | 58.2ms | 42.66MiB | 37.40MiB | 40.26MiB |
| general | string-cleanup-replace-window | 252.1ms | 9486.7ms | 60.2ms | 9.70MiB | 14.40MiB | 48.14MiB |
| general | string-log-token-slice | 1006.4ms | 2261.6ms | 56.1ms | 10.68MiB | 14.02MiB | 40.76MiB |

## Correctness

- `agentjs`: 5/5 cases passed
- `boa`: 5/5 cases passed
- `node`: 5/5 cases passed

## Reference / AgentJS geometric-mean ratio

- `boa`: all=2.039x, general=4.858x, pressure=0.554x (>1 means AgentJS is faster)
- `node`: all=0.188x, general=0.339x, pressure=0.078x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `boa`: all=1.504x, general=1.461x, pressure=1.571x (>1 means AgentJS uses less memory)
- `node`: all=2.871x, general=4.463x, pressure=1.481x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10859520 bytes
- `boa`: 29693440 bytes
- `node`: 92279112 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
