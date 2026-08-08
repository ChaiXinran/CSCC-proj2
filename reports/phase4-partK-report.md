# Phase 4 Part K — Runtime side-registry lifetime

Date: 2026-08-08  
Baseline: `main@b6168e86a3a91ac7ad43293e806c070663a488ad`

## What changed

- Added a stable recyclable arena (`Vec<Option<T>>` plus a free list) without renumbering live IDs.
- Moved Promise, ArrayBuffer, TypedArray-view, and DataView registries to stable arenas.
- Added a side-mark closure to each normal heap collection:
  - live Promise/ArrayBuffer/TypedArray/DataView objects mark their side IDs;
  - TypedArray/DataView records mark their backing ArrayBuffer;
  - Promise records trace state and reaction values and mark result promises;
  - queued and active jobs mark Promise IDs that are not represented by ordinary `JsValue` roots;
  - pending Test262 agent waiters mark shared ArrayBuffers.
- Side registries now sweep at the same collection boundary as the heap and recycle dead slots.
- Promise records are no longer unconditional context roots. Scheduled reactions are already removed
  from the source Promise via `mem::take`; completed jobs leave the queue and active-root set.
- Added bounded capacity cleanup after materially sparse collections.
- Exposed side collection and retained-capacity diagnostics through
  `runtime_side_memory_stats()` and `last_side_collection()`.

## Files touched

- `src/runtime/stable_arena.rs`
- `src/runtime/mod.rs`
- `src/runtime/gc.rs`
- `src/runtime/context.rs`
- `src/runtime/job.rs`
- `src/runtime/buffer.rs`
- `src/runtime/agent.rs`
- `src/vm/interpreter.rs`
- `tests/native_gc.rs`
- `tests/runtime_side_gc.rs`
- `reports/phase4-partK-report.md`

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --all-targets` | PASS |
| `cargo test --test runtime_side_gc` | PASS, 3/3 |
| `cargo test --test native_gc` | PASS, 14/14 |
| `cargo test --all-targets` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --release --locked` | PASS |

## Coverage and deltas

- Dead Promise and binary-data records are reclaimed and their stable IDs are reused.
- A live TypedArray preserves both its metadata and backing payload across GC.
- A queued Promise job preserves its target record; after completion the record is reclaimable.
- Existing active Promise callback GC regression coverage remains passing.
- No Test262 or JetStream aggregate baseline was changed in this track.

## Coordination notes

- Track J can consume `RuntimeSideMemoryStats` and `SideCollectionStats`; K does not alter the GC
  trigger policy.
- Full WSL/jsdom/threejs RSS measurements remain an integration benchmark task. This report does
  not claim the Phase 4 25% RSS target without protected-run measurements.
