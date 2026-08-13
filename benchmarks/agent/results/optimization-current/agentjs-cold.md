# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 116.1ms | 14.45MiB |
| general | json-record-aggregate | 37.4ms | 10.47MiB |
| pressure | json-parse-transform | 32.2ms | 12.40MiB |
| pressure | short-lived-object-churn | 66.0ms | 18.01MiB |
| general | startup-noop | 16.1ms | 6.80MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10894336 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
