import { FileBlob, PresentationFile } from "@oai/artifact-tool";
const input="D:/00_OS/CSCC/presentation/PPT-report-slide10-updated.pptx";
const output="D:/00_OS/CSCC/presentation/PPT-report-slide10-reflow.pptx";
const presentation=await PresentationFile.importPptx(await FileBlob.load(input));

// Remove the fourth metric card: background, headline and caption.
for(const id of ["sh/0bit8b2d","sh/1cru1g3y","sh/e9gb61k7"]){
  presentation.resolve(id).delete();
}

function move(id,left,top,width,height){
  presentation.resolve(id).position={left,top,width,height};
}

// Reflow the three remaining cards into an even three-column composition.
move("sh/ydcnatkn",50.33,220,360,185.47);
move("sh/l036l83y",100,248,260,78);
move("sh/kzu5c32t",75,338,310,34);

move("sh/725onyl4",444,220,360,185.47);
move("sh/m1cnetkj",494,248,260,78);
move("sh/s7yt4b21",469,338,310,34);

move("sh/d87uxg3m",838,220,365,185.47);
move("sh/q5gb21kb",890,248,260,78);
move("sh/r6pcv6lw",863,338,315,34);

const pptx=await PresentationFile.exportPptx(presentation);
await pptx.save(output);
console.log(output);
