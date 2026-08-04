# JetStream 2 Part B report

## Scope

This branch implements only Part B from the 2026-08-04 JetStream repair plan:
RegExp translation, RegExp continuation semantics, and focused regression
tests. Runner/resource, Intl, diagnostics, object-model, and performance work
are intentionally excluded.

Base SHA: `7bcd72a`

Branch: `fix/jetstream-regexp`

## Fixes

- Preserve `\-` as a literal hyphen while translating legacy character
  classes, including the validatorjs `/[@_\- ]/g` pattern.
- Continue global and sticky matching against the complete input with an
  explicit start offset instead of slicing at `lastIndex`. This preserves the
  correct context for anchors, word boundaries, and lookbehind assertions.
- Keep sticky checks, UTF-16 match indices, and `d`-flag indices consistent
  with the full-input capture offsets.
- Add focused parser, constructor, validatorjs, word-boundary, and deterministic
  Octane/JetStream checksum regressions.

No VM exception-propagation change was required: after the character-class
translation fix, validatorjs no longer reports the RegExp compilation error or
the cascading `undefined is not callable` error.

## Validation

- Supported correctness gate: `24 / 24` passed.
- `test/built-ins/RegExp`: `1,759 / 1,879` passed, two above the reported
  `1,757 / 1,879` baseline.
- `test/built-ins/RegExp/property-escapes`: `611 / 613`, unchanged.
- `cargo test --locked --all-targets`: passed.
- `cargo check --locked --all-targets`: passed.
- `cargo fmt --all -- --check`: passed after formatting.
- `cargo clippy --locked --all-targets -- -D warnings`: blocked only by the
  pre-existing `collapsible_if` diagnostic in `src/builtins/date_intl.rs:13729`,
  outside Part B ownership.

The checked-in generated `regexp.js` runner now validates its checksum. The
validatorjs runner proceeds beyond RegExp construction and currently stops on
an unrelated Date assertion (`2010-07-02,[object Object]`), outside Part B.
