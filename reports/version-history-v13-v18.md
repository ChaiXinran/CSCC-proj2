# Native Version History: V13-V18

This consolidated history replaces the retained per-version runtime reports
for V13, V16, and V18 while preserving their major correctness results.

| Version | Consolidated delivery |
| --- | --- |
| V13 | Module namespace/live-binding work, dynamic import, class and `super` runtime structures, and async integration follow-up. |
| V16 | Shared abstract operations and VM-mediated object protocol; Object focused work produced a measured +79 gross pass gain and a +52 net gain across the complete Object suite. Promise capability handling was also generalized. |
| V18 | Descriptor/property-key precision, Proxy-observable object operations, `Object.groupBy`, generic `Array.of`, sparse-array behavior, and runtime prototype normalization. The recorded Object and Array focused suites gained 190 passes with no Proxy regression. |

V17 frontend work eliminated all 40 failures in the targeted block-scope
syntax cluster, added 15 object-expression and 9 async-arrow passes, and later
added three class-statement passes without regressions. Temporal work added
focused PlainDate/PlainDateTime/PlainTime passes; the WeakRef and
FinalizationRegistry follow-up added 68 focused passes and moved its then-full
baseline from 45,654 to 45,725 (+71 overall).

The remote V17 continuation added calendar-relative rounding for PlainDate and
PlainDateTime (+46 full-suite passes), exact range and time balancing for
Temporal add/subtract (+24), Duration calendar-unit guards, and a later
Temporal sweep worth 103 focused built-in passes. That sweep improved
PlainYearMonth by 46 and ZonedDateTime by 35 focused passes. These are retained
as historical deltas; the current official aggregate below remains the sole
accuracy authority.

Later fixes continued on top of these milestones. The current authoritative
full result is 48,557 / 53,379 (90.9665%) in
`Test262-final/full-test262-summary.json`.
