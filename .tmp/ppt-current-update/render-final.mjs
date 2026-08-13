import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const deck = await PresentationFile.importPptx(
  await FileBlob.load('D:/00_OS/CSCC/presentation/PPT-agentjs-updated.pptx'),
);
const out = 'D:/00_OS/CSCC/.tmp/ppt-current-update/final-render';
const layouts = 'D:/00_OS/CSCC/.tmp/ppt-current-update/final-layout';
await fs.mkdir(out,{recursive:true}); await fs.mkdir(layouts,{recursive:true});
for (const [i,slide] of deck.slides.items.entries()) {
  const stem=`slide-${String(i+1).padStart(2,'0')}`;
  const png=await deck.export({slide,format:'png',scale:1.5});
  await fs.writeFile(`${out}/${stem}.png`,new Uint8Array(await png.arrayBuffer()));
  const layout=await slide.export({format:'layout'});
  await fs.writeFile(`${layouts}/${stem}.layout.json`,await layout.text(),'utf8');
}
const montage=await deck.export({format:'png',montage:true,scale:0.5});
await fs.writeFile(`${out}/montage.png`,new Uint8Array(await montage.arrayBuffer()));
