// AgentBench: ordinary numeric array elements with a few descriptor overrides.
//
// Most JS array writes create default writable/enumerable/configurable elements.
// The side table keeps those hot elements light while preserving rare descriptor
// semantics such as non-configurable indices.

var n = 150000;
var arr = new Array(n);
var sum = 0;

for (var i = 0; i < n; i++) {
    arr[i] = (i * 17) & 1023;
}

Object.defineProperty(arr, "90000", {
    value: 123,
    writable: true,
    enumerable: true,
    configurable: false
});

for (var round = 0; round < 4; round++) {
    for (var j = 0; j < n; j++) {
        sum += arr[j] & 31;
    }
}

var shrinkOk = true;
try {
    arr.length = 10;
} catch (e) {
    shrinkOk = false;
}

if (arr.length !== 90001) {
    throw "ERROR: bad length after protected shrink: " + arr.length;
}

if (sum <= 0 || shrinkOk !== true) {
    throw "ERROR: bad descriptor side-table result: " + sum;
}
