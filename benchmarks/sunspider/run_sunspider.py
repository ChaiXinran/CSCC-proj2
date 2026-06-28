#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SunSpider standalone runner for AgentJS (and comparison engines).

Usage:
  python benchmarks/sunspider/run_sunspider.py --engine ./target/release/agentjs.exe --label agentjs
  python benchmarks/sunspider/run_sunspider.py --engine node --label node --subcommand ""

By default runs sunspider-1.0.2 with 3 repeats, 60s timeout.
"""

import argparse
import io
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from statistics import median

# Force UTF-8 line-buffered output on Windows
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8",
                              errors="replace", line_buffering=True)
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8",
                              errors="replace", line_buffering=True)

TESTS_DIR = Path(__file__).parent / "webkit-sunspider" / "PerformanceTests" / "SunSpider" / "tests" / "sunspider-1.0.2"

# Cases grouped by category (for table display)
CATEGORIES = {
    "3d":           ["3d-cube", "3d-morph", "3d-raytrace"],
    "access":       ["access-binary-trees", "access-fannkuch", "access-nbody", "access-nsieve"],
    "bitops":       ["bitops-3bit-bits-in-byte", "bitops-bits-in-byte", "bitops-bitwise-and", "bitops-nsieve-bits"],
    "controlflow":  ["controlflow-recursive"],
    "crypto":       ["crypto-aes", "crypto-md5", "crypto-sha1"],
    "date":         ["date-format-tofte", "date-format-xparb"],
    "math":         ["math-cordic", "math-partial-sums", "math-spectral-norm"],
    "regexp":       ["regexp-dna"],
    "string":       ["string-base64", "string-fasta", "string-tagcloud", "string-unpack-code", "string-validate-input"],
}

STATUS_EMOJI = {
    "pass":          "[PASS]",
    "wrong-result":  "[WRONG]",
    "runtime-error": "[ERROR]",
    "timeout":       "[TIMEOUT]",
    "skip":          "[SKIP]",
}


def classify_error(returncode: int, stderr: str, stdout: str) -> str:
    """Determine failure type from process output."""
    combined = (stderr + stdout).lower()
    if "bad result" in combined or "wrong result" in combined:
        return "wrong-result"
    if "stack overflow" in combined or "call stack" in combined:
        return "runtime-error"
    return "runtime-error"


def run_once(cmd: list[str], js_path: Path, timeout: int) -> dict:
    """Run one case once, return result dict."""
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd + [str(js_path)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
        )
        elapsed_ms = (time.perf_counter() - t0) * 1000
        stdout = proc.stdout
        stderr = proc.stderr

        if proc.returncode != 0:
            status = classify_error(proc.returncode, stderr, stdout)
            return {
                "status": status,
                "time_ms": round(elapsed_ms, 1),
                "stderr": (stderr + stdout).strip()[-300:],
            }
        return {
            "status": "pass",
            "time_ms": round(elapsed_ms, 1),
            "stderr": "",
        }
    except subprocess.TimeoutExpired:
        elapsed_ms = (time.perf_counter() - t0) * 1000
        return {
            "status": "timeout",
            "time_ms": round(elapsed_ms, 1),
            "stderr": f"timed out after {timeout}s",
        }


def run_case(cmd: list[str], js_path: Path, repeat: int, timeout: int) -> dict:
    """Run one case `repeat` times, aggregate results."""
    runs = []
    for _ in range(repeat):
        r = run_once(cmd, js_path, timeout)
        runs.append(r)
        # Stop early on definitive failure (wrong result or runtime error)
        if r["status"] in ("wrong-result", "runtime-error"):
            break

    pass_times = [r["time_ms"] for r in runs if r["status"] == "pass"]
    if pass_times:
        final_status = "pass"
        med_ms = round(median(pass_times), 1)
    else:
        final_status = runs[0]["status"]
        med_ms = None

    return {
        "status":    final_status,
        "median_ms": med_ms,
        "runs":      runs,
        "error":     runs[0].get("stderr", "") if final_status != "pass" else "",
    }


def format_ms(ms) -> str:
    if ms is None:
        return "—"
    if ms >= 10_000:
        return f"{ms/1000:.1f}s"
    return f"{ms:.0f}ms"


def speedup(agentjs_ms, ref_ms) -> str:
    if agentjs_ms is None or ref_ms is None or ref_ms == 0:
        return "—"
    ratio = agentjs_ms / ref_ms
    if ratio < 1:
        return f"{1/ratio:.1f}× faster"
    return f"{ratio:.1f}×"


def generate_markdown(all_results: dict[str, dict], label_a: str, label_b: str | None) -> str:
    lines = []
    lines.append(f"# SunSpider 1.0.2 — {label_a}" +
                 (f" vs {label_b}" if label_b else "") + "\n")

    # ── Correctness summary ─────────────────────────────────────────────────
    lines.append("## Correctness Summary\n")
    header = ["Category", "Cases", "Pass", "Wrong", "Error", "Timeout"]
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "|".join([":---"] + ["---:"] * (len(header)-1)) + "|")

    total_pass = total_wrong = total_error = total_timeout = 0
    for cat, cases in CATEGORIES.items():
        res_a = all_results.get(label_a, {})
        p = w = e = t = 0
        for c in cases:
            s = res_a.get(c, {}).get("status", "skip")
            if s == "pass":          p += 1
            elif s == "wrong-result":  w += 1
            elif s == "timeout":       t += 1
            elif s != "skip":          e += 1
        lines.append(f"| {cat} | {len(cases)} | {p or '—'} | {w or '—'} | {e or '—'} | {t or '—'} |")
        total_pass += p; total_wrong += w; total_error += e; total_timeout += t

    total = sum(len(v) for v in CATEGORIES.values())
    lines.append(f"| **Total** | **{total}** | **{total_pass}** | "
                 f"**{total_wrong}** | **{total_error}** | **{total_timeout}** |\n")

    # ── Per-case performance table ──────────────────────────────────────────
    lines.append("## Per-Case Results\n")
    if label_b:
        header2 = ["Case", f"{label_a} status", f"{label_a} median",
                   f"{label_b} median", f"{label_a}/{label_b}"]
    else:
        header2 = ["Case", "Status", "Median time"]
    lines.append("| " + " | ".join(header2) + " |")
    lines.append("|" + "|".join([":---"] + ["---:"] * (len(header2)-1)) + "|")

    for cases in CATEGORIES.values():
        for c in cases:
            ra = all_results.get(label_a, {}).get(c, {})
            status_a = ra.get("status", "skip")
            emoji_a  = STATUS_EMOJI.get(status_a, "?")
            ms_a     = ra.get("median_ms")

            if label_b:
                rb   = all_results.get(label_b, {}).get(c, {})
                ms_b = rb.get("median_ms")
                sp   = speedup(ms_a, ms_b)
                lines.append(f"| {c} | {emoji_a} {status_a} | {format_ms(ms_a)} "
                              f"| {format_ms(ms_b)} | {sp} |")
            else:
                lines.append(f"| {c} | {emoji_a} {status_a} | {format_ms(ms_a)} |")

    lines.append("")

    # ── Failure detail ──────────────────────────────────────────────────────
    failures = {c: r for c, r in all_results.get(label_a, {}).items()
                if r.get("status") != "pass"}
    if failures:
        lines.append("## Failure Details\n")
        lines.append("| Case | Status | Error |")
        lines.append("|:---|:---|:---|")
        for c, r in sorted(failures.items()):
            err = r.get("error", "").replace("\n", " ").replace("|", "\\|")[:120]
            lines.append(f"| {c} | {r['status']} | `{err}` |")
        lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="SunSpider runner for AgentJS")
    parser.add_argument("--engine",   required=True,
                        help="Path to engine binary (e.g. ./target/release/agentjs.exe or node)")
    parser.add_argument("--label",    default=None,
                        help="Label for this engine in output (default: basename of --engine)")
    parser.add_argument("--subcommand", default="jetstream",
                        help="Subcommand to prepend (default: 'jetstream'; use '' for node)")
    parser.add_argument("--ref-engine", default=None,
                        help="Reference engine for comparison (e.g. node)")
    parser.add_argument("--ref-label",  default=None,
                        help="Label for the reference engine")
    parser.add_argument("--ref-subcommand", default="",
                        help="Subcommand for reference engine (default: '' = none)")
    parser.add_argument("--tests",   default=str(TESTS_DIR),
                        help="Path to sunspider-1.0.2 directory")
    parser.add_argument("--cases",   default=None,
                        help="Comma-separated list of cases to run (default: all)")
    parser.add_argument("--repeat",  type=int, default=3,
                        help="Runs per case (default: 3)")
    parser.add_argument("--timeout", type=int, default=60,
                        help="Timeout in seconds per run (default: 60)")
    parser.add_argument("--out-json", default=None,
                        help="Output JSON file (default: results/<label>-sunspider.json)")
    parser.add_argument("--out-md",   default=None,
                        help="Output Markdown file (default: results/<label>-sunspider.md)")
    args = parser.parse_args()

    tests_dir = Path(args.tests)
    if not tests_dir.exists():
        sys.exit(f"ERROR: tests directory not found: {tests_dir}")

    label_a = args.label or Path(args.engine).stem
    results_dir = Path(__file__).parent / "results"
    results_dir.mkdir(exist_ok=True)

    out_json = Path(args.out_json) if args.out_json else results_dir / f"{label_a}-sunspider.json"
    out_md   = Path(args.out_md)   if args.out_md   else results_dir / f"{label_a}-sunspider.md"

    # Build engine command
    def build_cmd(engine: str, subcommand: str) -> list[str]:
        cmd = [engine]
        if subcommand:
            cmd.append(subcommand)
        return cmd

    cmd_a = build_cmd(args.engine, args.subcommand)

    # Select cases
    all_cases = []
    if args.cases:
        all_cases = [c.strip() for c in args.cases.split(",")]
    else:
        for js in sorted(tests_dir.glob("*.js")):
            all_cases.append(js.stem)

    print(f"\n{'='*60}")
    print(f"Engine : {' '.join(cmd_a)}")
    print(f"Label  : {label_a}")
    print(f"Cases  : {len(all_cases)}")
    print(f"Repeat : {args.repeat}×  Timeout: {args.timeout}s")
    print(f"{'='*60}\n")

    # Run primary engine
    all_results: dict[str, dict] = {label_a: {}}
    for case in all_cases:
        js_path = tests_dir / f"{case}.js"
        if not js_path.exists():
            print(f"  SKIP  {case}  (file not found)")
            all_results[label_a][case] = {"status": "skip", "median_ms": None}
            continue

        result = run_case(cmd_a, js_path, args.repeat, args.timeout)
        all_results[label_a][case] = result

        emoji  = STATUS_EMOJI.get(result["status"], "?")
        ms_str = format_ms(result["median_ms"])
        print(f"  {emoji}  {case:<35} {ms_str}")
        if result["status"] != "pass" and result.get("error"):
            snippet = result["error"].splitlines()[0][:100]
            print(f"       └─ {snippet}")

    # Run reference engine if provided
    label_b = None
    if args.ref_engine:
        label_b = args.ref_label or Path(args.ref_engine).stem
        cmd_b   = build_cmd(args.ref_engine, args.ref_subcommand)
        all_results[label_b] = {}

        print(f"\n{'='*60}")
        print(f"Reference: {' '.join(cmd_b)}  [{label_b}]")
        print(f"{'='*60}\n")

        for case in all_cases:
            js_path = tests_dir / f"{case}.js"
            if not js_path.exists():
                all_results[label_b][case] = {"status": "skip", "median_ms": None}
                continue
            result = run_case(cmd_b, js_path, args.repeat, args.timeout)
            all_results[label_b][case] = result
            emoji  = STATUS_EMOJI.get(result["status"], "?")
            ms_str = format_ms(result["median_ms"])
            print(f"  {emoji}  {case:<35} {ms_str}")

    # Summary stats
    res_a   = all_results[label_a]
    n_pass  = sum(1 for r in res_a.values() if r.get("status") == "pass")
    n_total = len([c for c in all_cases if (tests_dir / f"{c}.js").exists()])
    print(f"\n{'='*60}")
    print(f"RESULT  {label_a}: {n_pass}/{n_total} passed")
    if label_b:
        res_b = all_results[label_b]
        pass_times_a = [r["median_ms"] for r in res_a.values()
                        if r.get("status") == "pass" and r.get("median_ms") is not None]
        pass_times_b = [res_b.get(c, {}).get("median_ms")
                        for c, r in res_a.items()
                        if r.get("status") == "pass" and res_b.get(c, {}).get("status") == "pass"
                        and res_b.get(c, {}).get("median_ms") is not None]
        if pass_times_a and pass_times_b:
            total_a = sum(pass_times_a)
            total_b = sum(pass_times_b)
            print(f"PERF    {label_a} total={format_ms(total_a)}  "
                  f"{label_b} total={format_ms(total_b)}  "
                  f"ratio={total_a/total_b:.1f}×")
    print(f"{'='*60}\n")

    # Write JSON
    out_data = {
        "meta": {
            "label_a": label_a,
            "label_b": label_b,
            "engine_a": args.engine,
            "engine_b": args.ref_engine,
            "repeat": args.repeat,
            "timeout": args.timeout,
            "tests_dir": str(tests_dir),
        },
        "results": all_results,
    }
    out_json.write_text(json.dumps(out_data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"JSON → {out_json}")

    # Write Markdown
    md = generate_markdown(all_results, label_a, label_b)
    out_md.write_text(md, encoding="utf-8")
    print(f"MD   → {out_md}\n")


if __name__ == "__main__":
    main()
