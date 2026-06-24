# Native V8 Scope: Parallel Feature Unlock Batch

Native V8 is the first post-V7 feature-growth milestone driven by the full
Test262 analysis in `reports/test262-analysis.md`.

V8 uses three parallel tracks. Each track develops a different feature area in
the same version window, then all tracks merge into one V8 integration pass.
No group should start V9/V10/V11 feature work before V8 integration is complete.

Shared contracts are defined in
[Native V8 Shared Interface](native-v8-interface.md), and file ownership is
defined in [Native V8 Team Plan](native-v8-team-plan.md).

## 1. Baseline

The 2026-06-24 direct full Test262 run completed with:

| Total | Passed | Failed | Skipped | Pass rate |
| ---: | ---: | ---: | ---: | ---: |
| 53,379 | 14,035 | 38,507 | 837 | 26.29% |

The dominant failure classes are:

| Failure class | Count | Share |
| --- | ---: | ---: |
| Parser syntax gap | 16,259 | 42.22% |
| Missing global / builtin / harness helper | 9,219 | 23.94% |
| Template literal substitutions unsupported | 5,307 | 13.78% |
| Assertion / semantic mismatch | 2,131 | 5.53% |
| Runtime call/property/object-model gap | 789 | 2.05% |
| Expected-error / early-error mismatch | 789 | 2.05% |
| Lexer/static syntax gap | 530 | 1.38% |
| RegExp engine feature gap | 528 | 1.37% |

V8 targets the first three classes: frontend blockers, module skips, and
missing global/builtin skeletons.

## 2. V8 Tracks

### V8-A — Frontend Unlockers

Owner: A group.

Scope:

- template literal substitutions: `` `${expr}` `` and multi-part templates;
- class syntax first stage: declarations, expressions, constructors, prototype
  methods, static methods, and parse-only `extends` support where full `super`
  behavior is not ready;
- spread/rest/destructuring first stage:
  - call spread: `f(...args)`;
  - array spread: `[a, ...b]`;
  - rest parameter: `function f(...args)`;
  - simple array/object destructuring binding.

Expected effect:

- reduce `Template literal substitutions unsupported`;
- reduce broad `Parser syntax gap`;
- move class/spread/destructuring tests from parse failure to runtime or
  semantic failure.

### V8-B — Module Runner Infrastructure

Owner: B group.

Scope:

- distinguish script and module execution in the native path;
- module top-level strict mode;
- module-specific error classification;
- module scope and runtime binding data structures;
- module registry to avoid duplicate execution;
- relative-path loader and acyclic dependency graph execution.

Parser-facing import/export syntax can land in A group when its AST contract is
ready. B owns the runtime and runner infrastructure needed to execute those AST
forms once available.

Expected effect:

- prepare the system to reduce `module runner not implemented yet` skips;
- make module failures independently classifiable.

### V8-C — Builtin Skeletons First Batch

Owner: C group.

Scope:

- TypedArray / ArrayBuffer skeletons:
  - constructors and prototypes;
  - `name`, `length`, and descriptor shape;
  - priority names: `Float64Array`, `Uint8Array`, `Int32Array`;
- Intl skeletons:
  - `Intl` namespace;
  - `Intl.DateTimeFormat`;
  - `Intl.NumberFormat`;
  - `Intl.Collator`;
  - minimal deterministic `resolvedOptions()` and `supportedLocalesOf()`;
- `$262` host helper first batch, limited to helpers that appear in the full
  analysis hotspots.

Expected effect:

- reduce high-frequency `is not defined` failures;
- convert missing-global failures into descriptor/semantic failures.

## 3. Explicit Non-Goals

V8 does not include:

- full Temporal semantics;
- complete Intl locale-sensitive formatting;
- complete TypedArray algorithms and detach semantics;
- Promise/job queue implementation;
- full async/generator execution semantics;
- full module cycle handling or live-binding edge cases;
- RegExp property escapes and backreferences;
- complete descriptor sweep for all builtins.

Those belong to later batches documented in `thoughts/plan.md`.

## 4. Test Areas

### V8-A focused commands

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/native-v8-a-language-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String --jobs 4 --progress --json reports/native-v8-a-string-summary.json
```

### V8-B focused command

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v8-b-module-summary.json
```

### V8-C focused commands

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v8-c-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --progress --json reports/native-v8-c-arraybuffer-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v8-c-intl402-summary.json
```

### V8 integration command

Lightweight V8 integration scan:

```sh
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
```

This command runs the locked 5,000-case manifest in
`reports/native-v8-scan-failures.txt`, sampled from cases that did not pass in
the 2026-06-24 full direct run. The initial summary is
`reports/native-v8-scan-summary.json`: 0/5,000 passed, 4,504 failed, and 496
skipped.

Full direct run:

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

## 5. Completion Criteria

V8 is complete only when:

- all three tracks have merged;
- old V1-V7 regression gates remain green;
- the V8 focused commands have current JSON summaries;
- `--native-v8-scan` has a current JSON summary;
- the full direct Test262 command has been rerun;
- `reports/test262-report.md` and `reports/test262-analysis.md` are updated;
- `AGENTS.md` and `thoughts/plan.md` describe the new status;
- no skipped tests are counted as passes.
