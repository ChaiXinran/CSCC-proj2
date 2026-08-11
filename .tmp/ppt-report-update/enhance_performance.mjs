import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const input = "D:/00_OS/CSCC/presentation/PPT-report-updated.pptx";
const output = "D:/00_OS/CSCC/presentation/PPT-report-performance-updated.pptx";
const presentation = await PresentationFile.importPptx(await FileBlob.load(input));

const slides = Array.isArray(presentation.slides?.items)
  ? [...presentation.slides.items]
  : Array.from({ length: presentation.slides.count }, (_, i) => presentation.slides.getItem(i));

const jetstreamSlide = slides[9].duplicate();
jetstreamSlide.moveTo(10);
const performanceSlide = slides[12].duplicate();
performanceSlide.moveTo(13);

const snapshot = await presentation.inspect({
  kind: "textbox,notes",
  include: "id,slide,name,text,bbox",
  maxChars: 250000,
});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));

function box(slide, x, y, text) {
  const candidates = records.filter((r) => r.kind === "textbox" && r.slide === slide && r.bbox);
  const record = candidates.reduce((best, r) => {
    const d = Math.abs(r.bbox[0] - x) + Math.abs(r.bbox[1] - y);
    return !best || d < best.d ? { r, d } : best;
  }, null);
  if (!record || record.d > 8) throw new Error(`Missing textbox near ${slide}:${x},${y}`);
  presentation.resolve(record.r.id).text = text;
}

function notes(slide, lines) {
  const record = records.find((r) => r.kind === "notes" && r.slide === slide);
  if (!record) throw new Error(`Missing notes on slide ${slide}`);
  presentation.resolve(record.id).setText(["[Sources]", ...lines.map((line) => `- ${line}`)].join("\n"));
}

// New slide 11: four-engine JetStream2 kernel comparison.
box(11, 56, 40, "JetStream 2：四引擎内核性能对比");
box(11, 56, 106, "官方 workload 生成 CLI 可移植内核；Release、同机、2 次预热 + 7 次测量，展示 P50。非浏览器综合总分。");
box(11, 94.47, 248, "6 / 6");
box(11, 57.87, 338, "四个引擎共同通过，结果确定性校验");
box(11, 398.07, 248, "0.945×");
box(11, 375.4, 344.87, "Oxide / AgentJS 几何平均耗时比");
box(11, 715.6, 248, "27.04");
box(11, 681.47, 344.87, "AgentJS 最大 RSS / MiB");
box(11, 1001.67, 246.6, "7×");
box(11, 995.33, 338, "每组有效测量进程");
box(11, 88, 449, "代表 workload");
box(11, 388, 449, "AgentJS P50");
box(11, 568, 449, "Boa P50");
box(11, 748, 449, "QuickJS P50");
box(11, 928, 449, "Oxide P50");
box(11, 88, 495, "n-body-SP");
box(11, 388, 495, "1.22 s");
box(11, 568, 495, "347 ms");
box(11, 748, 495, "74 ms");
box(11, 928, 495, "970 ms");
box(11, 88, 541, "crypto-sha1-SP");
box(11, 388, 541, "4.03 s");
box(11, 568, 541, "527 ms");
box(11, 748, 541, "200 ms");
box(11, 928, 541, "3.90 s");
box(11, 88, 587, "richards");
box(11, 388, 587, "3.72 s");
box(11, 568, 587, "625 ms");
box(11, 748, 587, "116 ms");
box(11, 928, 587, "3.54 s");
box(11, 56, 664, "来源：benchmarks/jetstream/results/four-engine；6 workload 全量数据与 RSS 见 JSON / environment.json");
notes(11, [
  "D:/00_OS/CSCC/benchmarks/jetstream/results/four-engine/results.json",
  "D:/00_OS/CSCC/benchmarks/jetstream/results/four-engine/environment.json",
  "D:/00_OS/CSCC/benchmarks/jetstream/results/summary.md",
]);

// Existing innovation slide moves from 12 to 13: strengthen novelty and evidence discipline.
box(13, 66, 48, "创新点：面向 Agent 场景的可审计执行栈");
box(13, 65.4, 111.73, "创新不只是一条 fast path，而是执行链、宿主边界、基准口径和数据结构的协同设计。");
box(13, 66, 206, "自研 Native 全链路");
box(13, 66, 255.87, "lexer → parser → bytecode → VM → runtime/builtins；稳定 contracts 解耦阶段，默认路径不嵌入 Boa 回退。");
box(13, 650, 206, "正确性门控性能");
box(13, 650, 258, "仅共同 PASS、确定性输出正确的样本进入统计；超时、错误和内存上限失败单独保留，不用删样本美化结论。");
box(13, 66, 404, "AgentBench 2.0");
box(13, 66, 451.07, "12 个 Agent 型 workload 覆盖 JSON、规则过滤、字符串、对象压力；统一测 cold、batch、RSS 与产物体积。");
box(13, 650, 404, "最小宿主 + 结构优化");
box(13, 650, 451.07, "冻结 console 与受控 Host；分段数组、descriptor 旁路表、ASCII 快速路径和 arena slot 复用聚焦短任务成本。");
box(13, 66, 668, "来源：架构实现、AgentBench 2.0、四引擎结果与实验报告第 4–5 章");
notes(13, [
  "D:/00_OS/CSCC/src/contracts.rs",
  "D:/00_OS/CSCC/src/backend/native.rs",
  "D:/00_OS/CSCC/benchmarks/agent/run_agentbench.py",
  "D:/00_OS/CSCC/benchmarks/agent/results/validation-all/agentjs.json",
  "D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(4).md",
]);

// New slide 14: an honest performance profile across operating modes.
box(14, 56, 40, "性能画像：优势、差距与优化优先级");
box(14, 56, 163.4, "不同 workload 不能混成一个总分：短任务看启动成本，连续任务看吞吐，复杂内核看执行效率，压力场景看 RSS 上界。");
box(14, 110.8, 297.4, "冷启动有亮点");
box(14, 285.8, 301.75, "AgentBench startup-noop：AgentJS 20.8 ms，Node 46.9 ms；但 12 项总体仍由对象与字符串热点主导。");
box(14, 110.8, 420.2, "批处理仍是短板");
box(14, 285.8, 420.2, "归档四引擎 batch：Boa / AgentJS = 0.625×，QuickJS / AgentJS = 0.112×；优先减少重复解析与调度成本。");
box(14, 110.8, 551.4, "复杂内核需追赶");
box(14, 273.37, 556.6, "JetStream 公共集 6/6 PASS；Boa、QuickJS 分别约快 4.95×、21.3×，AgentJS RSS 27.04 MiB，远低于 Oxide 峰值。");
notes(14, [
  "D:/00_OS/CSCC/benchmarks/agent/results/validation-all/agentjs.json",
  "D:/00_OS/CSCC/benchmarks/jetstream/results/four-engine/results.json",
  "D:/00_OS/CSCC/benchmarks/jetstream/results/four-engine/environment.json",
  "D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(4).md",
]);

// Closing slide moved to 18; make the performance conclusion explicit and appropriately scoped.
box(18, 851.67, 180.33, "最后回顾");
presentation.resolve("sh/47il0nah").text =
  "Test262 90.97%，SunSpider 26 / 26\n\nJetStream 四引擎公共内核 6 / 6 PASS\n\n冷启动具备亮点，批处理与复杂内核仍需追赶\n\n创新集中在 Native 全链路、最小宿主与可审计 benchmark";

// Refresh page markers after inserting two slides.
for (let slide = 1; slide <= 18; slide += 1) {
  const page = records.find((r) => r.kind === "textbox" && r.slide === slide && r.bbox && r.bbox[0] >= 1160 && r.bbox[1] >= 650);
  if (page) presentation.resolve(page.id).text = String(slide);
}

const pptx = await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
