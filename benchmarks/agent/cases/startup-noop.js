// AgentBench general: the smallest useful Agent action.
// This isolates process startup, source loading and a trivial evaluation.
var value = 1 + 2;
if (value !== 3) throw "ERROR: bad startup result";
