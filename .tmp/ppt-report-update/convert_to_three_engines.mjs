import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const input="D:/00_OS/CSCC/presentation/PPT-report-slide9-redesigned.pptx";
const output="D:/00_OS/CSCC/presentation/PPT-report-three-engines.pptx";
const presentation=await PresentationFile.importPptx(await FileBlob.load(input));
const snapshot=await presentation.inspect({kind:"textbox,notes",include:"id,slide,bbox,text",maxChars:300000});
const records=snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line)=>JSON.parse(line));
function box(slide,x,y,text){
  const found=records.filter((r)=>r.kind==="textbox"&&r.slide===slide&&r.bbox).reduce((best,r)=>{
    const d=Math.abs(r.bbox[0]-x)+Math.abs(r.bbox[1]-y); return !best||d<best.d?{r,d}:best;
  },null);
  if(!found||found.d>8) throw new Error(`Missing ${slide}:${x},${y}`);
  presentation.resolve(found.r.id).text=text;
}
function move(id,left,top,width,height){presentation.resolve(id).position={left,top,width,height};}

box(11,56,40,"JetStream 2：三引擎公共 workload 对比");
box(11,56,106,"选取 AgentJS、Boa、QuickJS 都能正确完成的 6 个 JavaScript workload；同机 Release，2 次预热 + 7 次测量，展示 P50。");
box(11,57.87,338,"三个引擎共同通过，结果确定性校验");

// Remove the Oxide ratio card and reflow the remaining three metrics.
for(const id of ["sh/72t03qp0","sh/61kzalof","sh/ofqtgnyt"]){presentation.resolve(id).delete();}
move("sh/250zy18b",50.33,220,360,185.47);
move("sh/98rytw72",100,248,260,78);
move("sh/o7ih0r6h",75,338,310,34);
move("sh/9gza9sze",444,220,360,185.47);
move("sh/mdobedg3",494,248,260,78);
move("sh/nehs7iho",469,338,310,34);
move("sh/0b6tc3yx",838,220,365,185.47);
move("sh/1cfa58zi",890,248,260,78);
move("sh/epobatgr",863,338,315,34);

// Remove the Oxide table column and spread the remaining four columns.
for(const id of ["sh/65wzelo7","sh/4zyhofml","sh/z2hw72lc","sh/ip8bmtor"]){presentation.resolve(id).delete();}
const cols=[
  ["sh/dofa18z6","sh/k3uhcb6h","sh/n61wbmlo","sh/14ze9c3i",88,300],
  ["sh/tsnip0ny","sh/j2lgjq5w","sh/o7axk7m9","sh/ydcfq1kb",428,220],
  ["sh/87ehgv6d","sh/y1czalob","sh/98jedc3u","sh/jylwj6lw",688,200],
  ["sh/7650nq5s","sh/p07ixkn6","sh/eh8fex47","sh/3qhsfypc",928,230],
];
for(const [h,r1,r2,r3,x,w] of cols){
  move(h,x,449,w,30); move(r1,x,495,w,26); move(r2,x,541,w,26); move(r3,x,587,w,26);
}
box(11,56,664,"来源：JetStream 2 公共 workload 归档 JSON；三引擎数据来自同机同批次测量");

// Remove the remaining Oxide comparison from the performance summary.
box(16,273.37,556.6,"JetStream 公共集 6/6 PASS；Boa、QuickJS 分别约快 4.95×、21.3×；AgentJS 峰值 RSS 为 27.04 MiB。");

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
