# JetStream2 AgentJS / Boa / QuickJS / Oxide comparison

## Final common-set performance run

The final run uses six pinned JetStream2 JavaScript workload kernels passed by
all four engines. Each process executes one workload iteration. There are two
warmup processes and seven measured processes per engine/workload pair.

| Workload | AgentJS P50 | Boa P50 | QuickJS P50 | Oxide P50 |
|:---|---:|---:|---:|---:|
| n-body-SP | 1.22s | 347ms | 74ms | 970ms |
| crypto-sha1-SP | 4.03s | 527ms | 200ms | 3.90s |
| crypto-md5-SP | 4.14s | 483ms | 129ms | 4.02s |
| 3d-cube-SP | 1.45s | 364ms | 97ms | 1.08s |
| navier-stokes | 594ms | 218ms | 34ms | 800ms |
| richards | 3.72s | 625ms | 116ms | 3.54s |

All engines passed 6/6 workloads and all seven measured samples. Geometric
mean reference/AgentJS kernel-time ratios are Boa 0.202x, QuickJS 0.047x, and
Oxide 0.945x. Equivalently, Boa is about 4.95x faster than AgentJS, QuickJS is
about 21.3x faster, and Oxide is about 1.06x faster on this common set.

## Memory

| Engine | Maximum observed peak RSS |
|:---|---:|
| AgentJS | 27.04 MiB |
| Boa | 16.43 MiB |
| QuickJS | 7.02 MiB |
| Oxide | 1,123.82 MiB |

Oxide's largest one-iteration peaks are 1,086.47 MiB on `crypto-sha1-SP`,
1,123.82 MiB on `crypto-md5-SP`, and 680.98 MiB on `richards`. AgentJS peaks
at 14.48 MiB, 16.05 MiB, and 12.03 MiB on those same workloads.

## Compatibility scans

The ten-workload classic scan produced:

- AgentJS: 7/10 passed. `ai-astar`, `cdjs`, and `splay` hit arena limits.
- Boa: 10/10 passed.
- QuickJS: 10/10 passed.
- Oxide: 3/10 passed. Failures include a 1.5 GiB memory limit, `u8` register
  limits, missing exponentiation support, and property/call runtime errors.

The five-workload SunSpider scan produced 5/5 passes for AgentJS, Boa, and
QuickJS and 4/5 for Oxide. Oxide rejects `base64-SP` because its generated
constructor requires register 290, beyond its current `u8` register limit.

## Repeated-iteration pressure

With three iterations in one process, AgentJS, Boa, and QuickJS pass all four
selected SunSpider kernels. Oxide passes `n-body-SP` and `3d-cube-SP`, but
`crypto-sha1-SP` and `crypto-md5-SP` exceed the 1,536 MiB RSS limit. This is
kept separate from the single-iteration performance comparison rather than
silently dropping the failed samples.

## Scope

These are portable, self-contained JetStream2 workload-kernel results, not the
browser suite's official composite score. The detailed JSON files contain all
samples, wall times, RSS measurements, executable SHA-256 fingerprints, and
pinned repository revisions:

- `four-engine/`: final six-workload, seven-sample comparison.
- `four-engine-smoke/`: ten-workload classic compatibility scan.
- `four-engine-sunspider-smoke/`: five-workload SunSpider scan.
- `four-engine-three-iteration-pressure/`: repeated-iteration memory pressure.
