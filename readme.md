<div align="center">

# AgentJS

### 面向 AI Agent 的轻量级 Rust JavaScript Runtime

<p>
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024_Edition-EA580C?style=for-the-badge&logo=rust&logoColor=white" />
  <img alt="Native Engine" src="https://img.shields.io/badge/Engine-Native_Only-0F766E?style=for-the-badge" />
  <img alt="Test262" src="https://img.shields.io/badge/Test262-90.98%25-7C3AED?style=for-the-badge" />
  <img alt="License MIT" src="https://img.shields.io/badge/License-MIT-2563EB?style=for-the-badge" />
</p>

**轻量 · 隔离 · 资源可控 · 结果可审计**

[快速开始](#4-快速开始) · [测试结果](#5-正确性验证) · [性能数据](#6-性能测试) · [创新点](#7-创新点) · [实验材料](#10-材料索引)

</div>

---

AgentJS 默认使用自研 Native 执行链，从源码解析、字节码生成到虚拟机和运行时均由项目独立实现。它面向 Agent 工具调用、受限脚本执行和短数据处理任务；Boa 与 QuickJS 只作为外部参考引擎参与同输入测试，Native 执行失败时不会静默回退到其他引擎。

## 1. 项目亮点

- **不是套壳**：Lexer、Parser/AST、Bytecode、VM、Runtime、Builtins 与 Heap/GC 构成独立 Native 执行链。
- **面向 Agent**：同时提供 fresh isolate 与可复用 Runtime，适配独立 action 和连续短任务。
- **资源可控**：对执行步数、递归深度、VM 栈、堆对象和大分配设置预算。
- **专项优化**：实现分段稠密数组、Descriptor 旁路表、Free List 与 ASCII fast path。
- **证据闭环**：Test262、SunSpider、AgentBench、JetStream 公共集和版本报告共同支撑结论。

### 核心数据

| 维度 | 结果 | 结论边界 |
| --- | ---: | --- |
| Test262 全量运行 | **48,566 / 53,379（90.98%）** | 失败、跳过、崩溃和超时均不计为通过 |
| Test262 失败 / 跳过 | 4,811 / 2 | 仍存在明确的标准兼容性缺口 |
| SunSpider 1.0.2 | **26 / 26 PASS** | 证明经典 workload 能正确完成，不代表全面快于成熟引擎 |
| JetStream 2 公共 workload | **三引擎 6 / 6 PASS** | AgentJS、Boa、QuickJS 的 CLI 公共子集，不是浏览器综合总分 |
| Native release 体积 | **约 7.11 MiB** | 以报告对应的 Windows release 产物为准 |

最新 Test262 统计为 **48,566 / 53,379（90.98%）**。该汇总尚未完整记录对应源码 commit、CPU 与运行命令，因此 README 不将其表述为“当前 HEAD 的即时结果”；正式复测应同时保存环境、revision、二进制指纹和原始输出。

## 2. 项目定位

浏览器引擎需要覆盖页面、DOM、JIT 和长生命周期应用，而 Agent 工具调用通常具有另一组约束：脚本短、调用频繁、隔离要求高，并且需要可预测的资源上限和可诊断错误。

AgentJS 的目标不是复制完整浏览器宿主，而是提供一条轻量、可控、可测试、可审计的 JavaScript 执行路径：

- `Engine` 面向彼此独立的 Agent action，使用 fresh isolate 隔离状态；
- `Runtime` 面向连续调用，可保留 isolate，并通过 LRU script cache 复用解析和编译结果；
- 通过执行步数、调用深度、VM 栈、堆对象和大分配预算限制失控脚本；
- Host surface 保持极小，仅提供 `print` 和冻结的 `console` facade，不开放文件、进程和网络能力。

## 3. 系统架构

```text
JavaScript source
      ↓
Lexer → Parser / AST → Bytecode Compiler → Stack VM
                                           ↓
                                Runtime / Builtins / Heap
                                           ↓
                                        JsValue
```

核心阶段通过 [`src/contracts.rs`](src/contracts.rs) 的稳定接口衔接，依赖方向保持为：

```text
lexer → parser → bytecode → vm → runtime / builtins
```

默认构建只有 `BackendKind::Native`。Boa 位于 pinned submodule 中，需要单独构建；QuickJS 同样作为外部参考程序运行，两者都不进入 AgentJS Native 的执行实现。

### 仓库结构

```text
AgentJS/
├── src/
│   ├── lexer/             # 词法分析
│   ├── parser/            # 语法分析与早期错误
│   ├── ast/               # 抽象语法树
│   ├── bytecode/          # 字节码生成
│   ├── vm/                # 栈式虚拟机
│   ├── runtime/           # 执行上下文、对象与资源预算
│   ├── builtins/          # ECMAScript 内建对象与算法
│   ├── contracts.rs       # 跨阶段稳定接口
│   ├── engine.rs          # 后端无关入口
│   └── main.rs            # CLI 与测试入口
├── tests/                 # 集成测试与 Native 固定门
├── benchmarks/            # AgentBench、SunSpider、JetStream 输入与结果
├── test262/               # pinned ECMAScript 一致性测试集
├── reports/               # 版本报告与扫描结果
├── presentation/          # PPT 与演示素材
├── boa/                   # 外部参考引擎 submodule
└── quickjs/               # 外部参考引擎 submodule
```

## 4. 快速开始

### 环境要求

- Rust toolchain 1.91 或更新版本；
- Git（用于拉取 pinned submodule）；
- Python 3（仅 benchmark runner 需要）。

### 构建与运行

```powershell
# 构建
git submodule update --init --recursive
cargo build --release --locked

# 表达式求值
.\target\release\agentjs.exe eval "1 + 2"
```

输出：

```text
3
```

```powershell
# 查看命令帮助
.\target\release\agentjs.exe --help

# 直接运行 JavaScript 文件
.\target\release\agentjs.exe run .\presentation\examples\demo.js

# 交互模式
.\target\release\agentjs.exe repl
```

项目还提供 Test262、benchmark 与 JetStream 相关入口。运行参数以 `agentjs.exe --help` 和各 runner 的 `--help` 为准。

## 5. 正确性验证

### Test262

最新全量统计如下：

| Total | Passed | Failed | Skipped | Pass rate | Elapsed |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 53,379 | **48,566** | 4,811 | 2 | **90.98%** | 444.347 s |

通过率比赛题 60% 门槛高 **30.98 个百分点**。早期 26.29% 仅用于展示迭代进展，不是当前基线；旧文档中的 71.78% 或 72.02% 也不应替代最新结果。

复测时建议写入新的带日期目录，避免覆盖历史证据：

```powershell
$commit = git rev-parse --short HEAD
$out = ".\reports\test262-$(Get-Date -Format yyyyMMdd)-$commit"
New-Item -ItemType Directory -Path $out -Force

.\target\release\agentjs.exe test262 `
  --root .\test262 `
  --suite test `
  --jobs 4 `
  --json "$out\full-summary.json"
```

### SunSpider 1.0.2

AgentJS 完成全部 26 个 SunSpider 用例，wrong result、runtime error 和 timeout 均为 0。历史 P50 显示位运算路径接近 Boa，而 `regexp-dna`、`string-tagcloud` 等仍是明显热点。由于现有 AgentJS 与 Boa 明细来自非同步历史批次，数据用于诊断，不用于宣称整体性能排名。

```powershell
cargo build --release --locked
cargo build --release --manifest-path .\boa\Cargo.toml -p boa_cli

python .\benchmarks\sunspider\run_sunspider.py `
  --engine .\target\release\agentjs.exe `
  --label agentjs `
  --ref-engine .\boa\target\release\boa.exe `
  --ref-label boa `
  --repeat 3 `
  --timeout 60 `
  --out-json .\benchmarks\sunspider\results\agentjs-vs-boa.json `
  --out-md .\benchmarks\sunspider\results\agentjs-vs-boa.md
```

## 6. 性能测试

不同测试回答不同问题，不能合并成一个模糊的“综合性能分数”：

| 测试 | 主要回答的问题 |
| --- | --- |
| SunSpider | 经典 JavaScript workload 能否正确运行，热点在哪里 |
| AgentBench 2.0 | Agent 短任务、数组、对象和字符串专项优化是否产生收益 |
| JetStream 2 公共集 | 三个 CLI 引擎在相同可移植 workload 上的耗时和内存差异 |

### AgentBench 2.0

AgentBench 覆盖局部稠密大索引数组、属性描述符、短数据过滤、规则窗口以及高频字符串处理。所有引擎必须先得到正确结果，用例才进入性能统计。

专项历史结果表明：

- `descriptor-side-table-array`：AgentJS 相对 Boa **1.67×**；
- `large-index-dense-array`：AgentJS 相对 Boa **2.39×**；
- `rule-filter-dense-window`：AgentJS 相对 Boa **1.22×**；
- ASCII fast path 将 `string-base64` 中位耗时从 769.9 ms 降至 273.0 ms，提升 **2.82×**。

这些数字只证明目标热点上的优化收益，不表示 AgentJS 在所有 JavaScript 程序上都快于 Boa 或 QuickJS。

### JetStream 2 三引擎公共集

最终 PPT 采用 AgentJS、Boa、QuickJS 三引擎口径。公共集包含 6 个来自 pinned JetStream 2 的自包含 workload；每个引擎和 workload 使用 2 次预热进程与 7 次测量进程，展示 P50：

| Workload | AgentJS P50 | Boa P50 | QuickJS P50 |
| --- | ---: | ---: | ---: |
| `n-body-SP` | 1.22 s | 347 ms | 74 ms |
| `crypto-sha1-SP` | 4.03 s | 527 ms | 200 ms |
| `crypto-md5-SP` | 4.14 s | 483 ms | 129 ms |
| `3d-cube-SP` | 1.45 s | 364 ms | 97 ms |
| `navier-stokes` | 594 ms | 218 ms | 34 ms |
| `richards` | 3.72 s | 625 ms | 116 ms |

三引擎均通过 6 / 6 workload 和全部 7 个有效样本。几何平均上，Boa 约比 AgentJS 快 **4.95×**，QuickJS 约快 **21.3×**；AgentJS 最大观测峰值 RSS 为 **27.04 MiB**。这说明 AgentJS 已具备公共复杂 workload 的正确运行能力，但吞吐仍与成熟引擎存在明显差距。

> 这里比较的是可移植、自包含的 JetStream 2 workload kernel，不包含 WebAssembly、Web Worker 等浏览器宿主项目，也不修改 workload 和 scoring；因此它不是完整浏览器 JetStream 2 官方综合分数。

原始结果保存在 [`benchmarks/jetstream/results/`](benchmarks/jetstream/results/)。目录名可能保留早期实验命名，README 和最终 PPT 只采用三引擎数据列。

## 7. 创新点

### 1. 自研 Native 执行链

Lexer、Parser/AST、Bytecode、VM、Runtime、Builtins 与 Heap/GC 均由项目实现。Boa 仅作为外部行为与性能参照；Native 未实现能力会返回分类错误，不会借助外部引擎制造通过结果。90.98% 的 Test262 结果和 SunSpider 26 / 26 均来自 Native 路径。

### 2. 面向 Agent 的轻量、隔离与资源可控设计

项目围绕短时、高频、即时执行设计 fresh isolate 与可复用 Runtime 两种入口，并使用资源预算将无限循环、过深递归、VM 栈溢出和异常大分配转化为可诊断的 `RuntimeLimit`。Native release 约 7.11 MiB，适合嵌入和审计。

### 3. 针对热点的数据结构与快速路径

| 机制 | 设计目的 |
| --- | --- |
| 分段稠密数组 | 前 64K 槽位 inline，之后按 4K 槽位惰性分段，超大索引转为 sparse property |
| Descriptor 旁路表 | 普通元素只保存 value，仅为非默认 descriptor 保存额外元数据 |
| Free List 堆槽复用 | GC 后复用 object、function、environment 等 arena slot，降低短任务重复分配 |
| String Primitive ASCII Fast Path | 对 ASCII code unit、长度与扫描使用更直接的字节路径 |

### 4. 面向场景的 AgentBench

项目没有只依赖传统浏览器 benchmark，而是设计确定性 AgentBench，用短文本清洗、规则过滤、大索引数组、descriptor 混合访问和连续 action 模拟实际 Agent 脚本负载，使优化目标与应用场景可以一一对应。

### 5. 可审计的测试驱动流程

开发采用“单元测试 → 固定回归门 → 重点目录 → 5,000-case 锁定扫描 → 完整 Test262”的分层验证，并为各版本保存 scope、接口、团队计划、失败清单和 part report。跳过、超时和崩溃均不算通过，历史结果不反向绑定当前 HEAD。

## 8. 开发与验证

提交前依次运行：

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

如果修改 Native stage，再运行：

```powershell
cargo test --no-default-features --test native_test262
```

完整的版本门、轻量扫描和模块边界说明见 [`AGENTS.md`](AGENTS.md) 与 [`docs/`](docs/)。

## 9. 结果可信度与局限

- Test262 90.98% 是最新全量统计，但还需补齐对应 commit 与完整环境元数据；
- SunSpider 的历史逐项耗时只用于定位热点，不进行跨批次总体排名；
- AgentBench 的优势限定在项目明确覆盖的短任务 workload；
- JetStream 2 数据是 CLI 公共子集，不是官方浏览器总分；
- AgentJS 当前无 JIT，复杂数值、RegExp、字符串/对象和部分 Intl/Temporal 路径仍需优化；
- Test262、SunSpider、JetStream 2、Boa 与 QuickJS 都是外部测试或参考输入，不进入 Native 核心实现。

## 10. 材料索引

| 材料 | 路径 |
| --- | --- |
| 实验报告 | [`reports/agentjs-experiment-report.md`](reports/agentjs-experiment-report.md) |
| 最新 PPT | [`presentation/PPT.pptx`](presentation/PPT.pptx) |
| Test262 历史归档 JSON | [`Test262-final/full-test262-summary.json`](Test262-final/full-test262-summary.json) |
| SunSpider 结果 | [`benchmarks/sunspider/results/`](benchmarks/sunspider/results/) |
| AgentBench 结果 | [`benchmarks/agent/results/`](benchmarks/agent/results/) |
| JetStream 公共集结果 | [`benchmarks/jetstream/results/`](benchmarks/jetstream/results/) |

- 展示视频

通过网盘分享的文件：video.mp4
链接: https://pan.baidu.com/s/1XgUcHQoKk6bl1a3EfDK_3A?pwd=3pbf 提取码: 3pbf 

## 11. 开源协议

[MIT](LICENSE)
