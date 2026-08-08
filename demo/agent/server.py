#!/usr/bin/env python3
"""Dependency-free HTTP orchestrator for the AgentJS chat demo."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
STATIC_ROOT = ROOT / "frontend"
RESULT_MARKER = "__AGENTJS_RESULT__"
RENDER_MARKER = "__AGENTJS_RENDER__"
MAX_REQUEST_BYTES = 256 * 1024
MAX_TASK_CHARS = 2_000
MAX_CODE_CHARS = 16_000
DEFAULT_MODEL = "deepseek-v4-pro"
DEFAULT_API_URL = "https://api.deepseek.com/chat/completions"
SCENARIOS = {"chat", "json_analysis", "rule_processing"}


SYSTEM_PROMPT = """You generate a small JavaScript function body for AgentJS.
Return one JSON object with exactly these string fields: title and code.
The variable `input` contains JSON data. The code is inserted into a function
body. It MUST call agent.render(tree) and end by returning a JSON-serializable
value. The RenderTree root must have type panel. Children may use text, metrics,
statuses, table, and list. Never generate HTML. Use conservative ES2015
JavaScript. Do not use DOM, fetch, network, filesystem, Node.js APIs,
import/export, eval, Function, WebAssembly, Worker, dynamic code generation,
print, async/await, or promises. console.log is allowed. Return JSON only."""


CHAT_CODE = """const modules = [
  { name: "Lexer / Parser", cases: 23140, passed: 22106 },
  { name: "Bytecode / VM", cases: 18420, passed: 16970 },
  { name: "Built-ins", cases: 12823, passed: 11262 }
];
const total = modules.reduce(function (sum, item) { return sum + item.cases; }, 0);
const passed = modules.reduce(function (sum, item) { return sum + item.passed; }, 0);
const passRate = (passed / total * 100).toFixed(2) + "%";
console.log("Native runtime checked", modules.length, "modules");
agent.render({
  type: "panel",
  title: "AgentJS compatibility report",
  subtitle: "Computed by the Rust Native Runtime",
  children: [
    { type: "metrics", items: [
      { label: "Cases", value: total },
      { label: "Passed", value: passed },
      { label: "Pass rate", value: passRate }
    ]},
    { type: "statuses", items: [
      { label: "Generate JavaScript", status: "done" },
      { label: "Execute in AgentJS", status: "done" },
      { label: "Render structured result", status: "done" }
    ]},
    { type: "table", columns: ["Module", "Cases", "Passed"],
      rows: modules.map(function (item) { return [item.name, item.cases, item.passed]; }) }
  ]
});
return passRate;"""

JSON_ANALYSIS_CODE = """const orders = Array.isArray(input.orders) ? input.orders : [];
const totals = {};
for (let i = 0; i < orders.length; i += 1) {
  const region = String(orders[i].region || "Uncategorized");
  totals[region] = (totals[region] || 0) + Number(orders[i].amount || 0);
}
const rows = Object.keys(totals).map(function (region) {
  return { region: region, total: totals[region] };
});
rows.sort(function (left, right) { return right.total - left.total; });
const top = rows.slice(0, 3);
agent.render({ type: "panel", title: "Sales by region", children: [
  { type: "table", columns: ["Region", "Total"],
    rows: top.map(function (row) { return [row.region, row.total]; }) }
] });
return top;"""

RULE_PROCESSING_CODE = """const orders = Array.isArray(input.orders) ? input.orders : [];
const rows = orders.map(function (order) {
  const amount = Number(order.amount || 0);
  const rate = order.member === "gold" ? 0.85 : (order.member === "silver" ? 0.92 : 1);
  return { id: order.id, valid: amount >= 0,
    payable: amount >= 0 ? Math.round(amount * rate * 100) / 100 : null };
});
agent.render({ type: "panel", title: "Order validation", children: [
  { type: "table", columns: ["Order", "Valid", "Payable"],
    rows: rows.map(function (row) { return [row.id, row.valid, row.payable]; }) }
] });
return rows;"""

OFFLINE_PROGRAMS = {
    "chat": {"plan": "Build a deterministic Test262 compatibility dashboard.", "code": CHAT_CODE},
    "json_analysis": {"plan": "Aggregate sales by region and return the top three.", "code": JSON_ANALYSIS_CODE},
    "rule_processing": {"plan": "Validate orders and calculate discounted payable amounts.", "code": RULE_PROCESSING_CODE},
}


class AgentError(RuntimeError):
    def __init__(self, code: str, message: str, status: int = 400):
        super().__init__(message)
        self.code = code
        self.status = status


@dataclass(frozen=True)
class ExecutionResult:
    result: Any
    elapsed_ms: float
    stdout: list[str]
    logs: list[str] = field(default_factory=list)
    render: Any | None = None


def validate_request(payload: Any) -> tuple[str, str, Any, str]:
    if not isinstance(payload, dict):
        raise AgentError("invalid_request", "request body must be a JSON object")
    is_chat = "prompt" in payload
    task = payload.get("prompt", payload.get("task"))
    scenario = payload.get("scenario", "chat" if is_chat else "json_analysis")
    mode = payload.get("mode", "offline")
    data = payload.get("input", {} if is_chat else None)
    if not isinstance(task, str) or not task.strip():
        raise AgentError("invalid_task", "prompt/task must be a non-empty string")
    if len(task) > MAX_TASK_CHARS:
        raise AgentError("task_too_large", f"prompt/task cannot exceed {MAX_TASK_CHARS} characters")
    if scenario not in SCENARIOS:
        raise AgentError("invalid_scenario", "unsupported scenario")
    if mode not in {"offline", "deepseek"}:
        raise AgentError("invalid_mode", "mode must be offline or deepseek")
    if data is None:
        raise AgentError("invalid_input", "input cannot be null")
    return task.strip(), scenario, data, mode


def validate_generated_program(program: Any) -> dict[str, str]:
    if not isinstance(program, dict):
        raise AgentError("model_output", "model did not return a JSON object", 502)
    plan = program.get("plan", program.get("title"))
    code = program.get("code")
    if not isinstance(plan, str) or not isinstance(code, str):
        raise AgentError("model_output", "model output is missing title/plan or code", 502)
    if not code.strip() or len(code) > MAX_CODE_CHARS:
        raise AgentError("model_output", "generated code is empty or too large", 502)
    forbidden = (
        "import(", "import ", "export ", "require(", "eval(", "Function(",
        "fetch(", "XMLHttpRequest", "WebAssembly", "Worker(", "process.",
        "Deno.", "Bun.", "print(",
    )
    if any(token in code.replace("\t", " ") for token in forbidden):
        raise AgentError("unsafe_code", "generated code contains a forbidden host capability", 502)
    if "return" not in code:
        raise AgentError("model_output", "generated code must return a result", 502)
    if "agent.render(" not in code:
        raise AgentError("model_output", "generated code must call agent.render", 502)
    return {"plan": plan.strip(), "code": code.strip()}


def generate_offline(scenario: str) -> dict[str, str]:
    return dict(OFFLINE_PROGRAMS[scenario])


def generate_with_deepseek(task: str, scenario: str, data: Any) -> dict[str, str]:
    api_key = os.environ.get("DEEPSEEK_API_KEY")
    if not api_key:
        raise AgentError("missing_api_key", "set DEEPSEEK_API_KEY before using DeepSeek", 503)
    sample = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    user_prompt = (
        f"Scenario: {scenario}\nUser task: {task}\nInput JSON sample: {sample[:12_000]}\n"
        "Respond with valid JSON containing title and code."
    )
    body = json.dumps({
        "model": os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_object"},
        "thinking": {"type": "disabled"},
        "max_tokens": 2_000,
        "stream": False,
    }).encode("utf-8")
    request = urllib.request.Request(
        os.environ.get("DEEPSEEK_API_URL", DEFAULT_API_URL),
        data=body,
        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            response_body = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")[:500]
        raise AgentError("deepseek_http", f"DeepSeek API {error.code}: {detail}", 502) from error
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        raise AgentError("deepseek_unavailable", f"DeepSeek API call failed: {error}", 502) from error
    try:
        content = response_body["choices"][0]["message"]["content"]
        return validate_generated_program(json.loads(content))
    except (KeyError, IndexError, TypeError, json.JSONDecodeError) as error:
        raise AgentError("model_output", "cannot parse DeepSeek response", 502) from error


def find_agentjs_binary() -> Path:
    configured = os.environ.get("AGENTJS_BIN")
    candidates = [Path(configured)] if configured else []
    candidates.extend([ROOT / "target" / "release" / "agentjs.exe", ROOT / "target" / "release" / "agentjs"])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AgentError("engine_missing", "AgentJS release binary is missing; run cargo build --release --locked", 503)


def build_wrapper(code: str, data: Any) -> str:
    input_json = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    encoded_input = json.dumps(input_json, ensure_ascii=False)
    return f'''"use strict";
const input = JSON.parse({encoded_input});
const __agentResult = (function (input) {{
{code}
}})(input);
"{RESULT_MARKER}" + JSON.stringify(__agentResult);
'''


def execute_agentjs(code: str, data: Any) -> ExecutionResult:
    binary = find_agentjs_binary()
    started = time.perf_counter()
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(build_wrapper(code, data))
            temporary_path = handle.name
        completed = subprocess.run(
            [str(binary), "run", temporary_path], cwd=ROOT, capture_output=True,
            text=True, encoding="utf-8", timeout=3, check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise AgentError("execution_timeout", "AgentJS execution exceeded 3 seconds", 422) from error
    finally:
        if temporary_path:
            Path(temporary_path).unlink(missing_ok=True)
    elapsed_ms = (time.perf_counter() - started) * 1_000
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "unknown execution error"
        raise AgentError("execution_failed", detail[:1_000], 422)
    lines = completed.stdout.splitlines()
    result_lines = [line[len(RESULT_MARKER):] for line in lines if line.startswith(RESULT_MARKER)]
    render_lines = [line[len(RENDER_MARKER):] for line in lines if line.startswith(RENDER_MARKER)]
    if not result_lines:
        raise AgentError("result_missing", "AgentJS did not return a structured result", 422)
    try:
        result = json.loads(result_lines[-1])
        render = json.loads(render_lines[-1]) if render_lines else None
    except json.JSONDecodeError as error:
        raise AgentError("result_invalid", "AgentJS returned invalid JSON", 422) from error
    logs = [line for line in lines if not line.startswith((RESULT_MARKER, RENDER_MARKER))]
    return ExecutionResult(result, elapsed_ms, lines, logs, render)


def run_agent(payload: Any) -> dict[str, Any]:
    task, scenario, data, mode = validate_request(payload)
    model_started = time.perf_counter()
    program = generate_offline(scenario) if mode == "offline" else generate_with_deepseek(task, scenario, data)
    program = validate_generated_program(program)
    model_ms = (time.perf_counter() - model_started) * 1_000
    execution = execute_agentjs(program["code"], data)
    elapsed_ms = round(execution.elapsed_ms, 2)
    return {
        "ok": True,
        "sessionId": payload.get("sessionId"),
        "prompt": task,
        "scenario": scenario,
        "mode": mode,
        "model": "offline-template" if mode == "offline" else os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
        "plan": program["plan"],
        "code": program["code"],
        "result": execution.result,
        "render": execution.render,
        "execution": {"value": execution.result, "logs": execution.logs, "elapsedMs": elapsed_ms},
        "metrics": {"modelMs": round(model_ms, 2), "agentjsMs": elapsed_ms},
        "error": None,
    }


class AgentHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args: Any, **kwargs: Any):
        super().__init__(*args, directory=str(STATIC_ROOT), **kwargs)

    def do_GET(self) -> None:
        request_path = self.path.split("?", 1)[0]
        if request_path in {"", "/"}:
            self.send_response(HTTPStatus.FOUND)
            self.send_header("Location", "/frontend/agent-chat.html")
            self.end_headers()
            return
        if request_path in {"/agent-chat.html", "/frontend/agent-chat.html"}:
            self.path = "/agent-chat.html"
            super().do_GET()
            return
        self.send_error(HTTPStatus.NOT_FOUND)

    def do_POST(self) -> None:
        if self.path != "/api/agent":
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > MAX_REQUEST_BYTES:
                raise AgentError("request_too_large", "request is empty or exceeds 256 KiB", 413)
            payload = json.loads(self.rfile.read(length))
            self.send_json(HTTPStatus.OK, run_agent(payload))
        except json.JSONDecodeError:
            self.send_json(HTTPStatus.BAD_REQUEST, {"ok": False, "error": {"code": "invalid_json", "message": "invalid JSON request"}})
        except AgentError as error:
            self.send_json(error.status, {"ok": False, "error": {"code": error.code, "message": str(error)}})
        except Exception as error:
            self.send_json(HTTPStatus.INTERNAL_SERVER_ERROR, {"ok": False, "error": {"code": "internal", "message": str(error)}})

    def send_json(self, status: int, payload: Any) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def main() -> None:
    parser = argparse.ArgumentParser(description="AgentJS chat demo")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8787)
    args = parser.parse_args()
    server = ThreadingHTTPServer((args.host, args.port), AgentHandler)
    print(f"AgentJS chat demo: http://{args.host}:{args.port}/frontend/agent-chat.html")
    print("Mode: DeepSeek enabled" if os.environ.get("DEEPSEEK_API_KEY") else "Mode: offline")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
