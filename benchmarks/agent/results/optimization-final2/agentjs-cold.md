# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 74.8ms | 14.44MiB |
| general | json-record-aggregate | 30.8ms | 10.47MiB |
| pressure | json-parse-transform | 25.3ms | 12.37MiB |
| pressure | short-lived-object-churn | 41.6ms | 18.01MiB |
| general | startup-noop | 14.6ms | 6.76MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10895872 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
