# Native V5 Scope: Exceptions and Lexical Semantics

Native V5 builds structured abrupt completion and lexical scope semantics on
the V4 object model. Development may begin while V4 fixes continue, but V5
branches must not modify V4 runtime hot spots until the V4 repair branch is
merged.

Shared contracts are defined in
[Native V5 Shared Interface](native-v5-interface.md), and ownership is defined
in [Native V5 Team Plan](native-v5-team-plan.md).

## 1. Delivery Stages

### V5.0 — Completion Contract

- Represent `normal`, `return`, `throw`, `break`, and `continue` consistently.
- Preserve thrown JavaScript values instead of immediately converting them to
  host `VmError`.
- Define exception-handler metadata and stack/environment restoration rules.

### V5.1 — `try` / `catch` / `finally`

- Parse and execute `try-catch`, `try-finally`, and
  `try-catch-finally`.
- Support optional catch binding.
- Ensure abrupt completion from `finally` overrides an earlier completion.
- Restore operand stack, lexical environment, and call-frame state.

### V5.2 — `switch`

- Support `case`, one optional `default`, fall-through, and `break`.
- Evaluate the discriminant once and case expressions in source order.
- Use strict equality for case matching.

### V5.3 — Lexical Bindings

- Implement block-scoped `let` and `const`.
- Implement temporal dead zone checks and immutable assignment errors.
- Detect duplicate lexical declarations and missing `const` initializers.
- Keep `var` function/global scoped.

### Deferred V5 Extensions

Arrow functions, rest/spread, destructuring, classes, inheritance, and `super`
remain planned V5 extensions. They begin only after V5.0–V5.3 pass end-to-end
tests and the V4 repair branch is integrated. Modules, generators, async
functions, and private class fields are not V5 targets.

## 2. Acceptance Examples

```javascript
var result = 0;
try { throw 3; } catch (error) { result = error; }
result; // 3
```

```javascript
function f() {
  try { return 1; } finally { return 2; }
}
f(); // 2
```

```javascript
var result = "";
switch (2) {
  case 1: result = "a"; break;
  case 2: result = "b";
  default: result = result + "c";
}
result; // "bc"
```

```javascript
let outer = 1;
{ let outer = 2; const fixed = 3; }
outer; // 1
```

## 3. Test262 Areas

Initial directory baselines:

| Area | Current files |
| --- | ---: |
| `language/statements/try` | 201 |
| `language/statements/switch` | 111 |
| `language/statements/let` | 145 |
| `language/statements/const` | 136 |

The first pinned gate should select simple tests without destructuring,
generators, async syntax, modules, or unsupported harness includes. Directory
scans remain diagnostic: skipped tests are not passes.

## 4. Completion Criteria

V5.0–V5.3 are complete only when:

- parser, compiler, VM, runtime, and Native end-to-end paths are connected;
- `finally` behavior is tested for normal, return, throw, break, and continue;
- lexical binding and TDZ behavior is tested directly and through source code;
- a zero-failure, zero-skip `--native-v5` pinned gate exists;
- V1–V4 fixed gates have no regressions;
- format, check, tests, and Clippy all pass.

