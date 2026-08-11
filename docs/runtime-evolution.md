# Runtime Evolution and Superseded Design Decisions

This document preserves the durable design decisions formerly spread across
`docs/version/`, `docs/fixup/`, and the phase-specific interface drafts. It is
historical design context, not a source of current performance or correctness
figures.

## Frontend and pipeline foundation (V1-V7)

The first native versions established the lexer, AST, parser, bytecode, VM,
runtime, and builtin pipeline. Development was split behind the stable
`SourceParser`, `ProgramCompiler`, and `ChunkExecutor` boundaries. Control
flow, functions, objects, environments, exceptions, and initial standard
builtins were added incrementally. The current normative boundary is
`docs/interface-spec.md`.

## Conformance expansion (V8-V12)

V8 added frontend unlockers, module-runner infrastructure, builtin skeletons,
and the Test262 host. V9 expanded generator, async, iterator, Promise, Map, and
Set support. V10 focused on BigInt/numeric syntax, binary data, Date, Intl, and
Temporal. V11 closed RegExp, Annex B, descriptor, and object-model gaps. V12
consolidated the native runtime and removed remaining integration shortcuts.
Fixup plans were temporary coordination documents for these stages and are
superseded by the consolidated result history.

## Runtime protocol consolidation (V13-V18)

Later versions standardized module live bindings, dynamic import, class and
`super` behavior, abstract operations, object internal methods, Promise
capabilities, property-key conversion, descriptors, Proxy observability, and
sparse Array behavior. Shared contracts belong in `src/contracts.rs`; runtime
semantics remain in their owning modules rather than `backend/native.rs`.

## Performance and memory phases

The hot-path phases introduced shared immutable chunks, compact property
storage, local and upvalue slots, shapes and monomorphic property inline
caches, runtime memory accounting, adaptive GC, side-registry sweeping, and
persistent host-script support. These mechanisms must preserve Test262
correctness and be measured using the methodology in `docs/benchmark.md`.

## Current authority

- Current architecture: `docs/architecture.md`
- Current shared interfaces: `docs/interface-spec.md`
- Current development process: `docs/version-development-workflow.md`
- Historical correctness results: `reports/version-history-v8-v12.md` and
  `reports/version-history-v13-v18.md`
- Historical performance and memory results:
  `reports/optimization-and-demo-history.md`
- Official current Test262 result:
  `Test262-final/full-test262-summary.json`
