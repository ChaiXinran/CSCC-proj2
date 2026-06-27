# V12 Part C Report

Date: 2026-06-27

Owner scope: builtins, descriptor-visible shape, RegExp runtime behavior, Set builtin harvest, Temporal skeleton. Parser/lexer/class lowering, iterator VM protocol, async and Promise queue changes remain outside C.

## Baseline

Locked project baseline from `docs/native-fix5-team-plan.md`:

```text
total = 53379
passed = 32097
failed = 21280
skipped = 2
conformance = 60.13%
```

Focused C baselines observed before this pass:

| Suite | Before |
| --- | ---: |
| `test/built-ins/Set` | 216 / 383 |
| `test/built-ins/Temporal` | 416 / 4603 |
| `test/built-ins/Temporal/PlainYearMonth` | 2 / 509 |
| `test/built-ins/Temporal/PlainMonthDay` | 2 / 199 |
| `test/built-ins/Temporal/ZonedDateTime` | 5 / 901 |
| `test/built-ins/RegExp/Symbol.species` | 0 / 4 |
| `test/built-ins/RegExp/dotall` | 1 / 4 |
| `test/built-ins/RegExp/named-groups` | 7 / 36 |

## Changes

- Added Temporal skeleton coverage for `Temporal.PlainYearMonth`, `Temporal.PlainMonthDay`, and `Temporal.ZonedDateTime`, including constructors, `from`, `compare` where applicable, basic string/json/locale methods, conversion helpers, equality, and low-risk ISO/UTC getters.
- Added `Set[Symbol.species]` on the actual Set constructor and implemented Set composition methods: `union`, `intersection`, `difference`, `symmetricDifference`, `isSubsetOf`, `isSupersetOf`, `isDisjointFrom`.
- Added `RegExp[Symbol.species]`.
- Adjusted RegExp dot translation before Rust regex compilation so JS `.` observes ECMAScript line terminators and non-Unicode supplementary-plane behavior.
- Added RegExp named capture `groups` object creation for builtin `exec` results.

## Final Focused Results

| Suite | After | Delta |
| --- | ---: | ---: |
| `test/built-ins/Set` | 272 / 383 | +56 |
| `test/built-ins/Set/Symbol.species` | 4 / 4 | +4 |
| `test/built-ins/Temporal` | 784 / 4603 | +368 |
| `test/built-ins/Temporal/PlainYearMonth` | 118 / 509 | +116 |
| `test/built-ins/Temporal/PlainMonthDay` | 69 / 199 | +67 |
| `test/built-ins/Temporal/ZonedDateTime` | 188 / 901 | +183 |
| `test/built-ins/RegExp/Symbol.species` | 4 / 4 | +4 |
| `test/built-ins/RegExp/dotall` | 4 / 4 | +3 |
| `test/built-ins/RegExp/named-groups` | 15 / 36 | +8 |

Representative JSON artifacts:

```text
reports/v12-c-set-final2.json
reports/v12-c-regexp-species-final2.json
reports/v12-c-regexp-dotall-final3.json
reports/v12-c-regexp-named-groups-final2.json
reports/v12-c-temporal-final.json
reports/v12-c-temporal-plainyearmonth-final.json
reports/v12-c-temporal-plainmonthday-final.json
reports/v12-c-temporal-zoneddatetime-final.json
```

## Validation

Passed:

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo build --release --no-default-features
cargo test --no-default-features --test native_collections
cargo test --no-default-features --test native_date
cargo test --no-default-features --test native_string
```

Focused test262 passed/improved as listed above.

`cargo test --no-default-features --all-targets` was run and reached one non-C failure:

```text
tests/parser_control_flow.rs::compiles_multiple_var_declarators_from_frontend_contract
assertion failed: left 4, right 2
```

This is in parser/control-flow ownership and is not caused by the touched C files.

## Remaining Failures

- `RegExp/property-escapes` and several `CharacterClassEscapes` generated cases still hit wall-clock limits or A-side repeated lexical binding failures. The generated property-escape tests spend most time building very large strings in interpreted JS, so a C-only RegExp patch is not sufficient.
- `RegExp/named-groups` remaining failures include duplicate names, lookbehind, `\k<name>` backreferences, special Unicode group names, and RegExp subclass/class-lowering behavior.
- Temporal remaining failures are mostly deep calendar/time-zone/arithmetic semantics plus known parser/harness blockers; V12 C intentionally implements only the low-risk skeleton.
- Set remaining failures are mostly advanced set-like ordering/mutation/iterator-close edge cases and exact proposal semantics.

## Next Action

Best next C-side work is narrow: improve Set set-like ordering and iterator-close behavior, then add named backreference translation if parser acceptance is already present. For bigger RegExp property-escape gains, A/B should first address generated-test execution speed and repeated binding/parser blockers.

## Fix6 Continuation

Date: 2026-06-27

Reference docs:

```text
docs/native-fix6.-teamplan.md
docs/native-fix6-interface.md
```

### Changes

- Reimplemented `RegExp.prototype[@@split]` around JS-visible species construction, flags coercion, `lastIndex`, `RegExpExec`, captures, `limit` as `ToUint32`, and UTF-16 slicing/advance.
- Added `Map.groupBy(items, callbackfn)` on the Map constructor, including raw Map keys, `-0` normalization, grouped arrays, callback index arguments, and iterator close on callback abrupt completion.
- Tightened Date builtins:
  - ISO date-only parsing now accepts `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, and signed expanded years while rejecting `-000000`.
  - `Date.UTC` now defaults missing month to `0` and applies the 0..99 year offset using truncated integer years.
  - `Date.prototype[Symbol.toPrimitive]` now uses ordinary `toString`/`valueOf` order on ordinary objects and has the correct non-writable descriptor.
  - `Date.prototype.toJSON` now follows `ToObject`, `ToPrimitive(number)`, `Invoke(O, "toISOString")`.
  - `Date.prototype.toTemporalInstant` was added, and `Temporal.Instant.prototype.epochNanoseconds` now exposes a BigInt slot.
  - Removed the non-standard Date internal slot from `Date.prototype`.
- Removed `RegExp.prototype[Symbol.toStringTag] = "RegExp"` so `Object.prototype.toString.call(RegExp.prototype)` reports `[object Object]` while RegExp instances still report via their object kind.

### Focused Results

| Suite | Fix6 Baseline | Final | Delta |
| --- | ---: | ---: | ---: |
| `test/built-ins/RegExp/prototype/Symbol.split` | 15 / 44 | 37 / 44 | +22 |
| `test/built-ins/RegExp/prototype` | 365 / 487 | 388 / 487 | +23 |
| `test/built-ins/Map/groupBy` | missing/static failures | 14 / 14 | +14 groupBy cases |
| `test/built-ins/Map` | 151 / 204 | 163 / 204 | +12 |
| `test/built-ins/Date` | 523 / 594 | 554 / 594 | +31 |
| `test/built-ins/Date/prototype/Symbol.toPrimitive` | failing cluster | 18 / 18 | fixed |
| `test/built-ins/Date/prototype/toTemporalInstant` | missing | 8 / 8 | fixed |
| `test/built-ins/Temporal` | 819 / 4603 | 828 / 4603 | +9 |
| `test/built-ins/Set` | 304 / 383 | 304 / 383 | unchanged |

Representative artifacts:

```text
reports/fix6-c-regexp-symbol-split-final2.json
reports/fix6-c-regexp-prototype-final3.json
reports/fix6-c-map-final.json
reports/fix6-c-map-groupby-after.json
reports/fix6-c-date-final2.json
reports/fix6-c-date-toprimitive-final.json
reports/fix6-c-date-totemporalinstant-final.json
reports/fix6-c-temporal-final.json
reports/fix6-c-set-final.json
```

### Validation

Passed in this continuation:

```powershell
cargo check --no-default-features --all-targets
cargo build --release --no-default-features
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\RegExp\prototype\Symbol.split --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\RegExp\prototype --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Map --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Set --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Date --jobs 4
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Temporal --jobs 4
```

`cargo fmt --all -- --check` was not used as a final gate in this pass because the repository already reports unrelated formatting diffs in non-C-touched areas from earlier work.

### Remaining C-Relevant Limits

- RegExp remaining failures are mostly exception propagation shape, cross-realm error constructors, Unicode lone-surrogate handling in the Rust regex backend, duplicate named group parser rejection, and UnicodeSets syntax validation. These are not good C-only targets without touching parser/VM/regex-engine boundaries.
- `RegExp.prototype[@@matchAll]` is still mostly routed through the String-side approximation and species/global flag semantics remain incomplete.
- Date remaining failures are concentrated in setter coercion ordering, exact legacy string formatting for negative years/time strings, and callback exception propagation.
- Temporal remains a broad skeleton. The new Instant nanosecond slot is enough for Date bridging, not a complete nanosecond-accurate Temporal implementation.
