# Native V6 Scope: Core Builtins and Coercion

Native V6 turns the existing partial standard-library code into a coherent,
testable builtin layer. V6 adds no new JavaScript syntax. Its priority is
correct coercion, primitive wrappers, core constructors, and reusable builtin
modules that increase real Test262 coverage without falling back to Boa.

Shared contracts are defined in
[Native V6 Shared Interface](native-v6-interface.md), and file ownership is
defined in [Native V6 Team Plan](native-v6-team-plan.md).

## 1. Baseline

The V1-V5 pinned Test262 gates currently pass without failures or skips:

| Gate | Result |
| --- | ---: |
| V1 | 6/6 |
| V2 | 15/15 |
| V3 | 26/26 |
| V4 | 11/11 |
| V5 | 4/4 |

The latest completed V5 diagnostic scan passed 191 of 593 tests, failed 373,
and skipped 29. V6 results must be reported separately from these language
tests.

## 2. Delivery Stages

### V6.0 — Coercion and Primitive Wrappers

- Implement shared `ToPrimitive`, `ToBoolean`, `ToNumber`, `ToString`, and
  `ToObject` behavior needed by builtins.
- Replace hidden synthetic wrapper properties with an explicit primitive
  wrapper object kind.
- Connect `String.prototype`, `Number.prototype`, and `Boolean.prototype`
  through stable intrinsics.
- Preserve JavaScript exceptions raised by `valueOf`, `toString`, or builtin
  callbacks.

### V6.1 — String

- Complete `String` call/construct behavior and indexed string access.
- Implement `charAt`, `charCodeAt`, `at`, `concat`, `includes`, `indexOf`,
  `lastIndexOf`, `slice`, `substring`, `substr`, `startsWith`, `endsWith`,
  `repeat`, `padStart`, `padEnd`, `trim`, `trimStart`, `trimEnd`,
  `toLowerCase`, and `toUpperCase`.
- Implement `String.fromCharCode` and `String.fromCodePoint`.
- Use UTF-16 code-unit indexing where ECMAScript requires it.

### V6.2 — Number, Boolean, Math, and Error

- Correct constructor/prototype relationships, `name`, `length`, and property
  descriptors.
- Complete core Number constants, predicates, parsing, `valueOf`, `toString`,
  `toFixed`, `toExponential`, and `toPrecision`.
- Verify Boolean call/construct and prototype methods.
- Correct Math constants and numeric edge cases, including NaN, infinities,
  signed zero, integer conversion, and function arity.
- Build the Error prototype hierarchy and preserve error `name` and `message`.

### V6.3 — JSON

- Implement `JSON.parse` and `JSON.stringify` for null, booleans, numbers,
  strings, arrays, and ordinary objects.
- Detect malformed JSON, unsupported cyclic structures, and correct escaping.
- Add reviver, replacer, and spacing behavior only after the core paths pass.

### V6.4 — V4 Builtin Stabilization

- Keep existing Object, Function, and Array behavior passing.
- Replace local ad-hoc conversions with the V6 coercion API.
- Fix regressions discovered by focused Test262 tests; do not rewrite the V4
  object model as part of V6.

## 3. Deferred Features

Map, Set, WeakMap, WeakSet, RegExp, Date, Promise, Symbol, TypedArray, Proxy,
Intl, Temporal, modules, generators, and async functions are not V6 targets.
New syntax such as destructuring, spread/rest, arrow functions, and classes
also remains outside this milestone.

## 4. Test262 Areas

Initial diagnostic directories in the pinned suite:

| Area | Files |
| --- | ---: |
| `built-ins/String` | 1,223 |
| `built-ins/Number` | 340 |
| `built-ins/Math` | 327 |
| `built-ins/Boolean` | 51 |
| `built-ins/Error` | 93 |
| `built-ins/JSON` | 165 |

The `--native-v6` pinned gate must contain only independently verified,
zero-failure, zero-skip cases. The broader `--native-v6-scan` remains
diagnostic and must report skipped tests separately.

## 5. Completion Criteria

V6 is complete only when:

- coercion and wrapper contracts are used by every V6 builtin module;
- String, Number, Boolean, Math, Error, and JSON execute end to end;
- callback or coercion exceptions remain catchable JavaScript throws;
- V1-V5 pinned gates have no regressions;
- a zero-failure, zero-skip V6 pinned gate exists;
- focused directory baselines and failure categories are recorded;
- format, check, tests, and Clippy pass.
