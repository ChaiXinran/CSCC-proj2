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
  "engine": "both",
  "input": {
    "modules": [{"module": "Array", "passed": 920, "total": 1000}]
  }
}
```

`mode` is `fixed` or `deepseek`. The legacy value `offline` maps to `fixed`.
`engine` is `agentjs`, `boa`, or `both` and defaults to `agentjs`. In
`both` mode the script is generated once, then executed independently by both engines.
For compatibility with the first demo, `task` is accepted as an alias for
`prompt`. A missing `sessionId` receives a generated demo ID.

Success response:

```json
{
  "ok": true,
  "sessionId": "demo-001",
  "prompt": "生成一个 Test262 通过率面板",
  "code": "agent.render(...); return '92%';",
  "engine": "both",
  "executions": {
    "agentjs": {"ok": true, "value": "92%", "logs": [], "elapsedMs": 18, "render": {"type": "panel", "title": "Test262 Result", "children": []}},
    "boa": {"ok": true, "value": "92%", "logs": [], "elapsedMs": 24, "render": {"type": "panel", "title": "Test262 Result", "children": []}}
  },
  "execution": {"value": "92%", "logs": [], "elapsedMs": 18},
  "render": {"type": "panel", "title": "Test262 Result", "children": []},
  "error": null
}
```

Error response preserves the same top-level shape. `code`, `execution`, and
`render` are `null`, while `error` contains stable `code` and `message` fields.

## RenderTree boundary

The accepted v1 node types are `panel`, `text`, `metrics`, `statuses`, `table`,
and `list`. The Rust Agent Host is authoritative for collecting render events;
the orchestrator validates the serialized events again at 64 KiB and eight
levels before returning them to the frontend.

Text nodes use `{ "type": "text", "value": "..." }`. For resilience at the
model boundary, the orchestrator normalizes the common `content` and `text`
aliases to `value` before returning the tree.

## Conversation history

Successful turns append one user message and one structured assistant message.
The service retains at most 20 messages and 10 response turns per session, with
an LRU cap of 128 sessions. Every execution still uses a fresh process. In comparison mode, `execution` and
`render` remain compatibility aliases for the AgentJS result, while `executions`
contains the per-engine reports.

`GET /api/sessions/{sessionId}` exposes the bounded history for integration
testing. `GET /api/health` reports model configuration and runtime availability
without returning secrets.

## POST `/api/benchmark`

The benchmark endpoint accepts `warmup` in the range 3-5 and `iterations` in
the range 30-100. It uses a deterministic 200,000-iteration arithmetic
workload, discards all warm-up runs, and rotates AgentJS, Boa, and OxideJS so
each engine occupies every order position.

Each engine returns two distributions with median, P95, minimum, maximum, and
the raw measured samples:

- `internal`: parsing/compilation/evaluation time reported inside the engine
  CLI. This is available for AgentJS and Boa. OxideJS has no equivalent timer
  on its generic `run` command, so its value is `null` rather than a
  non-comparable measurement from its cached benchmark mode.
- `endToEnd`: Python-observed time including process startup, temporary-file IO,
  output collection, and internal execution.

AgentJS additionally returns `cached`: the same measured iteration count in one
persistent isolate. Its parsed/compiled-script LRU remains live, while the
benchmark's mutable values are function-local. `cacheHits`, `cacheMisses`, and
the cached internal distribution are reported separately from fresh-process
results.

All three engines must produce the same checksum or the benchmark fails with
`benchmark_mismatch`.
