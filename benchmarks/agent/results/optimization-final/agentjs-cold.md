# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 69.5ms | 14.41MiB |
| general | json-record-aggregate | 25.8ms | 10.45MiB |
| pressure | json-parse-transform | 25.5ms | 12.37MiB |
| pressure | short-lived-object-churn | 42.4ms | 17.99MiB |
| general | startup-noop | 14.2ms | 6.77MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10895872 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
