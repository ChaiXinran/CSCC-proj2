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
