# Native Version History: V8-V12

This consolidated history replaces the per-track reports for V8 through V12
and the closely related Fix4/Fixup8/Fix9 reports.

| Version | Consolidated delivery |
| --- | --- |
| V8 | Frontend unlockers, module-runner infrastructure, builtin skeletons, and Test262 host integration. |
| V9 | Generator/async/for-of frontend support, Promise/job queue/iterator runtime work, and Map/Set/Iterator builtins. |
| V10 | BigInt/numeric/Unicode syntax completion, TypedArray/ArrayBuffer/DataView runtime work, and Date/Intl/Temporal semantics. |
| V11 | RegExp parser/static errors, object descriptor precision, Annex B, and RegExp/descriptor builtin corrections. |
| V12 | Native runtime and builtin consolidation after the V8-V11 feature expansion. |

The Fix4 reports covered class/destructuring frontend and adjacent runtime and
builtin repairs. Fixup8 and Fix9 closed remaining class/destructuring and
regression-gate issues. Historical 5,000-case selector manifests remain under
`src/test262_manifests/` because they are CLI inputs; verbose analysis and raw
scan history were removed.

These versions established the layered correctness workflow later superseded
by the official full-suite result in
`Test262-final/full-test262-summary.json`.
