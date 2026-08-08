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

## Standalone Windows package

- Added a PyInstaller specification and reproducible PowerShell build script.
- `dist/AgentJS-Demo.exe` embeds the optimized Rust runtime and chat HTML.
- Frozen-resource lookup uses PyInstaller's extraction directory; development
  execution continues to use repository paths.
- Double-click startup selects a free localhost port and opens the default
  browser. Closing the console window stops the server.
- Packaged end-to-end validation passed: page HTTP 200, health OK, embedded
  engine detected, Agent request OK, value `92.56%`, one native log, and a
  `panel` RenderTree. Final executable size: 11.83 MiB.
- The package includes the fixed viewport/message-only scrolling layout and
  omits the ENGINE and raw RESULT labels.

## Native desktop shell

- Replaced external-browser startup with a pywebview desktop window backed by
  the installed Microsoft Edge WebView2 runtime.
- The HTTP orchestrator runs on a daemon thread at a random loopback port and
  shuts down when the desktop window closes.
- The packaged executable uses the windowed PyInstaller bootloader, so no
  browser address bar or console window is shown.
- `--browser` remains available for explicit external-browser debugging and
  `--no-browser` for automated HTTP testing.
- Frozen build completed successfully with the pywebview and pythonnet hooks;
  launch verification observed the packaged process and new WebView2 child
  processes. Final desktop executable size: 16.92 MiB.

## Session-only desktop API key prompt

- Desktop launches now reuse an existing `DEEPSEEK_API_KEY` or show a masked
  prompt before the main window when no key is configured.
- The entered key is held only in the current process environment and is not
  persisted or embedded in the executable. Canceling or leaving it empty
  keeps the fixed-script offline path available.
- Browser and HTTP-only development modes remain non-interactive.
- Validation: Python compile PASS; orchestrator tests PASS, 25/25; standalone
  PyInstaller build PASS; packaged desktop process remained running at the key
  prompt until closed by the launch check. Output size: 17,742,489 bytes; SHA-256
  `4C2F4B55BBB560EB5B36EB2961DBF147B17F017EDEEBB41558BCF6292B026DF4`.

## Desktop launch responsiveness fix

- Removed synchronous WebView navigation from the exposed JavaScript API
  callback. Navigation now runs on a short-lived background thread so the JS
  bridge can return before the page is replaced, avoiding a callback/navigation
  deadlock after API-key submission.
- The launch page disables its controls and displays startup progress while
  switching pages. Duplicate clicks are ignored to prevent competing loads.
- Validation: Python compile PASS; orchestrator tests PASS, 26/26; fixed-mode
  browser request PASS with a complete `92.56%` RenderTree response in 1.08 s;
  standalone build PASS; packaged process remained responsive during the
  launch check. Output size: 17,743,679 bytes; SHA-256
  `4C24A4C15B653EF40C4C01716D0E394961B6EF43AEB30829CCC38E2E26366DAB`.

## Cross-backend WebView navigation fix

- Removed `window.load_url` from Python completely. The bridge now only stores
  the session key and returns the loopback chat URL; the launch page performs
  `window.location.replace(...)` after the bridge promise resolves.
- This keeps all navigation on the WebView page/UI side and works around both
  bridge-thread deadlocks and worker-thread navigation stalls observed with
  different WebView2/pywebview combinations.
- Frontend Agent requests now stop waiting after 35 seconds and display a
  concrete network/API-key message instead of leaving the turn in an indefinite
  `Working` state.
- Validation: Python compile PASS; orchestrator tests PASS, 26/26; clicking the
  compatibility preset and sending it through the refreshed UI returned the
  complete `92.56%` panel; standalone build PASS. Output size: 17,743,464
  bytes; SHA-256
  `0991C0724AF616F7DADD93355B046EFD1409B2570047699D74290666003A92B9`.
