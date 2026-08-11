#!/usr/bin/env python3
"""Run portable JetStream workload kernels across four JavaScript engines."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import re
import statistics
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

try:
    import psutil  # type: ignore
except ImportError:  # pragma: no cover
    psutil = None


ROOT = Path(__file__).resolve().parents[2]
JETSTREAM_ROOT = ROOT / "benchmarks" / "JetStream2"
GENERATOR = ROOT / "scripts" / "prepare-simple-benchmark.mjs"
RUNNER_ROOT = ROOT / "target" / "jetstream2-four-engine" / "runners"
DEFAULT_RESULTS = ROOT / "benchmarks" / "jetstream" / "results" / "four-engine"
DEFAULT_TESTS = [
    "ai-astar",
    "crypto",
    "gaussian-blur",
    "hash-map",
    "cdjs",
    "navier-stokes",
    "raytrace",
    "richards",
    "splay",
    "stanford-crypto-sha256",
]


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def geomean(values: list[float]) -> float | None:
    positive = [value for value in values if value > 0]
    if not positive:
        return None
    return math.exp(sum(math.log(value) for value in positive) / len(positive))


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def generate_runner(test: str, iterations: int) -> Path:
    RUNNER_ROOT.mkdir(parents=True, exist_ok=True)
    output = RUNNER_ROOT / f"{test}.js"
    completed = subprocess.run(
        [
            "node",
            str(GENERATOR),
            str(JETSTREAM_ROOT),
            test,
            str(iterations),
            str(output),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=60,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr or completed.stdout)
    return output


def engine_commands(runner: Path) -> dict[str, list[str]]:
    return {
        "agentjs": [str(ROOT / "target" / "release" / "agentjs.exe"), "run", str(runner)],
        "boa": [
            str(ROOT / "boa" / "target" / "release" / "boa.exe"),
            str(ROOT / "reports" / "jetstream2-2026-08-04" / "boa-visible-prelude.js"),
            str(runner),
        ],
        "quickjs": [str(ROOT / "quickjs" / "qjs.exe"), str(runner)],
        "oxide": [
            str(ROOT / "target" / "oxide-compare" / "release" / "oxide.exe"),
            "run",
            str(runner),
        ],
    }


def run_once(
    command: list[str], test: str, timeout_seconds: int, max_rss_mb: int
) -> dict[str, Any]:
    started = time.perf_counter()
    process: subprocess.Popen[str] | None = None
    peak_rss = 0
    try:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        sampled = psutil.Process(process.pid) if psutil else None
        deadline = started + timeout_seconds
        status_override: str | None = None
        while process.poll() is None:
            if sampled:
                try:
                    peak_rss = max(peak_rss, sampled.memory_info().rss)
                except (psutil.Error, OSError):
                    pass
            if peak_rss > max_rss_mb * 1024 * 1024:
                status_override = "memory-limit"
                process.kill()
                break
            if time.perf_counter() >= deadline:
                status_override = "timeout"
                process.kill()
                break
            time.sleep(0.01)
        stdout, stderr = process.communicate()
        elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
        combined = (stdout + "\n" + stderr).strip()
        marker = re.search(
            rf"{re.escape(test)} avg:\s+([0-9]+(?:\.[0-9]+)?)ms", combined
        )
        if status_override:
            status = status_override
        elif process.returncode != 0:
            status = "runtime-error"
        elif not marker:
            status = "incomplete"
        else:
            status = "pass"
        return {
            "status": status,
            "wall_time_ms": elapsed_ms,
            "workload_avg_ms": float(marker.group(1)) if marker else None,
            "peak_rss_mb": round(peak_rss / 1024 / 1024, 2) if peak_rss else None,
            "exit_code": process.returncode,
            "output_tail": combined[-1000:],
        }
    except OSError as error:
        return {
            "status": "spawn-error",
            "wall_time_ms": 0.0,
            "workload_avg_ms": None,
            "peak_rss_mb": None,
            "exit_code": None,
            "output_tail": str(error),
        }
    finally:
        if process and process.poll() is None:
            process.kill()


def summarize(samples: list[dict[str, Any]]) -> dict[str, Any]:
    passing = [sample for sample in samples if sample["status"] == "pass"]
    if not passing:
        first = samples[0]
        return {
            "status": first["status"],
            "passes": 0,
            "median_workload_ms": None,
            "median_wall_ms": None,
            "p90_wall_ms": None,
            "peak_rss_mb": first.get("peak_rss_mb"),
            "error": first.get("output_tail", ""),
            "samples": samples,
        }
    workloads = [sample["workload_avg_ms"] for sample in passing]
    walls = [sample["wall_time_ms"] for sample in passing]
    rss = [sample["peak_rss_mb"] for sample in passing if sample["peak_rss_mb"]]
    return {
        "status": "pass" if len(passing) == len(samples) else "partial",
        "passes": len(passing),
        "median_workload_ms": round(statistics.median(workloads), 3),
        "median_wall_ms": round(statistics.median(walls), 3),
        "p90_wall_ms": round(percentile(walls, 0.9), 3),
        "peak_rss_mb": round(max(rss), 2) if rss else None,
        "error": "",
        "samples": samples,
    }


def format_ms(value: float | None) -> str:
    if value is None:
        return "-"
    if value >= 1000:
        return f"{value / 1000:.2f}s"
    return f"{value:.1f}ms"


def write_markdown(path: Path, report: dict[str, Any]) -> None:
    meta = report["meta"]
    labels = meta["engines"]
    lines = [
        "# JetStream2 four-engine kernel comparison",
        "",
        f"- Workload iterations per process: `{meta['iterations']}`",
        f"- Warmup processes: `{meta['warmup']}`",
        f"- Measured processes: `{meta['repeat']}`",
        f"- Timeout: `{meta['timeout_seconds']}s`",
        f"- Peak RSS limit: `{meta['max_rss_mb']} MiB`",
        "",
        "This is a portable comparison of JetStream2 JavaScript workload kernels, not the browser suite's official composite score.",
        "Only cases that print their deterministic completion summary are performance-eligible.",
        "",
        "| Test | " + " | ".join(f"{label} kernel P50" for label in labels) + " |",
        "|:---|" + "|".join("---:" for _ in labels) + "|",
    ]
    for test in meta["tests"]:
        cells = []
        for label in labels:
            result = report["results"][label][test]
            cells.append(
                format_ms(result["median_workload_ms"])
                if result["status"] == "pass"
                else result["status"].upper()
            )
        lines.append(f"| {test} | " + " | ".join(cells) + " |")
    lines.extend(["", "## Correctness", ""])
    for label in labels:
        passed = sum(
            report["results"][label][test]["status"] == "pass"
            for test in meta["tests"]
        )
        lines.append(f"- `{label}`: {passed}/{len(meta['tests'])} passed")
    lines.extend(["", "## Reference / AgentJS kernel-time ratio", ""])
    for label, ratio in report["geomean_ratios"].items():
        rendered = "-" if ratio is None else f"{ratio:.3f}x"
        lines.append(f"- `{label}`: {rendered} (>1 means AgentJS is faster)")
    lines.extend(["", "## Peak RSS", ""])
    for label in labels:
        values = [
            report["results"][label][test]["peak_rss_mb"]
            for test in meta["tests"]
            if report["results"][label][test]["peak_rss_mb"] is not None
        ]
        rendered = "-" if not values else f"{max(values):.2f} MiB"
        lines.append(f"- `{label}` maximum observed: {rendered}")
    lines.extend(
        [
            "",
            "## Reproduction",
            "",
            "See `environment.json` and the JSON report for commands, revisions, executable fingerprints, and all samples.",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tests", default=",".join(DEFAULT_TESTS))
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--warmup", type=int, default=0)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--max-rss-mb", type=int, default=1536)
    parser.add_argument("--out-dir", default=str(DEFAULT_RESULTS))
    args = parser.parse_args()

    tests = [item.strip() for item in args.tests.split(",") if item.strip()]
    iterations = max(1, args.iterations)
    warmup = max(0, args.warmup)
    repeat = max(1, args.repeat)
    output = Path(args.out_dir)
    output.mkdir(parents=True, exist_ok=True)

    runners = {test: generate_runner(test, iterations) for test in tests}
    command_templates = engine_commands(next(iter(runners.values())))
    labels = list(command_templates)
    results: dict[str, dict[str, dict[str, Any]]] = {label: {} for label in labels}

    for label in labels:
        print(f"\n[{label}]")
        for test in tests:
            command = engine_commands(runners[test])[label]
            warmup_failure: dict[str, Any] | None = None
            for _ in range(warmup):
                warm = run_once(command, test, args.timeout, args.max_rss_mb)
                if warm["status"] != "pass":
                    warmup_failure = warm
                    break
            samples = (
                [warmup_failure]
                if warmup_failure
                else [
                    run_once(command, test, args.timeout, args.max_rss_mb)
                    for _ in range(repeat)
                ]
            )
            result = summarize(samples)
            results[label][test] = result
            value = format_ms(result["median_workload_ms"])
            print(f"  {test:<28} {result['status']:<12} {value:>10}")

    ratios: dict[str, float | None] = {}
    for label in labels[1:]:
        values = []
        for test in tests:
            primary = results["agentjs"][test]
            reference = results[label][test]
            if primary["status"] == reference["status"] == "pass":
                values.append(
                    reference["median_workload_ms"] / primary["median_workload_ms"]
                )
        ratios[label] = geomean(values)

    executables = {label: Path(command[0]) for label, command in command_templates.items()}
    try:
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, timeout=5
        ).stdout.strip()
        jetstream_revision = subprocess.run(
            ["git", "ls-tree", "HEAD", "benchmarks/JetStream2"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=5,
        ).stdout.split()[2]
    except (OSError, subprocess.SubprocessError, IndexError):
        revision = "unknown"
        jetstream_revision = "unknown"
    environment = {
        "generated_at_utc": datetime.now(UTC).isoformat(),
        "platform": platform.platform(),
        "python": platform.python_version(),
        "project_revision": revision,
        "jetstream_revision": jetstream_revision,
        "memory_sampler": "psutil" if psutil else "unavailable",
        "commands": {
            label: [*command[:-1], "<generated-runner.js>"]
            for label, command in command_templates.items()
        },
        "binary_size_bytes": {label: path.stat().st_size for label, path in executables.items()},
        "binary_sha256": {label: fingerprint(path) for label, path in executables.items()},
    }
    report = {
        "meta": {
            "benchmark": "JetStream2 portable workload kernels",
            "iterations": iterations,
            "warmup": warmup,
            "repeat": repeat,
            "timeout_seconds": args.timeout,
            "max_rss_mb": args.max_rss_mb,
            "tests": tests,
            "engines": labels,
        },
        "environment": environment,
        "results": results,
        "geomean_ratios": ratios,
    }
    json_path = output / "results.json"
    md_path = output / "results.md"
    env_path = output / "environment.json"
    json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    env_path.write_text(json.dumps(environment, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_markdown(md_path, report)
    print(f"\nJSON -> {json_path}\nMD   -> {md_path}\nENV  -> {env_path}")

    return int(
        any(results[label][test]["status"] != "pass" for label in labels for test in tests)
    )


if __name__ == "__main__":
    sys.exit(main())
