#!/usr/bin/env python3
"""
Delivery correctness gate for the AgentJS supported subset.

This runner intentionally avoids unsupported or not-yet-claimed features such as
large arbitrary-precision BigInt, RegExp Unicode set `v` syntax, workers, and the
QuickJS `std` host APIs. A failure here should be treated as a correctness bug in
the subset we are willing to ship.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RESULTS = ROOT / "benchmarks" / "correctness" / "results"


@dataclass(frozen=True)
class Case:
    name: str
    category: str
    source: str
    expected: str


@dataclass
class CaseResult:
    name: str
    category: str
    status: str
    expected: str
    actual: str
    returncode: int | None
    elapsed_ms: float
    stdout: str
    stderr: str


CASES: list[Case] = [
    Case("arithmetic_precedence", "expression", "1 + 2 * 3;", "7"),
    Case(
        "bitwise_unsigned_shift",
        "expression",
        "(-4 >>> 1) === 0x7ffffffe;",
        "true",
    ),
    Case(
        "parenthesized_unary_exponent",
        "expression",
        "(-2) ** 3;",
        "-8",
    ),
    Case(
        "object_is_same_value",
        "object",
        "Object.is(NaN, NaN) + ':' + Object.is(0, -0);",
        "true:false",
    ),
    Case(
        "descriptor_delete_and_in",
        "object",
        "var a = {x: 1, y: 2}; delete a.x; ('x' in a) + ':' + ('y' in a);",
        "false:true",
    ),
    Case(
        "delete_primitive_and_nullish",
        "object",
        "var ok = delete 'abc'[100]; try { delete null.a; ok = false; } catch (e) { ok = ok && e instanceof TypeError; } ok;",
        "true",
    ),
    Case(
        "prototype_lookup",
        "object",
        "var base = {x: 3}; var child = {__proto__: base, y: 4}; child.x + child.y;",
        "7",
    ),
    Case(
        "define_property_descriptor",
        "object",
        "var o = {}; Object.defineProperty(o, 'x', {value: 1, writable: false, enumerable: false, configurable: false}); var d = Object.getOwnPropertyDescriptor(o, 'x'); d.value + ':' + d.writable + ':' + d.enumerable + ':' + d.configurable;",
        "1:false:false:false",
    ),
    Case(
        "array_holes_and_length",
        "array",
        "var a = [1,,3]; a.length + ':' + (1 in a) + ':' + a[2];",
        "3:false:3",
    ),
    Case(
        "sparse_array_high_index",
        "array",
        "var a = []; a[70000] = 5; a.length + ':' + a[70000];",
        "70001:5",
    ),
    Case(
        "for_in_member_target",
        "iteration",
        "var a = {x: 0}; var r = []; for (a.x in {x:1, y:2}) { r.push(a.x); } r.join(',');",
        "x,y",
    ),
    Case(
        "for_let_closure_capture",
        "scope",
        "var tab = []; for (let i = 0; i < 3; i++) { tab.push(function(){ return i; }); } tab[0]() + ':' + tab[1]() + ':' + tab[2]();",
        "0:1:2",
    ),
    Case(
        "eval_closure_for_let_capture",
        "scope",
        "var tab = []; for (let i = 0; i < 3; i++) { eval('tab.push(function(){ return i; })'); } tab[0]() + ':' + tab[1]() + ':' + tab[2]();",
        "0:1:2",
    ),
    Case(
        "with_eval_lookup",
        "scope",
        "var o1 = {x:'o1', y:'o1'}; var x = 'local'; var r; with (o1) { r = x + ':' + eval('x'); } r;",
        "o1:o1",
    ),
    Case(
        "arrow_lexical_arguments",
        "function",
        "function f() { return (() => arguments)().length; } f(1, 2);",
        "2",
    ),
    Case(
        "constructor_returns_function_object",
        "function",
        "function replacement() {} function F() { return replacement; } new F() === replacement;",
        "true",
    ),
    Case(
        "function_prototype_call_saved",
        "function",
        "function read() { return this.x; } var saved = Function.prototype.call; Function.prototype.call = Object; read.saved = saved; read.saved({x: 9});",
        "9",
    ),
    Case(
        "class_instance_static_and_getter_name",
        "class",
        "class C { constructor(){ this.x = 10; } f(){ return 1; } static F(){ return -1; } get y(){ return 12; } } var o = new C(); C.F() + ':' + o.f() + ':' + o.x + ':' + Object.getOwnPropertyDescriptor(C.prototype, 'y').get.name;",
        "-1:1:10:get y",
    ),
    Case(
        "class_extends_static_super_computed",
        "class",
        "class C { static F(){ return -1; } f(){ return 1; } } class D extends C { static H(){ return super['F'](); } h(){ return super.f(); } } var d = new D(); (Object.getPrototypeOf(D) === C) + ':' + D.F() + ':' + D.H() + ':' + d.h();",
        "true:-1:-1:1",
    ),
    Case(
        "delete_super_reference_error",
        "class",
        "var ok = false; var a = { f() { delete super.a; } }; try { a.f(); } catch (e) { ok = e instanceof ReferenceError; } ok;",
        "true",
    ),
    Case(
        "try_catch_finally",
        "control",
        "var s = ''; try { s += 't'; throw 'c'; } catch (e) { s += e; } finally { s += 'f'; } s;",
        "tcf",
    ),
    Case(
        "json_roundtrip",
        "stdlib",
        "var o = JSON.parse('{\"x\":1,\"y\":[2,3]}'); JSON.stringify(o);",
        "{\"x\":1,\"y\":[2,3]}",
    ),
    Case(
        "map_same_value_zero",
        "stdlib",
        "var m = new Map(); m.set(NaN, 1); m.set(0, 2); m.get(NaN) + ':' + m.get(-0);",
        "1:2",
    ),
    Case(
        "string_raw_tagged_template",
        "stdlib",
        "var b = 'X'; String.raw `abc${b}d`;",
        "abcXd",
    ),
]


def default_agentjs() -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    release = ROOT / "target" / "release" / f"agentjs{suffix}"
    debug = ROOT / "target" / "debug" / f"agentjs{suffix}"
    return release if release.exists() else debug


def normalize(text: str | None) -> str:
    if not text:
        return ""
    return text.replace("\r\n", "\n").replace("\r", "\n")


def observed_value(stdout: str) -> str:
    lines = [line for line in normalize(stdout).split("\n") if line != ""]
    return lines[-1] if lines else "undefined"


def run_case(case: Case, command_prefix: list[str], timeout: int) -> CaseResult:
    started = time.perf_counter()
    timed_out = False
    try:
        proc = subprocess.run(
            command_prefix + [case.source],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
        )
        returncode = proc.returncode
        stdout = normalize(proc.stdout)
        stderr = normalize(proc.stderr)
    except subprocess.TimeoutExpired as error:
        timed_out = True
        returncode = None
        stdout = normalize(error.stdout.decode("utf-8", "replace") if isinstance(error.stdout, bytes) else error.stdout)
        stderr = normalize(error.stderr.decode("utf-8", "replace") if isinstance(error.stderr, bytes) else error.stderr)

    elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
    actual = observed_value(stdout)
    if timed_out:
        status = "timeout"
    elif returncode != 0:
        status = "runtime-error"
    elif actual == case.expected:
        status = "pass"
    else:
        status = "wrong-answer"
    return CaseResult(
        name=case.name,
        category=case.category,
        status=status,
        expected=case.expected,
        actual=actual,
        returncode=returncode,
        elapsed_ms=elapsed_ms,
        stdout=stdout,
        stderr=stderr,
    )


def write_markdown(path: Path, label: str, command: list[str], results: list[CaseResult]) -> None:
    passed = sum(1 for result in results if result.status == "pass")
    lines = [
        "# Supported Correctness Gate",
        "",
        f"- Label: `{label}`",
        f"- Command: `{' '.join(command)}`",
        f"- Result: **{passed}/{len(results)} passed**",
        "",
        "| Case | Category | Status | Expected | Actual | Time ms |",
        "|:---|:---|:---|:---|:---|---:|",
    ]
    for result in results:
        lines.append(
            f"| {result.name} | {result.category} | {result.status} | `{result.expected}` | `{result.actual}` | {result.elapsed_ms:.1f} |"
        )

    failures = [result for result in results if result.status != "pass"]
    if failures:
        lines.extend(["", "## Failure Details"])
        for result in failures:
            output = (result.stderr or result.stdout).strip()
            lines.extend(
                [
                    "",
                    f"### {result.name}",
                    "",
                    f"- Status: `{result.status}`",
                    f"- Expected: `{result.expected}`",
                    f"- Actual: `{result.actual}`",
                ]
            )
            if output:
                lines.extend(["", "```text", output[-2000:], "```"])
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run AgentJS supported-subset correctness cases.")
    parser.add_argument("--engine", default=str(default_agentjs()), help="agentjs executable")
    parser.add_argument("--timeout", type=int, default=10, help="timeout seconds per case")
    parser.add_argument("--label", default="agentjs-supported-correctness")
    parser.add_argument("--category", default=None, help="comma-separated categories to include")
    parser.add_argument("--out-json", default=None)
    parser.add_argument("--out-md", default=None)
    parser.add_argument("--list", action="store_true", help="list cases and exit")
    args = parser.parse_args()

    selected = CASES
    if args.category:
        categories = {part.strip() for part in args.category.split(",") if part.strip()}
        selected = [case for case in CASES if case.category in categories]

    if args.list:
        for case in selected:
            print(f"{case.category}\t{case.name}")
        return 0

    command = [args.engine, "eval"]
    results = [run_case(case, command, args.timeout) for case in selected]

    DEFAULT_RESULTS.mkdir(parents=True, exist_ok=True)
    out_json = Path(args.out_json) if args.out_json else DEFAULT_RESULTS / f"{args.label}.json"
    out_md = Path(args.out_md) if args.out_md else DEFAULT_RESULTS / f"{args.label}.md"
    payload = {
        "label": args.label,
        "command": command,
        "results": [asdict(result) for result in results],
    }
    out_json.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_markdown(out_md, args.label, command, results)

    passed = sum(1 for result in results if result.status == "pass")
    print(f"RESULT {args.label}: {passed}/{len(results)} passed")
    print(f"JSON -> {out_json}")
    print(f"MD   -> {out_md}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
