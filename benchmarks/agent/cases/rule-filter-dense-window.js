// AgentBench: simple rule filtering over a dense data window.
//
// This represents an agent checking many tool-result records with predictable
// numeric and boolean fields.

var n = 90000;
var records = new Array(n);
var accepted = 0;
var score = 0;

for (var i = 0; i < n; i++) {
    records[i] = {
        price: (i * 37) % 211,
        rating: ((i * 13) % 50) / 10,
        active: (i & 7) !== 0
    };
}

for (var round = 0; round < 3; round++) {
    for (var j = 0; j < n; j++) {
        var item = records[j];
        if (item.active && item.price < 100 && item.rating > 3.7) {
            accepted++;
            score += item.price + (item.rating * 10);
        }
    }
}

if (accepted <= 0 || score <= 0) {
    throw "ERROR: bad result: accepted=" + accepted + " score=" + score;
}
