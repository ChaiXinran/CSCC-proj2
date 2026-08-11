import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const input = "D:/00_OS/CSCC/presentation/PPT-report-performance-updated.pptx";
const output = "D:/00_OS/CSCC/presentation/PPT-report-innovation-updated.pptx";
const report = "D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(5).md";

const presentation = await PresentationFile.importPptx(await FileBlob.load(input));
const slides = Array.isArray(presentation.slides?.items)
  ? [...presentation.slides.items]
  : Array.from({length:presentation.slides.count}, (_, i) => presentation.slides.getItem(i));

const runtimeInnovation = slides[12].duplicate();
runtimeInnovation.moveTo(13);
const benchmarkInnovation = slides[12].duplicate();
benchmarkInnovation.moveTo(14);

const snapshot = await presentation.inspect({kind:"textbox,notes",include:"id,slide,bbox",maxChars:300000});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));

function box(slide, x, y, text) {
  const candidates = records.filter((r) => r.kind === "textbox" && r.slide === slide && r.bbox);
  const found = candidates.reduce((best, r) => {
    const d = Math.abs(r.bbox[0] - x) + Math.abs(r.bbox[1] - y);
    return !best || d < best.d ? {r, d} : best;
  }, null);
  if (!found || found.d > 8) throw new Error(`Missing textbox near ${slide}:${x},${y}`);
  presentation.resolve(found.r.id).text = text;
}

function notes(slide, sources) {
  const record = records.find((r) => r.kind === "notes" && r.slide === slide);
  if (!record) throw new Error(`Missing notes on slide ${slide}`);
  presentation.resolve(record.id).setText(["[Sources]", ...sources.map((s) => `- ${s}`)].join("\n"));
}

function innovationSlide(slide, title, subtitle, items, footer) {
  box(slide, 66, 48, title);
  box(slide, 65.4, 111.73, subtitle);
  const slots = [
    [66,206,66,255.87],
    [650,206,650,258],
    [66,404,66,451.07],
    [650,404,650,451.07],
  ];
  for (let i = 0; i < 4; i += 1) {
    box(slide, slots[i][0], slots[i][1], items[i].heading);
    box(slide, slots[i][2], slots[i][3], items[i].body);
  }
  box(slide, 66, 668, footer);
}

innovationSlide(13,
  "创新点 1/3：执行架构围绕 Agent 生命周期设计",
  "核心贡献不是调用外部引擎，而是把 Native 执行、状态生命周期、跨层契约和失败模型组合成独立内核。",
  [
    {heading:"自研 Native 全链路", body:"Lexer → Parser/AST → Bytecode → Stack VM → Runtime/GC；Boa、QuickJS 仅作外部参照。"},
    {heading:"Engine / Runtime 双入口", body:"独立 action 使用 fresh isolate；关联调用使用 persistent isolate，在隔离成本与会话复用间显式选择。"},
    {heading:"稳定的跨层契约", body:"SourceParser、ProgramCompiler、ChunkExecutor 解耦阶段；可注入 fake stage 做分层测试。"},
    {heading:"校验字节码 + 统一结果", body:"Chunk 校验栈效果、跳转和异常区间；ExecutionReport / EvalFailure 统一成功与分类失败。"},
  ],
  "来源：实验报告 1.3、2.2–2.4、3.3；src/contracts.rs、src/engine.rs");
notes(13, [report, "D:/00_OS/CSCC/src/contracts.rs", "D:/00_OS/CSCC/src/engine.rs", "D:/00_OS/CSCC/src/backend/native.rs"]);

innovationSlide(14,
  "创新点 2/3：受控宿主与 Agent 型运行时优化",
  "从资源上界、数据结构到结构化输出，所有优化都服务于短时、不可信、结果可编排的 Agent action。",
  [
    {heading:"action 级资源预算", body:"循环、递归、VM 栈、堆对象、堆字节、大对象和 deadline 分层限额；超限返回 RuntimeLimit。"},
    {heading:"最小 Host 暴露面", body:"默认无文件、网络、进程、DOM 与 Node API；加载器必须显式安装并限制根目录。"},
    {heading:"面向 Agent 的数据路径", body:"64K inline + 4K 惰性分段、descriptor 旁路、ASCII 快路、非移动 GC 与 Free List 复用。"},
    {heading:"引擎无关 RenderTree", body:"agent.render 先校验类型、循环、JSON、深度与字节；前端只消费规范化 render events。"},
  ],
  "来源：实验报告 1.4–1.5、2.5–2.7；src/runtime、src/host");
notes(14, [report, "D:/00_OS/CSCC/src/runtime", "D:/00_OS/CSCC/src/host/mod.rs", "D:/00_OS/CSCC/demo/agent/protocol.md"]);

innovationSlide(15,
  "创新点 3/3：AgentBench 与可审计实验方法",
  "不虚构综合总分：先验证结果，再按调用模式拆分性能，并把每项结论绑定到可复核证据和适用边界。",
  [
    {heading:"AgentBench 2.0", body:"12 个确定性 action 覆盖 JSON、工具结果、规则过滤、对象压力、大索引数组与字符串处理。"},
    {heading:"正确性门控性能", body:"只有共同 PASS、无 error/timeout 且结果校验正确的 case 进入跨引擎几何平均。"},
    {heading:"cold / batch 分开解释", body:"P50、耗时比和 RSS 统一定义；batch 不冒充 persistent Runtime，也不冒充缓存消融。"},
    {heading:"证据边界同步归档", body:"JSON、环境、命令、revision 与 SHA-256 绑定批次；历史结果不反向套用到当前 HEAD。"},
  ],
  "来源：实验报告 4.1–4.3、5.2–5.4；benchmarks/agent");
notes(15, [report, "D:/00_OS/CSCC/benchmarks/agent/manifest.json", "D:/00_OS/CSCC/benchmarks/agent/run_agentbench.py"]);

// Update every visible bottom-right page marker, including duplicated markers.
for (let slide = 1; slide <= 20; slide += 1) {
  for (const record of records.filter((r) => r.kind === "textbox" && r.slide === slide && r.bbox && r.bbox[0] >= 1160 && r.bbox[1] >= 650)) {
    presentation.resolve(record.id).text = String(slide);
  }
}

// Closing slide moved from 18 to 20.
box(20, 851.67, 238.53,
  "Test262 90.97%，SunSpider 26 / 26\n\nJetStream 四引擎公共内核 6 / 6 PASS\n\n三层创新：执行架构、受控运行时、可审计评测\n\n冷启动有亮点，批处理与复杂内核仍需追赶");

const pptx = await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
