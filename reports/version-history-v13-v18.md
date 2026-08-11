# Native Version History: V13-V18

This consolidated history replaces the retained per-version runtime reports
for V13, V16, and V18 while preserving their major correctness results.

| Version | Consolidated delivery |
| --- | --- |
| V13 | Module namespace/live-binding work, dynamic import, class and `super` runtime structures, and async integration follow-up. |
| V16 | Shared abstract operations and VM-mediated object protocol; Object focused work produced a measured +79 gross pass gain and a +52 net gain across the complete Object suite. Promise capability handling was also generalized. |
| V18 | Descriptor/property-key precision, Proxy-observable object operations, `Object.groupBy`, generic `Array.of`, sparse-array behavior, and runtime prototype normalization. The recorded Object and Array focused suites gained 190 passes with no Proxy regression. |

Later fixes continued on top of these milestones. The current authoritative
full result is 48,557 / 53,379 (90.9665%) in
`Test262-final/full-test262-summary.json`.
