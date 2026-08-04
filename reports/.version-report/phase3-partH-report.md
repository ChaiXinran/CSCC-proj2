# Phase 3 Part H — Shape and Monomorphic Property IC

## Scope

This change implements H-owned ordinary-object shapes, `PropertyMap` structural
generations, and monomorphic named-property Get/Set caches. G bytecode binding,
compiler, environment, and upvalue code is untouched. I lifecycle, runner,
lexer, and parser code is untouched. The only shared runtime integration is the
minimal export of H-owned types and reading the existing deadline state.

## Implementation

- Per-isolate `ShapeTable` shares transitions for identical ordinary-object
  property layouts.
- `PropertyMap` increments generation for insertion, deletion, compaction, and
  descriptor structure changes; writable data-value replacement is stable.
- Cache sites use chunk address plus instruction offset and contain only shape,
  generation, and stable property slot — never an object handle or GC root.
- Get caches only own data values on ordinary objects. Set caches only existing
  writable own data values and preserves assignment results.
- Accessor, Proxy, prototype, missing, symbol, computed, array, TypedArray, and
  exotic paths continue through the complete existing slow path.
- Sites quicken after a second observation and negatively cache ineligible
  sites, avoiding repeated shape work on polymorphic/exotic traffic.
- VM-internal private names and coercion hooks (`valueOf`/`toString`) never
  enter the ordinary-property IC, preserving brand and observable coercion
  semantics.
- Executions with an active cooperative wall-clock deadline skip IC probing;
  this avoids spending strict short-task budgets on cache warm-up. Ordinary and
  JetStream execution without an internal deadline uses the IC normally.
- Delete, prototype changes, and non-extensibility invalidate or place objects
  in dictionary mode. Diagnostics expose Get/Set hit/miss, transitions,
  dictionary objects, and invalidations.

## Validation

- `cargo fmt --all -- --check`: passed.
- `cargo check --locked --all-targets`: passed.
- `cargo test --locked --all-targets`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- Full Test262 (`--release --no-default-features`, 4 jobs):
  `48,560 / 53,379` passed (`90.97%`, 2 skipped), exactly equal to the isolated
  pre-H `6f44bfa` baseline; no H regression.
- A prior shared Phase 2 report recorded `48,594`; the frozen Phase 3 input was
  already at `48,560`, so that pre-existing 34-test difference is outside H.
- The three TypedArray `copyWithin` deadline regressions found during testing
  were fixed; its focused suite is `65 / 65`, equal to baseline.
- Property/Object/Proxy/Reflect/assignment/class focused suites were compared
  against the isolated baseline with identical pass counts.
- Final Test262 summary:
  `reports/.native-test262-tmp/phase3-partH-full-final-rerun.json`.

## Performance

`hash-map` was measured five times on the same machine for both the isolated
`6f44bfa` baseline and final H build, with identical generated runner SHA and
JetStream revision.

| Version | Passes | Median | p90 | Peak working set |
|---|---:|---:|---:|---:|
| `6f44bfa` baseline | 5/5 | 19.051 s | 21.643 s | 1325 MiB |
| Phase 3 H final | 5/5 | 16.616 s | 19.698 s | 1236 MiB |

Median wall time improved by `12.78%`, satisfying the phase target of at least
10% on one property-heavy workload. Measurement outputs are
`reports/phase3-partH/baseline.json` and
`reports/phase3-partH/current-final.json`.
