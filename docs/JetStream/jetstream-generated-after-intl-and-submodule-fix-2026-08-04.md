# JetStream generated runner：最新分支复测（2026-08-04）

> 测试日期：2026-08-04  
> 代码版本：`ec64a7d`  
> 对照报告：`docs/JetStream/jetstream-generated-after-ab-merge-2026-08-04.md`（版本 `638750f`）  
> JetStream 源码：`benchmarks/JetStream2`，revision `b7babdf323e64e69bd2f6c376189c15825f5c73a`  
> 测试对象：`benchmarks/generated/` 的 19 个 canonical runner  
> 排除项：6 个 `*-node.js` 参考副本  
> AgentJS：当前源码重新构建的 `target/release/agentjs.exe`  
> 参数：顺序单进程、每项外部墙钟上限 150 秒，并传入 `--wall-clock-seconds 150`

## 构建与 Rust 验收

此前 `cargo test`/`clippy --all-targets` 的阻塞不是 RegExp 实现错误，而是 JetStream 子模块工作树为空，导致：

```text
couldn't read tests\../benchmarks/JetStream2/Octane/regexp.js
```

本轮将官方 JetStream 仓库检出到标准目录，并对齐父仓库锁定的 revision。测试代码继续使用标准路径，没有改为读取 generated runner，也没有复制基准源码。

正式验收结果：

- `cargo fmt --all -- --check`：PASS
- `cargo test --locked --no-default-features`：PASS；库测试 `265/265`，全部集成测试与 doc tests 通过
- `cargo test --locked --no-default-features --test jetstream_regexp`：`4/4` PASS，包含 Octane checksum 校验
- `cargo clippy --locked --no-default-features --all-targets -- -D warnings`：PASS
- `cargo build --release --locked --no-default-features`：PASS

## 结果总览

| Runner | 最新结果 | Overall Score | runner Wall-Time | 对照结果 |
|:--|:--:|--:|--:|:--:|
| ai-astar | PASS | 0.07 | 108.553s | PASS |
| crypto | PASS | 0.53 | 9.496s | PASS |
| gaussian-blur | PASS | 0.10 | 53.200s | PASS |
| hash-map | PASS | 0.15 | 33.623s | PASS |
| jetstream2-cdjs | PASS | 0.09 | 53.666s | PASS |
| jetstream2-intl | PASS | 27.40 | 1.105s | RESULT_MISMATCH |
| jetstream2-jsdom-d3-startup | CALL_ERROR | - | 142.028s（进程） | MEMORY_LIMIT |
| jetstream2-mobx | PASS | 0.39 | 12.786s | PASS |
| jetstream2-threejs | TIMEOUT | - | >150s | TIMEOUT |
| jetstream2-validatorjs | ASSERTION_FAILURE | - | 2.393s（进程） | ASSERTION_FAILURE |
| jetstream2-web-ssr | PASS | - | 13.526s（进程） | CALL_ERROR |
| jetstream2-WSL | TIMEOUT | - | >150s | MEMORY_LIMIT |
| navier-stokes | PASS | 1.27 | 4.040s | PASS |
| raytrace | PASS | 0.16 | 32.209s | PASS |
| regexp | PASS | 0.59 | 14.517s | PASS |
| richards | PASS | 0.29 | 17.489s | PASS |
| splay | PASS | 0.50 | 15.933s | PASS |
| stanford-crypto-sha256 | PASS | 0.21 | 23.619s | PASS |
| test-cdjs | PASS | 0.11 | 46.644s | PASS |

`jetstream2-web-ssr` 正常退出并输出 `JETSTREAM_RUN_COMPLETE`，但没有产生 `Overall` 指标，因此只计功能 PASS，不用于性能比较。

## 覆盖率变化

- 对照版本：13/19 PASS（68.4%）。
- 最新版本：15/19 PASS（78.9%）。
- 净变化：新增 2 个 PASS，提升 10.5 个百分点；非 PASS 从 6 项下降到 4 项。
- 新增 PASS：`jetstream2-intl`、`jetstream2-web-ssr`。
- 原有 PASS 没有变为功能 FAIL/TIMEOUT。

## 共同通过项目的单次性能变化

| Runner | 对照 Wall-Time | 最新 Wall-Time | 变化 |
|:--|--:|--:|--:|
| ai-astar | 35.714s | 108.553s | +204.0% |
| crypto | 3.126s | 9.496s | +203.8% |
| gaussian-blur | 14.845s | 53.200s | +258.4% |
| hash-map | 13.198s | 33.623s | +154.8% |
| jetstream2-cdjs | 20.012s | 53.666s | +168.2% |
| navier-stokes | 2.242s | 4.040s | +80.2% |
| raytrace | 13.404s | 32.209s | +140.3% |
| richards | 7.581s | 17.489s | +130.7% |
| splay | 9.161s | 15.933s | +73.9% |
| stanford-crypto-sha256 | 7.967s | 23.619s | +196.5% |
| test-cdjs | 20.012s | 46.644s | +133.1% |

这组结果显示功能覆盖提升，但相对于对照报告存在广泛的单次性能回退风险。为排除单个批次噪声，额外复测了三项：

| Runner | 全量批次 | 独立复测 | 对照 |
|:--|--:|--:|--:|
| crypto | 9.496s | 6.971s | 3.126s |
| navier-stokes | 4.040s | 4.692s | 2.242s |
| richards | 17.489s | 13.631s | 7.581s |

独立复测有一定波动，但仍明显慢于对照，因此不能仅解释为某一个 runner 的偶发离群值。由于两份报告来自不同时间的单次墙钟测试，这里应视为需要进一步稳定采样和提交二分的性能风险，而不是最终微基准结论。

## 功能变化分析

### Intl

对照版本在 validate 阶段失败：

```text
Invalid totalLength = 15556, expected >= 40000
```

最新版本完整通过，`Overall Score = 27.40`，`Overall Wall-Time = 1.105s`。NumberFormat/DateTimeFormat 相关修复已经从局部 Test262 语义覆盖转化为完整 JetStream Intl PASS。

### web-ssr

对照版本在 iteration 阶段报 `undefined is not callable`。最新版本正常输出 `JETSTREAM_RUN_COMPLETE`，由 CALL_ERROR 变为 PASS。

### jsdom-d3-startup

不再以外部 working-set 规则归类为 `MEMORY_LIMIT`，但约 142 秒后仍失败：

```text
JetStream2 failed: undefined is not callable
```

说明大型 payload 已推进到真实执行错误，但尚未通过。

### validatorjs

仍失败于相同日期/对象字符串化断言：

```text
Assertion failure: 2010-07-02,[object Object]
```

RegExp 编译和 checksum 已通过，剩余问题不是本轮子模块路径问题。

### threejs 与 WSL

两者在 150 秒内没有形成有效完成输出，继续归类为 TIMEOUT。CLI 内部墙钟限制不能覆盖所有解析/初始化阶段，因此本轮使用独立进程的外部硬超时保证批次可继续执行。

## 结论

1. Cargo test/clippy 阻塞已按项目标准解决：恢复 `benchmarks/JetStream2` 官方源码目录，而不是改变测试引用边界。
2. 功能覆盖从 13/19 提升到 15/19，主要收益是 Intl 和 web-ssr 转为 PASS。
3. 当前剩余 4 项非 PASS：jsdom-d3 CALL_ERROR、validatorjs ASSERTION_FAILURE、threejs TIMEOUT、WSL TIMEOUT。
4. 共同通过项目的墙钟时间普遍高于对照报告，存在整体性能回退风险；建议下一步用固定机器状态、每项至少 5 轮 median/p90，并按 `638750f..ec64a7d` 做提交二分。

## 原始数据

- 批次摘要：`reports/jetstream2-2026-08-04-latest/summary.json`
- 每项 stdout/stderr：`reports/jetstream2-2026-08-04-latest/<runner>.txt`
- generated runner revision：JetStream `b7babdf323e64e69bd2f6c376189c15825f5c73a`
