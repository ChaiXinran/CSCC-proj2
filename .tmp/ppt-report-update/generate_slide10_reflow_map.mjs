import fs from "node:fs/promises";
const output="D:/00_OS/CSCC/.tmp/ppt-report-update/slide10-reflow-map.json";
const map={
  outputSlides:Array.from({length:20},(_,index)=>({outputSlide:index+1,sourceSlide:index+1,narrativeRole:"preserve template frame",reuseMode:"duplicate-slide",editTargets:[]})),
  omittedSourceSlides:[]
};
map.outputSlides[9].narrativeRole="SunSpider compatibility and diagnostic hotspots";
map.outputSlides[9].editTargets=[
  {
    action:"rewrite-and-reposition",
    sourceElementIds:["sh/ydcnatkn","sh/l036l83y","sh/kzu5c32t","sh/725onyl4","sh/m1cnetkj","sh/s7yt4b21","sh/d87uxg3m","sh/q5gb21kb","sh/r6pcv6lw"],
    reason:"reflow three inherited metric cards evenly after removing the redundant diagnostic card"
  },
  {
    action:"delete",
    sourceElementIds:["sh/0bit8b2d","sh/1cru1g3y","sh/e9gb61k7"],
    reason:"remove the redundant diagnostic card requested by the user"
  }
];
await fs.writeFile(output,JSON.stringify(map,null,2),"utf8");
console.log(output);
