# Native V11 Test262 Report

## Scope

Native V11 is a conformance-expansion milestone for the self-developed native JavaScript engine. It follows the V8-V10 frontend/runtime work and focuses on three coordinated areas:

- **V11-A:** RegExp parser, RegExp literal static errors, and lexer/parser recovery around RegExp bodies.
- **V11-B:** object model precision, property descriptors, receiver behavior, and own-key ordering.
- **V11-C:** RegExp builtins, Annex B legacy builtins, and descriptor sweep work around JS-visible builtin functions.

All commands in this report use the native backend path: native lexer, parser, bytecode compiler, VM, runtime, builtin implementation, and host harness. Boa is not used as a fallback.

## Data Source and Completeness Note

The direct evidence for this report is the uploaded PowerShell-captured Test262 output file, decoded as UTF-16LE.

The captured log ends at:

```text
[53377/53379 100.0%] pass=18050 fail=35325 skip=2
```

That means **53,377 records were captured out of a planned 53,379 records**. The final two outcomes are not present in the uploaded file. This report therefore treats the numbers below as the **last captured known state**, not as a formal final JSON summary.

## Acceptance Gate

No separate zero-regression V11 pinned gate output was included with the uploaded log. The available run is a broad direct scan over the full `test262/test` tree, so it should be interpreted as a diagnostic conformance baseline rather than a merge gate.

Recommended pinned-gate commands for V11 changes remain:

```powershell
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test frontend_v11
cargo test --no-default-features --test native_v11_runtime
cargo test --no-default-features --test native_v11_builtins
```

## Diagnostic V11 Full-Suite Scan

Command captured in the uploaded output:

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

Result, using the last complete progress line:

| Planned Total | Captured Records | Passed | Failed | Skipped | Known Conformance vs Planned | Known Conformance vs Captured |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 53,379 | 53,377 | 18,050 | 35,325 | 2 | 33.81% | 33.82% |

Skipped tests are not counted as passes. Because the uploaded output does not include the final two records or a JSON summary, the safest statement is: **Native V11 has at least 18,050 passing Test262 records in this full-suite direct run, with a known lower-bound conformance of 33.81% against the planned 53,379-record run.**

For comparison, the repository also contains a locked V11 hotspot scan summary:

| Scan | Total | Passed | Failed | Skipped | Conformance |
| --- | ---: | ---: | ---: | ---: | ---: |
| `reports/native-v11-scan-summary.json` | 5,000 | 876 | 4,124 | 0 | 17.52% |

The locked 5,000-case scan and the full `test262/test` run are not directly comparable. The former is a selected V11 hotspot manifest; the latter is a much broader full-suite diagnostic run.

## Failure Distribution by Top-Level Area

The progress log only prints file paths for failing and skipped tests, not for passing tests. Therefore, directory-level pass rates cannot be reconstructed from this log alone. The table below reports **failure counts only**.

| Area | Failed | Share of Captured Failures |
| --- | ---: | ---: |
| `language` | 16,967 | 48.03% |
| `built-ins` | 13,451 | 38.08% |
| `intl402` | 3,082 | 8.72% |
| `staging` | 1,086 | 3.07% |
| `annexB` | 653 | 1.85% |
| `harness` | 86 | 0.24% |

The dominant failure surface is still ordinary ECMAScript language syntax and semantics, followed by broad builtin coverage. V11-specific RegExp / Annex B work is visible, but the full-suite failure profile is much wider than the V11 implementation scope.

## Largest Failure Hotspots

| Hotspot | Failed | Share of Captured Failures |
| --- | ---: | ---: |
| `language/expressions` | 7,867 | 22.27% |
| `language/statements` | 7,631 | 21.60% |
| `built-ins/Temporal` | 4,221 | 11.95% |
| `intl402/Temporal` | 2,026 | 5.74% |
| `built-ins/TypedArray` | 1,155 | 3.27% |
| `built-ins/Array` | 1,109 | 3.14% |
| `built-ins/RegExp` | 1,078 | 3.05% |
| `staging/sm` | 1,011 | 2.86% |
| `built-ins/Object` | 708 | 2.00% |
| `built-ins/Promise` | 703 | 1.99% |
| `annexB/language` | 481 | 1.36% |
| `built-ins/TypedArrayConstructors` | 464 | 1.31% |
| `built-ins/Iterator` | 418 | 1.18% |
| `language/module-code` | 397 | 1.12% |
| `built-ins/Atomics` | 388 | 1.10% |

The largest hotspots are `language/expressions` and `language/statements`, especially class syntax, private fields, async/generator forms, `for-await-of`, dynamic import, and module-related syntax. These dominate the full-suite result and should not be interpreted as only V11-A/V11-B/V11-C failures.

## Failure Classification by Reported Error Type

The following categories are derived from the verbose failure details in the uploaded output. They are diagnostic heuristics, not a formal ECMAScript taxonomy.

| Reported failure type | Count | Share of Captured Failures |
| --- | ---: | ---: |
| SyntaxError | 20,863 | 59.06% |
| TypeError | 4,885 | 13.83% |
| Test262Error | 3,653 | 10.34% |
| ReferenceError | 2,648 | 7.50% |
| Unsupported | 1,706 | 4.83% |
| Error | 889 | 2.52% |
| RangeError | 346 | 0.98% |
| Other | 335 | 0.95% |

### Syntax, parser, and static-negative failures

This is the largest bucket. It includes unsupported or partially modeled syntax and cases where Test262 expects an early error but the engine either accepts the program or reports a different syntax shape.

Representative failures:

- `test262/test/annexB/built-ins/Function/createdynfn-html-close-comment-body.js`
- `test262/test/staging/top-level-await/tla-hang-entry.js`
- `test262/test/annexB/language/statements/class/subclass/superclass-emulates-undefined.js`
- `test262/test/language/expressions/dynamic-import/always-create-new-promise.js`

Common patterns include:

- class and private-field syntax;
- async generator and `for-await-of` forms;
- module, import/export, and top-level-await syntax;
- HTML comment syntax in Annex B / dynamic `Function` inputs;
- static-negative mismatches where the expected `SyntaxError` is not thrown.

### Object-model, descriptor, and builtin semantic failures

The TypeError and Test262 assertion buckets show that many failures are not parser crashes but JS-visible semantic gaps: missing own properties, missing descriptors, wrong error timing, incomplete receiver behavior, or incomplete descriptor attributes.

Representative failures:

- `test262/test/annexB/built-ins/Date/prototype/getYear/length.js`
- `test262/test/staging/sm/types/8.12.5-01.js`
- `test262/test/built-ins/Object/defineProperty/15.2.3.6-2-11.js`
- `test262/test/annexB/built-ins/Array/from/iterator-method-emulates-undefined.js`

Important recurring messages include:

- `cannot get own property descriptor on undefined`;
- `undefined is not callable`;
- `cannot read property on undefined`;
- `property setter is undefined`;
- `Expected ... to be thrown but no exception was thrown`;
- `Expected SameValue(...) to be true`.

This is the main V11-B follow-up surface.

### RegExp and Annex B failures

V11-C has added basic RegExp and Annex B builtin support, but the full Test262 run still exposes substantial RegExp semantics that are outside the current implementation depth.

Representative failures:

- `test262/test/annexB/built-ins/RegExp/RegExp-decimal-escape-not-capturing.js`
- `test262/test/built-ins/RegExp/property-escapes/binary-property-with-value-ASCII_-_F-negated.js`
- `test262/test/annexB/built-ins/RegExp/named-groups/non-unicode-malformed.js`
- `test262/test/annexB/built-ins/RegExp/prototype/compile/duplicate-named-capturing-groups-syntax.js`

Notable remaining patterns include:

- ECMAScript RegExp backreferences and named-group edge cases;
- Unicode property escapes and RegExp property-escape behavior;
- `RegExp.prototype.compile`, legacy accessors, and descriptor exactness;
- RegExp-facing `String.prototype` dispatch and split/match/search replacement edge cases.

### Host, realm, and large-feature gaps

Some failures require Test262 host features or large ECMAScript subsystems that are not part of this V11 pass.

Representative failures:

- `test262/test/annexB/built-ins/RegExp/legacy-accessors/index/this-cross-realm-constructor.js`
- `test262/test/built-ins/Array/isArray/proxy-revoked.js`
- `test262/test/built-ins/ShadowRealm/WrappedFunction/length-throws-typeerror.js`
- `test262/test/staging/sm/strict/10.4.2.js`

Examples include:

- `$262.createRealm`;
- `Proxy` and cross-realm constructor identity;
- `ShadowRealm`;
- Temporal and Intl402 surface area;
- complete module loading and top-level await;
- BigInt runtime semantics;
- generator / yield runtime semantics.

## Feature-Term Hotspots

The following table counts failure records whose path or detail mentions a major feature term. A single failure may contribute to multiple terms, so these counts are not disjoint.

| Feature term | Mentioned in Failed Records | Share of Captured Failures |
| --- | ---: | ---: |
| `class` | 7,762 | 21.97% |
| `Temporal` | 6,343 | 17.96% |
| `async` | 5,714 | 16.18% |
| `private` | 3,484 | 9.86% |
| `Intl` | 3,083 | 8.73% |
| `BigInt` | 1,967 | 5.57% |
| `generator` | 1,873 | 5.30% |
| `await` | 1,816 | 5.14% |
| `RegExp` | 1,308 | 3.70% |
| `yield` | 1,298 | 3.67% |
| `Iterator` | 1,039 | 2.94% |
| `import` | 890 | 2.52% |
| `module` | 632 | 1.79% |
| `Proxy` | 553 | 1.57% |

The key takeaway is that the full-suite result is heavily shaped by large modern-language surfaces: class syntax, Temporal, async/generator behavior, private fields, Intl402, BigInt, modules, and Proxy. These areas can hide the smaller V11-specific signal unless future scans use a focused manifest or JSON grouping.

## Skipped Tests

Only two explicit skips were captured:

| Skipped file |
| --- |
| `test262/test/built-ins/Atomics/wait/bigint/cannot-suspend-throws.js` |
| `test262/test/built-ins/Atomics/wait/cannot-suspend-throws.js` |

Both are Atomics wait tests. They should remain explicit skips until the native host can model suspension behavior accurately.

## Interpretation

The current data says:

1. The uploaded full-suite run is broad enough to prove that the native backend can execute tens of thousands of Test262 records and reach at least **18,050 known passes**.
2. The known full-suite conformance lower bound is **33.81%** against the planned 53,379-record run, or **33.82%** against the 53,377 captured records.
3. The largest remaining failure source is syntax/parser/static-negative behavior, especially modern language constructs outside the narrow V11 feature scope.
4. V11-B and V11-C work should still matter for conformance, but their signal is mixed with much larger unsupported surfaces such as Temporal, Intl402, modules, private fields, BigInt, Proxy, and async/generator semantics.
5. The uploaded log should not be used as a final formal dashboard because it ends two records before the planned total and lacks a JSON summary.

## Suggested Follow-Up Order

1. **Rerun the full scan with JSON output** so the final two records and the exact elapsed time are captured:

   ```powershell
   cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-v11-full-summary.json *> reports
ative-v11-full-verbose.txt
   ```

2. **Keep the locked V11 hotspot scan separate from the full-suite scan.** The full suite is useful for headline progress, but V11 development should continue to use focused manifests for A/B/C ownership.
3. **Prioritize parser/static-negative work** in `language/expressions` and `language/statements`, especially class/private fields, async/generator forms, module syntax, and dynamic import.
4. **Continue V11-B descriptor/object-model precision**, especially undefined descriptors, property setters, own-key ordering, and receiver behavior.
5. **Continue V11-C RegExp/Annex B precision**, especially RegExp backreferences/named groups/property escapes, legacy accessors, `RegExp.prototype.compile`, and Annex B legacy builtin descriptor shapes.
6. **Treat Temporal, Intl402, Proxy, ShadowRealm, module loading, BigInt runtime, and full generator semantics as larger future milestones** unless the V11 scope is explicitly expanded.

## Quality Gates

Recommended commands before updating this report again:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test frontend_v11
cargo test --no-default-features --test native_v11_runtime
cargo test --no-default-features --test native_v11_builtins
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-v11-full-summary.json *> reports
ative-v11-full-verbose.txt
```

The dashboard and broad scan reports are diagnostic. They must not count failed, skipped, crashed, timed-out, or uncaptured suites as passes.
