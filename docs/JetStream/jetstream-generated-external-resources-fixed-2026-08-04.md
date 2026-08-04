# JetStream generated runner：外置资源与隔离语义修复后复测（2026-08-04）

> Git 基线：`ec64a7dec2231421adb42c7f40c688338af26fe3`，含当前未提交 A 组改造  
> 对象：`benchmarks/generated/` 的 19 个 canonical manifest v2 runner  
> 参数：1 iteration；顺序独立进程；150 秒；诊断矩阵使用 1.5 GiB working-set 保护

## 修复内容

1. Rooted Host loader 恢复生成器原有的文本语义：CRLF 和孤立 CR 统一为 LF。StartupBenchmark 的 cache-bust 正则重新匹配，MobX、web-ssr 和 Node startup 参考路径恢复。
2. 入口执行改成有界混合策略：
   - 入口源码总量不超过 640 KiB：`isolated-host`，从 Host 运行期读取，在原隔离函数中编译一次；
   - 超过 640 KiB：`staged`，由 Rust 在同一 Runtime 中逐文件 parse/compile/execute。
3. 删除生成器中旧 Driver 的 `string.join()`/`new Function` 兼容改写，防止未来重新接回整包 workload。
4. driver inline harness 仍需要一次 `new Function`，但入口 marker 在拼接前被过滤，并设置 1 MiB 硬限制；大型 workload 不可能进入该路径。
5. 生成器和 verifier 均拒绝 `__jetstreamResources`、`scripts.join("\n")`、超过 512 KiB 的 runner。

## 功能结果

按“能够运行完成”的功能口径为 **15/19 PASS（78.9%）**：

- PASS：ai-astar、crypto、gaussian-blur、hash-map、cdjs、intl、mobx、web-ssr、navier-stokes、raytrace、regexp、richards、splay、stanford-crypto-sha256、test-cdjs。
- 非 PASS：jsdom-d3-startup、threejs、WSL、validatorjs。

其中 ai-astar、splay 的无外部 working-set 终止聚焦运行分别完成于约 34.8 秒、6.6 秒，并输出 `JETSTREAM_RUN_COMPLETE`；regexp 和 MobX 同样恢复 PASS。

## 1.5 GiB 保护矩阵

| Runner | 保护矩阵结果 | 墙钟时间 | 峰值 working set |
|:--|:--:|--:|--:|
| ai-astar | MEMORY_LIMIT | 9.183s | 1561.0 MiB |
| crypto | PASS | 3.084s | 135.4 MiB |
| gaussian-blur | PASS | 14.594s | 181.9 MiB |
| hash-map | PASS | 12.669s | 1172.6 MiB |
| jetstream2-cdjs | PASS | 18.003s | 1192.6 MiB |
| jetstream2-intl | PASS | 0.960s | 167.1 MiB |
| jetstream2-jsdom-d3-startup | MEMORY_LIMIT | 1.053s | 1549.4 MiB |
| jetstream2-mobx | PASS | 7.133s | 763.4 MiB |
| jetstream2-threejs | MEMORY_LIMIT | 90.124s | 1536.8 MiB |
| jetstream2-validatorjs | ASSERTION_FAILURE | 1.711s | 224.9 MiB |
| jetstream2-web-ssr | PASS | 7.809s | 1423.4 MiB |
| jetstream2-WSL | MEMORY_LIMIT | 1.303s | 1595.1 MiB |
| navier-stokes | PASS | 2.104s | 29.6 MiB |
| raytrace | PASS | 13.767s | 917.1 MiB |
| regexp | PASS | 6.074s | 1077.4 MiB |
| richards | PASS | 7.466s | 739.0 MiB |
| splay | MEMORY_LIMIT | 4.907s | 1544.8 MiB |
| stanford-crypto-sha256 | PASS | 7.975s | 182.1 MiB |
| test-cdjs | PASS | 18.278s | 1315.2 MiB |

因此保护矩阵是 13/19 PASS；ai-astar、splay 属于“功能通过但超过当前 1.5 GiB 预算”，不能再误记为正确性失败。

## 大型 runner 双重源码审计

| Runner | entry 源码 | 模式 | runner 大小 | 资源正文内嵌 | workload 整包拼接 |
|:--|--:|:--:|--:|:--:|:--:|
| WSL | 684,016 bytes / 148 文件 | staged | 约 122 KiB | 否 | 否 |
| threejs | 1,284,409 bytes / 2 文件 | staged | 约 116 KiB | 否 | 否 |
| jsdom-d3-startup | 9,173 bytes entry；约 3.6 MB preload data | isolated-host | 约 116 KiB | 否 | 否 |

审计结论：

- canonical runner 中没有 `var __jetstreamResources`；
- 没有 `this.scripts.join("\n")`；
- WSL/threejs 的 workload 不进入 runner parser，也不进入 inline `new Function`；
- jsdom 的大型 bundle 是 benchmark 运行时主动读取和变换的 preload 数据，不属于 runner 源码内嵌或 runner 首次解析；
- isolated-host 路径最多处理 640 KiB entry，且 inline compiler 边界另有 1 MiB 硬限制，无法退化为 WSL/threejs 式整包拼接。

## 验证

- 19 个 canonical runner 重复生成 SHA-256 全部一致；
- Host 默认关闭、路径逃逸拒绝、CRLF/CR/LF 规范化测试通过；
- Node：MobX、web-ssr、jsdom startup 均到达 `JETSTREAM_RUN_COMPLETE`；
- AgentJS 聚焦：ai-astar、splay、regexp、MobX 均到达 `JETSTREAM_RUN_COMPLETE`；
- 最终保护矩阵无 TIMEOUT，结束后无残留 AgentJS 进程。

原始数据：`reports/jetstream2-generated-final-2026-08-04/summary.json` 及同目录逐项日志。
