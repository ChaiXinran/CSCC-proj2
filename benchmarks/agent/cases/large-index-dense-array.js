// AgentBench: locally dense large-index array access.
//
// This models agent-side data processing where a task touches a large logical
// array window instead of browser-style long-lived object graphs.

var base = 70000;
var count = 180000;
var flags = new Array(base + count + 1);
var sum = 0;

for (var round = 0; round < 3; round++) {
    for (var i = 0; i < count; i++) {
        flags[base + i] = ((i + round) & 3) !== 0;
    }

    for (var j = 0; j < count; j++) {
        if (flags[base + j]) {
            sum += (j & 255);
        }
    }
}

if (sum <= 0 || flags[base] !== true || flags[base + count - 1] !== true) {
    throw "ERROR: bad large-index dense-array result: " + sum;
}
