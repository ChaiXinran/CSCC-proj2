# Test262 85% Target Stage Summary

This document replaces the fine-grained JSON artifacts formerly stored in
`reports/85-target/`. Those files recorded an intermediate correctness push;
the project has since exceeded the 85% target, so only the decision-relevant
milestones are retained here.

| Stage | Passed / Total | Failed | Skipped | Conformance | Elapsed |
| --- | ---: | ---: | ---: | ---: | ---: |
| Stage baseline | 41,069 / 53,379 | 12,308 | 2 | 76.9385% | 331.202 s |
| B0-B2 / stage closeout | 41,476 / 53,379 | 11,901 | 2 | 77.7010% | 271.294 s |
| Current official full run (2026-08-11) | 48,557 / 53,379 | 4,820 | 2 | 90.9665% | 452.354 s |

The intermediate stage added 407 passes and reduced measured elapsed time by
59.908 seconds. Subsequent runtime, builtin, frontend, and integration work
added another 7,081 passes, taking the project 5.9665 percentage points above
the original 85% target. The authoritative current result is
`Test262-final/full-test262-summary.json`.

The deleted detail set covered before/after scans for classes, modules,
Promise/async iteration, Atomics, TypedArray, Temporal, Intl, RegExp, Array,
Map/Set, and related focused suites. Those details were useful during repair
but are no longer a release input or correctness authority.
