import fs from "node:fs/promises";
import path from "node:path";
import { FileBlob, PresentationFile } from "@oai/artifact-tool";

const input = process.argv[2] ?? "D:/00_OS/CSCC/presentation/PPT-report-updated.pptx";
const outputDir = process.argv[3] ?? "D:/00_OS/CSCC/.tmp/ppt-report-update/final-layout";

const presentation = await PresentationFile.importPptx(await FileBlob.load(input));
await fs.mkdir(outputDir, { recursive: true });
const slides = Array.isArray(presentation.slides?.items)
  ? presentation.slides.items
  : Array.from({ length: presentation.slides.count }, (_, index) => presentation.slides.getItem(index));

for (let index = 0; index < slides.length; index += 1) {
  const slide = slides[index];
  const blob = await presentation.export({ slide, format: "layout" });
  const filename = `final-slide-${String(index + 1).padStart(3, "0")}.layout.json`;
  await fs.writeFile(path.join(outputDir, filename), Buffer.from(await blob.arrayBuffer()));
}

console.log(`Exported ${slides.length} layouts to ${outputDir}`);
