import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const p=await PresentationFile.importPptx(await FileBlob.load("D:/00_OS/CSCC/presentation/PPT-report-slide10-reflow.pptx"));
const s=await p.inspect({kind:"textbox,shape,notes",include:"id,slide,name,text,bbox",maxChars:200000});
for(const line of s.ndjson.split(/\r?\n/).filter(Boolean)){
  const r=JSON.parse(line); if(r.slide===9) console.log(JSON.stringify(r));
}
