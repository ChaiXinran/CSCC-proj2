# Phase 4 Part L：Persistent staged Host-script session

Baseline: `main@04d3eb4` (`K-finished`), requested closeout baseline `main@b6168e86`.

## Implemented

- Added `SourceKind::HostScriptFragment`.
- Added public `PreparedHostFragment`, `HostFragmentDeclarations`, and
  `HostScriptSession` APIs through `Runtime`.
- JetStream now reads and prepares all staged entry resources before executing
  them, without concatenating source text.
- Session startup validates cross-fragment lexical/var/function conflicts,
  predeclares cross-fragment `var` names, and instantiates top-level function
  declarations before the first fragment executes.
- Host fragment execution preserves the same global environment and runs jobs at
  the same stage boundary as the previous runner.
- Hoisted `var` initialization in a later fragment no longer overwrites a value
  assigned by an earlier fragment.
- Workloads marked with deterministic randomness now install the seeded
  `Math.random` implementation in the host prelude before any staged resource
  executes. This preserves the original setup-before-resource ordering.

## Focused validation

`tests/host_script_session.rs` covers:

- later function hoisting;
- cross-fragment `var` visibility and assignment preservation;
- lexical conflict rejection before execution;
- bundle export visibility in a following fragment.

All four focused tests pass.

## Regression gates

- `cargo fmt --all -- --check`: PASS
- `cargo check --locked --all-targets`: PASS
- `cargo test --locked --all-targets`: PASS (281 tests)
- `cargo clippy --locked --all-targets -- -D warnings`: PASS
- `cargo test --release --no-default-features --test native_test262`: 15/15 PASS
- `git diff --check`: PASS

## JetStream classification

The staged session path is active for generated JetStream resources. After
moving deterministic randomness into the host prelude, `regexp-octane` changes
from `ENGINE_FAILURE` to `PASS` (one iteration: 2746 ms wall time). The
validatorjs bundle export and `ValidatorJSBenchmark.runTest` are visible as
object/function across the two fragments, but its full test still fails with
`undefined is not callable` inside the benchmark body. It therefore remains a
separately classified runtime/builtin issue rather than being attributed to
host-session lifetime.

No J/K files or GC/side-registry behavior were changed in this part.
