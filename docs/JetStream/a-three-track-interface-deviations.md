# A 组共享接口未定义事项记录

日期：2026-08-04  
基线：`ec64a7dec2231421adb42c7f40c688338af26fe3`

## 逐文件执行的阶段边界

共享接口要求资源不进入 runner、按顺序逐文件执行，并保持同一 Runtime、Realm 和 global environment，但没有定义 Rust CLI 如何在 runner 的 driver 初始化与启动之间插入资源执行。

A 组采用以下内部协议：

- runner 用 `// AGENTJS_RESOURCE:<normalized-path>` 声明有序入口文件；
- runner 用 `/*__AGENTJS_LOAD_RESOURCES__*/` 标出 driver prelude 与 launch 的边界；
- CLI 在同一个 `Runtime` 中依次执行 prelude、入口资源、launch；
- 运行期数据文件和动态发现文件继续通过受根目录约束的 `readFile()` 读取；
- 该协议不进入 `src/contracts.rs`，也不修改 B/C 的字节码或 GC 接口。

原因：当前 JS `new Function` 能隔离 driver 与 workload，但多个调用不共享函数作用域；全局 `eval` 又会让 driver 的 `Benchmark` 与 workload 同名声明冲突。由 CLI 在顶层逐文件执行，才能同时去掉巨型拼接并保持文件间共享的全局词法环境。

## 诊断开关

共享接口规定 `--diagnostics` 和阶段标记，但示例 `RuntimeConfig` 没有对应字段。A 组新增可复制的 `RuntimeConfig::diagnostics: bool`，默认 `false`，仅用于控制 `parse_start/end`、`compile_start/end` 和 `execute_start`。这符合“RuntimeConfig 只保存轻量配置”的约束，但属于文档示例未列出的字段，集成时需要确认是否并入冻结接口。

## Manifest v2 的入口执行范围

CLI 顶层执行 `entryFiles`；`preloadFiles` 与 `runtimeDiscoveredFiles` 可能是 CSV、JSON 或其他数据，不应作为 JavaScript 执行。它们保留在 manifest 的哈希表中，并由运行期 `readFile()` 按需读取。共享文档只写了“验证 manifest 所列文件存在”，没有明确三类资源是否都应执行，本实现按资源语义区分。

## 入口隔离语义

早期实现曾为不超过 640 KiB 的入口保留 `isolated-host` 路径。该路径虽然不把源码嵌入 runner，却仍会在 Driver 内拼接完整 workload，并通过 `new Function` 再次编译，违反共享接口的逐文件要求，现已删除。

所有入口现在统一使用 `staged`：CLI 按 manifest 顺序读取并在同一 Runtime、Realm 和 global environment 中逐文件执行。这个选择完全消除了 workload 的整包拼接和二次源码编译。当前引擎尚无独立且可持久化的 Host script function environment，因此少数依赖函数级隔离的 workload 可能暴露顶层声明冲突；这属于共享接口未定义的语义差异，不能通过恢复整包 `new Function` 规避。inline harness 的 1 MiB 上限和 runner 的 512 KiB 上限继续保留，但 inline harness 不包含 workload 入口源码。
