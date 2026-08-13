# AgentBench 2.0

- Mode: `cold`
- Warmup: `1`
- Repeat: `3`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| general | object-property-hot-loop | 86.0ms | 14.66MiB |
| general | json-record-aggregate | 37.0ms | 10.51MiB |
| pressure | json-parse-transform | 33.9ms | 12.38MiB |
| pressure | short-lived-object-churn | 55.1ms | 18.07MiB |
| general | startup-noop | 15.8ms | 6.82MiB |

## Correctness

- `agentjs`: 5/5 cases passed

## Executable size

- `agentjs`: 10892288 bytes

## Reproduction

See `environment-cold.json`/the JSON report for machine, compiler, command and binary fingerprints.
