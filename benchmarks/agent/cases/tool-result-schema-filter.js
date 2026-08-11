// AgentBench general: validate nested records returned by a tool.
var records = [];
var n = 3200;
var accepted = 0;
var score = 0;
var i;

for (i = 0; i < n; i++) {
    records.push({
        id: "task-" + i,
        meta: { source: i % 3 === 0 ? "search" : "db", version: 1 },
        result: { ok: i % 11 !== 0, score: (i * 19) % 101, tags: ["agent", i % 2 ? "fast" : "safe"] }
    });
}

for (i = 0; i < records.length; i++) {
    var item = records[i];
    if (item.meta && item.meta.version === 1 && item.result && item.result.ok &&
        item.result.score >= 50 && item.result.tags.indexOf("agent") >= 0) {
        accepted++;
        score += item.result.score;
    }
}
if (accepted < 1000 || score <= 0) throw "ERROR: bad schema filter";
