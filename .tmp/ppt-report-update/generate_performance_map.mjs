import fs from "node:fs/promises";
const input = "D:/00_OS/CSCC/.tmp/ppt-report-update/template-frame-map.json";
const output = "D:/00_OS/CSCC/.tmp/ppt-report-update/performance-frame-map.json";
const map = JSON.parse(await fs.readFile(input, "utf8"));
const result = [];
for (const entry of map.outputSlides) {
  result.push({...entry, outputSlide: result.length + 1});
  if (entry.sourceSlide === 10) {
    result.push({...entry, outputSlide: result.length + 1, narrativeRole: "four-engine JetStream kernel performance"});
  }
  if (entry.sourceSlide === 12) {
    const source13 = map.outputSlides.find((item) => item.sourceSlide === 13);
    result.push({...source13, outputSlide: result.length + 1, narrativeRole: "cross-mode performance profile"});
  }
}
map.outputSlides = result;
await fs.writeFile(output, JSON.stringify(map, null, 2), "utf8");
const baselineOutput = "D:/00_OS/CSCC/.tmp/ppt-report-update/performance-baseline-map.json";
map.outputSlides = result.map((entry) => ({...entry, narrativeRole: "preserve template frame", editTargets: []}));
await fs.writeFile(baselineOutput, JSON.stringify(map, null, 2), "utf8");
console.log(output);
