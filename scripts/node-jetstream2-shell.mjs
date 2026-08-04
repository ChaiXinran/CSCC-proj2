import fs from "node:fs";
import vm from "node:vm";

const runner = process.argv[2];
if (!runner)
    throw new Error("usage: node scripts/node-jetstream2-shell.mjs <runner.js>");

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
vm.runInThisContext(source, { filename: runner });
process.on("beforeExit", () => {
    if (!completed || failed)
        process.exitCode = 1;
});
