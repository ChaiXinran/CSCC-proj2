# Phase 4 Part J — Memory Accounting and Adaptive GC

## Owner and scope

J owns runtime memory snapshots, allocation-pressure accounting, the adaptive
non-moving GC controller, and GC diagnostics. This change does not implement K's
side-registry sweep/recycling and does not modify L's staged Host-script
compilation or execution semantics.

The locked source requested by the Phase 4 plan was `main@b6168e86`, but that
object is not present in this checkout. Work was performed on the current clean
`main@9ec36e0`; the pre-existing untracked `.claude/` directory was left alone.

## Implementation

- Added the frozen `RuntimeMemoryStats`, `MemoryClass`, `AllocationPressure`,
  `GcPolicy`, and `GcControllerState` contracts in `runtime/memory.rs`.
- Runtime snapshots now expose heap live counts and arena capacity, Promise/job
  counts, ArrayBuffer payload capacity, TypedArray/DataView counts, major side
  table counts, shapes/caches, tracked runtime bytes, and charged bytes.
- Added allocation, byte-pressure, and tracked-growth collection reasons.
- Added post-collection reclaim-ratio tracking. A reclaim ratio below the
  configured minimum doubles the next soft allocation interval, bounded by the
  hard cap, to avoid fixed-low-threshold GC thrashing.
- Preserved compatibility for legacy thresholds at or below 250,000 allocations.
  In particular, Test262's 25,000 threshold follows the old allocation-only
  behavior. Larger thresholds (including JetStream's 1,000,000 default) use the
  conservative adaptive defaults: 20k eligibility, 16 MiB pressure, 1.5x tracked
  growth, and a 250k hard cap.
- Extended diagnostics with a stable `runtime_memory:` record and GC trigger /
  reclaim fields. These are approximate tracked bytes, not an RSS claim.

## Validation

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --all-targets` | PASS |
| `cargo test --all-targets` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo test --test runtime_memory --test native_gc` | PASS, 18/18 |
| supported correctness gate | PASS, 24/24 |
| full Test262, release/no-default-features, 4 jobs | PASS baseline, 48,560/53,379, 4,817 failed, 2 skipped |

The final full Test262 summary is
`reports/.native-test262-tmp/phase4-partJ-full-final.json`. This exactly matches
the protected Phase 3 best baseline and therefore does not reduce correctness.
Two diagnostic pre-final runs produced 48,557 before legacy-threshold
compatibility was restored and 48,559 after restoration; both are retained as
local ignored evidence, while the required final identical rerun is committed.

## Adaptive workload evidence

A protected WSL smoke run used the generated runner, 1,000,000 legacy threshold,
32 MiB thread stack, diagnostics, and a 30-second absolute deadline. It reached
job drain, performed 62 collections, and exited with the expected `RuntimeLimit`
at the absolute deadline without a missing-object failure. The final diagnostic
snapshot reported:

```text
gc_reason=Growth
gc_count=62
gc_total_pause_ns=4820152100
tracked_runtime_bytes=27965523
heap_estimated_bytes=11791955
gc_reclaim_percent=42
```

This short run verifies controller activation and semantic stability, but it is
not a 1.5 GiB / 150-second acceptance measurement and makes no peak-RSS claim.
The full WSL/jsdom threshold matrix remains an integration task after K's
side-registry reclamation is available.

## Cross-track dependencies and risk

- K can call `note_runtime_growth` and provide reclaimable side-registry
  capacities without changing the policy implementation.
- L has no dependency on this change.
- `property_ic_entries` remains zero because the current IC metadata is VM-local;
  no J-owned reverse dependency was introduced merely for diagnostics.
- Job queue capacity currently reports its live length because K owns the queue
  representation and has not exposed retained `VecDeque` capacity.

## Next action

Merge K's side-registry sweep before final policy tuning, then run the protected
WSL/jsdom/threejs matrix at 10k, 100k, 1m, and adaptive settings with OS peak RSS.

## J/K merge validation

The remote K stable-arena and side-sweep changes were merged into J's shared
`NativeContext` GC boundary. `RuntimeMemoryStats` now consumes K's live/capacity
provider, and J computes reclaim ratio only after both side registries and the
ordinary heap have been swept. J/K focused tests passed 21/21, all-target tests
and Clippy passed, and the post-merge full Test262 run reached
48,563/53,379 (90.98%, 2 skipped), three passes above the protected 48,560
baseline.
