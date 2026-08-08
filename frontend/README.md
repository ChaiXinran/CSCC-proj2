# AgentJS Conversation Frontend

`agent-chat.html` is the conversational frontend for the AgentJS demo. It
keeps prompt history, displays the generated JavaScript, renders the returned
UI tree, and exposes execution logs and timing per turn.

## Runtime Contract

The page calls a same-origin `POST /api/agent` endpoint by default:

```json
{
  "sessionId": "demo-001",
  "prompt": "Create a Test262 compatibility dashboard"
}
```

The successful response is:

```json
{
  "ok": true,
  "code": "...generated JavaScript...",
  "render": {
    "type": "panel",
    "title": "Result",
    "children": []
  },
  "execution": {
    "value": "success",
    "logs": [],
    "elapsedMs": 18
  }
}
```

For the standalone local adapter, pass its origin in the query string:

```text
http://127.0.0.1:8765/frontend/agent-chat.html?apiBase=http://127.0.0.1:8766
```

If `/api/agent` is unavailable, the page falls back to `/api/eval` with a
small deterministic Rust AgentJS preset. The fallback is for frontend and
engine integration testing only; it does not call a model provider.

## Render Tree

The first frontend implementation accepts these child types:

```text
panel, text, metrics, statuses, table, list
```

Unknown nodes are ignored and the response is displayed as a non-fatal empty
render result. The backend should validate tree depth, node count, and string
size before returning it to the browser.
