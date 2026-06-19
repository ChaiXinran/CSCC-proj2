# Architecture

AgentJS targets short-lived, high-frequency JavaScript execution for AI agent
tools. The current implementation separates agent-facing runtime policy from
the ECMAScript backend so the two can evolve independently.

## Components

1. `Engine` creates a fresh isolate for each unrelated execution. This prevents
   globals and prototype mutations from leaking between agent actions.
2. `Runtime` keeps one isolate alive for related calls such as a REPL session.
3. The host surface is intentionally small: `print` and a frozen `console`
   facade. Filesystem, process, and network access are not exposed.
4. Runtime limits bound loops, recursion, VM stack growth, and backtrace size.
5. A bounded per-isolate LRU reuses parsed and compiled scripts for repeated
   agent calls without sharing mutable globals across isolates.
6. `test262` discovers tests, loads harness files, runs strict/non-strict
   variants in parallel, catches per-case engine panics, and reports
   pass/fail/skip counts.

The default `conformance` Cargo feature enables larger Intl, Temporal, and
experimental components. Disabling default features produces the smaller
agent-oriented binary while retaining the same host isolation API.

## Backend Boundary

Boa currently supplies parsing, bytecode execution, garbage collection, and
standard built-ins. QuickJS is used as a design and performance reference, not
linked into the binary. AgentJS owns isolate lifecycle, policy, CLI behavior,
output capture, conformance orchestration, and benchmark workflow.

This is an explicit bootstrap architecture. AgentJS already owns the bounded
script cache and isolation policy; planned native backend work includes:

- compact immutable bytecode shared safely across warm isolates;
- isolate pooling with deterministic reset;
- per-execution allocation accounting and hard memory budgets;
- snapshotting of initialized agent tool environments;
- tiered fast paths for JSON, property access, and host calls.

The backend should remain replaceable through the `Runtime` interface so these
features can be developed without changing the CLI or Test262 reporting layer.
