import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const path = "D:/00_OS/CSCC/presentation/PPT-report-updated.pptx";
const presentation = await PresentationFile.importPptx(await FileBlob.load(path));
const snapshot = await presentation.inspect({
  kind: "textbox",
  include: "id,slide,name,text",
  maxChars: 100000,
});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));

function setText(slide, name, text) {
  const record = records.find((item) => item.kind === "textbox" && item.slide === slide && item.name === name);
  if (!record) throw new Error(`Missing ${name} on slide ${slide}`);
  presentation.resolve(record.id).text = text;
}

setText(5, "矩形 25",
  "当前状态：Native 执行链与 Agent Host 已闭环，Test262 90.97%，SunSpider 26/26；\n交付重点转向 batch 吞吐、字符串热点、峰值 RSS 与可复现元数据。");

setText(8, "矩形 14", "452s");
setText(8, "矩形 38", "—");
setText(8, "矩形 46", "—");

setText(10, "矩形 14", "历史");
setText(10, "矩形 15", "非同步批次，仅定位热点");

setText(16, "矩形 34",
  "自研 Native Runtime，默认路径不依赖外部引擎\n\nTest262 90.97%，高于门槛 30.97 个百分点\n\nSunSpider 26 / 26；cold 相对 Boa 有竞争力\n\nbatch、字符串热点和峰值 RSS 是下一阶段重点");

const pptx = await PresentationFile.exportPptx(presentation);
await pptx.save(path);
console.log(path);
