import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const path = "D:/00_OS/CSCC/presentation/PPT-report-performance-updated.pptx";
const presentation = await PresentationFile.importPptx(await FileBlob.load(path));
const snapshot = await presentation.inspect({kind:"textbox",include:"id,slide,bbox",maxChars:200000});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));

function box(slide, x, y, text) {
  const candidates = records.filter((r) => r.kind === "textbox" && r.slide === slide && r.bbox);
  const found = candidates.reduce((best, r) => {
    const d = Math.abs(r.bbox[0] - x) + Math.abs(r.bbox[1] - y);
    return !best || d < best.d ? {r, d} : best;
  }, null);
  if (!found || found.d > 8) throw new Error(`Missing ${slide}:${x},${y}`);
  presentation.resolve(found.r.id).text = text;
}

box(11, 398.07, 248, "0.95");
box(11, 375.4, 344.87, "Oxide / AgentJS 时间比");
box(11, 715.6, 248, "27.0");
box(11, 681.47, 344.87, "AgentJS 峰值 RSS / MiB");

box(13, 66, 255.87, "lexer → parser → bytecode → VM → runtime；稳定 contracts 解耦阶段，默认不嵌入 Boa 回退。");
box(13, 650, 258, "仅共同 PASS 且确定性输出正确的样本进入统计；超时、错误与内存上限失败均保留。");
box(13, 66, 451.07, "12 个 Agent 型 workload 覆盖 JSON、规则过滤、字符串与对象压力；统一测 cold、batch、RSS。");
box(13, 650, 451.07, "冻结 console 与受控 Host；分段数组、descriptor 旁路表、ASCII 快路与 arena 复用。");

box(18, 851.67, 238.53,
  "Test262 90.97%，SunSpider 26 / 26\n\nJetStream 四引擎公共内核 6 / 6 PASS\n\n冷启动具备亮点，批处理与复杂内核仍需追赶\n\n创新：Native 全链路、最小宿主、可审计 benchmark");
for (const record of records.filter((r) => r.kind === "textbox" && r.slide === 18 && r.bbox && r.bbox[0] >= 1160 && r.bbox[1] >= 650)) {
  presentation.resolve(record.id).text = "18";
}

const pptx = await PresentationFile.exportPptx(presentation);
await pptx.save(path);
console.log(path);
