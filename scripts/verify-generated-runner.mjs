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

if (!process.argv[2]) {
    throw new Error(
        "usage: node scripts/verify-generated-runner.mjs <runner.js> [engine] [timeout-ms]",
    );
}

const manifestPath = runner.replace(/\.js$/i, ".manifest.json");
const source = fs.readFileSync(runner, "utf8");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
new vm.Script(source, { filename: runner });
const runnerSha256 = createHash("sha256").update(source).digest("hex");
if (runnerSha256 !== manifest.runnerSha256)
    throw new Error("runner SHA-256 does not match manifest");

const required = new Set([
    ...manifest.entryFiles,
    ...manifest.preloadFiles,
    ...manifest.runtimeDiscoveredFiles,
]);
const embedded = new Set(manifest.embeddedFiles);
const missingFromRunner = [...required].filter((file) => !embedded.has(file));
if (manifest.missingFiles.length || missingFromRunner.length) {
    throw new Error(
        `manifest is incomplete: ${[
            ...manifest.missingFiles,
            ...missingFromRunner,
        ].join(", ")}`,
    );
}

const execution = spawnSync(engine, ["jetstream", runner], {
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
