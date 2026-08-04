# JetStream generated runner：外置资源改造后复测（2026-08-04）

> **已被后续修复复测取代。** 本文的 10/19 暴露了 CRLF 和入口隔离回归；
> 修复后的正式结果见 `jetstream-generated-external-resources-fixed-2026-08-04.md`。

> 测试日期：2026-08-04  
> Git 基线：`ec64a7dec2231421adb42c7f40c688338af26fe3`，工作区包含尚未提交的 A 组外置资源改造  
> 测试对象：`benchmarks/generated/` 的 19 个 manifest v2 canonical runner  
> AgentJS：当前源码重新构建的 `target/release/agentjs.exe`  
> 参数：1 iteration，顺序单进程，每项 150 秒，working set 上限 1.5 GiB  
> Host：`--resource-root benchmarks/JetStream2 --diagnostics`

## 结果总览

| Runner | 结果 | 墙钟时间 | 峰值 working set | 上一份 A/B 合并报告 |
|:--|:--:|--:|--:|:--:|
| ai-astar | MEMORY_LIMIT | 10.168s | 1557.6 MiB | PASS |
| crypto | PASS | 3.152s | 133.9 MiB | PASS |
| gaussian-blur | PASS | 14.847s | 182.1 MiB | PASS |
| hash-map | PASS | 12.878s | 1172.9 MiB | PASS |
| jetstream2-cdjs | PASS | 18.495s | 1192.3 MiB | PASS |
| jetstream2-intl | PASS | 0.708s | 159.6 MiB | RESULT_MISMATCH |
| jetstream2-jsdom-d3-startup | CALL_ERROR | 0.132s | 18.2 MiB | MEMORY_LIMIT |
| jetstream2-mobx | CALL_ERROR | 0.128s | 18.4 MiB | PASS |
| jetstream2-threejs | MEMORY_LIMIT | 91.750s | 1537.5 MiB | TIMEOUT |
| jetstream2-validatorjs | ASSERTION_FAILURE | 1.796s | 232.5 MiB | ASSERTION_FAILURE |
| jetstream2-web-ssr | CALL_ERROR | 0.157s | 18.5 MiB | CALL_ERROR |
| jetstream2-WSL | MEMORY_LIMIT | 1.295s | 1609.8 MiB | MEMORY_LIMIT |
| navier-stokes | PASS | 2.079s | 30.5 MiB | PASS |
| raytrace | PASS | 13.828s | 916.0 MiB | PASS |
| regexp | ENGINE_FAILURE | 0.785s | 123.6 MiB | PASS |
| richards | PASS | 7.703s | 662.8 MiB | PASS |
| splay | MEMORY_LIMIT | 4.903s | 1568.0 MiB | PASS |
| stanford-crypto-sha256 | PASS | 8.148s | 180.6 MiB | PASS |
| test-cdjs | PASS | 18.287s | 1192.2 MiB | PASS |

`jetstream2-cdjs.js` 与 `test-cdjs.js` 是两个 canonical 文件，本轮按用户要求分别执行；结果和峰值接近，但各自拥有独立进程测量。

## 覆盖率与分类

- PASS：10/19（52.6%）。
- MEMORY_LIMIT：4 项（ai-astar、threejs、WSL、splay）。
- CALL_ERROR：3 项（jsdom-d3-startup、mobx、web-ssr）。
- ASSERTION_FAILURE：1 项（validatorjs）。
- ENGINE_FAILURE：1 项（regexp）。
- TIMEOUT：0 项；所有进程都在统一门限内完成或被 working-set 保护终止。
- 19 项累计受测墙钟时间为 211.239 秒，不包含 release 构建时间。

相对上一份 `jetstream-generated-after-ab-merge-2026-08-04.md`：

- 新增 PASS：`jetstream2-intl`。
- PASS 变为非 PASS：`ai-astar`、`jetstream2-mobx`、`regexp`、`splay`。
- 净变化：13/19 降至 10/19。
- `threejs` 从 150 秒 TIMEOUT 变为 91.750 秒触发 1.5 GiB MEMORY_LIMIT；这是更明确的内存分类，不代表性能改善。

## 外置资源与生成物核验

- 19 个 runner/manifest 的 `runnerSha256` 全部匹配实际文件，损坏或错配数为 0。
- 19 个 runner 总大小为 2,194,698 bytes。
- manifest 合计引用 191 个去重后的资源文件，总大小为 10,869,597 bytes。
- 资源正文不再写入 runner；CLI 通过同一个 Runtime 顺序执行入口文件，运行期读取统一经过 rooted Host loader。
- 本轮直接运行现有 generated 文件，没有在测试前重新生成 runner。

## 失败与阶段分析

### ai-astar

driver prelude、入口资源和 launch 均完成 parse/compile 并进入 execute，但在产生 JetStream JavaScript 输出前，于 10.168 秒超过 1.5 GiB。旧报告为 PASS，因此外置资源后的代码/环境持有方式存在明显内存回归风险。

### jsdom-d3-startup、mobx、web-ssr

三项都完成 runner prelude 与两个入口文件的 parse/compile/execute，随后在 launch 中很快报 `undefined is not callable`。Node 参考宿主用相同 staged runner 复测时，三项均在 `StartupBenchmark.init()` 报：

```text
Cannot read properties of null (reading 'length')
```

进一步定位为 `this.sourceCode.match(CACHE_BUST_COMMENT_RE)` 返回 `null`。这说明当前 startup preload/文本归一化协议本身存在问题，不能把三项直接归因于 AgentJS builtin；其中 MobX 从上一报告的 PASS 回退，必须作为 A 组 runner/Host 协议回归处理。

### threejs

两个入口文件和 launch 均进入 execute；91.750 秒时超过 1.5 GiB，峰值 1537.5 MiB。相较旧 runner 的 payload 前超时，本轮确认资源已经逐文件解析执行，但内存仍持续增长。

### validatorjs

进入 workload 后保持原失败：

```text
Assertion failure: 2010-07-02,[object Object]
```

分类与上一报告一致，仍属于日期/对象字符串化语义问题。

### WSL

148 个入口文件均逐个完成 parse/compile/execute，随后 launch 也进入 execute；约 1.295 秒达到 1609.8 MiB。外置源码消除了巨型 runner 解析，但没有消除运行时函数模板/环境/代码持有造成的内存增长，结果继续支持 B/C 组的共享字节码和 GC 工作。

### regexp

在执行 `./Octane/regexp.js` 时抛出 `uncaught Error`，尚未进入 driver launch。Node 对同一 staged runner 给出了具体 checksum：

```text
Wrong checksum. Found 1665254 but expected 1666156
```

AgentJS 当前错误对象没有保留该 message，因此正式分类保持 `ENGINE_FAILURE`，但 checksum 路径是首要调查方向。

### splay

入口文件和 launch 均进入 execute，4.903 秒达到 1568.0 MiB。上一报告为 PASS，属于新的高优先级内存回归。

## PASS 项单次结果

| Runner | 当前 | 上一报告 | 单次变化 |
|:--|--:|--:|--:|
| crypto | 3.152s | 3.126s | +0.8% |
| gaussian-blur | 14.847s | 14.845s | +0.0% |
| hash-map | 12.878s | 13.198s | -2.4% |
| jetstream2-cdjs | 18.495s | 20.012s | -7.6% |
| navier-stokes | 2.079s | 2.242s | -7.3% |
| raytrace | 13.828s | 13.404s | +3.2% |
| richards | 7.703s | 7.581s | +1.6% |
| stanford-crypto-sha256 | 8.148s | 7.967s | +2.3% |
| test-cdjs | 18.287s | 20.012s | -8.6% |

这些都是单次墙钟结果，只用于发现大幅变化；不能替代至少五次独立采样。Intl 上一报告未通过，因此没有可比较的有效耗时。

## 测试命令与判定口径

构建：

```powershell
cargo build --release
```

矩阵：

```powershell
powershell.exe -ExecutionPolicy Bypass `
  -File scripts/run-generated-jetstream2-diagnostics.ps1
```

单项等价命令：

```powershell
target/release/agentjs.exe jetstream `
  benchmarks/generated/<runner>.js `
  --resource-root benchmarks/JetStream2 `
  --diagnostics
```

判定规则：

- 退出码 0 且出现 `JETSTREAM_RUN_COMPLETE`：PASS。
- working set 超过 1536 MiB：MEMORY_LIMIT，并终止进程。
- 150 秒未退出：TIMEOUT，并终止进程。
- 其余按稳定错误文本分类；无法提取具体错误类型时保留 ENGINE_FAILURE。
- 当前 canonical manifest 的 `phaseMarkers` 为 `false`，因此阶段判断使用 Rust 的 `parse/compile/execute` diagnostics 和 benchmark 输出，而不是 `JETSTREAM_PHASE`。

## 原始数据

- 汇总 JSON：`reports/jetstream2-generated-2026-08-04/summary.json`
- 每项 stdout/stderr：`reports/jetstream2-generated-2026-08-04/*.txt`
- 运行脚本：`scripts/run-generated-jetstream2-diagnostics.ps1`
- 测试结束后存活的 `agentjs` 进程：0

## 结论

1. 外置资源显著缩小 runner，并使 WSL 的 148 个文件能够逐个进入 execute；原先“巨型 runner 源码内嵌”问题已经解除。
2. 当前结果尚不能作为 A 组修复完成后的性能胜利：startup preload 协议出现跨 AgentJS/Node 的共同回退，MobX 从 PASS 变为失败；ai-astar 和 splay 也新增内存限制失败。
3. Intl 从结果不一致变为 PASS，是本轮明确的正向变化。
4. WSL、threejs 仍证明大型 workload 存在严重运行时内存压力，后续应结合 B 组共享 Chunk 与 C 组 GC/root 结果复测。
5. 在合并前应优先修复 startup preload/cache-bust 文本协议，并调查 ai-astar、splay 的 retained code/environment 内存增长；修复后按同一 runner SHA 和同一矩阵重新生成对比报告。
