# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 76.1ms | 14.45MiB |
| general | json-record-aggregate | 31.1ms | 10.50MiB |
| pressure | json-parse-transform | 26.4ms | 12.38MiB |
| pressure | short-lived-object-churn | 48.1ms | 18.05MiB |
| general | startup-noop | 15.1ms | 6.83MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
