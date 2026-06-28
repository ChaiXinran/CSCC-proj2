# AgentBench

- Primary: `agentjs`
- Reference: `boa`
- Repeat: `3`

| Case | AgentJS | Reference | Result |
|:---|---:|---:|---:|
| descriptor-side-table-array | 770ms | 1282ms | 1.67x faster |
| large-index-dense-array | 1147ms | 2741ms | 2.39x faster |
| rule-filter-dense-window | 808ms | 982ms | 1.22x faster |
