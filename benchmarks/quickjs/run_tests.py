#!/usr/bin/env python3
"""
Run QuickJS' bundled JavaScript correctness tests with AgentJS or another engine.

This does not build or execute qjs. It treats quickjs/tests/*.js as a borrowed
correctness suite and records pass/fail/error/timeout per file.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
QUICKJS_TESTS = ROOT / "quickjs" / "tests"
DEFAULT_RESULTS = ROOT / "benchmarks" / "quickjs" / "results"

NODE_TEST_CASES = [
    "test_closure.js",
    "test_language.js",
    "test_builtin.js",
    "test_loop.js",
    "test_bigint.js",
]

MAKEFILE_CASES = [
    "test_closure.js",
    "test_language.js",
    "test_builtin.js",
    "test_loop.js",
    "test_bigint.js",
    "test_cyclic_import.js",
    "test_worker.js",
    "test_std.js",
    "test_rw_handler.js",
    "test_bjson.js",
]


@dataclass
class CaseResult:
    case: str
    status: str
    returncode: int | None
    elapsed_ms: float
    stdout: str
    stderr: str


def default_agentjs() -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    release = ROOT / "target" / "release" / f"agentjs{suffix}"
    debug = ROOT / "target" / "debug" / f"agentjs{suffix}"
    if release.exists():
        return release
    return debug


def split_csv(value: str | None) -> list[str]:
    if not value:
        return []
    return [part.strip() for part in value.split(",") if part.strip()]


def resolve_case(name: str) -> Path:
    path = Path(name)
    if path.exists():
        return path
    candidate = QUICKJS_TESTS / name
    if candidate.exists():
        return candidate
    if not name.endswith(".js"):
        candidate = QUICKJS_TESTS / f"{name}.js"
        if candidate.exists():
            return candidate
    raise SystemExit(f"QuickJS test case not found: {name}")


def collect_cases(args: argparse.Namespace) -> list[Path]:
    explicit = split_csv(args.cases)
    if explicit:
        return [resolve_case(name) for name in explicit]

    if args.suite == "node":
        return [resolve_case(name) for name in NODE_TEST_CASES]
    if args.suite == "makefile":
        return [resolve_case(name) for name in MAKEFILE_CASES]
    if args.suite == "all":
        return sorted(QUICKJS_TESTS.glob("test_*.js"))
    raise SystemExit(f"unknown suite: {args.suite}")


def normalize(text: str | bytes | None) -> str:
    if text is None:
        return ""
    if isinstance(text, bytes):
        return text.decode("utf-8", errors="replace")
    return text.replace("\r\n", "\n").replace("\r", "\n")


def classify(returncode: int | None, stdout: str, stderr: str, timeout: bool) -> str:
    if timeout:
        return "timeout"
    if returncode == 0:
        return "pass"
    combined = (stderr + "\n" + stdout).lower()
    if "syntaxerror" in combined or "parse error" in combined:
        return "syntax-error"
    if "runtimelimit" in combined or "limit exceeded" in combined:
        return "runtime-limit"
    if "assertion failed" in combined:
        return "assert-fail"
    return "runtime-error"


def run_case(case: Path, engine: list[str], timeout: int) -> CaseResult:
    command = engine + [str(case)]
    started = time.perf_counter()
    timed_out = False
    try:
        proc = subprocess.run(
            command,
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
        stdout = normalize(error.stdout)
        stderr = normalize(error.stderr)

    elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
    status = classify(returncode, stdout, stderr, timed_out)
    return CaseResult(
        case=str(case.relative_to(ROOT)),
        status=status,
        returncode=returncode,
        elapsed_ms=elapsed_ms,
        stdout=stdout,
        stderr=stderr,
    )


def write_markdown(path: Path, label: str, engine: list[str], results: list[CaseResult]) -> None:
    total = len(results)
    passed = sum(1 for result in results if result.status == "pass")
    lines: list[str] = []
    lines.append("# QuickJS Correctness Tests")
    lines.append("")
    lines.append(f"- Label: `{label}`")
    lines.append(f"- Engine: `{' '.join(engine)}`")
    lines.append(f"- Result: **{passed}/{total} passed**")
    lines.append("")
    lines.append("| Case | Status | Time ms |")
    lines.append("|:---|:---|---:|")
    for result in results:
        lines.append(f"| {result.case} | {result.status} | {result.elapsed_ms:.1f} |")

    failures = [result for result in results if result.status != "pass"]
    if failures:
        lines.append("")
        lines.append("## Failure Details")
        for result in failures:
            lines.append("")
            lines.append(f"### {result.case}")
            lines.append("")
            lines.append(f"- Status: `{result.status}`")
            lines.append(f"- Return code: `{result.returncode}`")
            output = (result.stderr or result.stdout).strip()
            if output:
                lines.append("")
                lines.append("```text")
                lines.append(output[-2000:])
                lines.append("```")

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run QuickJS correctness tests with AgentJS.")
    parser.add_argument("--engine", default=str(default_agentjs()), help="engine executable")
    parser.add_argument(
        "--subcommand",
        default="jetstream",
        help="engine subcommand before each JS file; use '' for node",
    )
    parser.add_argument("--suite", choices=["node", "makefile", "all"], default="node")
    parser.add_argument("--cases", default=None, help="comma-separated case names or paths")
    parser.add_argument("--timeout", type=int, default=60, help="timeout seconds per case")
    parser.add_argument("--label", default="agentjs-quickjs-tests", help="report label")
    parser.add_argument("--out-json", default=None, help="JSON report path")
    parser.add_argument("--out-md", default=None, help="Markdown report path")
    args = parser.parse_args()

    if not QUICKJS_TESTS.exists():
        raise SystemExit(f"QuickJS tests directory not found: {QUICKJS_TESTS}")

    engine = [args.engine]
    if args.subcommand:
        engine.append(args.subcommand)

    cases = collect_cases(args)
    results = [run_case(case, engine, args.timeout) for case in cases]

    DEFAULT_RESULTS.mkdir(parents=True, exist_ok=True)
    out_json = Path(args.out_json) if args.out_json else DEFAULT_RESULTS / f"{args.label}.json"
    out_md = Path(args.out_md) if args.out_md else DEFAULT_RESULTS / f"{args.label}.md"

    payload = {
        "label": args.label,
        "engine": engine,
        "suite": args.suite,
        "results": [asdict(result) for result in results],
    }
    out_json.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_markdown(out_md, args.label, engine, results)

    passed = sum(1 for result in results if result.status == "pass")
    print(f"RESULT {args.label}: {passed}/{len(results)} passed")
    print(f"JSON -> {out_json}")
    print(f"MD   -> {out_md}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
