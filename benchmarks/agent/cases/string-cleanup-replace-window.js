// AgentBench: short-lived text cleanup with literal String replacement.
//
// This models normalizing tool or API payloads before matching rules. It uses
// string replacement rather than RegExp so the benchmark isolates ordinary
// String builtin dispatch, ASCII storage, and result construction.

var row = "  User-ID: A-17 ; Status: OK ; Tag: Alpha-Beta ; Route: /Agent/Run ;\n";
var raw = "";

for (var i = 0; i < 3000; i++) {
    raw += row;
}

var normalized = raw.replaceAll(" ", "").replaceAll("-", "").toLowerCase();
var count = 0;
var cursor = 0;
var needle = "status:ok";

while (true) {
    var found = normalized.indexOf(needle, cursor);
    if (found < 0) {
        break;
    }
    count++;
    cursor = found + needle.length;
}

if (count !== 3000 || normalized.indexOf("userid:a17") < 0) {
    throw "ERROR: bad string cleanup result: count=" + count;
}
