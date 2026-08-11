#!/usr/bin/env python3
"""AgentBench 2.0: deterministic workloads for short Agent JS actions.

The runner deliberately separates cold process cost from in-process batch
throughput.  A case is eligible for a performance comparison only when it
exits successfully; every case also contains its own deterministic result
assertion so a fast-but-wrong engine cannot win.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from statistics import median
from typing import Any

try:
    import psutil  # type: ignore
except ImportError:  # pragma: no cover - optional measurement dependency
    psutil = None


def process_peak_rss(process: subprocess.Popen[str], ps_process: Any) -> int | None:
    if ps_process:
        try:
            return int(ps_process.memory_info().rss)
        except (psutil.Error, OSError):
            return None
    if os.name != "nt":
        return None
    # Windows keeps peak working-set counters on the process handle, so RSS is
    # available without adding psutil as a mandatory benchmark dependency.
    try:
        import ctypes
        from ctypes import wintypes

        class ProcessMemoryCounters(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
            ]

        counters = ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        ok = ctypes.windll.psapi.GetProcessMemoryInfo(
            wintypes.HANDLE(int(process._handle)),
            ctypes.byref(counters),
            counters.cb,
        )
        return int(counters.PeakWorkingSetSize) if ok else None
    except (AttributeError, OSError, TypeError, ValueError):
        return None


ROOT = Path(__file__).resolve().parents[2]
BENCH_ROOT = Path(__file__).resolve().parent
CASES_DIR = BENCH_ROOT / "cases"
RESULTS_DIR = BENCH_ROOT / "results"
MANIFEST_PATH = BENCH_ROOT / "manifest.json"


def load_manifest() -> dict[str, Any]:
    with MANIFEST_PATH.open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("version") != "2.0":
        raise SystemExit(f"unsupported AgentBench manifest: {MANIFEST_PATH}")
    return manifest


def default_agentjs() -> str:
    suffix = ".exe" if os.name == "nt" else ""
    release = ROOT / "target" / "release" / f"agentjs{suffix}"
    debug = ROOT / "target" / "debug" / f"agentjs{suffix}"
    return str(release if release.exists() else debug)


def split_csv(value: str | None) -> list[str]:
    return [part.strip() for part in (value or "").split(",") if part.strip()]


def format_ms(value: float | None) -> str:
    if value is None:
        return "-"
    if value >= 10_000:
        return f"{value / 1000:.2f}s"
    return f"{value:.1f}ms"


def format_mb(value: float | None) -> str:
    return "-" if value is None else f"{value:.2f}MiB"


def geomean(values: list[float]) -> float | None:
    positive = [value for value in values if value > 0 and math.isfinite(value)]
    if not positive:
        return None
    return math.exp(sum(math.log(value) for value in positive) / len(positive))


def format_ratio(value: float | None) -> str:
    return "-" if value is None else f"{value:.3f}"


def split_command(command: str) -> list[str]:
    """Split an engine command while preserving Windows path separators."""
    parts = shlex.split(command, posix=False)
    return [
        part[1:-1] if len(part) >= 2 and part[0] == part[-1] and part[0] in {'"', "'"} else part
        for part in parts
    ]


def resolve_command(command: str) -> str | None:
    parts = split_command(command)
    if not parts:
        return None
    path = Path(parts[0])
    if path.exists():
        return str(path.resolve())
    return shutil.which(parts[0])


def file_fingerprint(command: str) -> str | None:
    resolved = resolve_command(command)
    if not resolved:
        return None
    try:
        digest = hashlib.sha256()
        with open(resolved, "rb") as handle:
            for block in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(block)
        return digest.hexdigest()
    except OSError:
        return None


def file_size(command: str) -> int | None:
    resolved = resolve_command(command)
    if not resolved:
        return None
    try:
        return Path(resolved).stat().st_size
    except OSError:
        return None


def build_cmd(engine: str, subcommand: str) -> list[str]:
    command = split_command(engine)
    if subcommand:
        command.append(subcommand)
    return command


def make_batch_source(case_path: Path, repeat: int) -> Path:
    source = case_path.read_text(encoding="utf-8")
    # Cases are top-level scripts using var declarations.  Wrapping each
    # iteration in a function gives every batch invocation fresh local state.
    wrapped = (
        "function __agentbench_case() {\n"
        + source
        + "\n}\n"
        + f"for (var __agentbench_i = 0; __agentbench_i < {repeat}; __agentbench_i++) {{\n"
        + "  __agentbench_case();\n}\n"
    )
    handle = tempfile.NamedTemporaryFile(
        mode="w", suffix=".agentbench-batch.js", encoding="utf-8", delete=False
    )
    try:
        handle.write(wrapped)
        return Path(handle.name)
    finally:
        handle.close()


def run_once(
    command: list[str],
    case_path: Path,
    timeout: int,
    mode: str,
    batch_repeat: int,
) -> dict[str, Any]:
    run_path = case_path
    temporary_path: Path | None = None
    if mode == "batch":
        temporary_path = make_batch_source(case_path, batch_repeat)
        run_path = temporary_path

    started = time.perf_counter()
    process: subprocess.Popen[str] | None = None
    peak_rss: int | None = None
    try:
        process = subprocess.Popen(
            command + [str(run_path)],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        ps_process = psutil.Process(process.pid) if psutil else None
        deadline = started + timeout
        while process.poll() is None:
            sampled_rss = process_peak_rss(process, ps_process)
            if sampled_rss is not None:
                peak_rss = max(peak_rss or 0, sampled_rss)
            if time.perf_counter() >= deadline:
                process.kill()
                stdout, stderr = process.communicate()
                elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
                return {
                    "status": "timeout",
                    "time_ms": elapsed_ms,
                    "peak_rss_mb": round(peak_rss / 1024 / 1024, 2) if peak_rss else None,
                    "stderr": (stderr + stdout).strip()[-500:],
                }
            time.sleep(0.005)
        stdout, stderr = process.communicate()
        sampled_rss = process_peak_rss(process, ps_process)
        if sampled_rss is not None:
            peak_rss = max(peak_rss or 0, sampled_rss)
        elapsed_ms = round((time.perf_counter() - started) * 1000, 1)
        if process.returncode == 0:
            return {
                "status": "pass",
                "time_ms": elapsed_ms,
                "peak_rss_mb": round(peak_rss / 1024 / 1024, 2) if peak_rss else None,
                "stderr": stderr.strip()[-500:],
            }
        return {
            "status": "runtime-error",
            "time_ms": elapsed_ms,
            "peak_rss_mb": round(peak_rss / 1024 / 1024, 2) if peak_rss else None,
            "stderr": (stderr + stdout).strip()[-500:],
        }
    except OSError as error:
        return {"status": "spawn-error", "time_ms": 0.0, "peak_rss_mb": None, "stderr": str(error)}
    finally:
        if temporary_path:
            temporary_path.unlink(missing_ok=True)


def summarize_runs(runs: list[dict[str, Any]], mode: str, batch_repeat: int) -> dict[str, Any]:
    pass_times = [run["time_ms"] for run in runs if run["status"] == "pass"]
    rss_values = [run["peak_rss_mb"] for run in runs if run.get("peak_rss_mb") is not None]
    if not pass_times:
        return {
            "status": runs[0]["status"],
            "median_ms": None,
            "p90_ms": None,
            "p95_ms": None,
            "min_ms": None,
            "max_ms": None,
            "tasks_per_second": None,
            "peak_rss_mb": max(rss_values) if rss_values else None,
            "runs": runs,
            "error": runs[0].get("stderr", ""),
        }
    ordered = sorted(pass_times)

    def percentile(percent: float) -> float:
        index = min(len(ordered) - 1, math.ceil(percent * len(ordered)) - 1)
        return ordered[index]

    median_ms = round(median(pass_times), 1)
    effective_tasks = batch_repeat if mode == "batch" else 1
    return {
        "status": "pass",
        "median_ms": median_ms,
        "p90_ms": round(percentile(0.90), 1),
        "p95_ms": round(percentile(0.95), 1),
        "min_ms": round(min(pass_times), 1),
        "max_ms": round(max(pass_times), 1),
        "tasks_per_second": round(effective_tasks * 1000 / median_ms, 2) if median_ms else None,
        "peak_rss_mb": round(max(rss_values), 2) if rss_values else None,
        "runs": runs,
        "error": "",
    }


def run_case(
    command: list[str],
    case_path: Path,
    warmup: int,
    repeat: int,
    timeout: int,
    mode: str,
    batch_repeat: int,
) -> dict[str, Any]:
    for _ in range(warmup):
        warm = run_once(command, case_path, timeout, mode, batch_repeat)
        if warm["status"] != "pass":
            break
    runs: list[dict[str, Any]] = []
    for _ in range(repeat):
        result = run_once(command, case_path, timeout, mode, batch_repeat)
        runs.append(result)
        if result["status"] != "pass":
            break
    return summarize_runs(runs, mode, batch_repeat)


def parse_references(values: list[str], legacy_engine: str | None, legacy_label: str | None) -> list[tuple[str, str]]:
    references: list[tuple[str, str]] = []
    if legacy_engine:
        references.append((legacy_label or Path(legacy_engine).stem, legacy_engine))
    for value in values:
        if "=" not in value:
            raise SystemExit(f"--ref must use LABEL=COMMAND, got: {value}")
        label, command = value.split("=", 1)
        if not label or not command:
            raise SystemExit(f"invalid --ref: {value}")
        references.append((label, command))
    seen: set[str] = set()
    unique: list[tuple[str, str]] = []
    for label, command in references:
        if label not in seen:
            unique.append((label, command))
            seen.add(label)
    return unique


def environment(primary: str, references: list[tuple[str, str]], manifest: dict[str, Any]) -> dict[str, Any]:
    try:
        rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True, timeout=5).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        rustc = None
    commands = {"primary": primary, "references": {label: command for label, command in references}}
    fingerprints = {"primary": file_fingerprint(primary)}
    fingerprints["references"] = {label: file_fingerprint(command) for label, command in references}
    binary_sizes: dict[str, Any] = {"primary": file_size(primary)}
    binary_sizes["references"] = {label: file_size(command) for label, command in references}
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "python": platform.python_version(),
        "rustc": rustc,
        "manifest_version": manifest["version"],
        "commands": commands,
        "binary_sha256": fingerprints,
        "binary_size_bytes": binary_sizes,
        "memory_sampler": "psutil" if psutil is not None else ("windows-psapi" if os.name == "nt" else "unavailable"),
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def common_speedups(results: dict[str, dict[str, dict[str, Any]]], primary: str, reference: str) -> dict[str, float | None]:
    values: list[float] = []
    by_group: dict[str, list[float]] = {"general": [], "pressure": []}
    for case, primary_result in results[primary].items():
        reference_result = results[reference].get(case, {})
        if primary_result.get("status") != "pass" or reference_result.get("status") != "pass":
            continue
        primary_ms = primary_result.get("median_ms")
        reference_ms = reference_result.get("median_ms")
        if not primary_ms or not reference_ms:
            continue
        ratio = reference_ms / primary_ms
        values.append(ratio)
        group = CASE_METADATA.get(case, {}).get("group", "general")
        by_group.setdefault(group, []).append(ratio)
    return {"all": geomean(values), **{group: geomean(items) for group, items in by_group.items()}}


def common_memory_ratios(results: dict[str, dict[str, dict[str, Any]]], primary: str, reference: str) -> dict[str, float | None]:
    values: list[float] = []
    by_group: dict[str, list[float]] = {"general": [], "pressure": []}
    for case, primary_result in results[primary].items():
        reference_result = results[reference].get(case, {})
        primary_rss = primary_result.get("peak_rss_mb")
        reference_rss = reference_result.get("peak_rss_mb")
        if primary_result.get("status") != "pass" or reference_result.get("status") != "pass":
            continue
        if not primary_rss or not reference_rss:
            continue
        ratio = reference_rss / primary_rss
        values.append(ratio)
        group = CASE_METADATA.get(case, {}).get("group", "general")
        by_group.setdefault(group, []).append(ratio)
    return {"all": geomean(values), **{group: geomean(items) for group, items in by_group.items()}}


CASE_METADATA: dict[str, dict[str, str]] = {}


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    meta = report["meta"]
    labels = [meta["label_a"], *meta["references"]]
    lines = ["# AgentBench 2.0", "", f"- Mode: `{meta['mode']}`", f"- Warmup: `{meta['warmup']}`", f"- Repeat: `{meta['repeat']}`"]
    if meta["mode"] == "batch":
        lines.append(f"- Batch iterations per process: `{meta['batch_repeat']}`")
    lines.extend(["", "`status=pass` is a correctness gate; only common passing cases enter speedup averages.", ""])
    lines.append("| Group | Case | " + " | ".join(f"{label} P50" for label in labels) + " | " + " | ".join(f"{label} RSS" for label in labels) + " |")
    lines.append("|:---|:---|" + "---:|" * len(labels) + "" + "---:|" * len(labels))
    for case in meta["cases"]:
        group = CASE_METADATA.get(case, {}).get("group", "general")
        times = [format_ms(report["results"].get(label, {}).get(case, {}).get("median_ms")) for label in labels]
        rss = [format_mb(report["results"].get(label, {}).get(case, {}).get("peak_rss_mb")) for label in labels]
        lines.append(f"| {group} | {case} | " + " | ".join(times + rss) + " |")
    lines.extend(["", "## Correctness", ""])
    for label in labels:
        passed = sum(1 for result in report["results"][label].values() if result.get("status") == "pass")
        lines.append(f"- `{label}`: {passed}/{len(meta['cases'])} cases passed")
    if meta["references"]:
        lines.extend(["", "## Reference / AgentJS geometric-mean ratio", ""])
        for reference in meta["references"]:
            speedups = report["speedups"][reference]
            lines.append(
                f"- `{reference}`: all={format_ratio(speedups['all'])}x, "
                f"general={format_ratio(speedups['general'])}x, pressure={format_ratio(speedups['pressure'])}x "
                "(>1 means AgentJS is faster)"
            )
        lines.extend(["", "## Reference / AgentJS peak-RSS ratio", ""])
        for reference in meta["references"]:
            ratios = report["memory_ratios"][reference]
            lines.append(
                f"- `{reference}`: all={format_ratio(ratios['all'])}x, "
                f"general={format_ratio(ratios['general'])}x, pressure={format_ratio(ratios['pressure'])}x "
                "(>1 means AgentJS uses less memory)"
            )
    sizes = report["environment"].get("binary_size_bytes", {})
    lines.extend(["", "## Executable size", ""])
    primary_size = sizes.get("primary")
    lines.append(f"- `{meta['label_a']}`: {primary_size} bytes" if primary_size else f"- `{meta['label_a']}`: unavailable")
    for reference in meta["references"]:
        size = sizes.get("references", {}).get(reference)
        lines.append(f"- `{reference}`: {size} bytes" if size else f"- `{reference}`: unavailable")
    lines.extend(["", "## Reproduction", "", f"See `environment-{meta['mode']}.json`/the JSON report for machine, compiler, command and binary fingerprints."])
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_suite(
    *,
    mode: str,
    primary_engine: str,
    primary_label: str,
    references: list[tuple[str, str]],
    cases: list[str],
    warmup: int,
    repeat: int,
    timeout: int,
    batch_repeat: int,
    out_json: Path,
    out_md: Path,
    manifest: dict[str, Any],
) -> int:
    labels = [primary_label, *[label for label, _ in references]]
    commands = {primary_label: build_cmd(primary_engine, "run")}
    commands.update({label: build_cmd(command, "") for label, command in references})
    all_results: dict[str, dict[str, dict[str, Any]]] = {label: {} for label in labels}

    print(f"\n{'=' * 72}")
    print(f"AgentBench 2.0 mode={mode}  cases={len(cases)}  warmup={warmup}  repeat={repeat}")
    if mode == "batch":
        print(f"batch iterations per process={batch_repeat}")
    print(f"{'=' * 72}\n")
    for label in labels:
        print(f"[{label}] {' '.join(commands[label])}")
        for case in cases:
            case_path = CASES_DIR / f"{case}.js"
            result = run_case(commands[label], case_path, warmup, repeat, timeout, mode, batch_repeat)
            all_results[label][case] = result
            print(f"  [{result['status'].upper():12}] {case:<34} {format_ms(result['median_ms'])}")
        print()

    report: dict[str, Any] = {
        "meta": {
            "benchmark": "AgentBench",
            "version": manifest["version"],
            "mode": mode,
            "label_a": primary_label,
            "references": [label for label, _ in references],
            "engine_a": primary_engine,
            "engine_b": {label: command for label, command in references},
            "warmup": warmup,
            "repeat": repeat,
            "timeout": timeout,
            "batch_repeat": batch_repeat,
            "cases": cases,
        },
        "environment": environment(primary_engine, references, manifest),
        "results": all_results,
        "speedups": {},
        "memory_ratios": {},
    }
    for label, _ in references:
        report["speedups"][label] = common_speedups(all_results, primary_label, label)
        report["memory_ratios"][label] = common_memory_ratios(all_results, primary_label, label)
    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    out_md.parent.mkdir(parents=True, exist_ok=True)
    write_markdown(out_md, report)
    environment_text = json.dumps(report["environment"], ensure_ascii=False, indent=2) + "\n"
    # Keep a mode-specific file when --mode both is used.  Also retain the
    # historical environment.json name for scripts that consume one mode.
    environment_path = out_json.with_name(f"environment-{mode}.json")
    environment_path.write_text(environment_text, encoding="utf-8")
    legacy_environment_path = out_json.with_name("environment.json")
    legacy_environment_path.write_text(environment_text, encoding="utf-8")
    print(f"JSON -> {out_json}\nMD   -> {out_md}\nENV  -> {environment_path}")
    primary_passed = sum(1 for result in all_results[primary_label].values() if result["status"] == "pass")
    return 0 if primary_passed == len(cases) else 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Run reproducible AgentBench workloads.")
    parser.add_argument("--engine", default=default_agentjs(), help="primary AgentJS executable")
    parser.add_argument("--label", default="agentjs", help="primary label")
    parser.add_argument("--ref-engine", default=None, help="legacy single reference executable")
    parser.add_argument("--ref-label", default=None, help="legacy reference label")
    parser.add_argument("--ref", action="append", default=[], metavar="LABEL=COMMAND", help="repeatable reference, e.g. --ref boa=boa.exe --ref node=node")
    parser.add_argument("--cases", default=None, help="comma-separated case names")
    parser.add_argument("--group", choices=["all", "general", "pressure"], default="all")
    parser.add_argument("--mode", choices=["cold", "batch", "both"], default="cold")
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--batch-repeat", type=int, default=5)
    parser.add_argument("--timeout", type=int, default=120, help="seconds per process")
    parser.add_argument("--out-dir", default=None, help="directory for JSON/Markdown/environment outputs")
    args = parser.parse_args()

    global CASE_METADATA
    manifest = load_manifest()
    CASE_METADATA = manifest["cases"]
    selected = split_csv(args.cases)
    if not selected:
        selected = sorted(
            case for case, metadata in CASE_METADATA.items()
            if args.group == "all" or metadata.get("group") == args.group
        )
    missing = [case for case in selected if not (CASES_DIR / f"{case}.js").exists()]
    if missing:
        raise SystemExit(f"case not found: {', '.join(missing)}")
    references = parse_references(args.ref, args.ref_engine, args.ref_label)
    out_dir = Path(args.out_dir) if args.out_dir else RESULTS_DIR
    modes = ["cold", "batch"] if args.mode == "both" else [args.mode]
    exit_code = 0
    for mode in modes:
        suffix = f"-{mode}" if len(modes) > 1 else ""
        exit_code |= run_suite(
            mode=mode,
            primary_engine=args.engine,
            primary_label=args.label,
            references=references,
            cases=selected,
            warmup=max(0, args.warmup),
            repeat=max(1, args.repeat),
            timeout=max(1, args.timeout),
            batch_repeat=max(1, args.batch_repeat),
            out_json=out_dir / f"{args.label}{suffix}.json",
            out_md=out_dir / f"{args.label}{suffix}.md",
            manifest=manifest,
        )
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
