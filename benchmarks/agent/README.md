# AgentBench

AgentBench contains small JavaScript workloads shaped around agent-oriented execution: short-lived, high-frequency data-processing scripts rather than browser-style long-lived applications.

The suite intentionally covers several hot shapes:

- dense logical arrays used by transient analysis tasks;
- mostly-default array elements with rare descriptor overrides;
- rule filtering over compact object records;
- ASCII string scanning, cleanup, and log-field extraction.

The String cases are deliberately separate from SunSpider. They model agent tasks such as scanning tool output, normalizing payload text, and extracting counters from short logs. This makes it clear which optimizations help agent-style text processing and which paths remain future work.

Run AgentJS against Boa:

```powershell
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --label agentjs --ref-engine boa --ref-label boa --repeat 3 --timeout 120
```

Run a focused case:

```powershell
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --label agentjs --ref-engine boa --ref-label boa --cases large-index-dense-array --repeat 5 --timeout 120
```

Run only the String-oriented cases:

```powershell
python benchmarks/agent/run_agentbench.py --engine .\target\release\agentjs.exe --label agentjs --ref-engine boa --ref-label boa --cases string-ascii-index-scan,string-cleanup-replace-window,string-log-token-slice --repeat 3 --timeout 120
```
