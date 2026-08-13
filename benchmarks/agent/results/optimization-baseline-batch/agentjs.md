# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 114.7ms | 21.95MiB |
| general | json-record-aggregate | 53.6ms | 19.63MiB |
| pressure | json-parse-transform | 43.8ms | 19.77MiB |
| pressure | short-lived-object-churn | 74.7ms | 19.04MiB |
| general | startup-noop | 15.1ms | 7.01MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10891776 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
