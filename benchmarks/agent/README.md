# AgentBench 2.0

AgentBench 面向赛题中的 **短时、高频、即时 Agent JavaScript 执行**。它模拟工具结果聚合、规则过滤、JSON 转换、文本清洗、日志提取、稳定对象访问和短命对象分配等任务；它不是浏览器 DOM 或长时间运行网页的测试。

## 设计原则

- 所有引擎执行完全相同的 JavaScript 文件。
- 每个用例都带有确定性的结果检查；退出错误、超时或结果错误的运行不进入性能平均值。
- `general` 是常见 Agent 数据处理，`pressure` 是数组、字符串、JSON 和对象分配压力场景。
- `cold` 每次启动一个新进程，测量启动、解析、编译和执行的总成本。
- `batch` 在同一个进程内重复执行同一个用例，测量连续短任务的吞吐量和内存变化。
- 总体速度使用各引擎共同通过用例的几何平均，不直接把不同用例的毫秒数相加。
- Core benchmark 不调用 `agent.render`：Boa、Node 等参考引擎没有这个 AgentJS 专用 Host API。渲染树应作为单独的 AgentJS Host benchmark，不能混入跨引擎排名。

用例及场景说明见 [`manifest.json`](manifest.json)。

## 快速运行

先构建 AgentJS：

```powershell
cargo build --release --locked
```

只测试 AgentJS 的常规和压力用例：

```powershell
python benchmarks/agent/run_agentbench.py `
  --engine .\target\release\agentjs.exe `
  --group all --mode cold --warmup 2 --repeat 5
```

同时进行冷启动和进程内批处理，并与 Node.js 比较：

```powershell
python benchmarks/agent/run_agentbench.py `
  --engine .\target\release\agentjs.exe `
  --ref node="D:\Program Files\nodejs\node.exe" `
  --group all --mode both --warmup 3 --repeat 15 --batch-repeat 25
```

与 Boa、QuickJS 和 Node.js 一起比较：

```powershell
python benchmarks/agent/run_agentbench.py `
  --engine .\target\release\agentjs.exe `
  --ref boa=.\boa\target\release\boa.exe `
  --ref quickjs=.\baseline-quickjs\qjs.exe `
  --ref node=node `
  --group all --mode both --warmup 3 --repeat 15 --batch-repeat 25
```

参考引擎参数使用 `LABEL=COMMAND`，可以重复指定 `--ref`。旧版的 `--ref-engine` 和 `--ref-label` 仍可用于单个参考引擎。

按组或按用例运行：

```powershell
python benchmarks/agent/run_agentbench.py --group general --mode cold
python benchmarks/agent/run_agentbench.py --group pressure --mode cold
python benchmarks/agent/run_agentbench.py `
  --cases startup-noop,string-cleanup-replace-window,descriptor-side-table-array `
  --mode both --warmup 2 --repeat 5 --batch-repeat 10
```

## 输出和统计口径

默认输出到 `benchmarks/agent/results/`；通过 `--out-dir` 可指定其它目录。

- `agentjs.json`：完整原始运行、P50/P90/P95、最小/最大耗时、吞吐量、峰值 RSS、正确性状态和相对比值。
- `agentjs.md`：适合答辩直接引用的表格报告。
- `environment.json`：单模式运行时的操作系统、CPU、Python/Rust 版本、命令、二进制 SHA-256 和文件大小。
- `--mode both`：分别生成 `agentjs-cold.*`、`agentjs-batch.*`、`environment-cold.json` 和 `environment-batch.json`；`environment.json` 保留为最后一次模式的兼容别名。

报告中的速度比定义为：

```text
参考引擎耗时 / AgentJS 耗时
```

因此大于 1 表示 AgentJS 更快，小于 1 表示 AgentJS 更慢。峰值 RSS 比也采用“参考引擎 / AgentJS”定义，大于 1 表示 AgentJS 占用更少内存。正式答辩应在空闲机器、固定电源模式下运行较多轮次（例如 `warmup=3, repeat=15`），并保留 JSON、环境信息和二进制指纹；不要只展示截图或挑选有利用例。

当前 benchmark 还会记录可执行文件体积。体积、冷启动和峰值内存是 AgentJS 的核心轻量化指标；Node/V8 在长时间热执行上通常更快，因此报告应同时展示优势和弱项，不能据此宣称总体性能全面领先。
