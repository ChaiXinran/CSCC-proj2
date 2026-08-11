# JetStream 2 Part B report

## 2026-08-11 performance continuation

- Locked the second-round correctness baseline at `48,564 / 53,379`
  Test262 passes and the JetStream baseline at 15/19 runners.
- Fold addition trees only when every leaf is provably numeric and the tree
  contains a string-literal unary-plus conversion. Conversion uses the
  runtime's existing ECMAScript string-to-number rules; the narrow trigger
  leaves ordinary V1 bytecode-shape contracts and dynamic conversion hooks
  unchanged while hoisting the opponent coercion loop's invariant expression.
- Expand named-property read sites from one shape to a fixed four-entry
  polymorphic cache. This avoids cache replacement churn for object cohorts
  that share a property slot but acquire a few optional properties.
- Select a polymorphic entry from one synchronized receiver-shape read before
  loading its slot, avoiding repeated arena lookups for each candidate shape.
- Final full Test262 remained exactly `48,564 / 53,379` (`90.98%`), with
  4,813 failures and 2 skips. The supported correctness gate remained 24/24.
- The final same-machine opponent timing suite averaged 19.462 ms for AgentJS
  versus 22.146 ms for OxideJS, so AgentJS is 12.1% faster on the suite while
  warm startup is effectively tied (20.340 ms versus 20.366 ms).
- The same 15-runner JetStream PASS set remained 15/15 and improved from
  135.159 s to 130.999 s in aggregate (3.1%); no passing workload regressed to
  an error or timeout.
- Validation passed `cargo test --locked --all-targets`, `cargo clippy
  --locked --all-targets -- -D warnings`, and `cargo fmt --all -- --check`.

## 2026-08-11 performance follow-up

- Locked the pre-change full Test262 baseline at `48,558 / 53,379`
  (`90.97%`, 4,819 failed, 2 skipped) in
  `reports/.native-test262-tmp/perf-fix-baseline-2026-08-11.json`.
- Locked the 19-runner JetStream baseline in
  `reports/jetstream2-perf-fix-baseline-2026-08-11/summary.json`: 15 PASS,
  two CALL_ERROR, and two TIMEOUT.
- Added guarded dense Array push/pop mutations and a narrow VM call fast path.
  Holes, inherited indexed setters, accessors, non-configurable elements,
  sparse indices, overridden methods, and non-writable length still use the
  semantic slow path.
- Added an invalidation-aware Array prototype cache for the canonical `push`
  and `pop` data properties.
- Moved ordinary CLI `eval`/`run` evaluation onto a 32 MiB stack thread so a
  200-level JavaScript recursion completes instead of terminating the process
  with a native stack overflow.
- Focused same-machine results so far: competitor `array.js` fell from
  11.09 s to about 0.14 s; `ic_cache.js` fell from 1.71 s to about 0.09 s;
  `call.js` now completes at about 21 ms.
- Final same-machine competitor-suite run:
  - 10 short-script process means: 21.969 ms (OxideJS 22.146 ms).
  - `array.js`: 97.982 ms (OxideJS 103.724 ms; pre-change AgentJS
    11,093.385 ms).
  - `call.js`: 21.437 ms and correct result (OxideJS 21.218 ms;
    pre-change AgentJS terminated with native stack overflow).
  - `ic_cache.js`: 69.420 ms (OxideJS 51.248 ms; pre-change AgentJS
    1,713.267 ms).
  - AgentJS remains faster on `property.js` and `string.js`; OxideJS remains
    faster on `code_forge.js`, `coercion.js`, and `gc.js`.
- Full post-change Test262 result:
  `48,561 / 53,379` (`90.97%`, 4,816 failed, 2 skipped), a net gain of three
  passes with no skipped-case increase.
- Post-change JetStream result: 15/19 PASS, unchanged status for all runners.
  The sum of wall time across the 15 passing runners fell from 228.853 s to
  123.584 s (1.85x aggregate speedup); every common PASS runner improved.
- Final validation: supported correctness 24/24, all-target Rust tests passed,
  and Clippy passed with warnings denied. Formatting was applied after the
  initial check reported only the newly added test wrapping.
- After adding Realm-scoped prototype-cache ownership, the final release scan
  improved further to `48,564 / 53,379` (`90.98%`, 4,813 failed, 2 skipped):
  six more passes than the locked baseline. The final 15-runner PASS-set
  JetStream rerun remained 15/15 PASS with a 135.159 s wall-time sum, still
  1.69x faster than the 228.853 s pre-change baseline. The complete 19-runner
  matrix immediately before that safety-only change remained 15/19 with no
  status transition.

## Scope

This branch implements only Part B from the 2026-08-04 JetStream repair plan:
RegExp translation, RegExp continuation semantics, and focused regression
tests. Runner/resource, Intl, diagnostics, object-model, and performance work
are intentionally excluded.

Base SHA: `7bcd72a`

Branch: `fix/jetstream-regexp`

## Fixes

- Preserve `\-` as a literal hyphen while translating legacy character
  classes, including the validatorjs `/[@_\- ]/g` pattern.
- Continue global and sticky matching against the complete input with an
  explicit start offset instead of slicing at `lastIndex`. This preserves the
  correct context for anchors, word boundaries, and lookbehind assertions.
- Keep sticky checks, UTF-16 match indices, and `d`-flag indices consistent
  with the full-input capture offsets.
- Add focused parser, constructor, validatorjs, word-boundary, and deterministic
  Octane/JetStream checksum regressions.

No VM exception-propagation change was required: after the character-class
translation fix, validatorjs no longer reports the RegExp compilation error or
the cascading `undefined is not callable` error.

## Validation

- Supported correctness gate: `24 / 24` passed.
- `test/built-ins/RegExp`: `1,759 / 1,879` passed, two above the reported
  `1,757 / 1,879` baseline.
- `test/built-ins/RegExp/property-escapes`: `611 / 613`, unchanged.
- `cargo test --locked --all-targets`: passed.
- `cargo check --locked --all-targets`: passed.
- `cargo fmt --all -- --check`: passed after formatting.
- `cargo clippy --locked --all-targets -- -D warnings`: blocked only by the
  pre-existing `collapsible_if` diagnostic in `src/builtins/date_intl.rs:13729`,
  outside Part B ownership.

The checked-in generated `regexp.js` runner now validates its checksum. The
validatorjs runner proceeds beyond RegExp construction and currently stops on
an unrelated Date assertion (`2010-07-02,[object Object]`), outside Part B.
