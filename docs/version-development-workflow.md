# Version Development Workflow

This is the current lightweight workflow. Historical per-version plans,
per-track interface freezes, and fixup coordination documents have been
consolidated into `docs/runtime-evolution.md` and the reports under `reports/`.

## Before implementation

1. Define the intended behavior, exclusions, affected modules, and acceptance
   tests in the issue, task, or a short design section in the relevant
   canonical document.
2. If a shared contract changes, update `src/contracts.rs` and
   `docs/interface-spec.md` together.
3. Do not modify pinned reference trees (`boa/`, `quickjs/`, `test262/`,
   `benchmarks/JetStream2/`, or `third_party/`) unless the task explicitly
   targets them.

## During implementation

1. Keep parser, compiler, VM, runtime, and builtin logic in their owning
   modules; `backend/native.rs` should assemble the pipeline.
2. Add focused tests near the affected subsystem.
3. Use `src/test262_manifests/` only for fixed manifests that are actual CLI
   inputs. Generated Test262 JSON belongs in ignored local output or, for the
   official full result, `Test262-final/full-test262-summary.json`.
4. Raw benchmark output remains ignored. Preserve decision-relevant medians,
   p95/p90, pass/fail state, and environment information in a consolidated
   report.

## Reporting boundary

- Design, architecture, interfaces, process, and current usage belong in
  `docs/`.
- Measured outcomes, version history, performance deltas, and delivery evidence
  belong in `reports/`.
- Update the applicable consolidated report in the same change:
  `version-history-v8-v12.md`, `version-history-v13-v18.md`,
  `optimization-and-demo-history.md`, or a new clearly scoped summary.
- Do not recreate per-track report directories or commit raw runner output.

## Validation

Run checks proportionate to the change. Before merging runtime behavior
changes, the standard gate is:

```powershell
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-default-features --test native_test262
```

Correctness changes must compare against the current official full Test262
summary. Performance changes must use like-for-like release builds on the same
machine and must not silently omit failing workloads.

## Completion criteria

- Focused regressions are covered.
- Test262 correctness does not decrease.
- Relevant benchmark performance does not regress outside the agreed tolerance.
- Canonical documentation and one consolidated report are updated.
- Generated and raw outputs remain ignored; only decision-relevant summaries
  are committed.
