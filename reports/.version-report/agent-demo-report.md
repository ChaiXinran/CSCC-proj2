# AgentJS Small Agent Demo

## Scope

This integration adds a standalone demonstration layer without changing the
parser, bytecode compiler, VM, runtime, builtins, backend contracts, or Test262
runner. It connects a browser UI to a dependency-free Python orchestrator,
DeepSeek V4 Pro, and a fresh AgentJS native process.

## Implementation

- Frozen the orchestrator protocol around `sessionId`, `prompt`, `code`,
  `execution`, `render`, and a stable nullable `error` object.
- Added a `CodeGenerator` boundary with fixed-script and DeepSeek V4 Pro
  implementations.
- Added a bounded, thread-safe in-memory session store: 20 messages / 10 turns
  per session and 128 LRU sessions globally.
- Added a fixed Test262 dashboard chain that executes in the real AgentJS
  binary and produces a panel containing metrics and a table.
- Added a temporary orchestration-side `agent.render(tree)` adapter, isolated
  behind the AgentJS subprocess wrapper for replacement by Rust Host reports.
- Added RenderTree validation for the six frozen v1 node types, 64 KiB output,
  eight-level nesting, and exactly one render call per generated script.
- Added complete success/error response aggregation plus health and session
  inspection endpoints. Legacy first-demo request and response aliases remain
  available for the existing frontend branch.
- Two bounded scenarios: JSON analysis and rule processing.
- DeepSeek `deepseek-v4-pro` Chat Completions adapter using JSON Output.
- Deterministic offline mode for demonstrations and tests without API access.
- Structured request, model-plan/code, AgentJS result, error, and timing data.
- Fresh-process isolation, three-second host timeout, request/code size limits,
  and rejection of host/dynamic-code APIs.
- Responsive static UI showing the plan, generated JavaScript, result, and
  model/engine latency independently.

## Correctness and performance boundary

The implementation is outside Rust production code and does not change any
JavaScript-visible semantics or runtime hot path. Full project gates and the
protected Test262 baseline must still be verified before delivery.

## Validation

| Command / check | Result |
| --- | --- |
| `python -m unittest discover -s demo/agent/tests -v` | PASS, 15/15 |
| Fixed Test262 dashboard through HTTP + release AgentJS | PASS, value `90%`, panel with metrics + table |
| `/api/health` and `/api/sessions/{sessionId}` | PASS |
| Offline JSON-analysis execution through release AgentJS | PASS |
| Offline rule-processing execution through release AgentJS | PASS |
| Browser UI/API end-to-end check | PASS |
| Desktop overflow check (`scrollWidth == clientWidth`) | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --locked --all-targets` | PASS |
| `cargo test --locked --all-targets` | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| Full Test262 rerun, release/native, 4 jobs | PASS, 48,563/53,379, 4,814 failed, 2 skipped |

The first full scan with high-frequency progress output produced 48,557 passes.
An immediate same-binary rerun without progress output reproduced the protected
J/K integration baseline exactly at 48,563. No file under `src/` or `tests/`
differs as part of this demo change. Live DeepSeek execution was not performed
because no API key was provided; its HTTP adapter follows the official JSON
Output Chat Completions request shape and is covered with a mocked API-response
test.

## Student 1 orchestrator continuation

This continuation changes only `demo/agent/server.py`, its tests, README and
protocol documentation, plus this report. It does not modify the static frontend or any
Rust source/test/Cargo file owned by the Agent Host track. The Rust format,
check, all-target test and Clippy gates passed again. Full Test262 was not rerun
for this Python-only continuation; the immediately preceding same-branch full
scan remains 48,563/53,379, and `git diff -- src tests Cargo.toml Cargo.lock` is
empty.
