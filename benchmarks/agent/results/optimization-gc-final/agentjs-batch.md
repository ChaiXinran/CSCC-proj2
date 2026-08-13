# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 136.6ms | 21.93MiB |
| general | json-record-aggregate | 60.9ms | 19.61MiB |
| pressure | json-parse-transform | 54.6ms | 19.81MiB |
| pressure | short-lived-object-churn | 93.4ms | 19.03MiB |
| general | startup-noop | 17.3ms | 7.02MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
