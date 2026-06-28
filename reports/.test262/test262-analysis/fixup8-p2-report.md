# Fixup8 P2 Report

## Owner and scope

P2 owns RegExp parsing/runtime behavior, Annex B RegExp compatibility, and
String-RegExp/custom-symbol dispatch for Fixup8.

Locked fixRTLE baseline:

```text
total = 53379
passed = 35472
failed = 17905
skipped = 2
conformance = 66.45%
```

The full suite was not rerun in this track, so this report claims focused-suite
deltas only.

## Changes

- Added exact ECMAScript validation for Unicode binary properties, General
  Category values, Script/Script_Extensions values, and properties of strings.
- Added `v`-mode class-set reserved syntax and double-punctuator early errors.
- Preserved JavaScript getter exceptions through RegExp native algorithms.
- Reworked `RegExp.prototype[@@matchAll]` to use SpeciesConstructor, a cloned
  matcher, copied `lastIndex`, flags-derived global/unicode behavior, and an
  iterator result. Its focused Test262 directory is now zero-failure.
- Added Annex B static accessor descriptors for `input`/`$_`, match/context
  aliases, and `$1` through `$9`, including strict receiver checks.
- Corrected String symbol dispatch order, object-only method lookup, raw receiver
  forwarding, observable replaceAll flags validation, RegExp fallback creation,
  and literal replacement-token expansion.

Files changed:

```text
src/lexer/mod.rs
src/builtins/annex_b.rs
src/builtins/regexp.rs
src/builtins/std_primitives.rs
tests/parser_regexp_errors.rs
tests/native_regexp.rs
docs/fix8-interface.md
reports/fixup8-p2-report.md
.gitignore
```

## Focused results

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/RegExp` | 1269/1879 | 1439/1879 | +170 |
| `test/built-ins/RegExp/property-escapes` | 400/613 | 526/613 | +126 |
| `test/built-ins/RegExp/prototype` | 388/487 | 459/487 | +71 |
| `test/built-ins/RegExp/prototype/Symbol.matchAll` | 7/26 | 26/26 | +19 |
| `test/annexB/built-ins/RegExp` | 26/62 | 44/62 | +18 |
| `String.prototype.match` | 36/51 | 47/51 | +11 |
| `String.prototype.matchAll` | 25/25 | 25/25 | 0 |
| `String.prototype.replace` | 50/55 | 54/55 | +4 |
| `String.prototype.replaceAll` | 21/45 | 39/45 | +18 |
| `String.prototype.search` | 34/43 | 42/43 | +8 |
| `String.prototype.split` | 111/120 | 116/120 | +5 |

Non-overlapping focused gain: +234 passes (`RegExp` total + Annex B + six
String directories). All listed suites had zero skipped cases.

## Commands and gates

Passed:

```text
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --test native_regexp
  15 passed, 0 failed
cargo test --release --no-default-features --test native_string
  8 passed, 0 failed
cargo test --release --no-default-features --test parser_regexp_errors
  11 passed, 0 failed
```

The focused Test262 commands used the standard native runner with `--jobs 4`
for every suite in the table.

Not fully passed:

```text
cargo test --release --no-default-features --all-targets
  lib: 212 passed, 1 failed
  failing test: parser::statement::tests::rejects_duplicate_lexical_declarations_in_same_scope

cargo clippy --release --no-default-features --all-targets -- -D warnings
  failed with 31 repository warnings across AST, builtins, compiler, parser, and VM
```

The failing unit test is in P3 parser ownership and is unrelated to the P2
files. Out-of-scope rustfmt changes were removed rather than included in this
track.

Tests not run: full Test262 and Fixup8 5,000-case scan. The scan selector and
manifest are not present yet and are integration-owned.

## Residual failures and risks

- Property-escape residuals are mostly Unicode-version mismatches, unsupported
  properties in the Rust regex backend, and deadline-heavy generated cases.
- RegExp prototype residuals include UTF-16/surrogate precision, duplicate named
  groups across alternatives, named-group runtime translation, and cross-realm
  error-constructor identity.
- Annex B legacy accessor values remain empty until per-realm match state is
  added to `NativeContext`; descriptor and direct receiver semantics are present.
- Six subclass legacy-accessor cases need P3 to preserve the original receiver
  when invoking an inherited accessor.
- The remaining String failures are concentrated in complex replacement-token
  combinations, duplicate named groups, and UTF-16 RegExp behavior.

No focused suite count regressed. The full-suite impact remains unknown until
integration runs the Fixup8 scan and full Test262 suite.

## Cross-track dependencies and next action

- P1: migrate the RegExp accessor installation to the shared builtin installer
  when that helper lands.
- P3: fix inherited accessor receiver propagation and the existing duplicate
  top-level function parser unit failure.
- Integration: add/run the Fixup8 selector, then compare the full result against
  the locked fixRTLE baseline before claiming project-wide conformance gain.
