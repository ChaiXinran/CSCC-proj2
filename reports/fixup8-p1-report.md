# Fixup8 P1 Report

## Owner and scope

P1: Builtin Core + Temporal + Descriptor.

Files changed:

- `src/builtins/date_intl.rs`
- `src/builtins/array.rs`
- `src/builtins/object.rs`

P2 RegExp/String work is intentionally not duplicated in this report.

## Locked baseline

FixRTLE full-run baseline from `docs/fix8-teamplan.md`:

```text
total = 53379
passed = 35472
failed = 17905
skipped = 2
conformance = 66.45%
```

Focused P1 baselines captured before this round:

```text
Temporal: 828 / 4603
Array:    2359 / 3081
Object:   3101 / 3411
Function: 416 / 509
Date:     554 / 594
Set:      304 / 383
```

## Implemented

- Added `Array[Symbol.species]`.
- Added `Object.fromEntries` and `Object.prototype.toLocaleString`.
- Expanded Temporal skeleton:
  - `Duration.compare`, `abs`, `add`, `subtract`, `negated`, `round`, `total`, `with`, `sign`, `blank`.
  - BigInt-safe `Temporal.Instant` construction plus basic `add`, `subtract`, `equals`, `round`, `until`, `since`, `toZonedDateTimeISO`.
  - PlainDate/PlainDateTime/PlainTime/PlainYearMonth/PlainMonthDay/ZonedDateTime prototype shape, simple stored-value methods, ISO date derived getters, and UTC-only conversion helpers.
  - Added `calendarId`, `monthCode`, `dayOfWeek`, `dayOfYear`, `weekOfYear`, `yearOfWeek`, `daysIn*`, `monthsInYear`, `inLeapYear`, `era`, `eraYear`, and `hoursInDay` where applicable.

## Focused deltas

```text
Temporal total:                 828 / 4603 -> 1637 / 4603  (+809)
Array total:                   2359 / 3081 -> 2366 / 3081  (+7)
Object total:                  3101 / 3411 -> 3133 / 3411  (+32)
Function total:                 416 / 509  -> 416 / 509    (+0)
Date total:                     554 / 594  -> 554 / 594    (+0)
Set total:                      304 / 383  -> 304 / 383    (+0)
Reflect final:                  153 / 153
```

Temporal prototype harvest:

```text
Instant/prototype:              31 / 373  -> 115 / 373  (+84)
PlainDate/prototype:            52 / 520  -> 202 / 520  (+150)
PlainDateTime/prototype:        80 / 632  -> 228 / 632  (+148)
PlainTime/prototype:            51 / 391  -> 143 / 391  (+92)
PlainYearMonth/prototype:       88 / 384  -> 137 / 384  (+49)
PlainMonthDay/prototype:        57 / 118  -> 63 / 118   (+6)
ZonedDateTime/prototype:       149 / 740  -> 281 / 740  (+132)
Duration:                      214 / 540  -> 216 / 540  (+2)
```

## Commands run

```powershell
cargo check --no-default-features --all-targets
cargo build --release --no-default-features
rustfmt --edition 2024 --check src\builtins\date_intl.rs src\builtins\array.rs src\builtins\object.rs
cargo test --release --no-default-features --all-targets
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Temporal --jobs 4 --json reports\fix8-p1-temporal-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Array --jobs 4 --json reports\fix8-p1-array-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Object --jobs 4 --json reports\fix8-p1-object-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Function --jobs 4 --json reports\fix8-p1-function-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Date --jobs 4 --json reports\fix8-p1-date-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Set --jobs 4 --json reports\fix8-p1-set-final.json
target\release\agentjs.exe test262 --backend native --root test262 --suite test\built-ins\Reflect --jobs 4 --json reports\fix8-p1-reflect-final.json
```

## Test notes

- `cargo check --no-default-features --all-targets`: passed.
- `cargo build --release --no-default-features`: passed.
- Modified-file rustfmt check: passed.
- `cargo test --release --no-default-features --all-targets`: failed 1 existing P3/parser test, `parser::statement::tests::rejects_duplicate_lexical_declarations_in_same_scope`; 212 tests passed.
- Full `cargo fmt --all -- --check` still reports unrelated formatting diffs outside this P1 change set, so this report uses modified-file rustfmt as the P1 formatting gate.

## Residual risks

- Temporal algorithms remain intentionally partial. Calendar protocol, timezone database behavior, exact rounding semantics, and full duration balancing are not complete.
- Several new methods are stored-value or UTC-only skeletons designed to satisfy prototype shape and simple ISO behavior.
- `Object.fromEntries` uses the existing iterator path but does not yet implement every IteratorClose abrupt-completion edge case.

## Next action

P1 has delivered the intended builtins/Temporal harvest for Fixup8. Remaining large gains should come from P2/P3 integration plus targeted Temporal semantic follow-up only if the integration scan shows it is still the cheapest source of passes.
