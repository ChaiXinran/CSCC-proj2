# AgentJS Small Agent Demo

This demo turns a natural-language data task into constrained JavaScript and
executes it with the native AgentJS runtime. It intentionally keeps the web UI
and model orchestration outside the JavaScript runtime.

## Run

```powershell
cargo build --release --locked
python demo/agent/server.py
```

Open `http://127.0.0.1:8787/frontend/agent-chat.html`. Offline mode is the
default and requires no API key. The same server hosts the conversational
frontend and `POST /api/agent`, so no second static-file server is required.

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

- Chat request: `{ "sessionId": "demo-001", "prompt": "..." }`.
- Legacy requests using `task`, `input`, and `scenario` remain supported.
- Request body: at most 256 KiB; prompt/task: at most 2,000 characters.
- Generated code: at most 16,000 characters and screened for host/dynamic APIs.
- Execution: a fresh AgentJS process with a three-second host timeout.
- AgentJS exposes no DOM, network, filesystem, or Node.js API to generated code.
- The online path uses DeepSeek JSON Output and non-thinking mode.

Run the dependency-free service tests with:

```powershell
python -m unittest discover -s demo/agent/tests -v
```
