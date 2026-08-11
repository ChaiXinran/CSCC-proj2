import fs from "node:fs/promises";
import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const source = process.argv[2] ?? "D:/00_OS/CSCC/presentation/PPT.pptx";
const output = process.argv[3] ?? "D:/00_OS/CSCC/.tmp/ppt-report-update/full-inspect.ndjson";
const presentation = await PresentationFile.importPptx(await FileBlob.load(source));
const snapshot = await presentation.inspect({
  kind: "slide,textbox,shape,image,table,chart,notes,layout",
  include: "id,slide,name,title,text,textPreview,textChars,textLines,bbox,bboxUnit,isPlaceholder,placeholders",
  maxChars: 200000,
});
await fs.writeFile(output, snapshot.ndjson, "utf8");
console.log(output);
