# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 83.6ms | 14.46MiB |
| general | json-record-aggregate | 38.1ms | 10.49MiB |
| pressure | json-parse-transform | 32.3ms | 12.39MiB |
| pressure | short-lived-object-churn | 54.2ms | 18.03MiB |
| general | startup-noop | 15.7ms | 6.82MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
