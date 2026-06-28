# AgentBench

AgentBench contains small JavaScript workloads shaped around agent-oriented execution: short data-processing scripts, large logical arrays, and mostly-default array elements with rare descriptor overrides.

Run AgentJS against Boa:

```powershell
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --label agentjs --ref-engine boa --ref-label boa --repeat 3 --timeout 120
```

Run a focused case:

```powershell
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --label agentjs --ref-engine boa --ref-label boa --cases large-index-dense-array --repeat 5 --timeout 120
```
