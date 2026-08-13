# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 162.3ms | 21.96MiB |
| general | json-record-aggregate | 81.3ms | 19.45MiB |
| pressure | json-parse-transform | 67.8ms | 19.79MiB |
| pressure | short-lived-object-churn | 98.4ms | 19.07MiB |
| general | startup-noop | 21.9ms | 7.03MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
