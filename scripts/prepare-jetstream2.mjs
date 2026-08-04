import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createHash } from "node:crypto";

const root = path.resolve(process.argv[2] ?? "benchmarks/JetStream2");
const testName = process.argv[3];
const iterationCount = Number(process.argv[4] ?? 0);
const phaseMarkers = process.argv.includes("--phase-markers");
const output = path.resolve(
    process.argv[5] ?? `benchmarks/generated/jetstream2-${testName}.js`,
);
const manifestOutput = output.replace(/\.js$/i, ".manifest.json");

if (!testName) {
    throw new Error(
        "usage: node scripts/prepare-jetstream2.mjs <root> <test> [iterations] [output]",
    );
}

function normalizeResourcePath(resource) {
    const raw = String(resource).replaceAll("\\", "/");
    const withoutPrefix = raw.replace(/^\.\//, "");
    const normalized = path.posix.normalize(withoutPrefix);
    if (
        normalized === ".." ||
        normalized.startsWith("../") ||
        path.posix.isAbsolute(normalized)
    ) {
        throw new Error(`resource escapes JetStream root: ${resource}`);
    }
    return `./${normalized}`;
}

function resolveResourcePath(resource) {
    const normalized = normalizeResourcePath(resource);
    const absolute = path.resolve(root, normalized.slice(2));
    const relativeToRoot = path.relative(root, absolute);
    if (relativeToRoot.startsWith("..") || path.isAbsolute(relativeToRoot))
        throw new Error(`resource escapes JetStream root: ${resource}`);
    return absolute;
}

function readTextResource(absolute) {
    return fs.readFileSync(absolute, "utf8").replace(/\r\n/g, "\n");
}

const discoveredResourceFiles = new Set();
const discoveryMissingFiles = new Set();

function readSourceCommit() {
    const dotGit = path.join(root, ".git");
    if (!fs.existsSync(dotGit))
        return null;
    const stat = fs.statSync(dotGit);
    let gitDirectory = dotGit;
    if (stat.isFile()) {
        const match = fs.readFileSync(dotGit, "utf8").trim().match(/^gitdir:\s*(.+)$/);
        if (!match)
            return null;
        gitDirectory = path.resolve(root, match[1]);
    }
    const head = fs.readFileSync(path.join(gitDirectory, "HEAD"), "utf8").trim();
    if (!head.startsWith("ref: "))
        return head;
    const ref = path.join(gitDirectory, ...head.slice(5).split("/"));
    return fs.existsSync(ref) ? fs.readFileSync(ref, "utf8").trim() : null;
}

const driverSource = fs.readFileSync(path.join(root, "JetStreamDriver.js"), "utf8");
let adaptedDriverSource = driverSource
    .replace('return `load("${url}");`', "return readFile(url);")
    .replace("return readFile(url);", "return __agentjsLoadEntry(url);")
    .replace(
        "if (JetStreamParams.testWorstCaseCount)",
        "if (JetStreamParams.testWorstCaseCount !== undefined)",
    )
    .replace("this.currentResolve = null;", "var currentResolve = null;")
    .replace("this.currentReject = null;", "var currentReject = null;")
    .replace("this.JetStream = new Driver(", "globalThis.JetStream = new Driver(")
    .replace(
        /this\._resourcesPromise = null;\r?\n\s*this\.fetchResources\(\);/,
        "this._resourcesPromise = Promise.resolve();\n        this.scripts = this.plan.files.map((file) => ({ __agentjsFile: file }));",
    )
    .replace(
        "addScript(`const isInBrowser = ${isInBrowser}; let performance = {now: Date.now.bind(Date)};`);",
        "addScript(`var performance = globalThis.performance = {now: Date.now.bind(Date)};`);",
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
    )
    .replace(
        /for \(const script of this\.scripts\)\r?\n\s*globalObject\.loadString\(script\);/,
        'globalObject.loadString(this.scripts.reduce((joined, script) => script && script.__agentjsFile ? joined : joined + "\\n" + script, ""));',
    );
if (adaptedDriverSource.includes('return `load("${url}");`'))
    throw new Error("failed to adapt ShellFileLoader.load");
if (!adaptedDriverSource.includes("return __agentjsLoadEntry(url);"))
    throw new Error("failed to route ShellFileLoader through the Host entry policy");
if (!adaptedDriverSource.includes("script && script.__agentjsFile ? joined"))
    throw new Error("failed to adapt ShellScripts.run");
adaptedDriverSource = adaptedDriverSource.replace(
    'const string = this.scripts.join("\\n");',
    'const string = this.scripts.reduce((joined, script) => joined + "\\n" + script, "");',
);

if (phaseMarkers) {
    adaptedDriverSource = adaptedDriverSource
        .replace(
            /const benchmark = new Benchmark\(\$\{JSON\.stringify\(this\.benchmarkArguments\)\}\);\r?\n\s*await benchmark\.init\?\.\(\);/g,
            '__jetstreamPhase("init:start");\n            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});\n            await benchmark.init?.();\n            __jetstreamPhase("init:end");',
        )
        .replace(
            /const benchmark = new Benchmark\(\$\{JSON\.stringify\(this\.benchmarkArguments\)\}\);\r?\n\s*const results = \[\];/g,
            '__jetstreamPhase("init:start");\n            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});\n            __jetstreamPhase("init:end");\n            const results = [];',
        )
        .replace(
            /^(\s*)await benchmark\.runIteration\(i\);$/gm,
            '$1__jetstreamPhase("iteration:" + i + ":start");\n$1await benchmark.runIteration(i);\n$1__jetstreamPhase("iteration:" + i + ":end");',
        )
        .replace(
            /^(\s*)benchmark\.runIteration\(i\);$/gm,
            '$1__jetstreamPhase("iteration:" + i + ":start");\n$1benchmark.runIteration(i);\n$1__jetstreamPhase("iteration:" + i + ":end");',
        )
        .replaceAll(
            "benchmark.validate?.(${this.iterations});",
            '__jetstreamPhase("validate:start");\n            benchmark.validate?.(${this.iterations});\n            __jetstreamPhase("validate:end");',
        );
}
const discovery = {
    console,
    isInBrowser: false,
    RAMification: false,
    testIterationCount: iterationCount || undefined,
    testList: testName,
    readFile: (name) => {
        const relative = normalizeResourcePath(name);
        discoveredResourceFiles.add(relative);
        const absolute = resolveResourcePath(relative);
        if (!fs.existsSync(absolute)) {
            discoveryMissingFiles.add(relative);
            return "";
        }
        return readTextResource(absolute);
    },
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
    JetStreamParams: {
        prefetchResources: false,
        forceGC: false,
        dumpJSONResults: false,
        testIterationCountMap: {},
        testWorstCaseCountMap: {},
        testList: testName,
    },
};
discovery.globalThis = discovery;
vm.createContext(discovery);
vm.runInContext(driverSource, discovery, { filename: "JetStreamDriver.js" });

const benchmarks = discovery.JetStream.benchmarks;
if (benchmarks.length !== 1) {
    throw new Error(
        `expected one benchmark, discovered ${benchmarks.length}: ${benchmarks.map((item) => item.name).join(", ")}`,
    );
}

const benchmark = benchmarks[0];
// Support both old API (benchmark.plan) and new API (benchmark directly).
const plan = benchmark.plan ?? benchmark;
const benchmarkFiles = [
    ...(plan.files ?? []),
    ...(benchmark._files ?? []),
    ...(benchmark.files ?? []),
].map(normalizeResourcePath);
const entryFiles = [...new Set(benchmarkFiles)];
const entrySourceBytes = entryFiles.reduce(
    (total, file) => total + fs.statSync(resolveResourcePath(file)).size,
    0,
);
// Every workload entry is read and evaluated individually by the Rust host.
// Keeping a size-based inline path would still concatenate the complete source
// of smaller workloads before compiling it through Function.
const entryExecutionMode = "staged";
const preloadEntries = Object.entries(
    plan.preload ?? Object.fromEntries(benchmark.preloadEntries ?? []),
).map(([name, resource]) => [name, normalizeResourcePath(resource)]);
const preloadFiles = preloadEntries.map(([, resource]) => resource);
if (plan.wasmPath || plan.benchmarkClass?.name === "WasmBenchmark") {
    throw new Error(`${testName} requires WebAssembly and cannot run in AgentJS yet`);
}

const missingFiles = new Set(discoveryMissingFiles);
const allResourceFiles = [
    ...entryFiles,
    ...preloadFiles,
    ...discoveredResourceFiles,
];
for (const relative of [...new Set(allResourceFiles)].sort()) {
    const absolute = resolveResourcePath(relative);
    if (!fs.existsSync(absolute)) {
        missingFiles.add(relative);
        continue;
    }
}

if (missingFiles.size) {
    throw new Error(
        `missing JetStream resources for ${testName}: ${[...missingFiles].sort().join(", ")}`,
    );
}

const compatibility = `
const isInBrowser = false;
const isD8 = false;
const isSpiderMonkey = false;
const jetStreamRawPrint = typeof globalThis.print === "function"
    ? globalThis.print
    : (...args) => globalThis.console.log(...args);
const jetStreamHostPrint = ${phaseMarkers ? '(...args) => { jetStreamRawPrint("JETSTREAM_OUTPUT_AT:" + Date.now()); jetStreamRawPrint(...args); }' : "jetStreamRawPrint"};
globalThis.print = jetStreamHostPrint;
var __jetstreamPhase = (phase) =>
    jetStreamHostPrint("JETSTREAM_PHASE:" + Date.now() + ":" + phase);
var console = {
    log: (...args) => jetStreamHostPrint(...args),
    warn: (...args) => jetStreamHostPrint(...args),
    error: (...args) => jetStreamHostPrint(...args),
    assert(condition, ...args) {
        if (!condition)
            throw new Error(args.join(" ") || "Assertion failed");
    },
};
var runString = () => {
    globalThis.loadString = (source) => {
        // Entry files are executed by the Rust host before launch. Only the
        // small driver-generated facade and iteration harness may reach this
        // compiler boundary; reject accidental workload re-concatenation.
        if (source.length > 1024 * 1024)
            throw new Error("JetStream inline harness unexpectedly exceeds 1 MiB");
        return new Function("top", source)(globalThis.top);
    };
    return globalThis;
};
var load = (name) => globalThis.loadString(readFile(name));
var performance = globalThis.performance = {
    now: Date.now.bind(Date),
    mark(name) { return { name }; },
    measure() {},
};
var document = globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
var testList = ${JSON.stringify(testName)};
var testIterationCount = ${iterationCount || "undefined"};
var RAMification = false;
var JetStreamParams = {
    prefetchResources: false,
    forceGC: false,
    dumpJSONResults: false,
    testIterationCount: ${iterationCount || "undefined"},
    testWorstCaseCount: ${iterationCount ? Math.max(0, Math.min(4, iterationCount - 1)) : "undefined"},
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: ${JSON.stringify(testName)},
};
var __agentjsLoadEntry = ${entryExecutionMode === "staged"
    ? '(url) => ({ __agentjsFile: url })'
    : '(url) => readFile(url)'};
var read = function (name, mode) {
    const text = readFile(name);
    if (mode !== "binary")
        return text;
    const bytes = [];
    for (let i = 0; i < text.length; i++)
        bytes.push(text.charCodeAt(i) & 0xff);
    return bytes;
};
`;

const launch = `
JetStream.initialize()
    .then(() => JetStream.start())
    .then(() => print("JETSTREAM_RUN_COMPLETE"))
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
`;

fs.mkdirSync(path.dirname(output), { recursive: true });
const resourceDirectives = entryFiles
    .filter(() => entryExecutionMode === "staged")
    .map((file) => `// AGENTJS_RESOURCE:${file}`)
    .join("\n");
const runnerSource = `${resourceDirectives}\n${compatibility}\n(function () {\n${adaptedDriverSource}\n})();\n/*__AGENTJS_LOAD_RESOURCES__*/\n${launch}`.replace(
    /[ \t]+$/gm,
    "",
);
if (runnerSource.includes("__jetstreamResources"))
    throw new Error("generated runner must not embed JetStream resources");
if (runnerSource.includes('scripts.join("\\n")'))
    throw new Error("generated runner must not concatenate workload scripts");
if (Buffer.byteLength(runnerSource, "utf8") > 512 * 1024)
    throw new Error("generated runner unexpectedly exceeds 512 KiB");
const runnerSha256 = createHash("sha256").update(runnerSource).digest("hex");

const manifest = {
    schemaVersion: 2,
    benchmark: testName,
    sourceCommit: readSourceCommit(),
    officialIterations: plan.iterations ?? 120,
    requestedIterations: iterationCount || plan.iterations || 120,
    resourceRootMode: "cli",
    entryExecutionMode,
    entrySourceBytes,
    entryFiles,
    preloadFiles,
    runtimeDiscoveredFiles: [...discoveredResourceFiles].sort(),
    resourceHashes: Object.fromEntries(
        [...new Set(allResourceFiles)].sort().map((relative) => [
            relative,
            `sha256:${createHash("sha256").update(readTextResource(resolveResourcePath(relative))).digest("hex")}`,
        ]),
    ),
    runnerSha256,
    phaseMarkers,
};
const manifestSource = `${JSON.stringify(manifest, null, 2)}\n`;
const runnerTemporary = `${output}.tmp-${process.pid}`;
const manifestTemporary = `${manifestOutput}.tmp-${process.pid}`;
try {
    fs.writeFileSync(runnerTemporary, runnerSource, "utf8");
    fs.writeFileSync(manifestTemporary, manifestSource, "utf8");
    fs.renameSync(runnerTemporary, output);
    fs.renameSync(manifestTemporary, manifestOutput);
} finally {
    fs.rmSync(runnerTemporary, { force: true });
    fs.rmSync(manifestTemporary, { force: true });
}

console.log(
    JSON.stringify(
        {
            test: testName,
            officialIterations: plan.iterations ?? 120,
            requestedIterations: iterationCount || plan.iterations || 120,
            files: entryFiles,
            preloadFiles,
            entryExecutionMode,
            entrySourceBytes,
            manifest: manifestOutput,
            output,
        },
        null,
        2,
    ),
);
