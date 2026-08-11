import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const input="D:/00_OS/CSCC/presentation/PPT-report-three-engines.pptx";
const output="D:/00_OS/CSCC/presentation/PPT-report-final-three-engines.pptx";
const presentation=await PresentationFile.importPptx(await FileBlob.load(input));
const snapshot=await presentation.inspect({kind:"textbox",include:"id,slide,bbox",maxChars:300000});
const records=snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line)=>JSON.parse(line));
function box(slide,x,y,text){
  const found=records.filter((r)=>r.kind==="textbox"&&r.slide===slide&&r.bbox).reduce((best,r)=>{
    const d=Math.abs(r.bbox[0]-x)+Math.abs(r.bbox[1]-y); return !best||d<best.d?{r,d}:best;
  },null);
  if(!found||found.d>8) throw new Error(`Missing ${slide}:${x},${y}`);
  presentation.resolve(found.r.id).text=text;
}

box(9,56,106,"归档结果表明通过率超过赛题门槛；由于缺少完整运行元数据，本页不把历史结果绑定到当前源码。");
box(9,54.59,213,
  "从 26.29% baseline 提升到 90.97%，累计增加 64.68 个百分点，并超过 60% 赛题门槛 30.97 个百分点。\n\n最终汇总：通过 48,557 / 总计 53,379；失败 4,820，跳过 2。失败与跳过均不计通过。");
box(9,934.53,332.2,"可以证明\n• 90.97% > 60%\n• 失败/跳过未计通过\n\n不能证明\n• 对应当前 HEAD\n• 完整复现实验环境");

box(16,285.8,420.2,"归档三引擎 batch：Boa / AgentJS = 0.625×，QuickJS / AgentJS = 0.112×；优先减少重复解析与调度成本。");
box(20,851.67,238.53,
  "Test262 90.97%，SunSpider 26 / 26\n\nJetStream 三引擎公共集 6 / 6 PASS\n\n三层创新：执行架构、受控运行时、可审计评测\n\n冷启动有亮点，批处理与复杂 workload 仍需追赶");

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
