# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 167.7ms | 21.96MiB |
| general | json-record-aggregate | 77.9ms | 19.46MiB |
| pressure | json-parse-transform | 68.9ms | 19.82MiB |
| pressure | short-lived-object-churn | 97.6ms | 18.99MiB |
| general | startup-noop | 16.8ms | 7.03MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
