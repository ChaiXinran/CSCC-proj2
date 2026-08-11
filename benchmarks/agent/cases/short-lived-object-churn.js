// AgentBench pressure: short-lived records created by an Agent action.
var checksum = 0;
// 12,000 total records cross the default allocation/GC checkpoint while
// keeping one pressure sample short enough for repeated statistical runs.
var rounds = 4;
var width = 3000;
var i;

for (var round = 0; round < rounds; round++) {
    var batch = [];
    for (i = 0; i < width; i++) {
        var item = { value: (i * 31 + round) & 1023, ok: (i & 7) !== 0 };
        if (item.ok) checksum += item.value;
        batch.push(item);
    }
    if (batch.length !== width) throw "ERROR: bad churn batch";
}
if (checksum <= 0) throw "ERROR: bad churn checksum";
