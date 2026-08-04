# JetStream generated runner：A/B 合并后复测（2026-08-04）

> 测试日期：2026-08-04  
> 代码版本：`638750f`  
> 对照版本：`7bcd72a`（合并前报告）  
> 测试对象：`benchmarks/generated/` 的 19 个 canonical runner  
> AgentJS：当前源码重新构建的 `target/release/agentjs.exe`  
> 参数：1 iteration，顺序单进程，每项 150 秒，working set 上限 1.5 GiB  
> 诊断：启用 init/iteration/validate marker 和输出时间戳

## 结果总览

| Runner | 合并后结果 | 墙钟时间 | 峰值 working set | 合并前结果 |
|:--|:--:|--:|--:|:--:|
| ai-astar | PASS | 35.714s | 975.5 MiB | PASS |
| crypto | PASS | 3.126s | 131.3 MiB | PASS |
| gaussian-blur | PASS | 14.845s | 40.3 MiB | PASS |
| hash-map | PASS | 13.198s | 234.3 MiB | PASS |
| jetstream2-cdjs | PASS | 20.012s | 645.5 MiB | PASS |
| jetstream2-intl | RESULT_MISMATCH | 0.803s | 52.4 MiB | RESULT_MISMATCH |
| jetstream2-jsdom-d3-startup | MEMORY_LIMIT | 1.253s | 1643.0 MiB | RESOURCE_MISSING |
| jetstream2-mobx | PASS | 7.993s | 367.8 MiB | RESOURCE_MISSING |
| jetstream2-threejs | TIMEOUT | >150s | 1243.5 MiB | TIMEOUT |
| jetstream2-validatorjs | ASSERTION_FAILURE | 1.877s | 134.1 MiB | REGEXP/CALL_ERROR |
| jetstream2-web-ssr | CALL_ERROR | 7.274s | 1002.3 MiB | RESOURCE_MISSING |
| jetstream2-WSL | MEMORY_LIMIT | 1.349s | 1646.3 MiB | TIMEOUT |
| navier-stokes | PASS | 2.242s | 29.1 MiB | PASS |
| raytrace | PASS | 13.404s | 105.7 MiB | PASS |
| regexp | PASS | 6.116s | 201.6 MiB | CHECKSUM_MISMATCH |
| richards | PASS | 7.581s | 80.4 MiB | PASS |
| splay | PASS | 9.161s | 877.3 MiB | PASS |
| stanford-crypto-sha256 | PASS | 7.967s | 96.5 MiB | PASS |
| test-cdjs | PASS | 20.012s | 645.5 MiB | PASS |

`jetstream2-cdjs.js` 与 `test-cdjs.js` 的 SHA-256 完全一致，因此本轮只执行一次并将相同结果展开到两个 canonical 文件。

## 覆盖率变化

- 合并前 AgentJS：11/19 PASS（57.9%）。
- A/B 合并后：13/19 PASS（68.4%）。
- 净变化：+2 PASS，非 PASS 从 8 项下降到 6 项。
- 新增 PASS：`jetstream2-mobx`、`regexp`。
- 原有 PASS 没有变为 FAIL/TIMEOUT。

## 共同通过项目的单次性能变化

| Runner | 合并前 | 合并后 | 变化 |
|:--|--:|--:|--:|
| ai-astar | 37.527s | 35.714s | -4.8% |
| crypto | 4.551s | 3.126s | -31.3% |
| gaussian-blur | 22.484s | 14.845s | -34.0% |
| hash-map | 14.412s | 13.198s | -8.4% |
| jetstream2-cdjs | 21.456s | 20.012s | -6.7% |
| navier-stokes | 3.244s | 2.242s | -30.9% |
| raytrace | 7.413s | 13.404s | +80.8% |
| richards | 41.201s | 7.581s | -81.6% |
| splay | 9.083s | 9.161s | +0.9% |
| stanford-crypto-sha256 | 9.389s | 7.967s | -15.1% |
| test-cdjs | 21.769s | 20.012s | -8.1% |

这些是不同代码版本上的单次墙钟结果，只用于发现明显变化，不替代完整多轮测试。`raytrace` 已额外复测 5 次：median 13.300 秒、p90 13.336 秒，离散度很低。因此合并后约 13.3 秒的表现稳定存在；与旧报告的 7.413 秒单次结果相比，存在明显性能回退风险，需要按 A/B 提交边界进一步二分。

## 剩余失败

### Intl

进入并完成 iteration，在 validate 阶段失败：

```text
Invalid totalLength = 15556, expected >= 40000
```

资源和 runner 均正常，仍是 NumberFormat 语义问题。

### jsdom-d3-startup

在产生第一条 JavaScript 输出前，于约 1.25 秒超过 1.5 GiB working set。资源已经完整嵌入，因此分类从 `RESOURCE_MISSING` 更新为 payload 解析/编译阶段的 `MEMORY_LIMIT`。

### threejs

150 秒内没有 JavaScript 输出或 phase marker，峰值约 1.24 GiB。继续分类为 payload 执行前 `TIMEOUT`。

### validatorjs

RegExp 编译和级联调用错误已经消失。当前进入 iteration 后失败于：

```text
Assertion failure: 2010-07-02,[object Object]
```

说明 B 组 RegExp 修复生效，剩余问题应转交日期/对象字符串化语义。

### web-ssr

资源正常，完成 init 并进入 iteration，随后报：

```text
undefined is not callable
```

### WSL

在产生第一条 JavaScript 输出前，于约 1.35 秒超过 1.5 GiB working set，属于 payload 解析/编译阶段内存增长。

## 合并效果判断

1. A 组资源修复使 MobX 成为真实 PASS，并将 jsdom/web-ssr 的问题从资源错误推进为真实引擎问题。
2. B 组修复使 regexp 从 checksum mismatch 变为 PASS；validatorjs 也不再是 RegExp 编译错误。
3. A 组 LoadName 快速路径对多个计算 workload 有明显收益，尤其是 richards、gaussian-blur、crypto 和 navier-stokes。
4. 当前主要剩余阻塞已集中为 Intl、日期/对象语义、调用语义，以及大型 payload 的解析/编译内存与时间问题。
5. `raytrace` 的 5 次复测 median 为 13.300 秒、p90 为 13.336 秒，合并后慢值稳定；应作为后续性能回归调查项。

## 原始结果

- 诊断 JSON：`target/jetstream2-diagnostics/summary.json`
- raytrace 5 次采样：`target/raytrace-after-ab-5.json`
- 生成物 revision：JetStream `b7babdf323e64e69bd2f6c376189c15825f5c73a`
- 测试结束后存活的 `agentjs` 进程：0
