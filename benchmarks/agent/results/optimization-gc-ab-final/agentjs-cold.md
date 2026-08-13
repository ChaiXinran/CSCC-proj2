# AgentBench 2.0

- Mode: `cold`
- Warmup: `3`
- Repeat: `9`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | baseline P50 | agentjs RSS | baseline RSS |
|:---|:---|---:|---:|---:|---:|
| general | object-property-hot-loop | 87.4ms | 79.3ms | 14.46MiB | 14.45MiB |
| general | json-record-aggregate | 31.9ms | 36.0ms | 10.49MiB | 10.38MiB |
| pressure | json-parse-transform | 31.6ms | 30.3ms | 12.40MiB | 12.39MiB |
| pressure | short-lived-object-churn | 50.0ms | 49.0ms | 18.07MiB | 18.02MiB |
| general | startup-noop | 15.0ms | 15.4ms | 6.79MiB | 6.81MiB |

## Correctness

- `agentjs`: 5/5 cases passed
- `baseline`: 5/5 cases passed

## Reference / AgentJS geometric-mean ratio

- `baseline`: all=0.998x, general=1.017x, pressure=0.969x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `baseline`: all=0.998x, general=0.997x, pressure=0.998x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10915840 bytes
- `baseline`: 10891776 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
