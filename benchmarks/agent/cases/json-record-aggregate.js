// AgentBench general: aggregate structured tool records.
var regions = ["east", "west", "north", "south"];
var statuses = ["ok", "warn", "ok", "ok", "error"];
var records = [];
var n = 2400;
var i;

for (i = 0; i < n; i++) {
    records.push({
        region: regions[i % regions.length],
        status: statuses[i % statuses.length],
        value: (i * 37) % 1000,
        tokens: 20 + (i % 80)
    });
}

var totals = {};
for (i = 0; i < records.length; i++) {
    var item = records[i];
    var key = item.region + ":" + item.status;
    if (totals[key] === undefined) totals[key] = { count: 0, value: 0, tokens: 0 };
    totals[key].count++;
    totals[key].value += item.value;
    totals[key].tokens += item.tokens;
}

var checksum = 0;
var keys = Object.keys(totals);
for (i = 0; i < keys.length; i++) {
    checksum += totals[keys[i]].count + totals[keys[i]].value + totals[keys[i]].tokens;
}
if (keys.length !== 12 || checksum <= 0) throw "ERROR: bad aggregate";
