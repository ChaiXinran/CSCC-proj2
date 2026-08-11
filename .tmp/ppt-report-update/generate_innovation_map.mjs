import fs from "node:fs/promises";
const output = "D:/00_OS/CSCC/.tmp/ppt-report-update/innovation-baseline-map.json";
const sourceSlides = [1,2,3,4,5,6,7,8,9,10,11,12,13,13,13,14,15,16,17,18];
const map = {
  outputSlides: sourceSlides.map((sourceSlide, index) => ({
    outputSlide: index + 1,
    sourceSlide,
    narrativeRole: "preserve template frame",
    reuseMode: "duplicate-slide",
    editTargets: [],
  })),
  omittedSourceSlides: [],
};
await fs.writeFile(output, JSON.stringify(map, null, 2), "utf8");
console.log(output);
