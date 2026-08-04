# Phase 2 Part E — Compact Property Storage

## Scope

This change implements Part E of the phase-two hot-path plan. D local-slot
lowering and execution are untouched. The only F-owned addition is the frozen
minimal `JsString` handle required by E's `PropertyName` contract; `JsValue`,
host strings, builtins, and F conversion hot paths are not migrated.

## Implementation

- Property names share one `Arc<str>` backing between the ordered entry and
  hash index.
- `PropertyMap` uses stable `PropertySlotId` values and tombstone deletion;
  delete no longer calls `Vec::remove` or rewrites every later index.
- Compaction starts at 64 slots and a 25% tombstone ratio, preserves live entry
  order, and rebuilds the index in one pass.
- Array-index names remain numerically ordered; ordinary names preserve
  insertion order; delete plus redefine appends the name.
- GC tracing skips tombstones, and a regression verifies that a deleted
  descriptor no longer retains its object value.
- Diagnostics expose live property, tombstone, compaction, key-byte, and delete
  counters directly from `PropertyMap`.
- Memory estimates count shared key backing bytes once while conservatively
  accounting for both Arc handles and allocated slot/index capacity.

## Validation

- `cargo check --locked --all-targets`: passed.
- `cargo test --locked --all-targets`: passed, including PropertyMap ordering,
  object descriptors, Proxy-adjacent behavior, and GC tombstone coverage.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- Full Test262: `48,594 / 53,379` passed (`91.04%`, 2 skipped), versus the
  pre-change shared baseline `48,586 / 53,379`; no regression (`+8` passes).
- Test262 summary: `reports/.native-test262-tmp/phase2-partE-full.json`.

## Performance

`hash-map` was measured five times on both the isolated `e4382f9` baseline and
this E implementation, using the same machine, JetStream revision, generated
runner SHA, one requested iteration, and the repository measurement script.

| Version | Passes | Median | p90 | Peak working set |
|---|---:|---:|---:|---:|
| `e4382f9` baseline | 5/5 | 21.119 s | 22.043 s | 1171.6 MiB |
| Phase 2 E | 5/5 | 21.515 s | 21.952 s | 1162.6 MiB |

Median wall time changed by `+1.88%`, within the allowed 5% regression bound.
Peak working set improved by about `0.77%`; this does **not** meet the phase
plan's aspirational 10% RSS target and is recorded as an outstanding
performance target rather than claimed as complete. Measurement output:
`reports/phase2-partE/hash-map-rerun.json`.
