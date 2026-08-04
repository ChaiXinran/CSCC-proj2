# Three-track structural repair — Part B report

## Scope

This change implements Part B of the shared three-track repair: immutable
shared bytecode ownership, cache retention reduction, and constant-pool
deduplication. It also includes the requested narrowly scoped parser memory
fix: parser advancement no longer clones payload-bearing tokens, and retained
source text uses `Arc<str>`.

## Implementation

- `ProgramCompiler` and `NativePipeline::compile` return `SharedChunk`, an
  `Arc<Chunk>`.
- Function templates, runtime functions, loaded modules, and the native script
  cache share immutable chunks rather than cloning instructions, constants,
  and nested templates.
- Script-cache entries no longer retain the parsed AST and keep only the shared
  chunk plus cache-safe metadata.
- `Chunk::add_constant` uses a hash index keyed by exact constant identity;
  numbers retain the previous `f64::to_bits` semantics. The transient index is
  discarded by `Chunk::into_shared`.
- `Parser::advance` only advances the cursor and does not clone `Token` string
  payloads. Regex re-lexing source ownership is shared through `Arc<str>`.
- Per-function heap estimates no longer repeatedly charge shared bytecode.

## Boundary notes

The core change is B-owned. Small integration adaptations in contracts,
backend cache assembly, VM/runtime chunk holders, and parser token consumption
are limited to carrying the shared type or implementing the explicitly
requested string-copy reduction. No RegExp, benchmark input, or A/C semantic
behavior was changed.

## Validation

- `cargo test --locked --all-targets`: passed, including the existing
  JetStream Octane RegExp checksum test.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- Supported correctness gate: `24 / 24` passed.
- Full Test262: `48,586 / 53,379` passed (`91.02%`, 2 skipped), compared with
  the checked-in baseline `48,419 / 53,379` (`90.7080%`). This is no
  correctness regression (`+167` passing tests).
- Full summary: `reports/.native-test262-tmp/three-track-part-b-full.json`.
