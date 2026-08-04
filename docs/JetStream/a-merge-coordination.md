# JetStream A 组后续与合并协调记录

更新时间：2026-08-04

## 用途

本文档只记录 A 组后续工作中与 B/C 组可能冲突、需要对方提供能力、或合并时需要特别处理的事项。A 组不直接修改 B/C 的独占实现。

## 当前 A 组基线

- runner/resource 修复已经位于当前工作区，尚未提交。
- 19 个 canonical runner 已重新生成，并各有确定性 manifest。
- Node 参考环境单轮运行 19/19 完成。
- AgentJS 单轮短时诊断：7 项 PASS、4 项明确引擎失败、7 项性能超时。
- 最近一次全量 Test262：48,423/53,379 通过，4,954 失败，2 跳过；相对用户提供的基线为 +4 通过、-4 失败、跳过不变。

## A 组负责范围

- `scripts/`
- `benchmarks/generated/`
- JetStream 运行、诊断和报告工具
- 第二轮计划中的 compiler/opcode/environment-slot 性能工作，但只有在 profiler 证据足够且不修改共享接口时推进
- `reports/v17-partA-report.md`

## B/C 独占边界

### B 组

- `src/builtins/regexp.rs`
- `src/unicode_set.rs`
- RegExp 正确性和 validatorjs/regexp checksum
- 对象模型、property map 和 inline cache 方向

### C 组

- `src/intl/`
- Intl builtin 与 NumberFormat
- `Cargo.toml`、`Cargo.lock` 中的 Intl 依赖
- GC、heap、JsValue 分配方向
- `src/main.rs` 中计划内的 JetStream 内部诊断入口

## 当前协调事项

| 状态 | A 组观察 | 需要的其他组能力 | 合并注意事项 |
|---|---|---|---|
| 待 B | `validatorjs` 为明确引擎失败 | B 修复 RegExp/调用链后重新生成并复测 | A 不修改 `regexp.rs` 或 VM |
| 待 B | `regexp-octane` 为明确引擎失败 | B 提供第一个 checksum 分歧或修复 | A 可提供 runner/checkpoint，不改语义 |
| 待 C | `intl` 为明确引擎失败 | C 完成真实 NumberFormat | A 不修改 Intl/Cargo 依赖 |
| 待 C | `threejs` 和 WSL 在 payload marker 前超时 | C 提供内部 limit/heap/GC 诊断入口 | A 负责消费诊断输出并生成矩阵 |
| 待协调 | `web-ssr` 已进入 iteration 后出现 `undefined is not callable` | 需要 B/C 或 builtin 负责人确认归属 | A 保留最小阶段证据，不改引擎语义 |
| 已知 | A 组已修改 `scripts/` 和 generated runner | B/C 复测依赖这些生成物 | 应先合并 A，再合并 B/C |
| 进行中 | 普通 `LoadName` 当前先构造 `name_resolution_chain`，随后再次遍历解析 binding | 涉及 `src/runtime/context.rs` 与 `src/vm/interpreter.rs`，可能与 B 的调用链或 C 的 runtime 工作重叠 | 仅增加不改变 observable semantics 的无 object-environment 快速路径；不改 `Instruction`、`contracts.rs` 或 chunk 格式 |
| 暂缓 | `LoadLocal`/`StoreLocal` slot opcode | 会改变稳定的 `Instruction` 公共边界并要求 compiler/VM/chunk 同步 | 需要团队评审后单独实施，不与当前快速路径混合 |

## A4 最终诊断结论

- threejs 的 1/2/5/10 四档均在 15 秒超时，CPU 约 14.5–14.8 秒，峰值 working set 约 250–254 MiB。
- threejs 四档均没有产生任何 JS 输出，也没有到达 `init:start`，说明停留在 payload 执行前的引擎解析/编译阶段；与迭代数无关。
- WSL 的 1/2/5/10 四档均在约 0.94–0.99 秒触发 1 GiB working-set 保护，观测峰值约 1.02–1.13 GiB。
- WSL 四档均没有产生任何 JS 输出，也没有到达 `init:start`，说明是 payload 执行前的内存膨胀；与迭代数无关。
- 诊断工具现在逐项落盘并记录 workload/init/iteration/validate 状态、最后阶段、最后输出 UTC、CPU 和峰值内存。没有输出时，最后输出字段明确为 `null`。
- 这些现象属于需要其他组引擎内部诊断能力继续定位的事项；A 组不修改 `src/main.rs`、VM、GC 或 builtin。

## A 组后续计划

### A5：诊断结果消费和可合并报告（已完成工具）

- 外部超时、阶段 marker、CPU、峰值内存和完成迭代数已由诊断脚本汇总为 JSON。
- 新增 `scripts/compare-jetstream2-status.mjs`，支持后续摘要对比并自动标记 improvement、regression、changed 和 unchanged。
- 对比结果同时检查 runner SHA-256；旧摘要没有哈希时明确输出 `null`。
- 禁止把短时诊断结果写成正式 benchmark 分数。

### A6：可信性能基线（已完成采样工具）

- 新增 `scripts/measure-jetstream2.ps1`，可运行指定 workload 的独立多轮采样。
- 报告 median、min/max、p90、MAD、标准差、CPU 时间和 peak working set。
- 固定 runner SHA-256、JetStream revision、AgentJS revision、dirty 状态和运行参数。
- navier-stokes 单样本 smoke 通过；正式基线仍要求至少 5 次采样。

### A7：基于证据选择 A 组性能修复

- 优先验证局部变量名称查找、环境链和 opcode 分布是否为热点。
- 若需要改变 `contracts.rs`、chunk/opcode 公共格式或 VM 共享行为，先登记并停止实现，等待合并协调。
- 不在缺少 profiler 数据时直接重写 environment 或引入大范围 opcode 变更。

## 合并顺序建议

1. 合并 A 组 runner、resource、manifest 和诊断脚本。
2. B/C 基于相同 manifest SHA-256 复测。
3. 合并 B 的 RegExp 修复和 C 的 Intl/内部诊断修复。
4. A 重新生成 19 个 runner，并运行状态变化矩阵。
5. 再决定 A 组是否进入 compiler/opcode/environment-slot 优化。
6. 集成阶段统一执行工程门禁、focused Test262、全量 Test262 和正式 JetStream 多轮测试。

## 验证规则

- A 组脚本变更先用 Node reference shell 验证生成物。
- 轻量阶段使用 focused 测试和 `cargo test --all-targets`，不反复运行全量 Test262。
- A 组引擎性能修改阶段完成后必须跑全量 Test262，并与 48,423 通过的当前工作区基线比较。
- 任意 correctness 回退都优先于性能收益处理。
