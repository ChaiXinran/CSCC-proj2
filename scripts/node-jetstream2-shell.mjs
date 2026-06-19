import fs from "node:fs";
import vm from "node:vm";

const runner = process.argv[2];
if (!runner)
    throw new Error("usage: node scripts/node-jetstream2-shell.mjs <runner.js>");

globalThis.print = (...args) => console.log(...args);
globalThis.__agentjsLoadString = (source) =>
    vm.runInThisContext(source, { filename: "<jetstream-payload>" });

const source = fs.readFileSync(runner, "utf8");
vm.runInThisContext(source, { filename: runner });

