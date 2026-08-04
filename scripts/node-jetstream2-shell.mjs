import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const runner = process.argv[2];
if (!runner)
    throw new Error("usage: node scripts/node-jetstream2-shell.mjs <runner.js>");
const resourceRoot = path.resolve(process.argv[3] ?? "benchmarks/JetStream2");

function resolveResource(name) {
    const normalized = String(name).replaceAll("\\", "/").replace(/^\.\//, "");
    const resolved = path.resolve(resourceRoot, normalized);
    const relative = path.relative(resourceRoot, resolved);
    if (!normalized || relative.startsWith("..") || path.isAbsolute(relative))
        throw new Error(`resource escapes JetStream root: ${name}`);
    return resolved;
}

globalThis.readFile = (name) =>
    fs.readFileSync(resolveResource(name), "utf8").replace(/\r\n?/g, "\n");

const hostLog = console.log.bind(console);
let completed = false;
let failed = false;
globalThis.print = (...args) => {
    const line = args.map(String).join(" ");
    if (line.includes("JETSTREAM_RUN_COMPLETE"))
        completed = true;
    if (line.includes("JetStream2 failed:"))
        failed = true;
    hostLog(...args);
};
RegExp.escape ??= (text) =>
    String(text).replace(/[\\^$.*+?()[\]{}|/]/g, "\\$&");
globalThis.__agentjsLoadString = (source) =>
    vm.runInThisContext(source, { filename: "<jetstream-payload>" });

const source = fs.readFileSync(runner, "utf8");
const boundary = "/*__AGENTJS_LOAD_RESOURCES__*/";
const boundaryIndex = source.indexOf(boundary);
if (boundaryIndex < 0)
    throw new Error("JetStream runner is missing the resource-load boundary");
const prelude = source.slice(0, boundaryIndex);
const launch = source.slice(boundaryIndex + boundary.length);
vm.runInThisContext(prelude, { filename: runner });
for (const line of prelude.split(/\r?\n/)) {
    const match = line.match(/^\/\/ AGENTJS_RESOURCE:(.+)$/);
    if (match)
        vm.runInThisContext(globalThis.readFile(match[1]), { filename: match[1] });
}
vm.runInThisContext(launch, { filename: runner });
process.on("beforeExit", () => {
    if (!completed || failed)
        process.exitCode = 1;
});
