import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const source = 'D:/00_OS/CSCC/presentation/PPT-report-final-three-engines.pptx';
const deck = await PresentationFile.importPptx(await FileBlob.load(source));
const result = await deck.inspect({
  kind: 'slide,textbox,shape,notes',
  include: 'id,slide,name,title,text,textPreview,textChars,textLines,bbox,isPlaceholder',
  maxChars: 100000,
});
await fs.writeFile('D:/00_OS/CSCC/.tmp/ppt-report6-update/full-inspect.ndjson', result.ndjson, 'utf8');
