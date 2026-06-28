// AgentBench: parse numeric fields from a compact log string.
//
// Agent runtimes frequently summarize logs returned by tools. This case uses
// indexOf + slice + Number conversion over a transient ASCII log buffer.

var log = "";
var expectedEntries = 6000;

for (var i = 0; i < expectedEntries; i++) {
    log += "ts=2026-06-29T00:00:00Z;tool=search;tokens=" +
        ((i * 17) % 997) + ";status=ok;\n";
}

var total = 0;
var entries = 0;
var pos = 0;

while (true) {
    var start = log.indexOf("tokens=", pos);
    if (start < 0) {
        break;
    }
    start += 7;
    var end = log.indexOf(";", start);
    total += Number(log.slice(start, end));
    entries++;
    pos = end + 1;
}

if (entries !== expectedEntries || total <= 0) {
    throw "ERROR: bad string log parse: entries=" + entries + " total=" + total;
}
