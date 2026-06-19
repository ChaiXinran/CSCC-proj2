import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const root = path.resolve(process.argv[2] ?? "benchmarks/JetStream2");
const testName = process.argv[3];
const iterationCount = Number(process.argv[4] ?? 0);
const output = path.resolve(
    process.argv[5] ?? `benchmarks/generated/jetstream2-${testName}.js`,
);

if (!testName) {
    throw new Error(
        "usage: node scripts/prepare-jetstream2.mjs <root> <test> [iterations] [output]",
    );
}

const driverSource = fs.readFileSync(path.join(root, "JetStreamDriver.js"), "utf8");
const adaptedDriverSource = driverSource
    .replace("class Benchmark {", "class JetStreamBenchmarkBase {")
    .replace(
        "class DefaultBenchmark extends Benchmark {",
        "class DefaultBenchmark extends JetStreamBenchmarkBase {",
    )
    .replace(
        "class WSLBenchmark extends Benchmark {",
        "class WSLBenchmark extends JetStreamBenchmarkBase {",
    )
    .replace(
        "class WasmBenchmark extends Benchmark {",
        "class WasmBenchmark extends JetStreamBenchmarkBase {",
    )
    .replace(
        "addScript(`const isInBrowser = ${isInBrowser}; let performance = {now: Date.now.bind(Date)};`);",
        "addScript(`globalThis.performance = {now: Date.now.bind(Date)};`);",
    );
const discovery = {
    console,
    isInBrowser: false,
    RAMification: false,
    testIterationCount: iterationCount || undefined,
    testList: testName,
    readFile: () => "",
    runString: () => ({
        print() {},
        loadString() {},
    }),
    print() {},
    setTimeout,
    clearTimeout,
    Promise,
    Date,
    Math,
    Symbol,
    Map,
    Blob,
};
discovery.globalThis = discovery;
vm.createContext(discovery);
vm.runInContext(driverSource, discovery, { filename: "JetStreamDriver.js" });

const benchmarks = discovery.JetStream.benchmarks;
if (benchmarks.length !== 1) {
    throw new Error(`expected one benchmark, discovered ${benchmarks.length}`);
}

const plan = benchmarks[0].plan;
if (plan.wasmPath || plan.benchmarkClass?.name === "WasmBenchmark") {
    throw new Error(`${testName} requires WebAssembly and cannot run in AgentJS yet`);
}

const resources = {};
for (const relative of plan.files) {
    const normalized = relative.replaceAll("\\", "/");
    resources[normalized] = fs.readFileSync(path.resolve(root, relative), "utf8");
}

const compatibility = `
const isInBrowser = false;
globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
globalThis.testList = ${JSON.stringify(testName)};
globalThis.testIterationCount = ${iterationCount || "undefined"};
globalThis.RAMification = false;
globalThis.__jetstreamResources = ${JSON.stringify(resources)};
globalThis.readFile = function (name) {
    const normalized = String(name).replaceAll("\\\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};
globalThis.runString = function (source) {
    if (source)
        __agentjsLoadString(source);
    const shellRealm = {
        print,
        loadString(text) { return __agentjsLoadString(text); }
    };
    Object.defineProperty(shellRealm, "console", {
        get() { return globalThis.console; },
        set(_) {}
    });
    Object.defineProperty(shellRealm, "top", {
        get() { return globalThis.top; },
        set(value) { globalThis.top = value; }
    });
    return shellRealm;
};
`;

const launch = `
JetStream.initialize()
    .then(() => JetStream.start())
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
`;

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(
    output,
    `${compatibility}\n${adaptedDriverSource}\n${launch}`,
    "utf8",
);

console.log(
    JSON.stringify(
        {
            test: testName,
            officialIterations: plan.iterations ?? 120,
            requestedIterations: iterationCount || plan.iterations || 120,
            files: plan.files,
            output,
        },
        null,
        2,
    ),
);
