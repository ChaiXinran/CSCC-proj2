# Phase 3 Part G — Upvalue Slot

Date: 2026-08-04  
Baseline: `6f44bfa58681dbd63460ae5bfc49a6aed959bad1`

## Implementation

- Added `UpvalueSlot`, `UpvalueDescriptor`, `UpvalueBindingLayout`, and shared `UpvalueLayout` metadata.
- Added validated `LoadUpvalue` and `StoreUpvalue` instructions with the same stack behavior as their Local counterparts.
- Compiler resolution now prefers current Local Slot, then a statically resolvable Upvalue Slot, then the existing name/global paths.
- Parent and ancestor activation slots are resolved from the function's captured environment using fixed environment hops. Function-creation block depth is included.
- Function-body and block lexical shadowing prevent an incorrect activation capture.
- A function containing direct eval or `with` rewrites its own and all descendant Upvalue instructions back to name instructions. `with` creation sites also decline static captures.
- Runtime access follows `CallFrame.function -> JsFunction.upvalue_layout -> JsFunction.environment -> fixed hops -> LocalSlot` without adding a GC root.
- Diagnostics now include load/store Upvalue counts and fixed Upvalue environment hops.

No `runtime/mod.rs` or `contracts.rs` export was changed; the shared types remain available through the existing public `bytecode` boundary.

## Correctness

`tests/upvalue_slots.rs` contains 8 tests covering parent read/write, ancestor hops, block-created closures, activation and lexical shadowing, arrow/loop closures, generator yield, async bytecode after await, dynamic-scope fallback, metric hit rate, and invalid-slot validation. Existing Local Slot tests remain 10/10.

Project gates passed:

- `cargo fmt --all -- --check`
- `cargo check --locked --all-targets`
- `cargo test --locked --all-targets`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo build --release --locked`
- `cargo test --release --no-default-features --test native_test262` (15/15)

Focused Test262 results:

| directory | result |
|---|---:|
| `language/expressions/function` | 249/264 |
| `language/expressions/arrow-function` | 333/343 |
| `language/expressions/generators` | 265/290 |
| `language/expressions/async-function` | 73/93 |
| `built-ins/eval` | 8/10 |
| `language/statements/with` | 127/181 |

The function, eval, and with totals match the locked Phase 2 results; the remaining focused totals establish the post-P0 baseline and showed no failure during the G run.

A full 53,379-case run initially reported 48,563 passes. Because this did not reproduce the 48,594 figure quoted by the earlier P0 merge report, G ran a controlled same-binary comparison with Upvalue lowering disabled and captured every verbose failure path. The enabled and disabled runs both produced 48,564 passes, 4,813 failures, and 2 skips, with identical failure-path sets (zero newly failing and zero newly passing cases). The one-pass aggregate difference of one case was not reproducible. Therefore G introduces no Test262 net regression relative to a directly reproducible post-P0 fallback baseline; the older 48,594 aggregate is not reproducible from `6f44bfa` under the current tree and must not be used as a path-level baseline.

Richards completed with GC thresholds 10k, 100k, and 1m, producing the completion marker at every threshold.

## Performance

Five independent one-iteration measurements used the same runner hashes and JetStream revision for the fixed P0 baseline and G build:

| workload | baseline median / p90 (ms) | G median / p90 (ms) | median change |
|---|---:|---:|---:|
| richards | 7123.892 / 7571.497 | 7107.488 / 7141.625 | -0.23% |
| mobx | 6641.528 / 6674.618 | 6505.777 / 7072.563 | -2.04% |
| splay | 4665.553 / 4706.428 | 4666.399 / 4707.619 | +0.02% |

No common PASS workload regressed by 5%. A closure-dense 500k-call micro-workload measured 506.857 ms baseline median and 406.148 ms G median over five process samples, a 19.87% improvement. The automated eligible-access test requires Upvalue accesses to represent at least 70% of Upvalue plus name accesses and passes using runtime counters.

Raw current measurement JSON is intentionally ignored under `reports/`; the relevant runner hashes, medians, p90 values, and baseline comparison are preserved here.

## Coordination

- G did not modify PropertyMap, object property operations, Shape/IC state, lifecycle deadlines, lexer/parser, or runner generation.
- `compiler.rs` changes are confined to capture resolution and dynamic fallback; an I-group compiler deadline hook should be applied after this lowering is merged.
- The internal compiler uses equivalent capture maps instead of introducing the suggested `CaptureScope` struct. This changes no frozen public shape or runtime semantics.
