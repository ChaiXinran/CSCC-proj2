#!/usr/bin/env python3
"""Small, dependency-free AgentJS demo server."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
STATIC_ROOT = Path(__file__).resolve().parent / "static"
RESULT_MARKER = "__AGENTJS_RESULT__"
MAX_REQUEST_BYTES = 256 * 1024
MAX_TASK_CHARS = 2_000
MAX_CODE_CHARS = 16_000
DEFAULT_MODEL = "deepseek-v4-pro"
DEFAULT_API_URL = "https://api.deepseek.com/chat/completions"
SCENARIOS = {"json_analysis", "rule_processing"}


SYSTEM_PROMPT = """You generate a small JavaScript function body for AgentJS.
Return one JSON object with exactly these string fields: plan and code.
The variable `input` contains JSON data. The code is inserted into a function body
and MUST end by returning a JSON-serializable value. Use conservative ES2015
JavaScript: functions, let/const/var, if, for/for-of, Array, Object, JSON, Math,
String, Number, Boolean, map/filter/reduce/sort. Do not use DOM, fetch, network,
filesystem, Node.js APIs, import/export, eval, Function, WebAssembly, Worker,
dynamic code generation, console, print, async/await, or promises. Do not wrap code
in Markdown. Keep the code deterministic and do not mutate input."""


OFFLINE_PROGRAMS = {
    "json_analysis": {
        "plan": "遍历订单，按地区聚合销售额，再按销售额降序返回。",
        "code": """const orders = Array.isArray(input.orders) ? input.orders : [];
const totals = {};
for (let i = 0; i < orders.length; i += 1) {
  const order = orders[i];
  const region = String(order.region || "未分类");
  const amount = Number(order.amount || 0);
  totals[region] = (totals[region] || 0) + amount;
}
const rows = Object.keys(totals).map(function (region) {
  return { region: region, total: totals[region] };
});
rows.sort(function (left, right) { return right.total - left.total; });
return rows.slice(0, 3);""",
    },
    "rule_processing": {
        "plan": "逐条校验订单金额，并按会员等级计算折扣后的应付金额。",
        "code": """const orders = Array.isArray(input.orders) ? input.orders : [];
return orders.map(function (order) {
  const amount = Number(order.amount || 0);
  const rate = order.member === "gold" ? 0.85 : (order.member === "silver" ? 0.92 : 1);
  return {
    id: order.id,
    valid: amount >= 0,
    payable: amount >= 0 ? Math.round(amount * rate * 100) / 100 : null
  };
});""",
    },
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


def validate_request(payload: Any) -> tuple[str, str, Any, str]:
    if not isinstance(payload, dict):
        raise AgentError("invalid_request", "请求体必须是 JSON 对象")
    task = payload.get("task")
    scenario = payload.get("scenario", "json_analysis")
    mode = payload.get("mode", "offline")
    data = payload.get("input")
    if not isinstance(task, str) or not task.strip():
        raise AgentError("invalid_task", "task 必须是非空字符串")
    if len(task) > MAX_TASK_CHARS:
        raise AgentError("task_too_large", f"task 不能超过 {MAX_TASK_CHARS} 个字符")
    if scenario not in SCENARIOS:
        raise AgentError("invalid_scenario", "不支持的场景")
    if mode not in {"offline", "deepseek"}:
        raise AgentError("invalid_mode", "mode 必须是 offline 或 deepseek")
    if data is None:
        raise AgentError("invalid_input", "input 不能为空")
    return task.strip(), scenario, data, mode


def validate_generated_program(program: Any) -> dict[str, str]:
    if not isinstance(program, dict):
        raise AgentError("model_output", "模型没有返回 JSON 对象", 502)
    plan = program.get("plan")
    code = program.get("code")
    if not isinstance(plan, str) or not isinstance(code, str):
        raise AgentError("model_output", "模型输出缺少 plan 或 code", 502)
    if not code.strip() or len(code) > MAX_CODE_CHARS:
        raise AgentError("model_output", "模型代码为空或过长", 502)
    forbidden = ("import(", "import ", "export ", "require(", "eval(", "Function(",
                 "fetch(", "XMLHttpRequest", "WebAssembly", "Worker(", "process.",
                 "Deno.", "Bun.", "print(", "console.")
    compact = code.replace("\t", " ")
    if any(token in compact for token in forbidden):
        raise AgentError("unsafe_code", "模型代码包含禁止的宿主或动态执行能力", 502)
    if "return" not in code:
        raise AgentError("model_output", "模型代码必须返回结果", 502)
    return {"plan": plan.strip(), "code": code.strip()}


def generate_offline(scenario: str) -> dict[str, str]:
    return dict(OFFLINE_PROGRAMS[scenario])


def generate_with_deepseek(task: str, scenario: str, data: Any) -> dict[str, str]:
    api_key = os.environ.get("DEEPSEEK_API_KEY")
    if not api_key:
        raise AgentError("missing_api_key", "请先设置 DEEPSEEK_API_KEY", 503)
    sample = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    user_prompt = (
        f"Scenario: {scenario}\nUser task: {task}\n"
        f"Input JSON sample: {sample[:12_000]}\n"
        "Respond with valid JSON containing plan and code."
    )
    request_body = json.dumps({
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
        data=request_body,
        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            body = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")[:500]
        raise AgentError("deepseek_http", f"DeepSeek API {error.code}: {detail}", 502) from error
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        raise AgentError("deepseek_unavailable", f"DeepSeek API 调用失败: {error}", 502) from error
    try:
        content = body["choices"][0]["message"]["content"]
        return validate_generated_program(json.loads(content))
    except (KeyError, IndexError, TypeError, json.JSONDecodeError) as error:
        raise AgentError("model_output", "无法解析 DeepSeek 返回内容", 502) from error


def find_agentjs_binary() -> Path:
    configured = os.environ.get("AGENTJS_BIN")
    candidates = [Path(configured)] if configured else []
    candidates.extend([ROOT / "target" / "release" / "agentjs.exe", ROOT / "target" / "release" / "agentjs"])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AgentError("engine_missing", "未找到 AgentJS release 可执行文件，请先运行 cargo build --release --locked", 503)


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
    source = build_wrapper(code, data)
    started = time.perf_counter()
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(source)
            temporary_path = handle.name
        completed = subprocess.run(
            [str(binary), "run", temporary_path],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=3,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise AgentError("execution_timeout", "AgentJS 执行超过 3 秒限制", 422) from error
    finally:
        if temporary_path:
            Path(temporary_path).unlink(missing_ok=True)
    elapsed_ms = (time.perf_counter() - started) * 1_000
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "未知执行错误"
        raise AgentError("execution_failed", detail[:1_000], 422)
    lines = completed.stdout.splitlines()
    marked = next((line[len(RESULT_MARKER):] for line in lines if line.startswith(RESULT_MARKER)), None)
    if marked is None:
        raise AgentError("result_missing", "AgentJS 没有返回结构化结果", 422)
    try:
        result = json.loads(marked)
    except json.JSONDecodeError as error:
        raise AgentError("result_invalid", "AgentJS 返回结果不是有效 JSON", 422) from error
    return ExecutionResult(result=result, elapsed_ms=elapsed_ms, stdout=lines)


def run_agent(payload: Any) -> dict[str, Any]:
    task, scenario, data, mode = validate_request(payload)
    model_started = time.perf_counter()
    program = generate_offline(scenario) if mode == "offline" else generate_with_deepseek(task, scenario, data)
    program = validate_generated_program(program)
    model_ms = (time.perf_counter() - model_started) * 1_000
    execution = execute_agentjs(program["code"], data)
    return {
        "ok": True,
        "scenario": scenario,
        "mode": mode,
        "model": "offline-template" if mode == "offline" else os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
        "plan": program["plan"],
        "code": program["code"],
        "result": execution.result,
        "metrics": {"modelMs": round(model_ms, 2), "agentjsMs": round(execution.elapsed_ms, 2)},
    }


class AgentHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args: Any, **kwargs: Any):
        super().__init__(*args, directory=str(STATIC_ROOT), **kwargs)

    def do_POST(self) -> None:
        if self.path != "/api/agent":
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > MAX_REQUEST_BYTES:
                raise AgentError("request_too_large", "请求体为空或超过 256 KiB", 413)
            payload = json.loads(self.rfile.read(length))
            self.send_json(HTTPStatus.OK, run_agent(payload))
        except json.JSONDecodeError:
            self.send_json(HTTPStatus.BAD_REQUEST, {"ok": False, "error": {"code": "invalid_json", "message": "请求不是有效 JSON"}})
        except AgentError as error:
            self.send_json(error.status, {"ok": False, "error": {"code": error.code, "message": str(error)}})
        except Exception as error:  # Keep one bad request from terminating the demo.
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
    parser = argparse.ArgumentParser(description="AgentJS small-agent demo")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8787)
    args = parser.parse_args()
    server = ThreadingHTTPServer((args.host, args.port), AgentHandler)
    print(f"AgentJS demo: http://{args.host}:{args.port}")
    print("Mode: DeepSeek enabled" if os.environ.get("DEEPSEEK_API_KEY") else "Mode: offline (set DEEPSEEK_API_KEY for DeepSeek V4 Pro)")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
