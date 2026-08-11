# AgentBench 2.0

- Mode: `batch`
- Warmup: `0`
- Repeat: `1`
- Batch iterations per process: `2`

`status=pass` is a correctness gate; only common passing cases enter speedup averages.

| Group | Case | agentjs P50 | agentjs RSS |
|:---|:---|---:|---:|
| pressure | descriptor-side-table-array | 434.4ms | 16.51MiB |
| pressure | json-parse-transform | 452.0ms | 15.61MiB |
| general | json-record-aggregate | 585.0ms | 13.37MiB |
| pressure | large-index-dense-array | 676.2ms | 21.48MiB |
| general | object-property-hot-loop | 1376.7ms | 19.84MiB |
| general | rule-filter-dense-window | 654.0ms | 112.28MiB |
| pressure | short-lived-object-churn | 2689.8ms | 18.56MiB |
| general | startup-noop | 32.9ms | 7.21MiB |
| pressure | string-ascii-index-scan | 658.1ms | 8.07MiB |
| general | string-cleanup-replace-window | 210.6ms | 8.61MiB |
| general | string-log-token-slice | 582.9ms | 9.51MiB |
| general | tool-result-schema-filter | 792.5ms | 28.30MiB |

## Correctness

- `agentjs`: 12/12 cases passed

## Executable size

- `agentjs`: 10859520 bytes

## Reproduction

See `environment.json`/the JSON report for machine, compiler, command and binary fingerprints.
