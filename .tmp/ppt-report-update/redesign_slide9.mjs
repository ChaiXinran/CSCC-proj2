import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const input="D:/00_OS/CSCC/presentation/PPT-report-slide10-reflow.pptx";
const output="D:/00_OS/CSCC/presentation/PPT-report-slide9-redesigned.pptx";
const presentation=await PresentationFile.importPptx(await FileBlob.load(input));
const snapshot=await presentation.inspect({kind:"textbox,notes",include:"id,slide,bbox",maxChars:250000});
const records=snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line)=>JSON.parse(line));
function box(x,y,text){
  const found=records.filter((r)=>r.kind==="textbox"&&r.slide===9&&r.bbox).reduce((best,r)=>{
    const d=Math.abs(r.bbox[0]-x)+Math.abs(r.bbox[1]-y); return !best||d<best.d?{r,d}:best;
  },null);
  if(!found||found.d>8) throw new Error(`Missing 9:${x},${y}`);
  presentation.resolve(found.r.id).text=text;
}

box(56,40,"Test262：从 26.29% 到 90.97%");
box(56,106,"进展结论清楚，证据边界也要清楚：归档结果证明超过赛题门槛，但不能反向绑定当前 HEAD。");
box(54.59,213,
  "从 26.29% baseline 提升到 90.97%，累计增加 64.68 个百分点，并超过 60% 赛题门槛 30.97 个百分点。\n\n最终汇总：48,557 passed / 53,379 total；4,820 failed，2 skipped。失败与跳过均不计通过。");
box(53.09,459.93,"早期全量 baseline");
box(53.09,511.93,"赛题兼容性门槛");
box(53.09,563.93,"最新归档全量汇总");
box(934.51,278.95,"证据边界");
box(934.53,332.2,
  "可以证明\n• 通过率超过 60%\n• 失败与跳过未计通过\n\n不能证明\n• 历史运行对应当前 HEAD\n• 已保存完整机器与命令");
box(56,664,"来源：实验报告 4.3–4.4、Test262 全量汇总 JSON；历史批次缺少完整运行元数据");

const note=records.find((r)=>r.kind==="notes"&&r.slide===9);
presentation.resolve(note.id).setText([
  "[Sources]",
  "- D:/电脑管家迁移文件/xwechat_files/wxid_l8rdiom7mvqf32_9d98/msg/file/2026-08/agentjs-experiment-report(5).md",
  "- D:/00_OS/CSCC/Test262-final/full-test262-summary.json",
  "- Archived Test262 summary does not contain the execution commit, command, machine fingerprint, or suite revision.",
].join("\n"));

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
