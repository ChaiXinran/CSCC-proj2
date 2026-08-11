import fs from 'node:fs/promises';
import { FileBlob, PresentationFile } from '@oai/artifact-tool';

const deck = await PresentationFile.importPptx(await FileBlob.load('D:/00_OS/CSCC/presentation/PPT-report-report6-data-update.pptx'));
const out = 'D:/00_OS/CSCC/.tmp/ppt-report6-update/final-layout';
await fs.mkdir(out, { recursive: true });
for (let i = 0; i < deck.slides.items.length; i++) {
  const blob = await deck.slides.items[i].export({ format: 'layout' });
  await fs.writeFile(`${out}/slide-${String(i + 1).padStart(2, '0')}.layout.json`, await blob.text(), 'utf8');
}
