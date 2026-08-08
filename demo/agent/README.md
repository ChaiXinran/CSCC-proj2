# AgentJS Small Agent Demo

This demo turns a natural-language prompt into constrained JavaScript and
executes it with the native AgentJS runtime. The orchestrator owns DeepSeek,
bounded conversation history, AgentJS invocation, and response aggregation.
The frozen protocol is documented in [protocol.md](protocol.md).

## Run

```powershell
cargo build --release --locked
python demo/agent/server.py
```

Open `http://127.0.0.1:8787`. Fixed-script mode is the default and requires no
API key. It provides JSON analysis, rule processing, and Test262 dashboard
paths before the DeepSeek or Rust Host integration is available.

To use DeepSeek V4 Pro:

```powershell
$env:DEEPSEEK_API_KEY = "your-key"
python demo/agent/server.py
```

Optional settings are `DEEPSEEK_MODEL` (default `deepseek-v4-pro`),
`DEEPSEEK_API_URL` (default `https://api.deepseek.com/chat/completions`), and
`AGENTJS_BIN` (path to a prebuilt AgentJS executable). Do not commit `.env`
files or API keys.

## Boundaries

- Request body: at most 256 KiB; task: at most 2,000 characters.
- Generated code: at most 16,000 characters and screened for host/dynamic APIs.
- Execution: a fresh AgentJS process with a three-second host timeout.
- AgentJS exposes no DOM, network, filesystem, or Node.js API to generated code.
- The online path uses DeepSeek JSON Output and non-thinking mode.
- Successful turns keep a bounded in-memory history; every turn still executes
  in a fresh AgentJS process.
- `GET /api/health` reports API/runtime readiness without exposing the key.
- `GET /api/sessions/{sessionId}` returns the bounded integration history.
- `agent.render(tree)` is currently captured by the orchestrator adapter. The
  Rust Agent Host will become authoritative when its `render_events` report is
  integrated.

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
