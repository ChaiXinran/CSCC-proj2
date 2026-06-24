# Native V11 Scope: Semantic Precision and RegExp Expansion

Native V11 starts while V10-A frontend work may still be in progress, but
V10-B and V10-C are treated as available integration inputs. V11 is a
three-track quality batch focused on tests that already reach execution but
still fail due to RegExp gaps, object-model precision, descriptor shape, or
Annex B behavior.

Shared contracts are defined in
[Native V11 Shared Interface](native-v11-interface.md), and file ownership is
defined in [Native V11 Team Plan](native-v11-team-plan.md).

## 1. Baseline

V11 uses the locked V11 lightweight scan manifest:

```text
reports/native-v11-scan-failures.txt
```

The manifest contains 5,000 previously non-passing Test262 cases sampled from
the locked full direct output, prioritizing:

- `test/built-ins/RegExp`
- RegExp-facing `String.prototype` methods
- RegExp literal/static-error language tests
- `test/annexB`
- `test/built-ins/Object`
- `test/built-ins/Function`
- `test/built-ins/Array`
- `test/built-ins/String`
- `test/built-ins/Iterator`
- descriptor/property-order/name/length/configuration precision cases

Initial scan command:

```sh
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Initial setup attempt:

```text
The first `--native-v11-scan` attempt exceeded the 300s local tool timeout and
did not produce `reports/native-v11-scan-summary.json`.
```

The selector and manifest are installed; rerun the command after replacing any
long-running samples or with a larger external timeout.

## 2. V11 Tracks

### V11-A — RegExp Parser / Static Errors

Owner: A group.

Scope:

- RegExp literal parser/static errors;
- Unicode property escape syntax;
- regexp duplicate/named group residual rules;
- early-error classification for RegExp-related syntax.

Expected effect:

- reduce RegExp `SyntaxError` and error-kind mismatch failures;
- expose runtime RegExp builtin failures to C group.

### V11-B — Object Model / Descriptor Precision

Owner: B group.

Scope:

- property descriptor exactness;
- receiver handling for get/set operations;
- getter/setter invocation order;
- property lookup and own-key ordering;
- expected-error ordering in shared runtime object paths.

Expected effect:

- reduce semantic mismatch after parser/builtin skeletons are present;
- give C group reliable descriptor/object-model behavior for builtin sweeps.

### V11-C — RegExp Builtins / Annex B / Descriptor Sweep

Owner: C group.

Scope:

- RegExp prototype/static builtin semantics;
- String methods that dispatch to RegExp;
- Annex B legacy accessors and behavior;
- descriptor sweep for implemented Object/Function/Array/String/Iterator
  builtins.

Expected effect:

- reduce RegExp and Annex B failures;
- tighten descriptor shape for already implemented builtin families.

## 3. Explicit Non-Goals

V11 does not include:

- full Temporal or Intl semantics;
- shared-memory Atomics semantics;
- full async-generator scheduling;
- module live-binding/cycle semantics;
- complete TypedArray/DataView algorithm coverage beyond V10 handoff;
- browser/Web API compatibility outside ECMAScript Test262.

## 4. Focused Commands

### V11-A

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json
```

### V11-B

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-b-object-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --progress --json reports/native-v11-b-reflect-summary.json
```

### V11-C

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress --json reports/native-v11-c-regexp-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB --jobs 4 --progress --json reports/native-v11-c-annexb-summary.json
```

### V11 Integration

```sh
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

## 5. Completion Criteria

V11 is complete only when:

- all three tracks have merged;
- old V1-V10 regression gates remain green;
- focused A/B/C summaries are current;
- `--native-v11-scan` has a current JSON summary;
- each `reports/v11-part*-report.md` records changes and deltas;
- `AGENTS.md`, `readme.md`, `docs/status.md`, and `thoughts/newplan.md`
  reflect the V11 status;
- skipped tests are never counted as passes.
