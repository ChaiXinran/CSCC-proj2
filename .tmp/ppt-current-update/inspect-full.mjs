import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const deck = await PresentationFile.importPptx(
  await FileBlob.load('D:/00_OS/CSCC/presentation/PPT.pptx'),
);
const snap = await deck.inspect({
  kind: 'slide,textbox,shape,notes',
  include: 'id,slide,name,text,title,bbox,isPlaceholder',
  maxChars: 300000,
});
await fs.writeFile('D:/00_OS/CSCC/.tmp/ppt-current-update/full-inspect.ndjson', snap.ndjson, 'utf8');
