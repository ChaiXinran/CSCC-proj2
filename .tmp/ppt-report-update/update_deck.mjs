import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const input = "D:/00_OS/CSCC/.tmp/ppt-report-update/template-starter.pptx";
const output = "D:/00_OS/CSCC/presentation/PPT-report-updated.pptx";
const report = "D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(4).md";

const presentation = await PresentationFile.importPptx(await FileBlob.load(input));
const snapshot = await presentation.inspect({
  kind: "slide,textbox,notes",
  include: "id,slide,text,title,name",
  maxChars: 200000,
});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));

function replace(slide, oldText, newText) {
  const record = records.find((item) => item.kind === "textbox" && item.slide === slide && item.text === oldText);
  if (!record) throw new Error(`Missing textbox on slide ${slide}: ${oldText}`);
  presentation.resolve(record.id).text.replace(oldText, newText);
}

function setNotes(slide, extra = "") {
  const record = records.find((item) => item.kind === "notes" && item.slide === slide);
  if (!record) throw new Error(`Missing notes on slide ${slide}`);
  presentation.resolve(record.id).setText([
    "[Sources]",
    `- ${report}`,
    extra,
  ].filter(Boolean).join("\n"));
}

replace(1,
  "面向短时、高频、可嵌入的脚本执行场景\n默认使用自研 Native Runtime\n以 Test262 与 benchmark 报告作为阶段成果证据",
  "面向短时、高频、受控资源的 Agent 脚本执行\n默认走自研 Native Runtime，不依赖外部引擎\nTest262 90.97% · SunSpider 26/26 · AgentBench 2.0");
replace(1, "71.78%", "90.97%");

replace(2, "展示 Test262 全量结果、SunSpider benchmark 与代表性修复。",
  "展示 Test262、SunSpider、AgentBench 2.0 与 Demo 协议闭环。");
replace(2, "单独章节说明 AI 工具、使用场景、成果、复核方式与交互记录。",
  "说明 AI 使用、实验边界、已知短板、复核方式与未来计划。");

replace(3, "关注启动、嵌入、短脚本和可控资源，而不是浏览器宿主环境。",
  "归档产物 10.29 MiB，小于 Boa；宿主仅保留 print、console 与受控 Host。");
replace(3, "用脚本缓存、对象复用、字符串快速路径和 benchmark 热点分析推进优化。",
  "以 cold / batch、RSS、产物体积和热点用例衡量工程取舍。");
replace(3, "以 Test262、分目录报告、SunSpider 和版本报告形成可复现证据。",
  "Test262、SunSpider、AgentBench 与原始 JSON / Markdown 共同留证。");

replace(4, "替换实名并核对 commit 贡献", "统一主线、报告与实验口径");
replace(4, "补充负责版本和测试证据", "补齐语法修复与测试证据");
replace(4, "补充具体模块与结果", "核对 Runtime 边界与结果");
replace(4, "补充运行环境与报告链接", "复跑并归档环境与哈希");
replace(4, "来源：git log 作者记录、docs/version-development-workflow.md、reports/.version-report/*",
  "来源：git log、docs/version-development-workflow.md、版本报告与实验报告");

replace(5, "Native V8-V11", "Native V8-V17");
replace(5, "扩展模块、异步、类型数组、日期、正则和属性描述符",
  "扩展模块、异步、日期、正则、属性描述符与 Agent Host");
replace(5, "Fixup 冲刺", "准确率冲刺");
replace(5, "围绕 Test262 高失败簇，定向修复内建对象、字符串和语言语义",
  "按失败簇修复 Builtins、字符串、RegExp 与精确错误语义");
replace(5, "交付收尾", "证据收束");
replace(5, "生成中文报告、全量测试、SunSpider 对比和答辩材料",
  "归档 Test262、AgentBench、SunSpider、Demo 与复测命令");
replace(5,
  "当前状态：主要实现已成型，最新 Test262 全量通过率 71.78%，SunSpider 26/26 正确；\n\t  交付前重点是数据核对、文档一致性和未支持范围说明。",
  "当前状态：Native 执行链与 Agent Host 已闭环，Test262 90.97%，SunSpider 26/26；\n交付重点转向批处理吞吐、字符串热点、峰值 RSS 与可复现元数据。");
replace(5, "来源：docs/version-development-workflow.md、reports/test262-final/final-latest-summary.md",
  "来源：版本开发流程、Test262 汇总、AgentBench 结果与最终实验报告");

replace(6, "默认路径为自研 Native Runtime；Boa、QuickJS、Node/V8 仅作为参考和对比对象。",
  "默认路径为自研 Native Runtime；Boa 与 QuickJS 仅以外部进程作为行为和性能参照。");
replace(6,
  "架构边界采用单向依赖：\n前端产生 AST，编译器生成 Chunk，VM 解释字节码\nruntime / builtins 访问对象模型、作用域、异常、堆对象和标准库能力\nbackend/native.rs 负责装配，不承载具体语义实现。",
  "架构边界保持单向依赖：\nlexer → parser / AST → bytecode → VM → runtime / builtins\ncontracts.rs 暴露跨阶段稳定接口，NativePipeline 负责组装执行链\nbackend/native.rs 只装配阶段，不承载具体语义。");
replace(6, "最终功能通过率来自 Native 路径；Boa 仅用于显式 reference backend 和差分验证。",
  "Native 是唯一运行后端；Boa / QuickJS 只作外部参照，不静默回退。");

replace(7, "项目不是一次性堆功能，而是用版本化文档和测试报告持续收敛。",
  "以版本化三轨协作收敛语义：前端、运行时、Builtins 同步交付代码与报告。");
replace(7,
  "每个版本先冻结 scope、interface 和 team plan，再分组实现、运行分层测试、更新报告。\n后期 Fixup 阶段按失败簇拆分任务，把 Test262 反馈转成明确的修复优先级。",
  "每个版本冻结 scope、只读 interface 与 team plan，再分轨实现、运行测试门禁并更新同批报告。\n失败簇进入下一轮 focused fix；跨层语义通过 contracts.rs 与栈约束协调。");
replace(7, "unit / native / focused Test262", "fmt / check / test / clippy / focused Test262");
replace(7, "version report / final report", "版本报告 / 扫描 JSON / 环境与哈希");
replace(7, "失败簇分析与下一轮 fixup", "失败簇、回归与复测证据闭环");
replace(7, "来源：docs/version-development-workflow.md", "来源：AGENTS.md、版本开发流程与最终实验报告");

replace(8, "最新中文 final 报告显示通过率 71.78%，超过 60% 赛题要求。",
  "仓库归档全量汇总通过率 90.97%，高于 60% 门槛 30.97 个百分点。");
replace(8, "38,315", "48,557");
replace(8, "通过用例 / 总 53,379", "通过 / 总 53,379");
replace(8, "71.78%", "90.97%");
replace(8, "4.22m", "452.354s");
replace(8, "整体全量运行耗时", "归档全量运行耗时");
replace(8, "目录", "统计口径");
replace(8, "总数", "Total");
replace(8, "通过", "Passed");
replace(8, "失败", "Failed");
replace(8, "跳过", "Skipped");
replace(8, "通过率", "Pass rate");
replace(8, "耗时", "Elapsed");
replace(8, "test/language", "Full run");
replace(8, "23,711", "53,379");
replace(8, "19,156", "48,557");
replace(8, "4,555", "4,820");
replace(8, "80.79%", "90.97%");
replace(8, "35.36 秒", "452.354s");
replace(8, "test/built-ins", "赛题门槛");
replace(8, "23,643", "—");
replace(8, "16,996", "—");
replace(8, "6,645", "—");
replace(8, "71.89%", "60.00%");
replace(8, "2.78 分钟", "—");
replace(8, "test/annexB", "超出门槛");
replace(8, "1,086", "—");
replace(8, "848", "—");
replace(8, "238", "—");
replace(8, "0", "—");
replace(8, "78.08%", "+30.97 pp");
replace(8, "2.20 秒", "—");
replace(8, "来源：reports/test262-final/final-latest-summary.md、final-all.md",
  "来源：Test262-final/full-test262-summary.json；失败与跳过均不计通过");

replace(9, "阶段数据用于说明迭代过程，最终展示以 clean full scan 和中文 final 报告为准。",
  "90.97% 支持门槛结论；历史汇总缺少运行 commit 与环境，不能反向绑定当前 HEAD。");
replace(9,
  "项目早期全量 baseline 约 26.29%，后续通过 Native V11、Fix、Fixup 等阶段持续提升。\n当前 final full run 已经稳定超过 70%，但 PPT 和提交文档需要避免混用旧 README 中的 72.02% 草稿数据。",
  "项目从 26.29% baseline 逐步越过 60% 赛题门槛，最新归档全量汇总达到 90.97%。\n结论只绑定归档 JSON；当前固定 Test262 revision 不能证明历史扫描版本。");
replace(9, "FixRTLE / Fixup8 baseline", "赛题兼容性门槛");
replace(9, "66.45%", "60.00%");
replace(9, "Final clean full scan", "最新归档全量汇总");
replace(9, "71.78%", "90.97%");
replace(9,
  "跳过不计入通过\n运行时错误不能伪通过分目录与全量交叉校验\n报告保存中文 md 与 JSON",
  "失败与跳过均不计通过\n汇总缺执行 commit / suite revision\n当前 HEAD 不反向证明历史扫描\n复测需归档命令、环境与 SHA-256");
replace(9, "来源：README-polished.md 草稿、reports/test262-final/final-latest-summary.md",
  "来源：实验报告 4.3–4.4、Test262 全量汇总 JSON");

replace(10, "Correctness 已全部通过；性能上仍显著落后成熟引擎，瓶颈集中在字符串和正则 workload。",
  "26 / 26 用于正确性结论；历史耗时批次未保存共同机器指纹，仅用于定位热点。");
replace(10, "25.03s", "3×");
replace(10, "AgentJS median 总耗时", "每个引擎历史重复");
replace(10, "2.52s", "P50");
replace(10, "Boa reference 总耗时", "单项中位耗时口径");
replace(10, "1.43s", "非同步");
replace(10, "Node/V8 control 总耗时", "耗时不作整体排名");
replace(10, "Node/V8", "Boa");
replace(10, "倍率", "AgentJS/Boa");
replace(10, "string-unpack-code", "bitops-bitwise-and");
replace(10, "9132ms", "262ms");
replace(10, "65ms", "286ms");
replace(10, "139.8x", "0.92x");
replace(10, "字符串拼接与替换路径仍重", "该用例 AgentJS 略快");
replace(10, "string-tagcloud", "regexp-dna");
replace(10, "3435ms", "3098ms");
replace(10, "60ms", "106ms");
replace(10, "57.0x", "29.2x");
replace(10, "文本处理和属性访问热点", "RegExp 路径差距明显");
replace(10, "regexp-dna", "string-tagcloud");
replace(10, "3257ms", "8208ms");
replace(10, "54ms", "148ms");
replace(10, "60.2x", "55.5x");
replace(10, "正则引擎特性与 dispatch 成本", "复杂字符串与对象处理热点");
replace(10, "来源：benchmarks/sunspider/results/agentjs-after-string-opt-sunspider.md/json、boa-sunspider.json",
  "来源：agentjs-sunspider.json、boa-sunspider.json；历史批次非同步");

replace(11, "Runner 稳定性", "跨层语义约束");
replace(11, "Test262 runner 增加运行时错误跳过/记录能力，避免单个崩溃中断整批测试。",
  "Parser、Compiler 与 VM 必须共享栈效果、跳转目标、异常区间和环境深度约束。");
replace(11, "SunSpider 正确性", "扫描证据闭环");
replace(11, "修复 date-format-tofte、3d-raytrace 等 benchmark 可运行性问题，最终 26/26 通过。",
  "结果包同步保存 commit、suite revision、命令、环境与二进制 SHA-256。");
replace(11, "字符串性能", "统计含义分离");
replace(11, "增加 ASCII fast path、replace 无 `$` 快速路径、预分配字符串拼接，降低 tagcloud / validate-input 等耗时。",
  "cold 与 batch 分开解释；当前 batch 不是持久 Runtime，也不是脚本缓存消融。");
replace(11, "执行路径开销", "Host 双重校验");
replace(11, "lazy arguments object、Array.sort merge sort、部分 RegExp literal replacement 快速路径。",
  "Python 编排器先筛脚本，Native Host 再校验 RenderTree 类型、深度、循环与字节数。");
replace(11, "来源：reports/test262-final/*、benchmarks/sunspider/results/*、近期修复记录",
  "来源：最终实验报告第五章“现象 → 根因 → 解决思路 → 证据边界”");

replace(12, "围绕短时高频脚本执行中的热点路径，优先做轻量、可控、可审计的结构性优化。",
  "结构性优化以归档 AgentBench 用例观察为证据，不把单项差异直接解释为因果。");
replace(12, "前 64K 槽位 inline，之后 4K 惰性分段；大索引转 sparse property。AgentBench 比 Boa 快 2.39x。",
  "前 64K 槽位 inline，之后 4K 惰性分段；large-index-dense-array 为 638.5 ms，Boa 715.9 ms。");
replace(12, "普通元素只存 value，非默认 descriptor 进入旁路表；混合访问用例比 Boa 快 1.67x。",
  "普通元素只存 value，非默认 descriptor 进旁路表；对应 cold 用例 424.3 ms，Boa 569.2 ms。");
replace(12, "GC 后复用 object、function、environment 等 arena slot，减少短任务重复分配压力。",
  "GC 后复用 object、function、environment 等 arena slot；收益需通过独立消融进一步验证。");
replace(12, "ASCII 字符串走 byte length / code unit 快速路径；string-base64 从 769.9ms 降至 273.0ms，提升 2.82x。",
  "清洗用例领先 Boa，但 ASCII index scan 仍落后；快速路径收益并非所有字符串 workload 通用。");
replace(12, "来源：report.md 10.4 数据结构与运行时优化",
  "来源：实验报告 4.6.2 与 5.5；归档 AgentBench cold P50");

replace(13, "当前风险与已交付内容", "AgentBench 2.0：结论与边界");
replace(13, "当前最终材料和可复现实验保持一致：Native 与 Boa/QuickJS/Node 口径分开，Test262 与 benchmark 数据注明命令、环境和报告路径，写明未支持特性。",
  "12 个共同通过 case 均经过结果、超时与确定性门控；比值统一为“参考引擎 / AgentJS”的几何平均。");
replace(13, "未支持范围", "cold 即时任务");
replace(13, "Intl402 / Temporal 完整语义、复杂 RegExp、模块/async 宿主行为仍需边界说明。",
  "耗时 Boa / AgentJS = 1.090x，QuickJS / AgentJS = 0.186x：相对 Boa 有竞争力，QuickJS 约快 5.4 倍。");
replace(13, "性能差距", "batch 连续执行");
replace(13, "字符串、正则、属性访问和函数调用仍是 SunSpider 热点，短期不承诺追平成熟引擎。",
  "耗时比值为 0.625x / 0.112x：AgentJS 正确完成 12 项，但总体吞吐尚未领先。");
replace(13, "测试口径", "轻量化边界");
replace(13, "Test262 是一致性主基准，配合 benchmark、差分测试和人工 code review 防止过拟合。",
  "产物 10.29 MiB，小于 Boa 28.55 MiB、大于 QuickJS 1.09 MiB；个别 workload RSS 峰值偏高。");

replace(14,
  "本队在开发过程中合理使用 AI 工具辅助代码阅读、错误定位建议、测试脚本生成、报告整理和答辩材料初稿。\nAI 产出不直接替代最终判断；代码修改、测试结果和最终结论均以队员复核后的仓库 commit、测试输出和报告文件为准。",
  "本队使用 Codex、Claude Code 等工具辅助代码阅读、任务拆分、失败聚类、表格整理和文字校对。\nAI 输出不直接作为实验结论；实现以源码和测试为准，数据以仓库 JSON / Markdown 原始产物为准。");
replace(14, "OpenAI Codex / GPT-5", "Codex / Claude Code");
replace(14, "代码检索、测试分析、PPT 初稿", "代码阅读、任务拆分、失败聚类");
replace(14, "修复建议、报告脚本、汇报结构", "修复建议、表格整理、文字校对");
replace(14, "队员阅读源码并运行测试", "源码、JSON / MD、测试门禁");

replace(15, "失败分类、runner 继续执行策略、分目录 final 报告命令",
  "失败聚类、runner 行为分析、全量复测模板");
replace(15, "重新运行 final 脚本并核对 JSON / md", "核对统计口径、原始 JSON 与报告边界");
replace(15, "reports/test262-final/*", "Test262-final/full-test262-summary.json");
replace(15, "SunSpider、Boa、QuickJS、Node 对比脚本和热点解释",
  "SunSpider 与 AgentBench 多引擎口径、热点解释");
replace(15, "用 release binary 复跑并保存结果", "release 二进制、哈希、环境与原始样本");
replace(15, "benchmarks/*/results/*", "benchmarks/agent + sunspider/results");
replace(15, "字符串快速路径、运行时错误处理、数组/RegExp 热点定位",
  "字符串、数组窗口、RegExp 与峰值 RSS 热点定位");
replace(15, "队员实现、review、focused tests", "队员实现、review、项目测试门禁");
replace(15, "git commit / reports/.version-report/*", "git commit / 版本报告 / scan JSON");
replace(15, "README、提交报告框架、答辩 PPT 初稿", "实验报告校对、答辩证据与边界整理");
replace(15, "队员真实分工与最终数据", "队员核对分工、数字、命令和结论");
replace(15, "docs / thoughts / outputs", "实验报告 / docs / benchmark results");

replace(16,
  "自研 Native Runtime，默认执行路径不依赖 Boa / QuickJS\n\nTest262 全量通过率 71.78%，超过 60% 赛题要求\n\nSunSpider 1.0.2 全部 26/26 正确通过\n\n面向短时高频脚本执行，保留可复现实验和报告链路",
  "自研 Native Runtime，默认路径不依赖外部引擎\n\nTest262 90.97%，高于门槛 30.97 个百分点\n\nSunSpider 26 / 26；cold 相对 Boa 有竞争力\n\nbatch、字符串热点和峰值 RSS 是下一阶段重点");

const noteExtras = {
  8: "- D:/00_OS/CSCC/Test262-final/full-test262-summary.json",
  10: "- D:/00_OS/CSCC/benchmarks/sunspider/results/agentjs-sunspider.json\n- D:/00_OS/CSCC/benchmarks/sunspider/results/boa-sunspider.json",
  13: "- D:/00_OS/CSCC/benchmarks/agent/manifest.json\n- D:/00_OS/CSCC/benchmarks/agent/run_agentbench.py",
};
for (let slide = 1; slide <= 16; slide += 1) setNotes(slide, noteExtras[slide] ?? "");

const pptx = await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
