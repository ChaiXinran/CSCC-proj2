# Agent Orchestrator Protocol v1

The orchestrator owns DeepSeek prompting, bounded in-memory conversation
history, AgentJS invocation, and response aggregation. It does not own the web
layout or Rust VM/Agent Host implementation.

## POST `/api/agent`

Request:

```json
{
  "sessionId": "demo-001",
  "prompt": "生成一个 Test262 通过率面板",
  "scenario": "test262_dashboard",
  "mode": "fixed",
  "input": {
    "modules": [{"module": "Array", "passed": 920, "total": 1000}]
  }
}
```

`mode` is `fixed` or `deepseek`. The legacy value `offline` maps to `fixed`.
For compatibility with the first demo, `task` is accepted as an alias for
`prompt`. A missing `sessionId` receives a generated demo ID.

Success response:

```json
{
  "ok": true,
  "sessionId": "demo-001",
  "prompt": "生成一个 Test262 通过率面板",
  "code": "agent.render(...); return '92%';",
  "execution": {"value": "92%", "logs": [], "elapsedMs": 18},
  "render": {"type": "panel", "title": "Test262 Result", "children": []},
  "error": null
}
```

Error response preserves the same top-level shape. `code`, `execution`, and
`render` are `null`, while `error` contains stable `code` and `message` fields.

## RenderTree boundary

The accepted v1 node types are `panel`, `text`, `metrics`, `statuses`, `table`,
and `list`. The temporary Python execution adapter enforces 64 KiB and eight
levels. These checks are defense in depth; the Rust Agent Host remains the
authoritative owner after its integration.

## Conversation history

Successful turns append one user message and one structured assistant message.
The service retains at most 20 messages and 10 response turns per session, with
an LRU cap of 128 sessions. Every execution still uses a fresh AgentJS process.

`GET /api/sessions/{sessionId}` exposes the bounded history for integration
testing. `GET /api/health` reports model configuration and runtime availability
without returning secrets.
