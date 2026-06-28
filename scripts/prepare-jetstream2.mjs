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
    .replace("this.currentResolve = null;", "var currentResolve = null;")
    .replace("this.currentReject = null;", "var currentReject = null;")
    .replace("this.JetStream = new Driver();", "var JetStream = new Driver();")
    .replace(
        "class Benchmark {",
        `function initializeJetStreamBenchmark(target, plan) {
    target.plan = plan;
    target.iterations = testIterationCount || plan.iterations || defaultIterationCount;
    target.isAsync = !!plan.isAsync;
    target.scripts = plan.files.map((file) => readFile(file));
    target._resourcesPromise = Promise.resolve();
}

class JetStreamBenchmarkBase {`,
    )
    .replace(
        "class DefaultBenchmark extends Benchmark {",
        "class DefaultBenchmark {",
    )
    .replace(
        "class AsyncBenchmark extends DefaultBenchmark {",
        `class AsyncBenchmark {
    constructor(plan) {
        initializeJetStreamBenchmark(this, plan);
        this.worstCaseCount = plan.worstCaseCount || defaultWorstCaseCount;
        this.firstIteration = null;
        this.worst4 = null;
        this.average = null;
    }`,
    )
    .replace(
        "class WSLBenchmark extends Benchmark {",
        "class WSLBenchmark {",
    )
    .replace(
        "class WasmBenchmark extends Benchmark {",
        "class WasmBenchmark {",
    )
    .replace(
        /this\._resourcesPromise = null;\r?\n\s*this\.fetchResources\(\);/,
        "this._resourcesPromise = Promise.resolve();\n        this.scripts = this.plan.files.map((file) => readFile(file));",
    )
    // ponytail: JetStream wrappers only forward one plan; avoid native's current super() gap.
    .replaceAll(
        /constructor\(\.\.\.args\) \{\r?\n\s*super\(\.\.\.args\);/g,
        "constructor(plan) {\n        initializeJetStreamBenchmark(this, plan);",
    )
    .replaceAll(
        "super.updateUIAfterRun();",
        "JetStreamBenchmarkBase.prototype.updateUIAfterRun.call(this);",
    )
    .replace(
        "const ARESGroup =",
        `Object.setPrototypeOf(DefaultBenchmark.prototype, JetStreamBenchmarkBase.prototype);
Object.setPrototypeOf(AsyncBenchmark.prototype, DefaultBenchmark.prototype);
Object.setPrototypeOf(AsyncBenchmark, DefaultBenchmark);
Object.setPrototypeOf(WSLBenchmark.prototype, JetStreamBenchmarkBase.prototype);
Object.setPrototypeOf(WasmBenchmark.prototype, JetStreamBenchmarkBase.prototype);

const ARESGroup =`,
    )
    .replace(
        "addScript(`const isInBrowser = ${isInBrowser}; let performance = {now: Date.now.bind(Date)};`);",
        "addScript(`var performance = globalThis.performance = {now: Date.now.bind(Date)};`);",
    )
    .replace(
        /let scripts = string;\r?\n\s*let globalObject = runString\(""\);[\s\S]*?for \(let script of scripts\)\r?\n\s*globalObject\.loadString\(script\);\r?\n\s*return globalObject;/,
        `let top = { currentResolve, currentReject };
            new Function("top", string.join("\\n"))(top);
            return globalThis;`,
    )
    .replace(
        "addScript(this.runnerCode);",
        'addScript("(() => {\\n" + this.runnerCode + "\\n})();");',
    )
    .replace(
        /let start = Date\.now\(\);\r?\n\s*__benchmark\.runIteration\(\);\r?\n\s*let end = Date\.now\(\);\r?\n\r?\n\s*results\.push\(Math\.max\(1, end - start\)\);/,
        `let __jetstreamIterationStart = Date.now();
                __benchmark.runIteration();
                let __jetstreamIterationEnd = Date.now();

                results.push(Math.max(1, __jetstreamIterationEnd - __jetstreamIterationStart));`,
    )
    .replace(
        /let start = Date\.now\(\);\r?\n\s*for \(let benchmark of this\.benchmarks\)/,
        `let __jetstreamSuiteStart = Date.now();
        for (let benchmark of this.benchmarks)`,
    )
    .replace(
        "let totalTime = Date.now() - start;",
        "let totalTime = Date.now() - __jetstreamSuiteStart;",
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
var console = { log: (...args) => print(...args) };
var document = globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
var testList = ${JSON.stringify(testName)};
var testIterationCount = ${iterationCount || "undefined"};
var RAMification = false;
var __jetstreamResources = ${JSON.stringify(resources)};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
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
