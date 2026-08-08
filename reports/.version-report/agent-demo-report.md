# AgentJS Small Agent Demo

## Scope

This integration adds a standalone demonstration layer without changing the
parser, bytecode compiler, VM, runtime, builtins, backend contracts, or Test262
runner. It connects a browser UI to a dependency-free Python orchestrator,
DeepSeek V4 Pro, and a fresh AgentJS native process.

## Implementation

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
| `python -m unittest discover -s demo/agent/tests -v` | PASS, 5/5 |
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
