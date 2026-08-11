import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const p = await PresentationFile.importPptx(await FileBlob.load("D:/00_OS/CSCC/presentation/PPT-report-performance-updated.pptx"));
const s = await p.inspect({kind:"textbox,notes",include:"id,slide,name,bbox",maxChars:200000});
for (const line of s.ndjson.split(/\r?\n/).filter(Boolean)) {
  const r = JSON.parse(line);
  if ([13,18].includes(r.slide)) console.log(JSON.stringify({slide:r.slide,id:r.id,name:r.name,bbox:r.bbox}));
}
