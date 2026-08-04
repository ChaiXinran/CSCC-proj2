# JetStream2 全 staged runner 与 Local Slot 补全验收

日期：2026-08-04  
代码基线：`main@05e160f` 加本轮未提交补全  
配置：release、1 iteration、150 秒单项上限、1.5 GiB working-set 上限

## 结论

19 个 canonical runner 均使用 manifest v2 和逐文件 staged 执行；runner 中没有 workload 源码、`__jetstreamResources` 或 `scripts.join()`。这关闭了“大型 runner 双份源码”的结构性遗漏。

受保护矩阵结果为 12/19 PASS：5 个 MEMORY_LIMIT、1 个 ASSERTION_FAILURE、1 个 ENGINE_FAILURE，零 TIMEOUT，测试结束后无残留 AgentJS 进程。该结果不等于 19/19 功能兼容：`regexp` 在资源阶段出现未捕获异常，说明逐文件顶层执行尚不能完整复现原 Shell 合并脚本的函数作用域。不得通过恢复整包 `new Function` 来绕过这一缺口。

## 完整矩阵

| workload | 状态 | wall (s) | peak RSS (MiB) |
|---|---|---:|---:|
| ai-astar | MEMORY_LIMIT | 8.618 | 1576.6 |
| crypto | PASS | 1.331 | 172.1 |
| gaussian-blur | PASS | 5.638 | 247.3 |
| hash-map | PASS | 10.023 | 1289.5 |
| cdjs | PASS | 15.635 | 1206.0 |
| intl | PASS | 0.779 | 175.6 |
| jsdom-d3-startup | MEMORY_LIMIT | 1.130 | 1587.8 |
| mobx | PASS | 6.576 | 759.6 |
| threejs | MEMORY_LIMIT | 109.185 | 1536.6 |
| validatorjs | ASSERTION_FAILURE | 1.701 | 206.7 |
| web-ssr | PASS | 6.633 | 1447.1 |
| WSL | MEMORY_LIMIT | 1.254 | 1540.7 |
| navier-stokes | PASS | 0.809 | 29.1 |
| raytrace | PASS | 12.734 | 968.2 |
| regexp | ENGINE_FAILURE | 0.719 | 120.6 |
| richards | PASS | 6.818 | 704.9 |
| splay | MEMORY_LIMIT | 3.875 | 1576.3 |
| stanford-crypto-sha256 | PASS | 6.828 | 188.0 |
| test-cdjs | PASS | 15.564 | 1134.6 |

## Local Slot 诊断

诊断脚本不再只取最后一次 evaluate 的计数，而是聚合同一 runner 的所有资源阶段样本。它记录 `nameResolutionSamples`、Local/Name load/store、`environmentHops` 和 `localFastPathPercent`。例如 regexp 聚合 2 个样本，Local load/store 为 685681/229056，Name load/store 为 147345/5，Local 快路径比例 86.13%；此前只读末条记录会严重低估真实命中。

原始逐项日志和机器可读汇总位于 `reports/jetstream2-generated-staged-local-slot-fixed-2026-08-04/`。这是一轮功能与资源保护验收，不是多轮性能评分；D 组 5 轮 median/p90 对照仍以 `reports/phase2-partD-report.md` 为准。

## 后续边界

- WSL、threejs、jsdom-d3-startup、ai-astar、splay 仍需引擎内存路径分析；runner 已不携带第二份 workload 源码。
- regexp 需要持久 Host-script 函数环境或等价的跨 staged 文件脚本语义，属于共享 runtime/compiler 接口工作。
- validatorjs 的断言失败是功能差异，不应归类为 runner 资源缺失或 OOM。
