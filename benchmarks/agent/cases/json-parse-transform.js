// AgentBench pressure: parse and transform a serialized tool response.
var source = [];
var n = 1800;
var i;
for (i = 0; i < n; i++) {
    source.push({ id: i, value: (i * 29) % 997, state: i % 5 ? "ok" : "warn" });
}
var encoded = JSON.stringify(source);
var parsed = JSON.parse(encoded);
var total = 0;
var warns = 0;
for (i = 0; i < parsed.length; i++) {
    total += parsed[i].value;
    if (parsed[i].state === "warn") warns++;
}
if (parsed.length !== n || warns !== 360 || total <= 0) throw "ERROR: bad JSON transform";
