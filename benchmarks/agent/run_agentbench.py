#!/usr/bin/env python3
"""AgentBench runner for comparing AgentJS with Boa or another JS engine."""

from __future__ import annotations

import argparse
import io
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from statistics import median


sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace", line_buffering=True)
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace", line_buffering=True)

ROOT = Path(__file__).resolve().parents[2]
CASES_DIR = Path(__file__).resolve().parent / "cases"
RESULTS_DIR = Path(__file__).resolve().parent / "results"


def default_agentjs() -> str:
    suffix = ".exe" if os.name == "nt" else ""
    release = ROOT / "target" / "release" / f"agentjs{suffix}"
    debug = ROOT / "target" / "debug" / f"agentjs{suffix}"
    return str(release if release.exists() else debug)


def split_csv(value: str | None) -> list[str]:
    if not value:
        return []
    return [part.strip() for part in value.split(",") if part.strip()]


def build_cmd(engine: str, subcommand: str) -> list[str]:
    command = [engine]
    if subcommand:
        command.append(subcommand)
    return command


def format_ms(value: float | None) -> str:
    if value is None:
        return "-"
    if value >= 10000:
        return f"{value / 1000:.1f}s"
    return f"{value:.0f}ms"


def ratio_text(lhs: float | None, rhs: float | None) -> str:
    if lhs is None or rhs is None or rhs == 0:
        return "-"
    ratio = lhs / rhs
    if ratio < 1:
        return f"{1 / ratio:.2f}x faster"
    return f"{ratio:.2f}x slower"


def run_once(command: list[str], case_path: Path, timeout: int) -> dict:
    started = time.perf_counter()
    try:
        proc = subprocess.run(
            command + [str(case_path)],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
        )
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        if proc.returncode == 0:
            return {"status": "pass", "time_ms": elapsed_ms, "stderr": ""}
        return {
            "status": "runtime-error",
            "time_ms": elapsed_ms,
            "stderr": (proc.stderr + proc.stdout).strip()[-500:],
        }
    except subprocess.TimeoutExpired as error:
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        stderr = error.stderr or ""
        stdout = error.stdout or ""
        if isinstance(stderr, bytes):
            stderr = stderr.decode("utf-8", errors="replace")
        if isinstance(stdout, bytes):
            stdout = stdout.decode("utf-8", errors="replace")
        return {
            "status": "timeout",
            "time_ms": elapsed_ms,
            "stderr": (stderr + stdout).strip()[-500:],
        }


def run_case(command: list[str], case_path: Path, repeat: int, timeout: int) -> dict:
    runs = []
    for _ in range(repeat):
        result = run_once(command, case_path, timeout)
        runs.append(result)
        if result["status"] != "pass":
            break

    pass_times = [run["time_ms"] for run in runs if run["status"] == "pass"]
    if pass_times:
        return {
            "status": "pass",
            "median_ms": round(median(pass_times), 1),
            "runs": runs,
            "error": "",
        }
    return {
        "status": runs[0]["status"],
        "median_ms": None,
        "runs": runs,
        "error": runs[0]["stderr"],
    }


def write_markdown(path: Path, report: dict) -> None:
    label_a = report["meta"]["label_a"]
    label_b = report["meta"]["label_b"]
    results = report["results"]

    lines = ["# AgentBench", ""]
    lines.append(f"- Primary: `{label_a}`")
    if label_b:
        lines.append(f"- Reference: `{label_b}`")
    lines.append(f"- Repeat: `{report['meta']['repeat']}`")
    lines.append("")

    if label_b:
        lines.append("| Case | AgentJS | Reference | Result |")
        lines.append("|:---|---:|---:|---:|")
    else:
        lines.append("| Case | Status | Median |")
        lines.append("|:---|:---|---:|")

    for case in report["meta"]["cases"]:
        primary = results[label_a][case]
        if label_b:
            ref = results[label_b][case]
            lines.append(
                f"| {case} | {format_ms(primary['median_ms'])} | "
                f"{format_ms(ref['median_ms'])} | {ratio_text(primary['median_ms'], ref['median_ms'])} |"
            )
        else:
            lines.append(f"| {case} | {primary['status']} | {format_ms(primary['median_ms'])} |")

    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run AgentBench workloads.")
    parser.add_argument("--engine", default=default_agentjs(), help="primary engine executable")
    parser.add_argument("--subcommand", default="jetstream", help="primary engine subcommand; use '' for none")
    parser.add_argument("--label", default="agentjs-agentbench", help="primary label")
    parser.add_argument("--ref-engine", default=None, help="reference engine executable, e.g. boa")
    parser.add_argument("--ref-subcommand", default="", help="reference engine subcommand")
    parser.add_argument("--ref-label", default=None, help="reference label")
    parser.add_argument("--cases", default=None, help="comma-separated case names without .js")
    parser.add_argument("--repeat", type=int, default=3, help="runs per case")
    parser.add_argument("--timeout", type=int, default=120, help="seconds per run")
    parser.add_argument("--out-json", default=None, help="JSON output path")
    parser.add_argument("--out-md", default=None, help="Markdown output path")
    args = parser.parse_args()

    case_names = split_csv(args.cases)
    if not case_names:
        case_names = [path.stem for path in sorted(CASES_DIR.glob("*.js"))]
    if not case_names:
        raise SystemExit(f"no AgentBench cases found in {CASES_DIR}")

    command_a = build_cmd(args.engine, args.subcommand)
    label_a = args.label

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_json = Path(args.out_json) if args.out_json else RESULTS_DIR / f"{label_a}.json"
    out_md = Path(args.out_md) if args.out_md else RESULTS_DIR / f"{label_a}.md"

    print(f"\n{'=' * 60}")
    print(f"AgentBench primary: {' '.join(command_a)} [{label_a}]")
    print(f"Cases: {len(case_names)}  Repeat: {args.repeat}x  Timeout: {args.timeout}s")
    print(f"{'=' * 60}\n")

    all_results: dict[str, dict] = {label_a: {}}
    for case in case_names:
        case_path = CASES_DIR / f"{case}.js"
        if not case_path.exists():
            raise SystemExit(f"case not found: {case_path}")
        result = run_case(command_a, case_path, args.repeat, args.timeout)
        all_results[label_a][case] = result
        print(f"  [{result['status'].upper():7}] {case:<34} {format_ms(result['median_ms'])}")
        if result["status"] != "pass" and result["error"]:
            print(f"          {result['error'].splitlines()[0][:120]}")

    label_b = None
    if args.ref_engine:
        label_b = args.ref_label or Path(args.ref_engine).stem
        command_b = build_cmd(args.ref_engine, args.ref_subcommand)
        all_results[label_b] = {}

        print(f"\n{'=' * 60}")
        print(f"AgentBench reference: {' '.join(command_b)} [{label_b}]")
        print(f"{'=' * 60}\n")

        for case in case_names:
            result = run_case(command_b, CASES_DIR / f"{case}.js", args.repeat, args.timeout)
            all_results[label_b][case] = result
            print(f"  [{result['status'].upper():7}] {case:<34} {format_ms(result['median_ms'])}")

    print(f"\n{'=' * 60}")
    passed = sum(1 for result in all_results[label_a].values() if result["status"] == "pass")
    print(f"RESULT {label_a}: {passed}/{len(case_names)} passed")
    if label_b:
        total_a = sum(
            result["median_ms"]
            for result in all_results[label_a].values()
            if result["status"] == "pass" and result["median_ms"] is not None
        )
        total_b = sum(
            result["median_ms"]
            for result in all_results[label_b].values()
            if result["status"] == "pass" and result["median_ms"] is not None
        )
        print(f"PERF   {label_a} total={format_ms(total_a)}  {label_b} total={format_ms(total_b)}  ratio={total_a / total_b:.2f}x")
    print(f"{'=' * 60}\n")

    report = {
        "meta": {
            "label_a": label_a,
            "label_b": label_b,
            "engine_a": args.engine,
            "engine_b": args.ref_engine,
            "repeat": args.repeat,
            "timeout": args.timeout,
            "cases": case_names,
        },
        "results": all_results,
    }
    try:
        out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        write_markdown(out_md, report)
        print(f"JSON -> {out_json}")
        print(f"MD   -> {out_md}")
    except PermissionError as error:
        print(f"WARNING: could not write result files: {error}")
    return 0 if passed == len(case_names) else 1


if __name__ == "__main__":
    raise SystemExit(main())
