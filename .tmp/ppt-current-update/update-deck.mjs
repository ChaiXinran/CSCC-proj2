import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const input = 'D:/00_OS/CSCC/.tmp/ppt-current-update/template-starter.pptx';
const output = 'D:/00_OS/CSCC/presentation/PPT-agentjs-updated.pptx';
const deck = await PresentationFile.importPptx(await FileBlob.load(input));
const snap = await deck.inspect({kind:'textbox,notes',include:'id,slide,name,text,bbox',maxChars:300000});
const records = snap.ndjson.split(/\r?\n/).filter(Boolean).map(JSON.parse);

function target(slide, name) {
  const hit = records.find(r => r.kind === 'textbox' && r.slide === slide && r.name === name);
  if (!hit) throw new Error(`Missing textbox ${slide}:${name}`);
  return deck.resolve(hit.id);
}
function set(slide, name, text) { target(slide, name).text = text; }
function notes(slide, paths) {
  const hit = records.find(r => r.kind === 'notes' && r.slide === slide);
  if (hit) deck.resolve(hit.id).setText(['[Sources]', ...paths.map(p => `- ${p}`)].join('\n'));
}

// Slide 11 — current three-engine JetStream2 evidence.
set(11,'矩形 1','JetStream 2：三引擎公共 workload 对比');
set(11,'矩形 2','六项可移植 JavaScript kernel 全部通过；每项 2 次预热 + 7 次测量，展示 workload P50。');
set(11,'矩形 6','三引擎 6 / 6 通过，全部测量样本有效');
set(11,'矩形 11','27.04');
set(11,'矩形 12','AgentJS 最大峰值 RSS / MiB');
set(11,'矩形 21','QuickJS P50');
set(11,'矩形 25','1.22 s'); set(11,'矩形 26','347 ms'); set(11,'矩形 27','74 ms');
set(11,'矩形 31','4.03 s'); set(11,'矩形 32','527 ms'); set(11,'矩形 33','200 ms');
set(11,'矩形 37','3.72 s'); set(11,'矩形 38','625 ms'); set(11,'矩形 39','116 ms');
set(11,'矩形 41','来源：实验报告 4.7.2；Boa、QuickJS 分别约快 4.95×、21.3×；非官方综合分数');
notes(11,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md','D:/00_OS/CSCC/benchmarks/jetstream/results/four-engine/results.json']);

// Slide 13 — innovation 1, not baseline execution functionality.
set(13,'矩形 2','创新点 1/3：面向 Agent 负载的数据路径');
set(13,'矩形 4','围绕工具结果、规则文本、大索引和短命对象，设计紧凑数据结构与可控快速路径。');
set(13,'矩形 5','分段稠密数组');
set(13,'矩形 6','前 64K 槽位 inline，之后按 4K 惰性分段；大索引避免一次性预分配。');
set(13,'矩形 8','Descriptor 旁路表');
set(13,'矩形 9','普通元素只保存值，特殊属性描述符进入覆盖表，保持常规访问路径紧凑。');
set(13,'矩形 11','短命对象友好回收');
set(13,'矩形 12','非移动 mark-and-sweep 保持对象 ID 稳定，Free List 复用回收槽位。');
set(13,'矩形 14','ASCII 快路 + 有界缓存');
set(13,'矩形 15','扫描、切片、查找与替换优先走 ASCII；LRU 容量 32，复用但不无限增长。');
set(13,'矩形 17','来源：实验报告 1.4、2.5–2.6；AgentBench 仅评价完整系统，不作单机制因果归因');
notes(13,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md']);

// Slide 14 — innovation 2.
set(14,'矩形 2','创新点 2/3：action 级预算与受控 Host');
set(14,'矩形 4','安全边界由运行时预算、进程生命周期和结构化 Host 共同构成，而非字符串黑名单。');
set(14,'矩形 5','action 级资源预算');
set(14,'矩形 6','循环、递归、VM 栈、堆、大对象和 deadline 分层限额；超限返回 RuntimeLimit。');
set(14,'矩形 8','隔离的生命周期');
set(14,'矩形 9','独立 action 使用 fresh isolate / 新进程，避免全局变量、原型与异常状态串扰。');
set(14,'矩形 11','最小能力暴露面');
set(14,'矩形 12','默认无文件、网络、进程、DOM 与 Node API；加载器必须显式安装并限制根目录。');
set(14,'矩形 14','受控 RenderTree');
set(14,'矩形 15','agent.render 校验类型、循环、JSON、深度和字节数；前端只消费规范事件。');
set(14,'矩形 17','来源：实验报告 1.5、2.5–2.7；安全结论限定于所述 API 暴露面和超时策略');
notes(14,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md']);

// Slide 15 — innovation 3 and Agent Demo complement.
set(15,'矩形 2','创新点 3/3：面向 Agent action 的 AgentBench 2.0');
set(15,'矩形 4','不是照搬浏览器 benchmark：从真实 Agent 工作流抽象任务，并用 Demo 验证调用闭环。');
set(15,'矩形 5','12 个确定性 action');
set(15,'矩形 6','覆盖 JSON、工具聚合、规则过滤、对象压力、Descriptor、大索引与字符串处理。');
set(15,'矩形 8','正确性先于性能');
set(15,'矩形 9','仅结果正确、无 error / timeout 的样本进入统计；共同通过 case 取几何平均。');
set(15,'矩形 11','四类指标各自回答问题');
set(15,'矩形 12','cold 看单次等待，batch 看连续吞吐，RSS 看进程内存，体积看部署成本。');
set(15,'矩形 14','Agent Demo 补齐场景闭环');
set(15,'矩形 15','输入 → Orchestrator → Native CLI → value / logs / RenderTree → 前端；不混入性能统计。');
set(15,'矩形 17','来源：实验报告 1.6、4.1–4.3、4.8；benchmarks/agent、demo/agent');
notes(15,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md','D:/00_OS/CSCC/benchmarks/agent/manifest.json','D:/00_OS/CSCC/demo/agent']);

// Slide 16 — concise performance synthesis.
set(16,'矩形 7','AgentBench cold：Boa / AgentJS = 1.090×；startup-noop 为 13.9 ms vs 20.0 ms，短 action 具备竞争力。');
set(16,'矩形 10','AgentBench batch：Boa / AgentJS = 0.625×，QuickJS / AgentJS = 0.112×；连续吞吐仍需优化。');
set(16,'矩形 13','JetStream 六项全样本通过；Boa、QuickJS 分别约快 4.95×、21.3×；AgentJS 峰值 RSS 27.04 MiB。');
notes(16,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md']);

// Slide 17 — current AgentBench ratios and binary sizes.
set(17,'矩形 7','耗时：Boa / AgentJS = 1.090×，QuickJS / AgentJS = 0.186×；AgentJS 优于 Boa，仍落后 QuickJS。');
set(17,'矩形 10','耗时：Boa / AgentJS = 0.625×，QuickJS / AgentJS = 0.112×；三方均正确完成 12 / 12。');
set(17,'矩形 13','产物：AgentJS 10.29 MiB，Boa 28.55 MiB，QuickJS 1.09 MiB；体积与峰值 RSS 分开评价。');
notes(17,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md','D:/00_OS/CSCC/benchmarks/agent/results/four-engine-comparison/agentjs-cold.json']);

// Slide 20 — closing synthesis.
set(20,'矩形 34','Test262 90.98%，SunSpider 26 / 26\n\n三项创新：Agent 数据路径、action 级受控 Host、AgentBench 2.0\n\nAgentBench：cold 对 Boa 有竞争力，batch 仍需追赶\n\nJetStream：三引擎六项全样本通过，复杂计算仍需优化');
notes(20,['D:/00_OS/CSCC/reports/agentjs-experiment-report.md']);

const pptx = await PresentationFile.exportPptx(deck);
await pptx.save(output);
const after = await deck.inspect({kind:'slide,textbox,notes',include:'id,slide,name,text,title,bbox',maxChars:300000});
await fs.writeFile('D:/00_OS/CSCC/.tmp/ppt-current-update/final-inspect.ndjson', after.ndjson, 'utf8');
