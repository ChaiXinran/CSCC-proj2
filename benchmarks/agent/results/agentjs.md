# AgentBench

- Primary: `agentjs`
- Reference: `boa`
- Repeat: `3`

| Case | AgentJS | Reference | Result |
|:---|---:|---:|---:|
| descriptor-side-table-array | 339ms | 447ms | 1.32x faster |
| large-index-dense-array | 522ms | 586ms | 1.12x faster |
| rule-filter-dense-window | 365ms | 259ms | 1.41x slower |
| string-ascii-index-scan | 1512ms | 83ms | 18.28x slower |
| string-cleanup-replace-window | 277ms | 963ms | 3.48x faster |
| string-log-token-slice | 1629ms | 433ms | 3.76x slower |
