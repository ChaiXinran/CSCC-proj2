# Native V17 Part A report

## Ownership and scope

This batch consolidates parser static semantics for block declarations and
object methods. It targets parse-phase Test262 failures without changing AST or
cross-team contracts.

## Architecture changes

- Added one Annex B declaration classifier that distinguishes ordinary
  FunctionDeclarations from async/generator declarations.
- Narrowed the sloppy-block duplicate-function exception to names declared
  exclusively by ordinary functions.
- Centralized ordinary, computed, async, and generator object-method
  parameter/body validation in `parse_object_method_function`.
- Enforced the no-LineTerminator rule between `yield` and the `*` token.

Files touched:

- `src/parser/statement.rs`
- `src/parser/expression.rs`

## Focused Test262 results

| Suite | Baseline failures | Final failures | Newly passing | Regressions |
|---|---:|---:|---:|---:|
| `language/block-scope/syntax` | 40 | 0 | 40 | 0 |
| `language/block-scope` | 42 | 2 | 40 | 0 |
| `language/expressions/object` | 95 | 80 | 15 | 0 |
| `language/expressions/async-arrow-function` | 18 | 9 | 9 | 0 |

The shared switch validator also improved from 40 failures in
`detail_latest.txt` to 26 current failures.

Focused cases:

- all 113 `language/block-scope/syntax` cases pass;
- `language/expressions/object/method-definition/yield-star-after-newline.js`
  passes.

## Unit coverage

- `annex_b_duplicate_function_exception_is_ordinary_function_only`
- `rejects_var_declarations_conflicting_with_lexical_names`
- `object_methods_share_static_semantics_validation`
- `async_arrows_validate_parameters_and_body_lexicals`

## Async arrow follow-up

- Validate that async-arrow formal parameter tokens do not contain `await`,
  including nested arrow parameter initializers.
- Apply UniqueFormalParameters after async-arrow cover grammar is resolved.
- Reuse function parameter-versus-body lexical declaration validation.
- Adjacent arrow, async-function, and async-generator directories show no
  regressions.

## Coordination notes

- No `contracts.rs` API changed.
- No Test262, Boa, or QuickJS submodule file changed.
- Object method validation is now a single extension point for future method
  static semantics.

## Class / super follow-up

This A-group batch restores native super-property semantics in class static
contexts without changing the frozen VM invocation or bytecode interfaces.

- Static blocks and class field initializers now parse `super.prop` while still
  retaining their separate early-error checks for `super()` and `arguments`.
- Plain `super.prop` and `super[key]` reads use the existing super lookup
  opcodes and discard the call receiver when no call follows.
- Static block helper functions receive the class constructor as their
  HomeObject, so lookup remains dynamic after prototype changes.
- The JetStream preparation script no longer rewrites
  `super.updateUIAfterRun()` into an explicit prototype call. The remaining
  extends/constructor/prototype-chain adaptations stay coupled for a later
  batch.

Focused results:

| Suite | Baseline | Final | Change |
|---|---:|---:|---:|
| `language/statements/class` | 4157/4367 | 4160/4367 | +3, 0 regressions |
| `language/expressions/class` | 3895/4059 | 3895/4059 | unchanged |
| `tests/native_classes.rs` | 13/13 | 15/15 | +2 regressions tests |

Newly passing Test262 cases include
`static-init-super-property.js`, `super/in-static-getter.js`, and
`super/in-static-setter.js`.

JetStream evidence:

- Added `docs/JetStream/jetstream2-agentjs-vs-boa-2026-07-28.md` using the
  existing 10-runner AgentJS/Boa comparison format.
- Five freshly generated full-Driver workloads pass without class/super
  errors: `ai-astar`, `richards`, `stanford-crypto-sha256`, `splay`, and
  `navier-stokes`.
- `crypto` remains blocked by call depth and `regexp` by unsupported
  look-around.
- The four checked-in legacy standard runners are invalid artifacts: their
  `DefaultBenchmark` has no `extends` but still calls `super(args)`. Both
  AgentJS and Boa reject them, so they are not counted as class regressions.

## Class ordering and early-error follow-up

Additional A-owned fixes:

- Evaluate computed static and instance field keys once in their common source
  order, then reuse the captured property keys during initialization.
- Keep abrupt completion during computed-key evaluation ahead of every later
  key and initializer.
- Apply strict binding-name validation to class expressions as well as class
  declarations, including escaped `let`, `static`, and `yield`.
- Parse class heritage under strict mode while continuing to allow `await` as
  a class-expression name in script code.
- For `class C extends null`, preserve `C.[[Prototype]]` as
  `%Function.prototype%` while keeping `C.prototype.[[Prototype]]` null.

Test262 progression:

| Suite | Before follow-up | Final | Change |
|---|---:|---:|---:|
| `language/statements/class` | 4160/4367 | 4164/4367 | +4, 0 regressions |
| `language/expressions/class` | 3895/4059 | 3901/4059 | +6, 0 regressions |

Shared-interface requirements for B:

- Super writes need a single VM operation implementing
  `base.[[Set]](key, value, receiver)` for named and computed keys. Ordinary
  `SetProperty` cannot represent a distinct super base and current-this
  receiver, so A cannot correctly lower assignment, updates, compound
  assignment, or logical assignment without this interface.
- Repeated `super()` and lexical-arrow access to an uninitialized derived
  `this` must be enforced by the B-owned frame/derived-this state.
- Private method/accessor slots need kind-aware runtime storage (field, method,
  accessor) before A can complete immutable private methods and private
  getter/setter dispatch without encoding a second runtime path.

## Optional method-call receiver follow-up

The JetStream3 runner investigation showed that object rest and
`super(args)` preserved their data. The shared engine defect was optional
method-call lowering: `obj.method?.()`, parenthesized optional member calls,
and `super.method?.()` used a plain call and lost the reference receiver.

Changes:

- Preserve `[callee, receiver]` through optional nullish branches.
- Use `CallWithThis` for optional named/computed member calls.
- Keep parenthesized member references and super home-object lookup.
- Merge each legacy JetStream3 workload's embedded resources into one
  `new Function` scope so its workload `Benchmark` class remains visible to
  the runner.

Focused results:

| Suite | Before | Final | Change |
|---|---:|---:|---:|
| `language/expressions/optional-chaining` | 28/38 | 30/38 | +2, 0 regressions |
| `language/statements/class` | 4165/4367 | 4165/4367 | unchanged |
| `language/expressions/class` | 3902/4059 | 3902/4059 | unchanged |

Newly passing Test262 cases:

- `optional-call-preserves-this.js`
- `super-property-optional-call.js`

## JetStream dynamic source and global binding follow-up

The repaired JetStream3 runners exposed three frontend/runtime boundary gaps:

- `return}` in minified dynamic `Function` source did not apply automatic
  semicolon insertion.
- Dynamic functions did not resolve data properties added to the global
  object after compilation (the UMD export pattern used by threejs).
- `LoadGlobal` only checked own global properties, so inherited
  `Object.prototype` bindings such as `toString` were invisible.

Changes:

- Treat a closing brace as an empty-return ASI boundary.
- Fall back from environment bindings to the global object record, including
  dynamic functions and inherited global properties.
- Keep global `var` bindings synchronized with successful `globalThis`
  writes.
- Give each generated workload a local `JetStream` facade while retaining a
  separate lexical Driver binding.

Focused validation:

| Suite | Result |
|---|---:|
| `language/statements/return` | 15/16, 0 skipped |
| `language/global-code` | 24/42, 0 skipped |
| `tests/jetstream_function_constructor.rs` | 16/16 |

Runner status after the changes:

- threejs advances past the missing `THREE` binding and reaches the 90-second
  diagnostic timeout.
- validatorjs advances through dynamic source compilation and global
  `toString`; its next unsupported feature is RegExp backreferences.
- jsdom-d3-startup, mobx, and web-ssr now identify missing embedded preload
  resources instead of failing with an undefined callable.

## Parallel repair A1: general spread argument lowering

Base SHA: `fc06c548339ac0d12ef7df40a4f4229e3092f811`

The compiler now builds one argument array with the existing
`ArrayCreateSparse`, `ArrayPush`, and `SpreadIntoArray` instructions whenever a
call or construct expression contains spread arguments. The completed array is
passed through the existing spread call/construct instruction with zero regular
arguments. This supports multiple and non-trailing spreads without changing the
frozen opcode, chunk, VM, runtime, or contracts interfaces.

Covered forms:

- `f(1, ...a, 2, ...b)`
- `obj.m(...a, 2, ...b)` with the original receiver
- `new C(1, ...a, ...b)`
- `super(...)`, super method calls, and optional member calls
- left-to-right argument evaluation and iterator consumption

Focused Test262 results:

| Suite | Before | After | Change |
|---|---:|---:|---:|
| `language/expressions/call` | 79/92 | 81/92 | +2 |
| `language/expressions/new` | 52/59 | 53/59 | +1 |
| `language/statements/class` | 4165/4367 | 4166/4367 | +1 |
| `language/expressions/class` | 3902/4059 | 3903/4059 | +1 |
| `language/expressions/optional-chaining` | 30/38 | 30/38 | unchanged |

No focused suite lost passing cases. Three runtime regression tests cover plain
and receiver-preserving calls, construction, and observable evaluation order.

## Parallel repair A2 interface blocker

`language/expressions/compound-assignment` remains at 365/454. A compiler-only
attempt to emit `ToPropertyKey` before duplicating a computed reference caused
11 regressions (354/454) and was fully reverted; the suite returned to 365/454.

The remaining Reference semantics require shared runtime/bytecode support:

- a property-reference representation or get/set instruction pair that reuses
  one already-coerced property key;
- environment references that preserve the initially resolved environment
  across RHS eval;
- VM-mediated accessor get/set;
- kind-aware immutable private method/accessor storage;
- super `[[Set]]` with distinct base and receiver.

No shared interface file was changed in this batch.

## Parallel repair A4: escaped contextual keywords

Two parser-only early-error gaps were closed without changing AST or bytecode:

- `target` in the `new.target` meta-property must be written literally and may
  not contain an identifier Unicode escape.
- `get` and `set` may not contain identifier Unicode escapes when used as
  object-literal accessor contextual keywords. Escaped names remain valid as
  ordinary data-property names and ordinary method names.

Focused Test262 results:

| Suite | Before | After | Change |
|---|---:|---:|---:|
| `language/expressions/new.target` | 13/14 | 14/14 | +1 |
| `language/expressions/object` | 1090/1170 | 1101/1170 | +11 |
| `language/statements/class` | 4166/4367 | 4166/4367 | unchanged |

Unit tests cover escaped `new.target`, escaped accessor keywords, and the
corresponding legal ordinary-property/method spelling. No runtime or shared
interface file changed.

## Parallel repair: template `undefined` semantics

Tagged template lowering now supplies indexed cooked string properties on the
template object instead of exposing only `.raw`. Tags therefore receive
`strings[0]`, `strings[1]`, and later segments rather than `undefined`.

The lexer also propagates legacy-escape metadata through all template token
kinds. Untagged templates reject octal and non-octal decimal escapes at parse
time, while tagged templates retain their separate invalid-escape grammar.

Focused Test262 results:

| Suite | Before | After | Change |
|---|---:|---:|---:|
| `language/expressions/template-literal` | 42/57 | 57/57 | +15 |
| `language/expressions/tagged-template` | 15/27 | 15/27 | unchanged |

Template tokens and AST nodes now carry distinct cooked and raw quasi strings.
Raw escape spelling is preserved, while physical CR and CRLF sequences are
normalized without altering the value produced by an escaped `\r`.
`language/expressions/template-literal` is consequently zero-failure and
zero-skip. Template caching, freezing, raw property descriptors, and invalid
tagged escapes remain runtime/interface work.

## Post-B-merge A repair batch: Unicode, early errors, and named evaluation

This batch stayed within the lexer/parser/compiler boundary and did not change
the shared contracts, chunk format, or opcode interface.

Implemented Unicode 5.2 through 17.0 identifier additions, U+2E2F and numeric
literal lexical restrictions, missing contextual-keyword and static-block
early errors, decorator syntax, BigInt property names, and NamedEvaluation for
object properties and logical assignments.

| Suite | Before | After | Change |
|---|---:|---:|---:|
| `language/identifiers` | 200/268 | 268/268 | +68 |
| `language/expressions/object` | 1101/1170 | 1108/1170 | +7 |
| `language/expressions/logical-assignment` | 48/78 | 57/78 | +9 |
| `language/expressions/class` | 3903/4059 | 3913/4059 | +10 |
| `language/statements/class` | 4166/4367 | 4180/4367 | +14 |
| `language/expressions/coalesce` | 18/24 | 22/24 | +4 |
| `language/statements/with` | 113/181 | 121/181 | +8 |
| `language/expressions/async-function` | 70/93 | 72/93 | +2 |
| `language/expressions/async-generator` | 576/623 | 578/623 | +2 |
| `language/statements/function` | 436/451 | 437/451 | +1 |
| **Conservative aggregate** |  |  | **+125** |

Additional final focused results were
`language/expressions/class/decorator` 8/8,
`language/statements/class/decorator` 12/12,
`language/future-reserved-words` 54/55,
`language/statements/for-of` 714/751,
`language/statements/for-await-of` 1218/1234, and
`language/literals/numeric` 156/157. Every listed Test262 run had zero skips.

Commands run included `cargo fmt --all`, `cargo check --all-targets`,
`cargo test lexer::`, `cargo test parser::`,
`cargo test bytecode::compiler::`, the release native build, and focused
release Test262 runs for every directory listed above.

Final project gates: formatting, `cargo check --all-targets`,
`cargo test --all-targets`, and
`cargo test --no-default-features --test native_test262` passed. Clippy still
reports two pre-existing post-merge Track B warnings in
`src/builtins/annex_b.rs` and `src/runtime/context.rs`; the Track A warnings
found during this batch were corrected.

## JetStream generated-runner resource and diagnostics follow-up

The pinned `benchmarks/JetStream2` submodule was restored from the stale local
checkout `60cdba1` to the revision already recorded by the parent repository,
`b7babdf`. No submodule source file or parent gitlink was changed.

Track A updated the generated-runner pipeline without changing Rust engine
semantics:

- `scripts/prepare-jetstream2.mjs` now merges entry files, preload files, and
  discovery-time `readFile()` requests, rejects root-escaping paths, and fails
  generation when any resource is missing.
- Every generated runner receives a deterministic manifest containing the
  source commit, resource classes, missing-resource list, and runner SHA-256.
- Runner and manifest writes use temporary files and rename only after the
  complete payload is available.
- `scripts/verify-generated-runner.mjs` verifies syntax, manifest coverage,
  SHA-256, runtime resource errors, timeouts, and engine failures separately.
- `scripts/run-jetstream2-diagnostics.ps1` generates 1/2/5/10-iteration
  runners with optional init/iteration/validate phase markers and records wall
  time, CPU time, peak working set, completed iterations, and last phase.

Resource result:

| Runner | Before | After | Subsequent result |
|---|---|---|---|
| `jetstream2-jsdom-d3-startup` | `RESOURCE_MISSING` | resources PASS | one iteration exceeds 60 seconds |
| `jetstream2-mobx` | `RESOURCE_MISSING` | resources PASS | PASS, about 8.3 seconds for one iteration |
| `jetstream2-web-ssr` | `RESOURCE_MISSING` | resources PASS | engine `undefined is not callable` |

The jsdom manifest includes its bundle, airports CSV, and US-data JSON. A
repeat generation of the MobX runner and manifest produced identical SHA-256
values.

The generator also supplies the current shell `read()` contract and normalizes
embedded text resources to LF. The latter is required on Windows because the
startup workloads count LF-delimited cache-busting comments. The Node 22 shell
adapter supplies `RegExp.escape` and requires an explicit
`JETSTREAM_RUN_COMPLETE` marker; all three regenerated startup workloads reach
that marker under Node, confirming that remaining AgentJS outcomes are no
longer generator/resource failures.

Diagnostic control: `navier-stokes` with one requested iteration completed one
iteration and reached `validate:end` in about 3.2 seconds. `threejs` and WSL did
not reach the first payload phase marker within the 30-second diagnostic
window. WSL additionally grew to roughly 2.3 GiB working set in the initial
attempt, placing the observed problem before workload iteration rather than at
a particular iteration count. The diagnostic launcher now forcibly terminates
timed-out processes with a bounded post-kill wait.

Verification:

- `node --check` passed for both generator/verification scripts and generated
  runners.
- `cargo fmt --all -- --check`: passed.
- `cargo check --all-targets`: passed.
- `cargo test --all-targets`: passed.
- Full Test262: 48,423/53,379 passed, 4,954 failed, 2 skipped (90.7154%).
  Against the user-provided baseline 48,419/53,379, this is +4 passes, -4
  failures, and unchanged skips; no aggregate regression.
- `cargo clippy --all-targets -- -D warnings` remains blocked by the unrelated
  pre-existing `collapsible_if` warning in `src/builtins/date_intl.rs:13729`.

### Complete current-runner matrix

Track A added `scripts/generate-jetstream2-current.ps1`, which owns the explicit
mapping between the 19 checked-in canonical runner names and their JetStream
selection names. In particular, `regexp.js` maps to `regexp-octane`; using the
broader `regexp` tag selects four benchmarks and is rejected as ambiguous.

All 19 runners now have deterministic manifests. All 19 complete under the
Node reference shell at one iteration. The AgentJS one-iteration diagnostic
matrix with a deliberately short 15-second classification limit produced:

- PASS: `crypto`, `hash-map`, `mobx`, `navier-stokes`, `richards`, `splay`,
  `stanford-crypto-sha256`.
- Explicit engine failure: `intl`, `validatorjs`, `web-ssr`, `regexp-octane`.
- Performance timeout at 15 seconds: `ai-astar`, `gaussian-blur`, `cdjs`,
  `jsdom-d3-startup`, `threejs`, WSL, `raytrace`.

This short matrix is a routing/diagnostic run, not a benchmark result. The
updated `scripts/run-jetstream2.ps1` continues after individual failures,
applies a per-runner timeout, and writes per-case plus aggregate JSON. A
phase-marked `web-ssr` run reached `init:end` and
`iteration:0:start` before `undefined is not callable`, placing that remaining
failure inside the workload iteration rather than resource loading.

## JetStream A follow-up: status comparison and repeatable sampling

Track A added two post-integration tools without changing engine semantics or
B/C-owned files:

- `scripts/compare-jetstream2-status.mjs` compares aggregate runner JSON by
  benchmark, classifies improvements/regressions/other changes, and reports
  whether runner SHA-256 stayed fixed. PASS-to-non-PASS produces a non-zero
  exit status.
- `scripts/measure-jetstream2.ps1` records independent wall-time, CPU-time,
  and peak-working-set samples. It reports median, min/max, p90, MAD, standard
  deviation, runner SHA-256, JetStream revision, AgentJS revision, dirty state,
  and timeout configuration.

Verifier JSON now includes runner SHA-256, JetStream source revision, and the
requested iteration count. A status self-comparison reported zero changes, and
a one-repeat navier-stokes smoke completed in about 3.25 seconds. That smoke
validates the sampler only; formal performance baselines require at least five
repeats. Cross-team dependencies are recorded in
`docs/JetStream/a-merge-coordination.md`.

## JetStream A4 completion: bounded threejs/WSL matrix

The timeout diagnostic runner now has a configurable working-set ceiling,
writes its summary after every case, and records explicit workload/init/
iteration/validation booleans. Diagnostic-mode `print` calls carry UTC
timestamps, allowing the report to preserve the last output time; a workload
that produces no JavaScript output records `null` rather than an inferred
timestamp.

The complete 1/2/5/10 matrix used a 15-second timeout and 1 GiB working-set
limit:

| Benchmark | Iterations | Result | Wall time | CPU time | Peak working set | First phase |
|---|---:|---|---:|---:|---:|---|
| threejs | 1 | TIMEOUT | 15.102 s | 14.844 s | 250.1 MiB | none |
| threejs | 2 | TIMEOUT | 15.117 s | 14.625 s | 253.7 MiB | none |
| threejs | 5 | TIMEOUT | 15.098 s | 14.688 s | 250.4 MiB | none |
| threejs | 10 | TIMEOUT | 15.059 s | 14.531 s | 250.6 MiB | none |
| WSL | 1 | MEMORY_LIMIT | 0.944 s | 0.875 s | 1043.3 MiB | none |
| WSL | 2 | MEMORY_LIMIT | 0.990 s | 0.953 s | 1132.4 MiB | none |
| WSL | 5 | MEMORY_LIMIT | 0.967 s | 0.922 s | 1081.4 MiB | none |
| WSL | 10 | MEMORY_LIMIT | 0.962 s | 0.922 s | 1075.0 MiB | none |

Neither workload produced JavaScript output or reached `init:start`. The
invariant behavior across iteration counts places both failures before
`runIteration`: threejs is a payload parse/compile performance timeout, while
WSL is payload parse/compile memory growth. No live AgentJS process remained
after the matrix.

`generate-jetstream2-current.ps1 -VerifyDeterminism` repeated generation for
all 19 canonical files and confirmed identical runner SHA-256 values for every
pair. The regenerated set also completed 19/19 under the Node reference shell.
Together with zero missing resources and complete manifests, this closes the
original A1-A4 runner/infrastructure acceptance scope. Remaining threejs/WSL
root-cause work requires engine-internal facilities owned outside Track A.
