# V17 Part B Report

## Scope and baseline

Part B owns ECMAScript RegExp parsing and translation, scoped modifiers,
Unicode-aware word semantics, and Annex B RegExp compatibility.

Baseline commit:

```text
3e4cf09f12256800b9c19032048abf05ba8a6276
```

The working tree already contained unrelated changes to
`reports/full-test262-summary.json` and `.claude/`; they were preserved and are
not part of this track.

## Root-cause clusters

The 375 `test/built-ins/RegExp` failures were concentrated in three large
feature gaps:

- `regexp-modifiers`: the lexer deliberately rejected every legal
  `(?add-remove:...)` group, while the translation layer treated `i`, `m`, and
  `s` as global backend options that could not be removed in a nested group.
- `property-escapes`: generated Unicode membership mismatches, backend compile
  failures, and runtime-limit failures all came from delegating ECMAScript
  property names to the regex backend's different property tables.
- `unicodeSets`: the current backend has no ECMAScript `v`-mode set AST,
  intersection/difference compiler, `\q{}` strings, or properties-of-strings
  matcher. Relaxing lexer validation alone would accept invalid syntax without
  making legal expressions executable, so this was not used as a workaround.

Annex B failures were separately clustered around legacy control/identity/octal
escapes and quantifiable assertions.

## Implementation

- Implemented full lexer validation for scoped modifier groups, including
  add-only, remove-only, add/remove, nesting, duplicates, invalid flags, and
  empty modifier rejection.
- Reworked the RegExp translation layer so global `i`, `m`, and `s` flags are
  represented as an outer scoped group. Nested modifiers can now override them
  correctly.
- Made dot translation scope-aware so `s` changes only the active group while
  preserving ECMAScript line-terminator and legacy astral-character behavior.
- Added ECMAScript word-character and word-boundary translation, including the
  Unicode-ignoreCase additions U+017F and U+212A and local modifier removal.
- Added Annex B translation for control digits/underscore, identity escapes,
  incomplete hex/Unicode escapes, decimal-vs-octal disambiguation, and class
  escape range compatibility.
- Added an ICU4X-backed ECMAScript property-set compiler with one-time caching
  for binary properties, general categories, scripts, script extensions,
  `Any`, `ASCII`, and `Assigned`. Surrogate code points are excluded when
  serializing sets for the scalar-value regex backend.
- Lowered Annex B quantifiable lookaheads to equivalent executable forms
  instead of passing unsupported zero-width quantifiers to the backend.
- Added parser and RegExp helper regression tests for valid/invalid modifier
  syntax, scoped flag removal, nesting, dotAll isolation, Unicode properties,
  and legacy quantifiable assertions.
- Added the backend-neutral `unicode_set` layer. `CodePointSet` normalizes range
  sets and implements union/intersection/difference/complement;
  `UnicodeSet` combines code points with finite strings; the `v`-mode parser
  supports nested classes, `&&`, `--`, implicit union, ranges, property
  escapes, class escapes, and `\q{}`.
- Added a lossless `Utf16String` storage codec and routed core String UTF-16
  conversion helpers through it. Lone surrogate code units now survive
  construction and can participate in Unicode property matching.
- Added Unicode 17 emoji sequence data and compiled all seven ECMAScript
  properties of strings, including Basic Emoji, keycaps, flags, modifiers,
  tag sequences, ZWJ sequences, and the aggregate RGI Emoji set.

Files changed:

```text
src/lexer/mod.rs
src/builtins/regexp.rs
src/builtins/string.rs
src/unicode_set.rs
src/lib.rs
tests/parser_regexp_errors.rs
Cargo.toml
Cargo.lock
reports/v17-partB-report.md
.gitignore
```

## Focused results

| Suite | Before | After | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/RegExp` | 1504/1879 | 1757/1879 | +253 |
| `test/built-ins/RegExp/regexp-modifiers` | 6/70 | 69/70 | +63 |
| `test/built-ins/RegExp/property-escapes` | 526/613 | 611/613 | +85 |
| `test/built-ins/RegExp/property-escapes/generated/strings` | 21/28 | 28/28 | +7 |
| `test/built-ins/RegExp/unicodeSets` | 9/114 | 114/114 | +105 |
| `test/annexB/built-ins/RegExp` | 48/62 | 53/62 | +5 |
| `test/annexB/language/literals/regexp` | 2/8 | 6/8 | +4 |

The top-level RegExp suite, Annex B built-ins suite, and Annex B literal suite
are non-overlapping, for a focused net gain of 262 passing tests. No focused
suite lost passes and all had zero skipped tests. `unicodeSets` and all
properties-of-strings now pass at 100%; the property directory rose from
85.81% to 99.67%.

## Verification

Passed during development:

```text
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --no-default-features --test parser_regexp_errors
  13 passed, 0 failed
cargo test --no-default-features builtins::regexp::tests
  10 passed, 0 failed
cargo test --no-default-features --test native_test262
  15 passed, 0 failed
git diff --check
```

The focused Test262 comparisons used the native runner with `--jobs 4` for the
main RegExp suite and `--jobs 1` for the small Annex B suites. JSON summaries
were retained locally as ignored report artifacts.

## Residual work and technical boundary

- The two remaining property failures are Unicode 17 general-category additions
  U+1E6E3 and U+10ED0, newer than ICU4X's compiled general-category payload.
- The lossless UTF-16 codec is wired into core construction and indexing
  helpers. Future String batches should continue replacing direct
  `str::encode_utf16` calls with the shared abstraction so every String
  algorithm observes the same code-unit representation.
- Remaining `RegExp.prototype.compile` failures involve receiver branding,
  observable property order, immutable `lastIndex`, and cross-realm identity,
  outside this parser/translation batch.
