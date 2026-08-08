# AgentJS Integrated Chat Demo

## Scope and merge boundary

The local integration combines the conversational frontend and Rust Agent Host
already on `main` with the bounded DeepSeek/orchestrator continuation from
`agent`. Parser, bytecode, VM semantics, builtins and Test262 selection are not
changed by the conflict resolution.

## Integrated implementation

- Serves `frontend/agent-chat.html` and `POST /api/agent` from one origin;
  unrelated repository paths remain unavailable.
- Uses the Rust Agent Host as the authoritative implementation of
  `agent.render(tree)` and console-log collection. The Python layer consumes
  the reserved result/render markers and never executes generated JavaScript.
- Freezes the response around `sessionId`, `prompt`, `code`, `execution`,
  `render`, and a nullable structured `error`.
- Preserves legacy `task`, `scenario`, `input`, and `offline` compatibility for
  the first demo while supporting the chat frontend request.
- Provides a `CodeGenerator` boundary with deterministic fixed scripts and
  DeepSeek V4 Pro JSON Output implementations.
- Retains at most 20 messages / 10 turns per session and 128 LRU sessions.
  Every turn still executes in a fresh AgentJS process.
- Supports the six v1 RenderTree types, validates 64 KiB / eight-level limits,
  and requires generated code to call `agent.render` exactly once.
- Provides fixed chat/Test262, JSON-analysis and rule-processing paths.
- Adds `/api/health` and `/api/sessions/{sessionId}` without exposing secrets.
- Preserves the three-second subprocess timeout and rejects DOM, network,
  filesystem, Node.js and dynamic-code capabilities.

## Validation history

Before this local merge, the independent tracks reported:

- Orchestrator suite: 15/15.
- Rust Agent Host suite: 4/4.
- All-target Rust tests and Clippy: pass.
- Full native Test262: 48,563/53,379, matching the protected J/K baseline.

## Final local-merge validation

| Command / check | Result |
| --- | --- |
| `python -m py_compile demo/agent/server.py demo/agent/tests/test_server.py` | PASS |
| `python -m unittest discover -s demo/agent/tests -v` | PASS, 15/15 |
| `cargo build --release --locked` | PASS |
| Release AgentJS fixed chat execution | PASS, `92.56%`, native log, panel/metrics/statuses/table |
| HTTP chat page + `/api/agent` + session history | PASS on isolated port 8791 |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked --test agent_host` | PASS, 4/4 |
| `cargo check --locked --all-targets` | PASS |
| `cargo test --locked --all-targets` | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| Conflict marker and staged whitespace checks | PASS |

The staged merge delta contains only the five demo/orchestrator documentation
and Python files; no Rust source, Rust test or Cargo file differs from the
pre-merge `main`. Therefore the merge cannot change Test262 semantics, and the
most recent same-engine full result remains 48,563/53,379. Live DeepSeek
execution is not performed without a user-provided API key; the request,
history and JSON-output path uses mocked HTTP coverage.

## Post-merge mode routing fix

- Fixed chat requests without an explicit `mode`: they now select `deepseek`
  when `DEEPSEEK_API_KEY` is configured and `fixed` otherwise.
- Changed the default chat scenario from the fixed Test262 alias to `chat`.
- The frontend now uses its built-in Rust preset only when the orchestrator is
  unreachable. A reachable API error is displayed instead of being silently
  replaced by the compatibility dashboard.
- Added focused routing coverage for configured and unconfigured API keys.

Validation: Python compile PASS; orchestrator tests PASS, 17/17. No Rust source,
Rust test or Cargo file changed, so Test262 and runtime performance are
unaffected by this routing-only correction.

## RenderTree text interoperability fix

- Documented every supported RenderTree node shape in the DeepSeek system
  prompt, including the required `value` field for text nodes.
- Normalized model-produced text-node `content` and `text` aliases to the
  frozen `value` field at the orchestrator boundary.
- Added focused nested-tree coverage for both aliases. This changes only the
  presentation protocol adapter; AgentJS execution and Test262 semantics are
  unchanged.

## Structured input UI

- Added an optional JSON text editor to the chat composer with send-time parse
  validation and a clear action.
- Added browser-local `.json` and `.csv` upload. CSV supports quoted fields,
  escaped quotes, CRLF, primitive number/boolean conversion, unique headers,
  consistent column counts, a 10,000-row cap, and a 200 KiB request-safe cap.
- Uploaded files are converted to JSON in the browser and sent through the
  existing `input` request field. No filesystem capability was added to the
  runtime or orchestrator.
- Browser validation covered JSON text submission, a real JSON report upload,
  and a real six-row CSV upload. JavaScript syntax and the existing Python and
  Rust Agent Host suites remain part of the final gate.
