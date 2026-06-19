# Test262 Conformance Report

Test date: 2026-06-19 (UTC+08:00)

- AgentJS build: release, Rust 1.91.0
- Test262 revision: `de8e621cdba4f40cff3cf244e6cfb8cb48746b4a`
- Binary: `target/release/agentjs.exe` (22,273,536 bytes)
- Platform: Windows NT 10.0.26200.0, x86-64

## Results

| Suite | Total | Passed | Failed | Skipped | Pass rate |
| --- | ---: | ---: | ---: | ---: | ---: |
| harness | 116 | 116 | 0 | 0 | 100.00% |
| annexB | 1,086 | 1,041 | 45 | 0 | 95.86% |
| intl402 | 3,341 | 2,627 | 714 | 0 | 78.63% |
| language | 23,711 | 22,430 | 472 | 809 | 94.60% |
| built-ins/Temporal | 4,603 | 4,591 | 12 | 0 | 99.74% |
| built-ins/Object | 3,411 | 3,409 | 2 | 0 | 99.94% |
| built-ins/Array | 3,081 | 3,081 | 0 | 0 | 100.00% |
| built-ins/RegExp | 1,879 | 1,864 | 15 | 0 | 99.20% |
| built-ins/TypedArray | 1,446 | 1,434 | 12 | 0 | 99.17% |
| built-ins/String | 1,223 | 1,223 | 0 | 0 | 100.00% |
| built-ins/TypedArrayConstructors | 738 | 733 | 5 | 0 | 99.32% |
| built-ins/Promise | 703 | 703 | 0 | 0 | 100.00% |
| built-ins/Date | 594 | 592 | 2 | 0 | 99.66% |
| built-ins/DataView | 561 | 549 | 12 | 0 | 97.86% |
| built-ins/Iterator | 514 | 431 | 83 | 0 | 83.85% |
| built-ins/Function | 509 | 486 | 23 | 0 | 95.48% |
| **Executed total** | **47,516** | **45,310** | **1,397** | **809** | **95.36%** |

There are 51,897 non-staging tests in this checkout. Even if all 4,381
unexecuted tests are counted as failures, the verified conservative lower bound
is `45,310 / 51,897 = 87.31%`. This exceeds the contest requirement of 60%.

## Caveats

- Module tests are reported as skipped because module execution is not yet
  implemented by the AgentJS runner.
- A monolithic run was stopped after 45 minutes because blocking agent/Atomics
  cases can prevent worker completion. Large suites were therefore run as
  isolated shards.
- `test/staging` is excluded from the formal result. It contains experimental
  stress cases; one requested a 64 GiB allocation and terminated its process.
- Individual JSON results are stored beside this report in `reports/`.

