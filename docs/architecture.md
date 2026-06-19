# Architecture

AgentJS targets short-lived, high-frequency JavaScript execution for AI agent
tools. The current implementation separates agent-facing runtime policy from
the ECMAScript backend so the two can evolve independently.

## Components

1. `engine.rs` owns backend-neutral configuration, errors, reports, `Engine`,
   and `Runtime`. It contains no Boa imports.
2. `backend/mod.rs` defines `BackendKind` and the internal `RuntimeBackend`
   contract used by the CLI and Test262 runner.
3. `backend/boa.rs` contains the complete compatibility implementation:
   context creation, host functions, limits, script caching, jobs, and error
   conversion.
4. `backend/native.rs` is the compiling entry point for the self-developed
   engine. It currently returns an explicit `Unsupported` error.
5. `Engine` creates a fresh isolate for each unrelated execution. This prevents
   globals and prototype mutations from leaking between agent actions.
6. `Runtime` keeps one isolate alive for related calls such as a REPL session.
7. The host surface is intentionally small: `print` and a frozen `console`
   facade. Filesystem, process, and network access are not exposed.
8. Runtime limits bound loops, recursion, VM stack growth, and backtrace size.
9. A bounded per-isolate LRU reuses parsed and compiled scripts for repeated
   agent calls without sharing mutable globals across isolates.
10. `test262` discovers tests, loads harness files, runs strict/non-strict
   variants in parallel, catches per-case engine panics, and reports
   pass/fail/skip counts.

The default `conformance` Cargo feature enables larger Intl, Temporal, and
experimental components. Disabling default features produces the smaller
agent-oriented binary while retaining the same host isolation API.

## Backend Boundary

`Runtime::new` and `Engine::new` currently select `BackendKind::Boa` to preserve
compatibility. `Runtime::with_backend` and `Engine::with_backend` make backend
selection explicit. Boa supplies parsing, bytecode execution, garbage
collection, and standard built-ins only inside `backend/boa.rs`. QuickJS is
used as a design and performance reference, not linked into the binary.

The replacement path is incremental:

1. Add native lexer and parser modules.
2. Compile the native AST into AgentJS bytecode.
3. Execute bytecode in `NativeRuntime`.
4. Add native values, environments, objects, GC, and built-ins.
5. Change the default backend only after targeted Test262 suites pass.

This is an explicit bootstrap architecture. AgentJS already owns the bounded
script cache and isolation policy; planned native backend work includes:

- compact immutable bytecode shared safely across warm isolates;
- isolate pooling with deterministic reset;
- per-execution allocation accounting and hard memory budgets;
- snapshotting of initialized agent tool environments;
- tiered fast paths for JSON, property access, and host calls.

The backend should remain replaceable through the `Runtime` interface so these
features can be developed without changing the CLI or Test262 reporting layer.
