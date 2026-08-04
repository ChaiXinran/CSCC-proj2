# Phase 2 D 组共享接口偏差记录

日期：2026-08-04  
基线：`e4382f93fc400aefd07247e301dc40132634e5fd`

## 接口冻结提交尚未单独存在

共享文档要求集成人先提交只包含 `LocalSlot` 的接口冻结提交，但当前基线尚未包含该类型。D 组为了实现和验证 Local Slot，在 `src/bytecode/chunk.rs` 定义了文档冻结形状的 `LocalSlot`、`LocalBindingLayout`、`LocalLayout` 和 `DynamicScopePolicy`，并只从 `src/bytecode/mod.rs` 导出供 crate 与测试使用。

D 组没有修改 `src/contracts.rs`、`src/runtime/mod.rs` 或 `src/lib.rs`。最终集成人仍负责决定这些类型是否进入稳定公共合同以及解决 F/E 合并后的导出位置。

## 动态作用域检测时点

共享文档规定含 direct eval 或 `with` 的函数整体回退，但没有冻结检测发生在 AST 扫描还是 lowering 后。D 组先按静态 layout lowering 当前函数，再检查该函数自身生成的 `DirectEval` 和 `EnterWithEnvironment` 指令；若存在，则将该函数的 Local opcode 回写为对应名字 opcode，并清空 layout。嵌套函数拥有独立 chunk，不会错误触发外层函数回退。

该方式避免修改解析器和 AST 公共结构，同时保证最终缓存字节码中不存在“动态策略 + Local opcode”的混合状态。

## 诊断统计边界

名字解析计数按每次 `NativeRuntime::evaluate` 重置并输出。JetStream staged runner 会分别执行 driver、资源和 launch，因此诊断日志包含多个 `name_resolution:` 记录；汇总工具应累加同一进程的全部记录，不能只读取最后一条。
