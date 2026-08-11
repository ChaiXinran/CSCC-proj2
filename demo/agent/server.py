#!/usr/bin/env python3
"""Dependency-free AgentJS orchestrator and HTTP adapter.

This module owns only the model/orchestration layer. The JavaScript `agent`
object below is a temporary compatibility adapter until the Rust Agent Host
publishes render events in ExecutionReport.
"""

from __future__ import annotations

import argparse
import http.client
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import urllib.error
import urllib.request
import uuid
import webbrowser
from collections import OrderedDict
from dataclasses import dataclass, field
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Protocol

try:
    import webview
except ImportError:  # Development mode may intentionally omit the desktop shell.
    webview = None


SOURCE_ROOT = Path(__file__).resolve().parents[2]
BUNDLE_ROOT = Path(getattr(sys, "_MEIPASS", SOURCE_ROOT))
ROOT = SOURCE_ROOT
STATIC_ROOT = BUNDLE_ROOT / "frontend"
RESULT_MARKER = "__AGENTJS_RESULT__"
RENDER_MARKER = "__AGENTJS_RENDER__"
AGENTJS_TIME_MARKER = "__AGENTJS_INTERNAL_MS__"
MAX_REQUEST_BYTES = 256 * 1024
MAX_PROMPT_CHARS = 2_000
MAX_CODE_CHARS = 16_000
MAX_HISTORY_MESSAGES = 20
MAX_SESSIONS = 128
MAX_RENDER_BYTES = 64 * 1024
MAX_RENDER_DEPTH = 8
DEFAULT_MODEL = "deepseek-v4-pro"
DEFAULT_API_URL = "https://api.deepseek.com/chat/completions"
ALLOWED_RENDER_TYPES = {"panel", "text", "metrics", "statuses", "table", "list"}
SCENARIOS = {"chat", "json_analysis", "rule_processing", "test262_dashboard"}
SESSION_ID_PATTERN = re.compile(r"^[A-Za-z0-9._-]{1,64}$")
LOG_ROOT = Path(os.environ.get("LOCALAPPDATA", tempfile.gettempdir())) / "AgentJS-Demo"
LOG_PATH = LOG_ROOT / "agentjs-demo.log"


def log_internal_error(error: BaseException) -> None:
    """Persist unexpected desktop errors without recording requests or keys."""
    try:
        LOG_ROOT.mkdir(parents=True, exist_ok=True)
        with LOG_PATH.open("a", encoding="utf-8") as handle:
            handle.write(f"\n[{time.strftime('%Y-%m-%d %H:%M:%S')}]\n")
            handle.writelines(traceback.format_exception(type(error), error, error.__traceback__))
    except OSError:
        pass


def configure_desktop_api_key(api_key: str | None) -> bool:
    """Install a desktop-provided API key for this process only."""
    if os.environ.get("DEEPSEEK_API_KEY", "").strip():
        return True
    api_key = normalize_deepseek_api_key(api_key)
    if not api_key:
        return False
    os.environ["DEEPSEEK_API_KEY"] = api_key
    return True


def normalize_deepseek_api_key(value: str | None) -> str:
    """Accept a raw key or extract one pasted inside a shell assignment."""
    text = (value or "").strip()
    if not text:
        return ""
    match = re.search(r"sk-[A-Za-z0-9._-]+", text)
    if match:
        return match.group(0)
    if text.lower().startswith("bearer "):
        text = text[7:].strip()
    text = text.strip("\"'“”‘’")
    try:
        text.encode("ascii")
    except UnicodeEncodeError as error:
        raise AgentError("invalid_api_key", "API Key 包含非 ASCII 字符，请只输入 sk- 开头的密钥", 400) from error
    return text


API_KEY_PROMPT_HTML = """<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><style>
*{box-sizing:border-box}body{margin:0;min-height:100vh;display:grid;place-items:center;
font-family:Segoe UI,Microsoft YaHei,sans-serif;background:#f4f6f3;color:#1d2a24}
.card{width:min(520px,calc(100vw - 40px));padding:34px;border:1px solid #d7dfd9;
border-radius:18px;background:#fff;box-shadow:0 18px 55px #1d2a2418}
h1{margin:0 0 10px;font-size:24px}p{margin:0 0 22px;color:#647169;line-height:1.65}
input{width:100%;padding:13px 14px;border:1px solid #bdc9c1;border-radius:10px;font-size:15px}
input:focus{outline:2px solid #82a991;border-color:transparent}.actions{display:flex;gap:10px;margin-top:18px}
button{padding:11px 18px;border:0;border-radius:9px;cursor:pointer;font-weight:600}
.primary{background:#244d38;color:white}.secondary{background:#edf1ee;color:#34443b}
.note{margin-top:16px;font-size:12px;color:#829087}.status{min-height:20px;margin-top:12px;color:#476453}
</style></head><body><main class="card"><h1>连接 DeepSeek</h1>
<p>输入 API Key 后进入 AgentJS。密钥只用于本次运行，关闭程序后即失效。</p>
<form onsubmit="start(event)"><input id="key" type="password" autocomplete="off"
placeholder="sk-..." autofocus><div class="actions"><button class="primary" type="submit">连接并运行</button>
<button class="secondary" type="button" onclick="offline()">离线演示</button></div></form>
<div id="status" class="status"></div><div class="note">密钥不会写入文件，也不会被打包进 EXE。</div></main><script>
let launching=false;function launch(k){if(launching)return;launching=true;
document.getElementById('status').textContent='正在启动…';document.querySelectorAll('button,input').forEach(x=>x.disabled=true);
pywebview.api.start(k).then(url=>window.location.replace(url)).catch(e=>{launching=false;document.getElementById('status').textContent='启动失败，请重试';
document.querySelectorAll('button,input').forEach(x=>x.disabled=false)})}
function start(e){e.preventDefault();const k=document.getElementById('key').value.trim();if(k)launch(k)}
function offline(){launch('')}
</script></body></html>"""


class DesktopLaunchApi:
    def __init__(self, url: str):
        self.url = url
        self._started = False
        self._lock = threading.Lock()

    def start(self, api_key: str) -> str:
        with self._lock:
            if self._started:
                return self.url
            self._started = True
            configure_desktop_api_key(api_key)
        # Let the page navigate itself after this bridge call has returned.
        # Calling window.load_url from either the bridge or a worker thread is
        # not reliable across pywebview backends.
        return self.url


SYSTEM_PROMPT = """You are the code generator for AgentJS, a constrained JavaScript runtime.
Return one JSON object with exactly two string fields: title and code.
The variable `input` contains JSON data. The code is inserted into a function body.
Treat the current user prompt and current input as authoritative. Generate a new
script for the current turn; do not repeat a previous script unless the user asks
to revise or reuse it.
It MUST call agent.render(tree) exactly once and MUST return a JSON-serializable value.
The render tree root must be a panel. Children may only use text, metrics,
statuses, table, list, or nested panels. Keep nesting at most 6 levels.
Use exactly these node fields:
- panel: {"type":"panel","title":"...","children":[...]}
- text: {"type":"text","value":"..."}; never use content or text for its value
- metrics: {"type":"metrics","items":[{"label":"...","value":"..."}]}
- statuses: {"type":"statuses","items":[{"label":"...","status":"..."}]}
- table: {"type":"table","columns":["..."],"rows":[["..."]]}
- list: {"type":"list","items":["..."]}
Use conservative ES2015 JavaScript: functions, let/const/var, if, for, Array,
Object, JSON, Math, String, Number, Boolean, map/filter/reduce/sort.
Do not use DOM, HTML, CSS, fetch, network, filesystem, Node.js APIs, import/export,
eval, Function, WebAssembly, Worker, dynamic code generation, print,
async/await, or promises. Do not wrap code in Markdown. Keep it deterministic and
do not mutate input. console.log is allowed for execution logs. Never claim that
the code ran; only generate the script."""


FIXED_PROGRAMS = {
    "json_analysis": {
        "title": "区域销售分析",
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
const top = rows.slice(0, 3);
agent.render({
  type: "panel",
  title: "区域销售 Top 3",
  children: [{ type: "table", columns: ["region", "total"], rows: top }]
});
return top;""",
    },
    "rule_processing": {
        "title": "订单规则计算",
        "code": """const orders = Array.isArray(input.orders) ? input.orders : [];
const results = orders.map(function (order) {
  const amount = Number(order.amount || 0);
  const rate = order.member === "gold" ? 0.85 : (order.member === "silver" ? 0.92 : 1);
  return { id: order.id, valid: amount >= 0, payable: amount >= 0 ? Math.round(amount * rate * 100) / 100 : null };
});
agent.render({
  type: "panel",
  title: "订单处理结果",
  children: [{ type: "statuses", items: results }]
});
return results;""",
    },
    "test262_dashboard": {
        "title": "Test262 兼容性分析",
        "code": """const modules = Array.isArray(input.modules) ? input.modules : [];
const rows = modules.map(function (item) {
  const total = Number(item.total || 0);
  const passed = Number(item.passed || 0);
  const rate = total > 0 ? Math.round(passed * 10000 / total) / 100 : 0;
  return { module: String(item.module || "unknown"), passed: passed, total: total, rate: rate };
});
const passed = rows.reduce(function (sum, row) { return sum + row.passed; }, 0);
const total = rows.reduce(function (sum, row) { return sum + row.total; }, 0);
const rate = total > 0 ? Math.round(passed * 10000 / total) / 100 : 0;
agent.render({
  type: "panel",
  title: "Test262 Result",
  children: [
    { type: "metrics", items: [{ label: "Passed", value: passed }, { label: "Total", value: total }, { label: "Rate", value: String(rate) + "%" }] },
    { type: "table", columns: ["module", "passed", "total", "rate"], rows: rows }
  ]
});
return String(rate) + "%";""",
    },
}

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

FIXED_PROGRAMS["chat"] = {
    "title": "AgentJS compatibility report",
    "code": CHAT_CODE,
}
FIXED_PROGRAMS["test262_dashboard"] = FIXED_PROGRAMS["chat"]


class AgentError(RuntimeError):
    def __init__(self, code: str, message: str, status: int = 400):
        super().__init__(message)
        self.code = code
        self.status = status


@dataclass(frozen=True)
class Message:
    role: str
    content: str


@dataclass(frozen=True)
class GeneratedScript:
    code: str
    title: str | None = None


@dataclass(frozen=True)
class AgentRequest:
    session_id: str
    prompt: str
    input: Any
    scenario: str
    mode: str
    engine: str = "agentjs"


@dataclass(frozen=True)
class ExecutionResult:
    value: Any
    logs: list[str]
    render_events: list[dict[str, Any]]
    elapsed_ms: float
    internal_ms: float | None = None

    @property
    def result(self) -> Any:  # Backward-compatible alias used by the old UI/tests.
        return self.value


@dataclass
class Session:
    messages: list[Message] = field(default_factory=list)
    turns: list[dict[str, Any]] = field(default_factory=list)


class CodeGenerator(Protocol):
    def generate(self, history: list[Message], request: AgentRequest) -> GeneratedScript: ...


class SessionStore:
    def __init__(self, capacity: int = MAX_SESSIONS):
        self.capacity = capacity
        self._sessions: OrderedDict[str, Session] = OrderedDict()
        self._lock = threading.Lock()

    def history(self, session_id: str) -> list[Message]:
        with self._lock:
            session = self._sessions.get(session_id)
            return list(session.messages[-MAX_HISTORY_MESSAGES:]) if session else []

    def append(self, session_id: str, prompt: str, script: GeneratedScript, turn: dict[str, Any]) -> None:
        with self._lock:
            session = self._sessions.setdefault(session_id, Session())
            session.messages.extend([
                Message("user", prompt),
                Message("assistant", json.dumps({"title": script.title, "code": script.code}, ensure_ascii=False)),
            ])
            session.messages = session.messages[-MAX_HISTORY_MESSAGES:]
            session.turns.append(turn)
            session.turns = session.turns[-MAX_HISTORY_MESSAGES // 2:]
            self._sessions.move_to_end(session_id)
            while len(self._sessions) > self.capacity:
                self._sessions.popitem(last=False)

    def snapshot(self, session_id: str) -> dict[str, Any] | None:
        with self._lock:
            session = self._sessions.get(session_id)
            if session is None:
                return None
            return {
                "sessionId": session_id,
                "messages": [{"role": item.role, "content": item.content} for item in session.messages],
                "turns": list(session.turns),
            }


SESSION_STORE = SessionStore()


class FixedCodeGenerator:
    def generate(self, history: list[Message], request: AgentRequest) -> GeneratedScript:
        del history
        program = FIXED_PROGRAMS[request.scenario]
        return GeneratedScript(code=program["code"], title=program["title"])


class DeepSeekCodeGenerator:
    def generate(self, history: list[Message], request: AgentRequest) -> GeneratedScript:
        api_key = normalize_deepseek_api_key(os.environ.get("DEEPSEEK_API_KEY"))
        if not api_key:
            raise AgentError("missing_api_key", "请先设置 DEEPSEEK_API_KEY", 503)
        sample = json.dumps(request.input, ensure_ascii=False, separators=(",", ":"))
        user_prompt = (
            f"Scenario: {request.scenario}\nUser prompt: {request.prompt}\n"
            f"Input JSON sample: {sample[:12_000]}\n"
            "Return valid JSON containing title and code."
        )
        messages = [{"role": "system", "content": SYSTEM_PROMPT}]
        messages.extend({"role": item.role, "content": item.content} for item in history[-MAX_HISTORY_MESSAGES:])
        messages.append({"role": "user", "content": user_prompt})
        request_body = json.dumps({
            "model": os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
            "messages": messages,
            "response_format": {"type": "json_object"},
            "thinking": {"type": "disabled"},
            "max_tokens": 2_500,
            "stream": False,
        }).encode("utf-8")
        api_request = urllib.request.Request(
            os.environ.get("DEEPSEEK_API_URL", DEFAULT_API_URL),
            data=request_body,
            headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(api_request, timeout=45) as response:
                body = json.load(response)
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", errors="replace")[:500]
            raise AgentError("deepseek_http", f"DeepSeek API {error.code}: {detail}", 502) from error
        except (
            urllib.error.URLError,
            TimeoutError,
            json.JSONDecodeError,
            http.client.HTTPException,
            UnicodeError,
            OSError,
        ) as error:
            raise AgentError("deepseek_unavailable", f"DeepSeek API 调用失败: {error}", 502) from error
        except Exception as error:
            log_internal_error(error)
            raise AgentError("deepseek_unavailable", f"DeepSeek API 调用异常: {type(error).__name__}", 502) from error
        try:
            content = body["choices"][0]["message"]["content"]
            generated = parse_model_json(content)
        except (KeyError, IndexError, TypeError, ValueError) as error:
            raise AgentError("model_output", "无法解析 DeepSeek 返回内容", 502) from error
        return validate_generated_script(generated, require_render=True)


def parse_model_json(content: Any) -> Any:
    """Parse model JSON with tolerance for Markdown fences and short preambles."""
    if not isinstance(content, str):
        raise ValueError("model content must be a string")
    text = content.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*", "", text, count=1, flags=re.IGNORECASE)
        text = re.sub(r"\s*```$", "", text, count=1).strip()
    try:
        return json.loads(text)
    except json.JSONDecodeError as original:
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end <= start:
            raise ValueError("model output does not contain a JSON object") from original
        try:
            return json.loads(text[start : end + 1])
        except json.JSONDecodeError as error:
            raise ValueError("model output contains invalid JSON") from error


def validate_request(payload: Any) -> AgentRequest:
    if not isinstance(payload, dict):
        raise AgentError("invalid_request", "请求体必须是 JSON 对象")
    prompt = payload.get("prompt", payload.get("task"))
    session_id = payload.get("sessionId") or f"demo-{uuid.uuid4().hex[:12]}"
    scenario = payload.get("scenario", "chat")
    raw_mode = payload.get("mode")
    if raw_mode is None:
        raw_mode = "deepseek" if os.environ.get("DEEPSEEK_API_KEY", "").strip() else "fixed"
    mode = "fixed" if raw_mode == "offline" else raw_mode
    engine = str(payload.get("engine", "agentjs")).lower()
    data = payload.get("input", {})
    if not isinstance(session_id, str) or not SESSION_ID_PATTERN.fullmatch(session_id):
        raise AgentError("invalid_session", "sessionId 只能包含字母、数字、点、下划线和连字符，长度不超过 64")
    if not isinstance(prompt, str) or not prompt.strip():
        raise AgentError("invalid_prompt", "prompt 必须是非空字符串")
    if len(prompt) > MAX_PROMPT_CHARS:
        raise AgentError("prompt_too_large", f"prompt 不能超过 {MAX_PROMPT_CHARS} 个字符")
    if scenario not in SCENARIOS:
        raise AgentError("invalid_scenario", "不支持的场景")
    if mode not in {"fixed", "deepseek"}:
        raise AgentError("invalid_mode", "mode 必须是 fixed、offline 或 deepseek")
    if engine not in {"agentjs", "boa", "both"}:
        raise AgentError("invalid_engine", "engine must be agentjs, boa, or both")
    return AgentRequest(session_id, prompt.strip(), data, scenario, mode, engine)


def validate_generated_script(program: Any, require_render: bool = False) -> GeneratedScript:
    if isinstance(program, GeneratedScript):
        program = {"code": program.code, "title": program.title}
    if not isinstance(program, dict):
        raise AgentError("model_output", "模型没有返回 JSON 对象", 502)
    code = program.get("code")
    title = program.get("title", program.get("plan"))
    if not isinstance(code, str) or (title is not None and not isinstance(title, str)):
        raise AgentError("model_output", "模型输出缺少 code 或 title 类型错误", 502)
    code = code.strip()
    if not code or len(code) > MAX_CODE_CHARS:
        raise AgentError("model_output", "模型代码为空或过长", 502)
    forbidden = (
        "import(", "import ", "export ", "require(", "eval(", "Function(", "fetch(",
        "XMLHttpRequest", "WebAssembly", "Worker(", "process.", "Deno.", "Bun.",
        "print(", "document.", "window.", "innerHTML",
    )
    if any(token in code.replace("\t", " ") for token in forbidden):
        raise AgentError("unsafe_code", "模型代码包含禁止的宿主、DOM 或动态执行能力", 502)
    if "return" not in code:
        raise AgentError("model_output", "模型代码必须返回结果", 502)
    if require_render:
        render_calls = re.sub(r"\s+", "", code).count("agent.render(")
        if render_calls != 1:
            raise AgentError("model_output", "模型代码必须且只能调用一次 agent.render(tree)", 502)
    return GeneratedScript(code=code, title=title.strip() if title else None)


def validate_generated_program(program: Any) -> dict[str, str]:
    """Compatibility facade for the first demo revision."""
    script = validate_generated_script(program)
    return {"plan": script.title or "", "code": script.code}


def generate_offline(scenario: str) -> dict[str, str]:
    program = FIXED_PROGRAMS[scenario]
    return {"plan": program["title"], "code": program["code"]}


def generate_with_deepseek(task: str, scenario: str, data: Any) -> dict[str, str]:
    request = AgentRequest("compat", task, data, scenario, "deepseek")
    script = DeepSeekCodeGenerator().generate([], request)
    return {"plan": script.title or "", "code": script.code}


def find_agentjs_binary() -> Path:
    configured = os.environ.get("AGENTJS_BIN")
    candidates = [Path(configured)] if configured else []
    candidates.extend([
        BUNDLE_ROOT / "agentjs.exe",
        BUNDLE_ROOT / "agentjs",
        SOURCE_ROOT / "target" / "release" / "agentjs.exe",
        SOURCE_ROOT / "target" / "release" / "agentjs",
    ])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AgentError("engine_missing", "未找到 AgentJS release 可执行文件，请先运行 cargo build --release --locked", 503)


def find_boa_binary() -> Path:
    configured = os.environ.get("BOA_BIN")
    candidates = [Path(configured)] if configured else []
    candidates.extend([
        BUNDLE_ROOT / "boa.exe",
        BUNDLE_ROOT / "boa",
        SOURCE_ROOT / "boa" / "target" / "release" / "boa.exe",
        SOURCE_ROOT / "boa" / "target" / "release" / "boa",
    ])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AgentError("boa_missing", "Boa release executable not found; build boa_cli first", 503)


def find_oxide_binary() -> Path:
    configured = os.environ.get("OXIDE_BIN")
    candidates = [Path(configured)] if configured else []
    candidates.extend([
        BUNDLE_ROOT / "oxide.exe",
        BUNDLE_ROOT / "oxide",
        SOURCE_ROOT / "target" / "oxide-compare" / "release" / "oxide.exe",
        SOURCE_ROOT / "target" / "oxide-compare" / "release" / "oxide",
    ])
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise AgentError("oxide_missing", "OxideJS release executable not found", 503)

def build_wrapper(code: str, data: Any) -> str:
    # JSON is a JavaScript expression already. Injecting the parsed value avoids
    # depending on the runtime's JSON.parse implementation for host input.
    input_json = json.dumps(data, ensure_ascii=True, separators=(",", ":"))
    return f'''"use strict";
const input = {input_json};
// Keep request data in the enclosing scope so generated code may safely
// declare its own `input` binding without conflicting with a parameter.
const __agentValue = (function () {{
{code}
}})();
const __agentJson = JSON.stringify(__agentValue === undefined ? null : __agentValue);
"{RESULT_MARKER}" + (__agentJson === undefined ? "null" : __agentJson);
'''


def build_boa_wrapper(code: str, data: Any) -> str:
    input_json = json.dumps(data, ensure_ascii=True, separators=(",", ":"))
    return f'''"use strict";
const input = {input_json};
const __renderEvents = [];
const agent = Object.freeze({{ render: function (tree) {{ __renderEvents.push(tree); }} }});
const __agentValue = (function () {{
{code}
}})();
const __agentJson = JSON.stringify(__agentValue === undefined ? null : __agentValue);
console.log("{RENDER_MARKER}" + JSON.stringify(__renderEvents[__renderEvents.length - 1] || null));
console.log("{RESULT_MARKER}" + (__agentJson === undefined ? "null" : __agentJson));
'''


BENCHMARK_PROGRAM = r'''
var checksum = 0;
var minimum = 2147483647;
var maximum = 0;
for (var i = 1; i <= 200000; i++) {
  var value = (i * 48271) % 2147483647;
  checksum = (checksum + value) % 2147483647;
  if (value < minimum) minimum = value;
  if (value > maximum) maximum = value;
}
agent.render({
  type: "panel",
  title: "Deterministic CPU benchmark",
  children: [{
    type: "metrics",
    items: [
      { label: "Iterations", value: "200000" },
      { label: "Checksum", value: String(checksum) },
      { label: "Range", value: String(maximum - minimum) }
    ]
  }]
});
return String(checksum);
'''


OXIDE_BENCHMARK_PROGRAM = r'''
var checksum = 0;
var minimum = 2147483647;
var maximum = 0;
for (var i = 1; i <= 200000; i++) {
  var value = (i * 48271) % 2147483647;
  checksum = (checksum + value) % 2147483647;
  if (value < minimum) minimum = value;
  if (value > maximum) maximum = value;
}
String(checksum);
'''

def subprocess_window_options() -> dict[str, Any]:
    """Prevent console-hosted child tools from flashing a window on Windows."""
    if os.name == "nt":
        return {"creationflags": subprocess.CREATE_NO_WINDOW}
    return {}


def warm_agentjs_binary() -> None:
    """Pay the OS/antivirus cold-start cost before the first user request."""
    try:
        subprocess.run(
            [str(find_agentjs_binary()), "eval", "0"],
            cwd=ROOT,
            capture_output=True,
            timeout=3,
            check=False,
            **subprocess_window_options(),
        )
    except (OSError, subprocess.TimeoutExpired, AgentError):
        pass


def validate_render_tree(tree: Any, depth: int = 0) -> dict[str, Any]:
    if depth > MAX_RENDER_DEPTH or not isinstance(tree, dict):
        raise AgentError("render_invalid", "RenderTree 类型错误或嵌套过深", 422)
    tree_type = tree.get("type")
    if tree_type not in ALLOWED_RENDER_TYPES:
        raise AgentError("render_invalid", f"不支持的 RenderTree 类型: {tree_type}", 422)
    normalized = dict(tree)
    if tree_type == "text" and "value" not in normalized:
        for alias in ("content", "text"):
            if alias in normalized:
                normalized["value"] = normalized.pop(alias)
                break
    children = normalized.get("children", [])
    if not isinstance(children, list):
        raise AgentError("render_invalid", "RenderTree children 必须是数组", 422)
    normalized["children"] = [validate_render_tree(child, depth + 1) for child in children]
    return normalized


def execute_agentjs(code: str, data: Any) -> ExecutionResult:
    binary = find_agentjs_binary()
    source = build_wrapper(code, data)
    started = time.perf_counter()
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(source)
            temporary_path = handle.name
        try:
            completed = subprocess.run(
                [str(binary), "run", "--time", temporary_path], cwd=ROOT, capture_output=True,
                text=True, encoding="utf-8", timeout=3, check=False,
                **subprocess_window_options(),
            )
        except FileNotFoundError as error:
            raise AgentError("engine_missing", "AgentJS release executable disappeared before execution", 503) from error
        except OSError as error:
            raise AgentError("engine_spawn_failed", f"AgentJS process could not start: {error}", 503) from error
        except UnicodeError as error:
            raise AgentError("engine_output_invalid", "AgentJS returned non-UTF-8 output", 422) from error
    except subprocess.TimeoutExpired as error:
        raise AgentError("execution_timeout", "AgentJS 执行超过 3 秒限制", 422) from error
    except OSError as error:
        raise AgentError("execution_setup_failed", f"AgentJS temporary file could not be created: {error}", 503) from error
    finally:
        if temporary_path:
            try:
                Path(temporary_path).unlink(missing_ok=True)
            except OSError:
                pass
    elapsed_ms = (time.perf_counter() - started) * 1_000
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "未知执行错误"
        raise AgentError("execution_failed", detail[:1_000], 422)
    lines = completed.stdout.splitlines()
    result_lines = [line[len(RESULT_MARKER):] for line in lines if line.startswith(RESULT_MARKER)]
    render_lines = [line[len(RENDER_MARKER):] for line in lines if line.startswith(RENDER_MARKER)]
    if not result_lines:
        raise AgentError("result_missing", "AgentJS 没有返回结构化结果", 422)
    try:
        value = json.loads(result_lines[-1])
        render_events = [json.loads(line) for line in render_lines]
    except json.JSONDecodeError as error:
        raise AgentError("result_invalid", "AgentJS 返回结果不是有效 JSON", 422) from error
    if len(json.dumps(render_events, ensure_ascii=False).encode("utf-8")) > MAX_RENDER_BYTES:
        raise AgentError("render_too_large", "RenderTree 超过 64 KiB", 422)
    validated_events = [validate_render_tree(tree) for tree in render_events]
    logs = [line for line in lines if not line.startswith((RESULT_MARKER, RENDER_MARKER))]
    return ExecutionResult(
        value, logs, validated_events, elapsed_ms, parse_agentjs_internal_ms(completed.stderr)
    )


def execute_boa(code: str, data: Any) -> ExecutionResult:
    binary = find_boa_binary()
    source = build_boa_wrapper(code, data)
    started = time.perf_counter()
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(source)
            temporary_path = handle.name
        completed = subprocess.run(
            [str(binary), "--quiet", "--time", temporary_path], cwd=ROOT, capture_output=True,
            text=True, encoding="utf-8", timeout=3, check=False,
            **subprocess_window_options(),
        )
    except subprocess.TimeoutExpired as error:
        raise AgentError("boa_timeout", "Boa execution exceeded 3 seconds", 422) from error
    except OSError as error:
        raise AgentError("boa_spawn_failed", f"Boa could not start: {error}", 503) from error
    finally:
        if temporary_path:
            try:
                Path(temporary_path).unlink(missing_ok=True)
            except OSError:
                pass
    elapsed_ms = (time.perf_counter() - started) * 1_000
    lines = completed.stdout.splitlines()
    result_lines = [line[len(RESULT_MARKER):] for line in lines if line.startswith(RESULT_MARKER)]
    render_lines = [line[len(RENDER_MARKER):] for line in lines if line.startswith(RENDER_MARKER)]
    if completed.returncode != 0 or not result_lines:
        detail = completed.stderr.strip() or completed.stdout.strip() or "Boa returned no structured result"
        raise AgentError("boa_execution_failed", detail[:1_000], 422)
    try:
        value = json.loads(result_lines[-1])
        render_events = [json.loads(line) for line in render_lines if line != "null"]
    except json.JSONDecodeError as error:
        raise AgentError("boa_result_invalid", "Boa returned invalid structured JSON", 422) from error
    validated_events = [validate_render_tree(tree) for tree in render_events]
    logs = [line for line in lines if not line.startswith((RESULT_MARKER, RENDER_MARKER))]
    return ExecutionResult(
        value, logs, validated_events, elapsed_ms, parse_boa_internal_ms(completed.stderr)
    )


def parse_agentjs_internal_ms(stderr: str) -> float | None:
    pattern = rf"^{re.escape(AGENTJS_TIME_MARKER)}([0-9]+(?:\.[0-9]+)?)$"
    match = re.search(pattern, stderr, re.MULTILINE)
    return float(match.group(1)) if match else None


def parse_boa_internal_ms(stderr: str) -> float | None:
    match = re.search(
        r"^Total:\s+([0-9]+(?:\.[0-9]+)?)(ns|µs|us|ms|s)$", stderr, re.MULTILINE
    )
    if not match:
        return None
    value = float(match.group(1))
    scale = {"ns": 0.000001, "µs": 0.001, "us": 0.001, "ms": 1.0, "s": 1000.0}
    return value * scale[match.group(2)]


def execute_oxide_benchmark() -> ExecutionResult:
    binary = find_oxide_binary()
    started = time.perf_counter()
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(OXIDE_BENCHMARK_PROGRAM)
            temporary_path = handle.name
        completed = subprocess.run(
            [str(binary), "--quiet", "run", temporary_path],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=3,
            check=False,
            **subprocess_window_options(),
        )
    except subprocess.TimeoutExpired as error:
        raise AgentError("oxide_timeout", "OxideJS execution exceeded 3 seconds", 422) from error
    except OSError as error:
        raise AgentError("oxide_spawn_failed", f"OxideJS could not start: {error}", 503) from error
    finally:
        if temporary_path:
            try:
                Path(temporary_path).unlink(missing_ok=True)
            except OSError:
                pass
    elapsed_ms = (time.perf_counter() - started) * 1_000
    lines = [line.strip() for line in completed.stdout.splitlines() if line.strip()]
    if completed.returncode != 0 or not lines:
        detail = completed.stderr.strip() or completed.stdout.strip() or "OxideJS returned no result"
        raise AgentError("oxide_execution_failed", detail[:1_000], 422)
    try:
        value = json.loads(lines[-1])
    except json.JSONDecodeError as error:
        raise AgentError("oxide_result_invalid", "OxideJS returned invalid JSON-compatible output", 422) from error
    return ExecutionResult(value, [], [], elapsed_ms, None)


def execute_agentjs_cached_benchmark(warmup: int, iterations: int) -> dict[str, Any]:
    binary = find_agentjs_binary()
    source = f"(function () {{\n{BENCHMARK_PROGRAM}\n}})();\n"
    temporary_path: str | None = None
    started = time.perf_counter()
    try:
        with tempfile.NamedTemporaryFile("w", suffix=".js", encoding="utf-8", delete=False) as handle:
            handle.write(source)
            temporary_path = handle.name
        completed = subprocess.run(
            [
                str(binary), "cache-bench", temporary_path,
                "--warmup", str(warmup), "--iterations", str(iterations),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=120,
            check=False,
            **subprocess_window_options(),
        )
    except subprocess.TimeoutExpired as error:
        raise AgentError("cache_benchmark_timeout", "AgentJS cached benchmark exceeded 120 seconds", 422) from error
    except OSError as error:
        raise AgentError("cache_benchmark_failed", f"AgentJS cached benchmark could not start: {error}", 503) from error
    finally:
        if temporary_path:
            try:
                Path(temporary_path).unlink(missing_ok=True)
            except OSError:
                pass
    elapsed_ms = (time.perf_counter() - started) * 1_000
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if completed.returncode != 0 or not lines:
        detail = completed.stderr.strip() or completed.stdout.strip() or "AgentJS returned no cache benchmark"
        raise AgentError("cache_benchmark_failed", detail[:1_000], 422)
    try:
        payload = json.loads(lines[-1])
        samples = [float(value) for value in payload["samplesMs"]]
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
        raise AgentError("cache_benchmark_invalid", "AgentJS returned invalid cached timing data", 422) from error
    return {
        "result": payload.get("result"),
        "internal": benchmark_summary(samples),
        "cacheHits": int(payload.get("cacheHits", 0)),
        "cacheMisses": int(payload.get("cacheMisses", 0)),
        "processTotalMs": round(elapsed_ms, 2),
    }


def execution_payload(result: ExecutionResult, model_ms: float = 0.0) -> dict[str, Any]:
    render = result.render_events[-1] if result.render_events else None
    return {
        "ok": True, "value": result.value, "logs": result.logs,
        "elapsedMs": round(result.elapsed_ms, 2),
        "internalMs": None if result.internal_ms is None else round(result.internal_ms, 4),
        "modelMs": round(model_ms, 2),
        "totalMs": round(model_ms + result.elapsed_ms, 2), "render": render, "error": None,
    }


def failed_execution_payload(error: AgentError) -> dict[str, Any]:
    return {"ok": False, "value": None, "logs": [], "elapsedMs": 0,
            "internalMs": None, "render": None,
            "error": {"code": error.code, "message": str(error)}}


def benchmark_summary(samples: list[float]) -> dict[str, Any]:
    ordered = sorted(samples)
    p95_index = max(0, min(len(ordered) - 1, (95 * len(ordered) + 99) // 100 - 1))
    return {
        "medianMs": round(statistics.median(ordered), 4),
        "p95Ms": round(ordered[p95_index], 4),
        "minMs": round(ordered[0], 4),
        "maxMs": round(ordered[-1], 4),
        "samplesMs": [round(value, 4) for value in samples],
    }


def run_benchmark(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise AgentError("invalid_request", "基准测试请求必须是 JSON 对象", 400)
    try:
        warmup = int(payload.get("warmup", 5))
        iterations = int(payload.get("iterations", 50))
    except (TypeError, ValueError) as error:
        raise AgentError("invalid_benchmark", "warmup 和 iterations 必须是整数", 400) from error
    if not 3 <= warmup <= 5 or not 30 <= iterations <= 100:
        raise AgentError("invalid_benchmark", "warmup 必须为 3-5，iterations 必须为 30-100", 400)

    runners = {
        "agentjs": lambda: execute_agentjs(BENCHMARK_PROGRAM, {}),
        "boa": lambda: execute_boa(BENCHMARK_PROGRAM, {}),
        "oxide": execute_oxide_benchmark,
    }
    engine_names = list(runners)
    for index in range(warmup):
        offset = index % len(engine_names)
        order = engine_names[offset:] + engine_names[:offset]
        for name in order:
            runners[name]()

    samples = {name: {"internal": [], "endToEnd": []} for name in runners}
    expected_value = None
    for index in range(iterations):
        offset = index % len(engine_names)
        order = engine_names[offset:] + engine_names[:offset]
        for name in order:
            result = runners[name]()
            if expected_value is None:
                expected_value = result.value
            elif result.value != expected_value:
                raise AgentError("benchmark_mismatch", "三个引擎产生了不同的基准结果", 422)
            samples[name]["endToEnd"].append(result.elapsed_ms)
            if result.internal_ms is not None:
                samples[name]["internal"].append(result.internal_ms)

    cached_agentjs = execute_agentjs_cached_benchmark(warmup, iterations)
    if cached_agentjs["result"] != expected_value:
        raise AgentError("benchmark_mismatch", "AgentJS 缓存模式产生了不同的基准结果", 422)

    engines = {
        name: {
            "internal": benchmark_summary(values["internal"]) if values["internal"] else None,
            "endToEnd": benchmark_summary(values["endToEnd"]),
        }
        for name, values in samples.items()
    }
    engines["agentjs"]["cached"] = cached_agentjs

    return {
        "ok": True,
        "workload": "deterministic-arithmetic-200k",
        "warmup": warmup,
        "iterations": iterations,
        "order": "rotating",
        "result": expected_value,
        "engines": engines,
        "notes": "AgentJS cached timing reuses one isolate and its parsed/compiled-script LRU. The benchmark source keeps mutable state function-local. Cold/fresh-process results remain separate.",
    }

def run_agent(payload: Any, store: SessionStore = SESSION_STORE) -> dict[str, Any]:
    request = validate_request(payload)
    history = store.history(request.session_id)
    generator: CodeGenerator = FixedCodeGenerator() if request.mode == "fixed" else DeepSeekCodeGenerator()
    model_started = time.perf_counter()
    script = validate_generated_script(generator.generate(history, request), require_render=True)
    model_ms = (time.perf_counter() - model_started) * 1_000

    runners = {"agentjs": execute_agentjs, "boa": execute_boa}
    selected = ["agentjs", "boa"] if request.engine == "both" else [request.engine]
    executions: dict[str, dict[str, Any]] = {}
    for engine_name in selected:
        try:
            executions[engine_name] = execution_payload(runners[engine_name](script.code, request.input), model_ms)
        except AgentError as error:
            if request.engine != "both":
                raise
            executions[engine_name] = failed_execution_payload(error)

    primary_name = "agentjs" if "agentjs" in executions else selected[0]
    primary = executions[primary_name]
    if not primary["ok"]:
        primary = next((item for item in executions.values() if item["ok"]), primary)
    response = {
        "ok": any(item["ok"] for item in executions.values()),
        "sessionId": request.session_id, "prompt": request.prompt, "code": script.code,
        "engine": request.engine, "execution": primary, "executions": executions,
        "render": primary.get("render"), "error": None, "scenario": request.scenario,
        "mode": "offline" if request.mode == "fixed" else request.mode,
        "model": "fixed-script" if request.mode == "fixed" else os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
        "plan": script.title or "", "result": primary.get("value"),
        "metrics": {"modelMs": round(model_ms, 2),
                    "agentjsMs": executions.get("agentjs", {}).get("elapsedMs"),
                    "boaMs": executions.get("boa", {}).get("elapsedMs"),
                    "totalMs": primary.get("totalMs")},
    }
    store.append(request.session_id, request.prompt, script, {
        "prompt": request.prompt, "code": script.code, "engine": request.engine,
        "executions": executions, "render": primary.get("render"), "result": primary.get("value"),
    })
    return response

def error_response(error: AgentError, payload: Any = None) -> dict[str, Any]:
    payload = payload if isinstance(payload, dict) else {}
    return {
        "ok": False,
        "sessionId": payload.get("sessionId"),
        "prompt": payload.get("prompt", payload.get("task")),
        "code": None,
        "execution": None,
        "render": None,
        "error": {"code": error.code, "message": str(error)},
    }


class AgentHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args: Any, **kwargs: Any):
        super().__init__(*args, directory=str(STATIC_ROOT), **kwargs)

    def do_GET(self) -> None:
        request_path = self.path.split("?", 1)[0]
        if request_path == "/api/health":
            self.send_json(HTTPStatus.OK, {
                "ok": True,
                "deepseekConfigured": bool(os.environ.get("DEEPSEEK_API_KEY", "").strip()),
                "model": os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL),
                "agentjsAvailable": any(path.is_file() for path in [
                    BUNDLE_ROOT / "agentjs.exe",
                    BUNDLE_ROOT / "agentjs",
                    SOURCE_ROOT / "target/release/agentjs.exe",
                    SOURCE_ROOT / "target/release/agentjs",
                ]),
                "boaAvailable": any(path.is_file() for path in [
                    BUNDLE_ROOT / "boa.exe",
                    BUNDLE_ROOT / "boa",
                    SOURCE_ROOT / "boa" / "target" / "release" / "boa.exe",
                    SOURCE_ROOT / "boa" / "target" / "release" / "boa",
                ]),
                "oxideAvailable": any(path.is_file() for path in [
                    BUNDLE_ROOT / "oxide.exe",
                    BUNDLE_ROOT / "oxide",
                    SOURCE_ROOT / "target" / "oxide-compare" / "release" / "oxide.exe",
                    SOURCE_ROOT / "target" / "oxide-compare" / "release" / "oxide",
                ]),
            })
            return
        if request_path.startswith("/api/sessions/"):
            session_id = request_path.removeprefix("/api/sessions/")
            snapshot = SESSION_STORE.snapshot(session_id)
            if snapshot is None:
                self.send_json(HTTPStatus.NOT_FOUND, error_response(AgentError("session_not_found", "会话不存在", 404)))
            else:
                self.send_json(HTTPStatus.OK, {"ok": True, **snapshot})
            return
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
        request_path = self.path.split("?", 1)[0]
        if request_path not in {"/api/agent", "/api/benchmark"}:
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        payload: Any = None
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > MAX_REQUEST_BYTES:
                raise AgentError("request_too_large", "请求体为空或超过 256 KiB", 413)
            payload = json.loads(self.rfile.read(length))
            response = run_benchmark(payload) if request_path == "/api/benchmark" else run_agent(payload)
            self.send_json(HTTPStatus.OK, response)
        except json.JSONDecodeError:
            error = AgentError("invalid_json", "请求不是有效 JSON", 400)
            self.send_json(error.status, error_response(error, payload))
        except AgentError as error:
            self.send_json(error.status, error_response(error, payload))
        except Exception as unexpected:
            log_internal_error(unexpected)
            error = AgentError("internal", "编排服务内部错误（详情已写入本地日志）", 500)
            self.send_json(error.status, error_response(error, payload))

    def send_json(self, status: int, payload: Any) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def main() -> None:
    parser = argparse.ArgumentParser(description="AgentJS orchestrator demo")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=0, help="listen port; 0 selects a free port")
    parser.add_argument("--browser", action="store_true", help="open the UI in the default browser")
    parser.add_argument("--no-browser", action="store_true", help="run only the HTTP service")
    args = parser.parse_args()
    server = ThreadingHTTPServer((args.host, args.port), AgentHandler)
    actual_port = server.server_address[1]
    url = f"http://{args.host}:{actual_port}/frontend/agent-chat.html"
    print(f"AgentJS demo: {url}")
    print("Mode: DeepSeek enabled" if os.environ.get("DEEPSEEK_API_KEY", "").strip() else "Mode: fixed scripts (set DEEPSEEK_API_KEY for DeepSeek V4 Pro)")
    if webview is not None and not args.browser and not args.no_browser:
        worker = threading.Thread(target=server.serve_forever, name="agentjs-http", daemon=True)
        worker.start()
        threading.Thread(target=warm_agentjs_binary, name="agentjs-warmup", daemon=True).start()
        try:
            launch_api = DesktopLaunchApi(url)
            window_options = {"url": url} if os.environ.get("DEEPSEEK_API_KEY", "").strip() else {"html": API_KEY_PROMPT_HTML}
            webview.create_window(
                "AgentJS Conversation",
                js_api=launch_api,
                width=1180,
                height=780,
                min_size=(760, 520),
                background_color="#f4f6f3",
                **window_options,
            )
            webview.start()
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=5)
        return
    if args.browser:
        threading.Timer(0.35, webbrowser.open, args=(url,)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
