# AgentJS Small Agent Demo

This demo turns a natural-language prompt into constrained JavaScript and
executes it with the native AgentJS runtime. The orchestrator owns DeepSeek,
bounded conversation history, AgentJS invocation, and response aggregation.
The frozen protocol is documented in [protocol.md](protocol.md).

## Run

```powershell
cargo build --release --locked
cargo build --release --locked --manifest-path boa/Cargo.toml -p boa_cli
python demo/agent/server.py
```

Open `http://127.0.0.1:8787/frontend/agent-chat.html`. Fixed-script mode is the
default and requires no API key. The same server hosts the chat frontend and
`POST /api/agent`. It provides chat, JSON analysis, rule processing, and
Test262 dashboard paths.

To use DeepSeek V4 Pro:

```powershell
$env:DEEPSEEK_API_KEY = "your-key"
python demo/agent/server.py
```

Optional settings are `BOA_BIN` (path to a prebuilt Boa CLI), `DEEPSEEK_MODEL` (default `deepseek-v4-pro`),
`DEEPSEEK_API_URL` (default `https://api.deepseek.com/chat/completions`), and
`AGENTJS_BIN` (path to a prebuilt AgentJS executable). Do not commit `.env`
files or API keys.

Chat requests that omit `mode` automatically use DeepSeek when
`DEEPSEEK_API_KEY` is configured; otherwise they use the fixed-script demo.
Explicit `fixed`, `offline`, and `deepseek` requests keep their selected mode.
Use the Engine selector in the title bar to run AgentJS, Boa, or both. The
comparison option sends one generated JavaScript program to both fresh processes and
shows their execution times and RenderTrees side by side. Model generation time is
reported separately and is not included in either engine time. Boa uses a minimal
JavaScript `agent.render` compatibility shim; AgentJS uses the native Rust Host API.

For repeatable measurements, click **3-engine benchmark 50×**. It discards five
warm-up runs, executes 50 measured samples in rotating AgentJS/Boa/OxideJS
order, and reports median/P95/min/max for internal engine time and end-to-end
process time. OxideJS exposes no comparable generic-run internal timer, so its
internal row is explicitly `N/A` rather than mixing in its cached bench mode.
The workload is intentionally heavier than the display presets. A single
workload is not a general JavaScript-engine ranking; add representative
workloads before drawing broad performance conclusions.

The AgentJS card also includes **Cached eval**. This keeps one Runtime alive and
reuses AgentJS's isolate-local 32-entry parsed/compiled-script LRU. It does not
silently enable shared state for normal chat requests: those continue to use a
fresh process. The benchmark source uses function-local `var` bindings so one
iteration cannot mutate the next iteration's application state.


## Input data

Expand **Optional input data** above the prompt box to attach structured data.
You can either paste JSON directly or choose a `.json`/`.csv` file. CSV files
must have a non-empty, unique header row and are converted in the browser to:

```json
{"rows": [{"name": "Product A", "sales": 120}]}
```

The current limits are 200 KiB for parsed input and 10,000 CSV data rows. The parsed
value is sent as the `/api/agent` request's `input` field; AgentJS does not
receive filesystem access or a local file path.

## Boundaries

- Request body: at most 256 KiB; task: at most 2,000 characters.
- Generated code: at most 16,000 characters and screened for host/dynamic APIs.
- Execution: a fresh selected-engine process with a three-second host timeout.
- Compare mode generates code once and times AgentJS and Boa independently.
- AgentJS exposes no DOM, network, filesystem, or Node.js API to generated code.
- The online path uses DeepSeek JSON Output and non-thinking mode.
- Successful turns keep a bounded in-memory history; every turn still executes
  in a fresh AgentJS process.
- `GET /api/health` reports API/runtime readiness without exposing the key.
- `GET /api/sessions/{sessionId}` returns the bounded integration history.
- `agent.render(tree)` and `console.log` are collected by the Rust Agent Host;
  the orchestrator only validates and aggregates their CLI protocol markers.

## Frozen API example

```powershell
$body = @{
  sessionId = "demo-001"
  prompt = "生成一个 Test262 通过率面板"
  scenario = "test262_dashboard"
  mode = "fixed"
  input = @{ modules = @(
    @{ module = "Array"; passed = 920; total = 1000 }
    @{ module = "RegExp"; passed = 880; total = 1000 }
  ) }
} | ConvertTo-Json -Depth 10

Invoke-RestMethod -Uri http://127.0.0.1:8787/api/agent `
  -Method Post -ContentType application/json -Body $body
```

Run the dependency-free service tests with:

```powershell
python -m unittest discover -s demo/agent/tests -v
```

## Build a standalone Windows EXE

Install PyInstaller once in the Python environment used for packaging, then run
the checked-in build script:

```powershell
python -m pip install pyinstaller pywebview
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-agentjs-demo.ps1
```

The three-engine build expects OxideJS at the sibling path
`D:\00_OS\project3136859-381686` and Rust 1.94. Override the source location
with `OXIDE_PROJECT_ROOT` or the script's `-OxideProjectRoot` parameter. The
external source tree is read-only; its output is redirected to
`target/oxide-compare` in this project.

The single-file result is `dist/AgentJS-Demo.exe`. It embeds the release
AgentJS, Boa, and OxideJS runtimes, chat frontend, and a native WebView desktop shell.
Double-clicking it opens a standalone AgentJS window without a browser address
bar; closing the window stops the local service. The packaged application does
not require Python, Cargo, or the source repository. It uses the Microsoft
Edge WebView2 runtime included with current Windows 10/11 installations.

On a desktop launch, an existing `DEEPSEEK_API_KEY` environment variable is
used directly. If it is absent, the application displays a masked API-key
prompt before opening the chat window. The entered key exists only in the
current process and is discarded when the application closes; canceling or
submitting an empty value starts the fixed-script offline demo. API keys are
never embedded in the packaged executable.
The prompt accepts either a raw `sk-...` value or a pasted PowerShell
`DEEPSEEK_API_KEY` assignment and extracts the key automatically. Non-ASCII
values are rejected before constructing the HTTP Authorization header.

Unexpected packaged-server failures are appended to
`%LOCALAPPDATA%\AgentJS-Demo\agentjs-demo.log`. Request bodies and API keys are
not written to this log. DeepSeek transport failures are returned as structured
`deepseek_unavailable` errors instead of generic HTTP 500 responses.
