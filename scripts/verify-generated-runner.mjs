import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";

const runner = path.resolve(process.argv[2] ?? "");
const engine = process.argv[3]
    ? path.resolve(process.argv[3])
    : path.resolve("target/release/agentjs.exe");
const timeoutMs = Number(process.argv[4] ?? 150_000);
const resourceRoot = path.resolve(process.argv[5] ?? "benchmarks/JetStream2");

if (!process.argv[2]) {
    throw new Error(
        "usage: node scripts/verify-generated-runner.mjs <runner.js> [engine] [timeout-ms] [resource-root]",
    );
}

const manifestPath = runner.replace(/\.js$/i, ".manifest.json");
const source = fs.readFileSync(runner, "utf8");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
new vm.Script(source, { filename: runner });
const runnerSha256 = createHash("sha256").update(source).digest("hex");
if (runnerSha256 !== manifest.runnerSha256)
    throw new Error("runner SHA-256 does not match manifest");
if (source.includes("__jetstreamResources"))
    throw new Error("runner embeds JetStream resources");
if (source.includes('scripts.join("\\n")'))
    throw new Error("runner concatenates workload scripts");
if (Buffer.byteLength(source, "utf8") > 512 * 1024)
    throw new Error("runner exceeds the 512 KiB structural limit");

const required = new Set([
    ...manifest.entryFiles,
    ...manifest.preloadFiles,
    ...manifest.runtimeDiscoveredFiles,
]);
const missingResources = [...required].filter(
    (file) => !fs.existsSync(path.resolve(resourceRoot, file.replace(/^\.\//, ""))),
);
if (manifest.schemaVersion !== 2 || missingResources.length) {
    throw new Error(
        `manifest is incomplete: ${missingResources.join(", ")}`,
    );
}

const execution = spawnSync(engine, ["jetstream", runner, "--resource-root", resourceRoot], {
    cwd: process.cwd(),
    encoding: "utf8",
    timeout: timeoutMs,
    maxBuffer: 16 * 1024 * 1024,
});
const output = `${execution.stdout ?? ""}${execution.stderr ?? ""}`;
const resourceFailure = /JetStream resource not embedded|readFile path failed/.test(
    output,
);
const completed = output.includes("JETSTREAM_RUN_COMPLETE");
const timedOut = execution.error?.code === "ETIMEDOUT";
const workloadStatus = timedOut
    ? "TIMEOUT"
    : execution.status === 0 && completed
      ? "PASS"
      : execution.status === 0
        ? "INCOMPLETE"
      : resourceFailure
        ? "RESOURCE_MISSING"
        : "ENGINE_FAILURE";

console.log(
    JSON.stringify(
        {
            benchmark: manifest.benchmark,
            runner,
            manifest: manifestPath,
            runnerSha256,
            sourceCommit: manifest.sourceCommit,
            requestedIterations: manifest.requestedIterations,
            syntax: "PASS",
            resources: resourceFailure ? "FAIL" : "PASS",
            workloadStatus,
            exitCode: execution.status,
            signal: execution.signal,
            timedOut,
            outputTail: output.replace(/[\r\n]+/g, " ").slice(-500),
        },
        null,
        2,
    ),
);

if (resourceFailure)
    process.exitCode = 1;
