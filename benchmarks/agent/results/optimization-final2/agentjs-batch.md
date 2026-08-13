# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 120.4ms | 21.92MiB |
| general | json-record-aggregate | 53.8ms | 19.43MiB |
| pressure | json-parse-transform | 49.8ms | 19.73MiB |
| pressure | short-lived-object-churn | 76.3ms | 19.05MiB |
| general | startup-noop | 15.5ms | 6.98MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10895872 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
