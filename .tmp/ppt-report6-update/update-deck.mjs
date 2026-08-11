import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const input = 'D:/00_OS/CSCC/.tmp/ppt-report6-update/template-starter.pptx';
const output = 'D:/00_OS/CSCC/presentation/PPT-report-report6-data-update.pptx';

const deck = await PresentationFile.importPptx(await FileBlob.load(input));
const snap = await deck.inspect({
  kind: 'slide,textbox,shape,notes',
  include: 'id,slide,name,text,title',
  maxChars: 120000,
});
const records = snap.ndjson.trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);

function record(slide, name, kind = 'textbox') {
  const hit = records.find(r => r.kind === kind && r.slide === slide && r.name === name);
  if (!hit) throw new Error(`Missing ${kind} on slide ${slide}: ${name}`);
  return hit;
}

function setText(slide, name, text) {
  const target = deck.resolve(record(slide, name).id);
  target.text = text;
}

// Opening, positioning, and progress.
setText(1, '矩形 22', '90.98%');
setText(3, '矩形 6', 'AgentJS 产物 10.39 MiB，为 Boa 的 36.68%；宿主仅保留 print、console 与受控 Host。');
setText(5, '矩形 25', '当前状态：Native 执行链与 Agent Host 已闭环，Test262 90.98%，SunSpider 26/26；\n交付重点转向 batch 吞吐、字符串热点、峰值 RSS 与可复现元数据。');

// Test262 summary.
setText(8, '矩形 2', '报告汇总通过率 90.98%，高于 60% 门槛 30.98 个百分点。');
setText(8, '矩形 5', '48,566');
setText(8, '矩形 8', '90.98%');
setText(8, '矩形 14', '444s');
setText(8, '矩形 28', '48,566');
setText(8, '矩形 29', '4,811');
setText(8, '矩形 30', '2');
setText(8, '矩形 31', '90.98%');
setText(8, '矩形 32', '444.347s');
setText(8, '矩形 47', '+30.98 pp');
setText(8, '矩形 49', '来源：实验报告（第 6 版）；报告统计未绑定当前 HEAD 与完整运行环境');

setText(9, '矩形 1', 'Test262：从 26.29% 到 90.98%');
setText(9, '矩形 4', '从 26.29% baseline 提升到 90.98%，累计增加 64.69 个百分点，并超过 60% 赛题门槛 30.98 个百分点。\n\n最终汇总：通过 48,566 / 总计 53,379；失败 4,811，跳过 2。失败与跳过均不计通过。');
setText(9, '矩形 16', '90.98%');
setText(9, '矩形 19', '可以证明\n• 90.98% > 60%\n• 失败/跳过未计通过\n\n不能证明\n• 对应当前 HEAD\n• 完整复现实验环境');

// Synchronized SunSpider batch.
setText(10, '矩形 2', 'AgentJS 与 Boa 同次运行均为 26 / 26 PASS；每项 3 次测量，P50 用于定位热点，不作跨平台排名。');
setText(10, '矩形 19', 'AgentJS 同步 P50');
setText(10, '矩形 20', 'Boa 同步 P50');
setText(10, '矩形 25', '299.9 ms');
setText(10, '矩形 26', '269.5 ms');
setText(10, '矩形 27', '1.11×');
setText(10, '矩形 28', '位运算路径耗时接近');
setText(10, '矩形 31', '126.3 ms');
setText(10, '矩形 32', '107.3 ms');
setText(10, '矩形 33', '1.18×');
setText(10, '矩形 34', 'RegExp 差距已明显收敛');
setText(10, '矩形 37', '431.8 ms');
setText(10, '矩形 38', '146.2 ms');
setText(10, '矩形 39', '2.95×');
setText(10, '矩形 40', '复杂字符串与对象仍是热点');
setText(10, '矩形 41', '来源：sunspider-video.json；同次 Runner、每项 3 次，未保存机器与二进制指纹');

// Report-6 AgentJS / Boa JetStream kernel comparison.
setText(11, '矩形 1', 'JetStream 2：AgentJS / Boa kernel 对照');
setText(11, '矩形 2', '六项 self-contained kernel 双方均正确完成；同构 Runner，2 次预热 + 7 次测量，展示 workload P50。');
setText(11, '矩形 6', '双方共同通过，全部测量样本有效');
setText(11, '矩形 21', 'Boa / AgentJS');
setText(11, '矩形 25', '1,316 ms');
setText(11, '矩形 26', '360 ms');
setText(11, '矩形 27', '0.274×');
setText(11, '矩形 31', '4,324 ms');
setText(11, '矩形 32', '673 ms');
setText(11, '矩形 33', '0.156×');
setText(11, '矩形 37', '4,934 ms');
setText(11, '矩形 38', '713 ms');
setText(11, '矩形 39', '0.145×');
setText(11, '矩形 41', '来源：实验报告（第 6 版）；六项几何平均 0.192×，不是浏览器官方综合分数');

// Performance synthesis and AgentBench boundary.
setText(16, '矩形 7', 'AgentBench cold：Boa / AgentJS = 1.097×；startup-noop 为 13.9 ms vs 20.0 ms，短任务具备竞争力。');
setText(16, '矩形 10', 'AgentBench batch：Boa / AgentJS = 0.619×；AgentJS 总体约慢 1.62×，连续执行吞吐仍需优化。');
setText(16, '矩形 13', 'JetStream 六项双方全样本通过；Boa 约快 5.21×；峰值 RSS 为 AgentJS 27.00、Boa 16.04 MiB。');

setText(17, '矩形 7', '耗时 Boa / AgentJS = 1.097×，RSS = 1.123×：AgentJS 在这组 cold 短任务中更快且总体峰值更低。');
setText(17, '矩形 10', '耗时 Boa / AgentJS = 0.619×，RSS = 0.979×：12 项均正确，但总体吞吐未领先，内存接近。');
setText(17, '矩形 13', '产物 10.39 MiB，为 Boa 28.32 MiB 的 36.68%；较小二进制不代表每个 workload 都有更低 RSS。');

setText(20, '矩形 34', 'Test262 90.98%，SunSpider 26 / 26\n\nJetStream 双引擎六项全样本通过\n\n三层创新：执行架构、受控运行时、可审计评测\n\ncold 短任务有竞争力，batch 与复杂内核仍需追赶');

const pptx = await PresentationFile.exportPptx(deck);
await pptx.save(output);

const after = await deck.inspect({ kind: 'slide,textbox,notes', include: 'id,slide,name,text,title', maxChars: 120000 });
await fs.writeFile('D:/00_OS/CSCC/.tmp/ppt-report6-update/final-inspect.ndjson', after.ndjson, 'utf8');
