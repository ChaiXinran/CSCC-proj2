// AgentBench: ASCII string primitive metadata and indexed character access.
//
// Agent workloads often scan compact tool output or protocol text. This case
// keeps the data ASCII-only so engines can benefit from cheap string length and
// code-unit access without allocating String wrapper objects for each index.

var alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789:|,_-/";
var payload = "";

for (var i = 0; i < 1024; i++) {
    payload += "id:" + (i & 255) + "|state:ok|tool:grep|path:/tmp/a_b-c|" + alphabet + "\n";
}

var checksum = 0;
var pipes = 0;

for (var round = 0; round < 2; round++) {
    for (var j = 0; j < payload.length; j += 3) {
        var ch = payload[j];
        if (ch === "|") {
            pipes++;
        }
        checksum += payload.charCodeAt(j);
    }
}

if (payload.length < 70000 || checksum <= 0 || pipes <= 0) {
    throw "ERROR: bad string ascii index scan: length=" + payload.length +
        " checksum=" + checksum + " pipes=" + pipes;
}
