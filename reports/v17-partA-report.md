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
