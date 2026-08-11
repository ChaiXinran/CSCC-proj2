import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const path = "D:/00_OS/CSCC/presentation/PPT-report-innovation-updated.pptx";
const presentation = await PresentationFile.importPptx(await FileBlob.load(path));
const snapshot = await presentation.inspect({kind:"textbox",include:"id,slide,bbox",maxChars:250000});
const records = snapshot.ndjson.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
function box(slide,x,y,text){
  const found=records.filter((r)=>r.kind==="textbox"&&r.slide===slide&&r.bbox).reduce((best,r)=>{
    const d=Math.abs(r.bbox[0]-x)+Math.abs(r.bbox[1]-y); return !best||d<best.d?{r,d}:best;
  },null);
  if(!found||found.d>8) throw new Error(`Missing ${slide}:${x},${y}`);
  presentation.resolve(found.r.id).text=text;
}

box(13,66,48,"创新点 1/3：Native 执行架构");
box(13,66,255.87,"自研 Lexer、Parser、字节码、VM 和运行时；外部引擎仅作参照。");
box(13,650,258,"Engine 为独立 action 创建 fresh isolate；Runtime 为关联调用保留会话状态。");
box(13,66,451.07,"Parser、Compiler、Executor 通过稳定接口协作，可注入 fake stage 分层测试。");
box(13,650,451.07,"Chunk 校验栈、跳转与异常区间；报告统一承载结果和分类失败。");

box(14,66,48,"创新点 2/3：受控运行时与 Host");
box(14,66,255.87,"循环、递归、VM 栈、堆、大对象和 deadline 分层限额；超限返回 RuntimeLimit。");
box(14,650,258,"默认无文件、网络、进程、DOM 与 Node API；加载器显式安装并限制根目录。");
box(14,66,451.07,"分段数组、descriptor 旁路、ASCII 快路、非移动 GC 与 Free List 复用。");
box(14,650,451.07,"agent.render 校验类型、循环、JSON、深度和字节；前端只消费规范事件。");

box(15,66,48,"创新点 3/3：AgentBench 评测体系");
box(15,66,255.87,"12 个确定性 action 覆盖 JSON、规则过滤、对象压力、大索引数组与字符串任务。");
box(15,650,258,"仅共同 PASS、无 error/timeout 且结果正确的 case 进入几何平均。");
box(15,66,451.07,"统一 P50、耗时比和 RSS；batch 不等同 persistent Runtime 或缓存消融。");
box(15,650,451.07,"JSON、环境、命令、revision、SHA256 绑定批次；历史结果不套用当前 HEAD。");

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(path);
console.log(path);
