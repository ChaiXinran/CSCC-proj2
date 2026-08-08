# AgentJS Integrated Chat Demo

## Scope

This batch connects the conversational frontend, dependency-free Python
orchestrator, DeepSeek adapter, and Rust Agent Host. Parser, bytecode, VM,
builtins, backend contracts, and Test262 selection remain unchanged.

## Implementation

- Accepts the frontend `{sessionId, prompt}` request while retaining the legacy
  `{task, input, scenario, mode}` data-demo contract.
- Returns one response containing generated `code`, execution value, logs,
  elapsed time, and the validated RenderTree.
- Serves `frontend/agent-chat.html` and `POST /api/agent` from one origin.
  Other repository paths are not exposed by the static-file handler.
- Adds a reserved CLI RenderEvent marker so the orchestrator can distinguish
  RenderTrees from ordinary console logs and the JavaScript return value.
- Provides a deterministic offline compatibility dashboard and an optional
  DeepSeek JSON-output path.
- Preserves fresh-process isolation, the three-second host timeout, request/code
  limits, and rejection of unsafe host capabilities.

## Correctness boundary

The only Rust integration change is CLI serialization of RenderEvents already
collected by the Agent Host. It does not change JavaScript-visible semantics or
runtime hot paths.

## Validation

| Command / check | Result |
| --- | --- |
| `python -m py_compile demo/agent/server.py demo/agent/tests/test_server.py` | PASS |
| `python -m unittest discover -s demo/agent/tests -v` | PASS, 6/6 |
| `cargo build --release --locked` | PASS |
| Release AgentJS offline chat execution | PASS: `92.56%`, panel RenderTree, native log captured |
| Real local HTTP GET of chat page and POST to `/api/agent` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --test agent_host` | PASS, 4/4 |
| `cargo check --all-targets` | PASS |
| `cargo test --all-targets` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS, zero warnings |
| `cargo test --no-default-features --test native_test262` | PASS, 15/15 |

Live DeepSeek execution was not performed because no API key was provided. Its
request and JSON-output parsing path is covered by a mocked HTTP response test.
