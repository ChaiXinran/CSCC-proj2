# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 299.2ms | 21.91MiB |
| general | json-record-aggregate | 115.2ms | 19.43MiB |
| pressure | json-parse-transform | 112.3ms | 19.79MiB |
| pressure | short-lived-object-churn | 119.9ms | 19.03MiB |
| general | startup-noop | 20.8ms | 7.02MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10894336 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
