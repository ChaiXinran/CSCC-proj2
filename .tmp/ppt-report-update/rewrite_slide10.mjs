import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const input="D:/00_OS/CSCC/presentation/PPT-report-innovation-updated.pptx";
const output="D:/00_OS/CSCC/presentation/PPT-report-slide10-updated.pptx";
const presentation=await PresentationFile.importPptx(await FileBlob.load(input));
const snapshot=await presentation.inspect({kind:"textbox,notes",include:"id,slide,bbox",maxChars:250000});
const records=snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line)=>JSON.parse(line));
function box(x,y,text){
  const found=records.filter((r)=>r.kind==="textbox"&&r.slide===10&&r.bbox).reduce((best,r)=>{
    const d=Math.abs(r.bbox[0]-x)+Math.abs(r.bbox[1]-y); return !best||d<best.d?{r,d}:best;
  },null);
  if(!found||found.d>8) throw new Error(`Missing 10:${x},${y}`);
  presentation.resolve(found.r.id).text=text;
}

box(56,40,"SunSpider：先证明兼容，再定位热点");
box(56,106,"AgentJS 26 / 26 PASS；两引擎结果来自非同步历史批次，因此只用于诊断热点，不进行整体性能排名。");
box(94.47,248,"26/26");
box(57.87,338,"全部用例正确完成");
box(398.07,248,"9 类");
box(375.4,344.87,"覆盖全部测试类别");
box(715.6,248,"0");
box(681.47,344.87,"错误 / 超时");
box(1001.67,246.6,"诊断");
box(995.33,338,"历史数据不做排名");

box(88,449,"代表用例");
box(388,449,"AgentJS 历史 P50");
box(568,449,"Boa 历史 P50");
box(748,449,"AgentJS 相对耗时");
box(928,449,"工程结论");

box(88,495,"bitops-bitwise-and");
box(388,495,"262 ms");
box(568,495,"286 ms");
box(748,495,"0.92×");
box(928,495,"位运算路径表现相近");

box(88,541,"regexp-dna");
box(388,541,"3,098 ms");
box(568,541,"106 ms");
box(748,541,"29.2×");
box(928,541,"正则匹配与替换是热点");

box(88,587,"string-tagcloud");
box(388,587,"8,208 ms");
box(568,587,"148 ms");
box(748,587,"55.5×");
box(928,587,"字符串与属性访问需优化");
box(56,664,"来源：agentjs-sunspider.json、boa-sunspider.json；历史批次非同步，倍率仅用于诊断");

const note=records.find((r)=>r.kind==="notes"&&r.slide===10);
presentation.resolve(note.id).setText([
  "[Sources]",
  "- D:/00_OS/CSCC/presentation/assets/video/sunspider-video.json",
  "- D:/00_OS/CSCC/presentation/assets/video/sunspider-video.md",
  "- D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(5).md",
  "- Historical AgentJS and Boa batches are not synchronized; timings are diagnostic only.",
].join("\n"));

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
