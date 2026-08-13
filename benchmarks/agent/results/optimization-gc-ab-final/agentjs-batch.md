# AgentBench 2.0

- Mode: `batch`
- Warmup: `3`
- Repeat: `9`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | baseline P50 | agentjs RSS | baseline RSS |
|:---|:---|---:|---:|---:|---:|
| general | object-property-hot-loop | 140.3ms | 155.5ms | 22.02MiB | 21.93MiB |
| general | json-record-aggregate | 65.2ms | 64.5ms | 19.59MiB | 19.39MiB |
| pressure | json-parse-transform | 59.6ms | 57.7ms | 19.73MiB | 19.75MiB |
| pressure | short-lived-object-churn | 99.6ms | 91.8ms | 19.20MiB | 19.03MiB |
| general | startup-noop | 31.8ms | 20.4ms | 7.02MiB | 6.98MiB |

## Correctness

- `agentjs`: 5/5 cases passed
- `baseline`: 5/5 cases passed

## Reference / AgentJS geometric-mean ratio

- `baseline`: all=0.911x, general=0.889x, pressure=0.945x (>1 means AgentJS is faster)

## Reference / AgentJS peak-RSS ratio

- `baseline`: all=0.994x, general=0.993x, pressure=0.996x (>1 means AgentJS uses less memory)

## Executable size

- `agentjs`: 10915840 bytes
- `baseline`: 10891776 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
