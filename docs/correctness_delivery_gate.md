# Correctness Delivery Gate

This document defines the correctness scope for the delivery build. The goal is
to protect implemented semantics, not to claim full ECMAScript coverage.

## Policy

Before delivery, treat failures in the supported gate as P0 correctness bugs.
Treat failures outside the supported gate as unsupported unless they affect a
feature we explicitly demonstrate.

Run:

```powershell
python .\benchmarks\correctness\run_supported.py --label agentjs-supported-delivery --out-json tmp\agentjs-supported-delivery.json --out-md tmp\agentjs-supported-delivery.md
```

Expected result at the time this gate was added:

```text
RESULT agentjs-supported-delivery: 24/24 passed
```

## Supported Gate Coverage

The gate covers:

- expressions: arithmetic, bitwise, exponentiation precedence
- objects: descriptors, prototype lookup, delete semantics, `Object.is`
- arrays: holes, sparse high indices
- iteration: `for-in`, including member targets
- scope: `let` closure capture, direct `eval`, `with`
- functions: lexical arrow captures, constructors, saved `Function.prototype.call`
- classes: constructors, instance/static methods, accessors, `extends`, `super`
- control flow: `try` / `catch` / `finally`
- standard library smoke: `JSON`, `Map`, `String.raw`

## Explicitly Out Of Scope For Delivery

Do not block delivery on:

- RegExp Unicode set syntax / `v` flag
- arbitrary precision BigInt beyond the current native range
- QuickJS `std`, worker, bjson, and rw-handler host tests
- browser or DOM APIs
- complete module loader behavior
- full Proxy invariants beyond currently tested paths
- exact error message compatibility unless the demo depends on it

## How To Use Larger Suites

Use QuickJS, SunSpider, JetStream, and V8-derived tests as discovery tools, not
as release gates. When a larger suite fails:

1. Check whether the failure uses an out-of-scope feature.
2. If it is in scope, reduce it to a small JavaScript snippet.
3. Add the reduced snippet to `benchmarks/correctness/run_supported.py`.
4. Fix the engine until the gate passes again.

This keeps correctness work focused and prevents unsupported features from
masking regressions in the supported subset.
