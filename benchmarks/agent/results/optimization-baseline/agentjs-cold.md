# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 63.8ms | 14.45MiB |
| general | json-record-aggregate | 24.3ms | 10.49MiB |
| pressure | json-parse-transform | 24.8ms | 12.38MiB |
| pressure | short-lived-object-churn | 41.1ms | 18.04MiB |
| general | startup-noop | 14.4ms | 6.81MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10891776 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
