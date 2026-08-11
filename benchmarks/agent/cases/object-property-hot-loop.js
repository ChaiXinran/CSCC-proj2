// AgentBench general: stable-shape object property access.
var records = [];
var n = 6000;
var checksum = 0;
var i;

for (i = 0; i < n; i++) {
    records.push({ id: i, price: (i * 17) % 211, count: (i % 9) + 1, active: (i & 3) !== 0 });
}

for (var round = 0; round < 6; round++) {
    for (i = 0; i < records.length; i++) {
        var item = records[i];
        if (item.active) checksum += item.price * item.count + item.id;
    }
}
if (checksum <= 0) throw "ERROR: bad property loop";
