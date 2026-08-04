import fs from "node:fs";
import path from "node:path";

const baselinePath = process.argv[2];
const candidatePath = process.argv[3];
const outputPath = process.argv[4];
if (!baselinePath || !candidatePath) {
    throw new Error(
        "usage: node scripts/compare-jetstream2-status.mjs <baseline.json> <candidate.json> [output.json]",
    );
}

function readResults(file) {
    const value = JSON.parse(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
    if (!Array.isArray(value)) throw new Error(`${file} must contain a JSON array`);
    return new Map(value.map((item) => [item.benchmark, item]));
}

const baseline = readResults(baselinePath);
const candidate = readResults(candidatePath);
const names = [...new Set([...baseline.keys(), ...candidate.keys()])].sort();
const transitions = names.map((benchmark) => {
    const before = baseline.get(benchmark);
    const after = candidate.get(benchmark);
    const beforeStatus = before?.workloadStatus ?? "MISSING";
    const afterStatus = after?.workloadStatus ?? "MISSING";
    const kind = beforeStatus === afterStatus
        ? "UNCHANGED"
        : beforeStatus === "PASS"
          ? "REGRESSION"
          : afterStatus === "PASS"
            ? "IMPROVEMENT"
            : "CHANGED";
    return {
        benchmark,
        beforeStatus,
        afterStatus,
        kind,
        sameRunner:
            before?.runnerSha256 && after?.runnerSha256
                ? before.runnerSha256 === after.runnerSha256
                : null,
        beforeOutputTail: before?.outputTail ?? null,
        afterOutputTail: after?.outputTail ?? null,
    };
});

const report = {
    baseline: path.resolve(baselinePath),
    candidate: path.resolve(candidatePath),
    counts: Object.fromEntries(
        ["IMPROVEMENT", "REGRESSION", "CHANGED", "UNCHANGED"].map((kind) => [
            kind.toLowerCase(),
            transitions.filter((item) => item.kind === kind).length,
        ]),
    ),
    transitions,
};
const json = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
    fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
    fs.writeFileSync(outputPath, json, "utf8");
}
process.stdout.write(json);
if (report.counts.regression) process.exitCode = 1;
