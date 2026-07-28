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

后续定位纠正：

- 对象 rest 和 `super(args)` 没有丢失 `tags`；
- 旧 harness 将每个资源放进独立 `new Function`，导致 workload 的
  `class Benchmark` 词法绑定丢失，runner 错误解析到 Driver 的同名类；
- 修复为把同一 workload 的资源和 runner 合并到一个共享函数作用域；
- 随后暴露并修复 `obj.method?.()`、`(obj?.method)()` 和
  `super.method?.()` 丢失 receiver 的 bytecode lowering。

修复后 7 个旧 runner 均越过原来的 `tags`/`this` 异常。当前剩余错误已
分别进入 Intl 结果校验、动态 Function、全局库绑定及其他 workload 能力；
`WSL` 在 90 秒诊断窗口内超时。

## 验证

- `node --check benchmarks/generated/*.js`：31/31 通过；
- `cargo fmt --all -- --check`：通过；
- `cargo check --all-targets`：通过；
- `cargo test --all-targets`：通过；
- `cargo clippy --all-targets -- -D warnings`：通过。

可选链 Test262 从 28/38 提升到 30/38，新通过
`optional-call-preserves-this.js` 和
`super-property-optional-call.js`。Class 目录复测保持：

- `language/statements/class`：4165/4367；
- `language/expressions/class`：3902/4059。

为清除 ABC 合并后新增的 clippy 阻塞，同时删除了
`src/vm/interpreter.rs` 中无调用点、仅转发到
`construct_value_with_new_target` 的私有 `construct_value` 包装方法。
