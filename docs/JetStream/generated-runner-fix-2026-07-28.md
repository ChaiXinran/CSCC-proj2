# JetStream generated runner 修复报告（2026-07-28）

## 修复范围

- 删除 `scripts/prepare-jetstream2.mjs` 中旧的 class 语义降级：
  - 不再删除 `extends`；
  - 不再把 `super(...args)` 改成普通初始化函数；
  - 不再通过 `Object.setPrototypeOf` 手工拼接 JetStream benchmark 类层级。
- 当前 JetStream2 生成物保留原生
  `Benchmark -> DefaultBenchmark -> AsyncBenchmark` 继承链。
- 重新生成当前 `benchmarks/JetStream2` 能提供的 runner。
- 修复 7 个来源版本早于当前 JetStream2 树的 JetStream3 生成物：
  - 恢复原生 class 继承；
  - 补齐 shell 的 `performance`、`console.assert`、`runString`、`load`
    和 `JetStream` 全局入口。
- 生成物同时支持 AgentJS 的宿主 `print` 和直接 Node 执行。

## 结果变化

修复前：

- AgentJS：8/25 通过；
- 13 个文件因基类 constructor 中存在非法 `super()` 而解析失败；
- 4 个文件因缺少 `JetStream` harness 失败；
- 6 个 `*-node.js` 文件无法直接由 Node 执行。

修复后：

- 全部 31 个 JavaScript 文件通过 `node --check`；
- 非法基类 `super()`：13 -> 0；
- 缺少 `JetStream` harness：4 -> 0；
- AgentJS：17/25 通过；
- Node 参考 runner：6/6 通过。

当前 AgentJS 剩余失败：

- `regexp.js`：RegExp look-around 尚未实现；
- 7 个旧 JetStream3 workload：已经进入原生 class/workload 路径，但在
  `DefaultBenchmark` 的对象 rest 参数传递后丢失 `tags`，最终在
  `_processTags(rawTags).map(...)` 失败。这是引擎的对象 rest/派生构造共享
  语义问题，不再通过修改测试文件绕过。

## 验证

- `node --check benchmarks/generated/*.js`：31/31 通过；
- `cargo fmt --all -- --check`：通过；
- `cargo check --all-targets`：通过；
- `cargo test --all-targets`：通过；
- `cargo clippy --all-targets -- -D warnings`：通过。

为清除 ABC 合并后新增的 clippy 阻塞，同时删除了
`src/vm/interpreter.rs` 中无调用点、仅转发到
`construct_value_with_new_target` 的私有 `construct_value` 包装方法。
