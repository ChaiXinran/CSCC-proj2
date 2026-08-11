# AgentBench four-engine comparison

This comparison runs the same 12 deterministic AgentBench 2.0 workloads on
AgentJS, Boa, QuickJS, and Oxide. Every engine passed 12/12 cases in both
modes. Cold mode uses a fresh process for every action. Batch mode runs five
actions per process. Both modes use three warmups and 15 measured samples.

Ratios below are `reference / AgentJS`. For time, a value above 1 means
AgentJS is faster. For peak RSS, a value above 1 means AgentJS uses less
memory. Values are geometric means over the common passing cases.

| Mode | Metric | Boa / AgentJS | QuickJS / AgentJS | Oxide / AgentJS |
|:---|:---|---:|---:|---:|
| cold | elapsed time | 1.090x | 0.186x | 1.237x |
| batch | elapsed time | 0.625x | 0.112x | 0.924x |
| cold | peak RSS | 1.138x | 0.472x | 2.450x |
| batch | peak RSS | 1.004x | 0.334x | 2.793x |

## Reading the result

- AgentJS has the best geometric-mean cold time against Boa and Oxide. Boa is
  1.090x and Oxide is 1.237x slower than AgentJS across the cold suite.
- QuickJS is the throughput leader: about 5.38x faster than AgentJS in cold
  mode and 8.93x faster in five-task batch mode.
- In batch mode Boa is about 1.60x faster than AgentJS overall. Oxide is about
  1.08x faster overall, but the split is workload-dependent: AgentJS leads
  Oxide on the general group while Oxide leads on the pressure group.
- AgentJS and Boa have nearly identical geometric-mean batch peak RSS.
  QuickJS uses substantially less memory than AgentJS. AgentJS uses much less
  memory than Oxide overall.
- Oxide's largest observed batch peaks are 1,494.00 MiB for
  `string-cleanup-replace-window` and 4,993.32 MiB for
  `string-log-token-slice`; AgentJS peaks at 10.03 MiB and 10.26 MiB on those
  same workloads.

## Executable size

| Engine | Bytes | Approx. MiB |
|:---|---:|---:|
| AgentJS | 10,785,280 | 10.29 |
| Boa | 29,936,640 | 28.55 |
| QuickJS | 1,142,784 | 1.09 |
| Oxide | 5,715,968 | 5.45 |

The detailed per-case medians and RSS values are in `agentjs-cold.md` and
`agentjs-batch.md`. The JSON reports contain all 15 samples. The matching
environment files record the exact commands, SHA-256 fingerprints, compiler,
platform, and memory sampler.
