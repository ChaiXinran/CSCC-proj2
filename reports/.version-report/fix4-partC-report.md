# Fix4 Part C Report

Date: 2026-06-27

## Owner And Scope

Part C covers JS-visible builtin shape and focused builtin behavior:

- Descriptor / builtin shape for globals and prototype methods.
- Array / String iterator bridge to the shared runtime iterator object.
- TypedArray / DataView verification after iterator changes.
- Date / String / AnnexB small-win completion.
- RegExp builtin verification for C-owned prototype, escape, dispatch, and object-shape surfaces.

Out of scope for this pass:

- Parser / lexer RegExp syntax.
- Generator / async / Promise scheduling internals.
- Large RegExp engine feature work such as lookbehind, unicode sets, named group parser gaps, and surrogate-preserving string storage.

## Implemented Changes

Newest first:

- `String.prototype.matchAll` is now installed in the String prototype method metadata.
- `String.prototype.matchAll` now performs RegExp `flags` validation through JS-visible property access, uses GetMethod-style `@@matchAll` dispatch for object arguments, falls back through `RegExpCreate(..., "g")`, and returns a runtime array iterator for fallback matches.
- Shared String symbol dispatch now uses GetMethod-like semantics: getter errors propagate and non-callable symbol methods throw TypeError instead of being treated as absent.
- RegExp instances no longer install default own `source` / `flags` / boolean flag data properties; those now come through the existing `RegExp.prototype` accessors, leaving `lastIndex` as the own observable data property.
- `RegExp.escape` now rejects non-string inputs, emits lowercase hex escapes, handles whitespace / line terminator escaping, and covers non-ASCII whitespace through `\uXXXX`.
- `RegExp.prototype[@@match]`, `@@search`, and `@@replace` now use a generic RegExpExec path that honors custom `exec`, validates result shape, preserves/restores `lastIndex` where required, and advances empty global matches.
- `RegExp.prototype.exec` now always reads/coerces `lastIndex`; non-global/non-sticky exec reads but does not write it, and `Infinity` no longer collapses to zero in `ToLength`.
- `RegExp.prototype[@@matchAll]` now advances on empty matches to avoid infinite loops in the current eager implementation.
- `RegExp.prototype[@@replace]` supports functional replacers, common substitution tokens (`$$`, `$&`, ``$` ``, `$'`, `$n`, `$nn`, `$<name>`), and a conservative replacement output allocation guard.
- `Array.from` consumes shared native Array/String iterator objects directly, while leaving JS iterator records on the existing `.next()` path.
- `String.prototype[Symbol.iterator]` is installed and returns a shared runtime string iterator with `Iterator.prototype` in its prototype chain.
- `String` global property is redefined with standard builtin attributes: writable true, enumerable false, configurable true.
- AnnexB String HTML methods were added: `anchor`, `big`, `blink`, `bold`, `fixed`, `fontcolor`, `fontsize`, `italics`, `link`, `small`, `strike`, `sub`, `sup`.
- AnnexB `trimLeft` / `trimRight` are aliases to the existing `trimStart` / `trimEnd` function objects.
- AnnexB global `escape` / `unescape` are installed as non-enumerable globals; `unescape` now treats only lowercase `%uXXXX` as the Unicode escape form.
- Date AnnexB support added: `getYear`, `setYear`, and `toGMTString === toUTCString`.
- Date setter tail added for UTC-oriented native subset: `setMilliseconds`, `setUTCMilliseconds`, `setSeconds`, `setUTCSeconds`, `setMinutes`, `setUTCMinutes`, `setHours`, `setUTCHours`, `setDate`, `setUTCDate`, `setMonth`, `setUTCMonth`, `setFullYear`, `setUTCFullYear`.

## Verification

Rust / build:

```text
cargo fmt --all -- --check
PASS

cargo check --no-default-features --all-targets
PASS

cargo build --release --no-default-features
PASS

cargo test --no-default-features --test native_regexp
PASS

cargo test --no-default-features --test native_stdlib
PASS
```

Known full Rust test status from earlier full-suite check:

```text
cargo test --no-default-features --all-targets
FAILED: tests/parser_control_flow.rs::compiles_multiple_var_declarators_from_frontend_contract
Observed assertion: left 4, right 2.
This is parser/control-flow ownership and outside C touched files.
```

Focused Test262 final runs:

```text
test/annexB/built-ins                          188 / 241   78.01%
test/built-ins/Date                            523 / 594   88.05%
test/built-ins/String                         1112 / 1223  90.92%
test/built-ins/String/prototype/matchAll        25 / 25    100.00%
test/built-ins/Array                          2359 / 3081  76.57%
test/built-ins/Object                         3100 / 3411  90.88%
test/built-ins/Function                        414 / 509   81.34%
test/built-ins/DataView                        561 / 561   100.00%
test/built-ins/TypedArray                     1085 / 1446  75.03%
test/built-ins/TypedArrayConstructors          554 / 738   75.07%
test/built-ins/RegExp/prototype                363 / 487   74.54%
test/built-ins/RegExp/escape                    18 / 20    90.00%
RegExp.prototype.exec                           74 / 79    93.67%
RegExp.prototype[@@replace]                     51 / 70    72.86%
RegExp.prototype[@@split]                       15 / 44    34.09%
```

Saved JSON artifacts:

- `reports/fix4-c-annexb-final5.json`
- `reports/fix4-c-string-final5.json`
- `reports/fix4-c-string-matchall-final5.json`
- `reports/fix4-c-regexp-prototype-final5.json`
- `reports/fix4-c-regexp-escape-final5.json`
- `reports/fix4-c-regexp-exec-final5.json`
- `reports/fix4-c-regexp-symbol-replace-final5.json`
- `reports/fix4-c-regexp-symbol-split-final5.json`
- Earlier stable artifacts remain for Date / Array / Object / Function / DataView / TypedArray / TypedArrayConstructors.

## Delta Against Locked Baselines

Known locked baselines from the Fix4 C work log:

```text
annexB/built-ins:      76 / 241 -> 188 / 241, +112 pass
Date:                 348 / 594 -> 523 / 594, +175 pass
String:              1097 / 1223 -> 1112 / 1223, +15 pass
DataView:             561 / 561 -> 561 / 561, unchanged full pass
Function:             414 / 509 -> 414 / 509, unchanged
TypedArray:          1078 / 1446 -> 1085 / 1446, +7 pass
RegExp/prototype:     311 / 487 -> 363 / 487, +52 pass
RegExp/escape:         14 / 20  -> 18 / 20,  +4 pass
String matchAll:        2 / 25  -> 25 / 25, +23 pass
```

Minimum confirmed gain from these locked baselines: +365 pass, counting only the listed focused suites.

## Remaining Failure Classes

- `RegExp.escape` remaining 2 failures are parser/string-storage side effects: `for (const c of ...)` binding reuse and invalid/lone surrogate escape handling.
- `RegExp.prototype[@@split]` remains the largest C-visible RegExp gap. Its remaining failures need species construction, cloned splitter semantics, sticky `y` lastIndex behavior, and generic result/capture access. One failure is still parser/engine surrogate handling.
- Remaining `RegExp.prototype` failures are mostly parser/regex-engine features, exception identity/reporting gaps in the Test262 harness, species behavior, named groups, backreferences, unicode sets, and deeper sticky/global edge cases.
- AnnexB remaining failures include RegExp legacy static/accessor behavior and `IsHTMLDDA`-style emulates-undefined cases.
- Date remaining failures include deeper parse/UTC edge semantics, subclass/realm behavior, and `Symbol.toPrimitive` precision.
- String remaining failures outside `matchAll` include advanced regexp dispatch tails, coercion-order edge cases, and parser-dependent cases.
- Array / TypedArray remaining failures include species, iterator-close edge cases, BigInt typed array tail, detached/resizable buffer edge behavior, and cross-realm constructor details.

## Cross-Group Notes

- C consumed the existing shared runtime iterator objects instead of introducing a second iterator representation.
- Parser/control-flow failing Rust test should be routed to A because it is outside C files and unrelated to builtin descriptor work.
- RegExp syntax and engine feature failures should stay out of C unless A exposes parser metadata and the engine layer has a bounded C-visible hook.
