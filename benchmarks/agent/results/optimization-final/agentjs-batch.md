# AgentBench 2.0

- Mode: `batch`
- Warmup: `1`
- Repeat: `3`
- Batch iterations per process: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 120.9ms | 21.91MiB |
| general | json-record-aggregate | 54.3ms | 19.43MiB |
| pressure | json-parse-transform | 48.5ms | 19.75MiB |
| pressure | short-lived-object-churn | 77.0ms | 19.04MiB |
| general | startup-noop | 15.7ms | 6.99MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10895872 bytes

## Reproduction

See `environment-batch.json`/the JSON report for machine, compiler, command and binary fingerprints.
