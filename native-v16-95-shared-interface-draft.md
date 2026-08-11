# Native V16 / 95% Shared Interface Draft

> 目标：从 Test262 83.60% 推进到 95%  
> 原则：一个功能一个 owner；一个抽象操作一个实现；跨层修改不拆责任。

## 1. Ownership

| Area | Owner |
|---|---|
| Parser / AST / language lowering / completion / scope | A |
| Object model / abstract ops / iterator / promise / module | B |
| Temporal / Intl / RegExp / binary data / Atomics | C |
| Integration branch / full Test262 evidence | 集成负责人 |

## 2. Shared Operations

建议新增 `src/runtime/abstract_ops.rs`，由 B 独占维护。

必须集中实现：

```text
Get / Set / HasProperty / GetMethod
Call / Construct
ToPrimitive / ToNumeric / ToString / ToPropertyKey
ToIntegerOrInfinity / ToLength / ToIndex
OrdinaryCreateFromConstructor
SpeciesConstructor / ArraySpeciesCreate
GetIterator / IteratorNext / IteratorClose
PromiseCapability / PerformPromiseThen
```

其他模块禁止复制等价 helper。

## 3. VM Callback Rule

以下行为必须走 VM call path：

```text
getter / setter
Proxy trap
callback / comparator
iterator method
Symbol.toPrimitive
species constructor
Temporal / Intl options getter
```

## 4. Internal Slots

复杂对象使用 typed runtime records 或 `ObjectKind`，不新增普通属性形式的 `__agentjs_*` 内部槽。

## 5. Error Phase

```text
Parser grammar error        -> ParseError
Static semantic early error -> ParseError / compile-stage SyntaxError
Runtime semantic error      -> VmError(TypeError/RangeError/ReferenceError...)
Abrupt completion           -> Completion
Unsupported feature         -> 只允许明确未实现路径
```

不得用 Unsupported 或统一 SyntaxError 掩盖错误阶段。

## 6. Realm Contract

- 默认 constructor/prototype 从当前 Realm 的 Intrinsics 获取；
- error constructor 身份不能只比较名字；
- cross-Realm brand check 使用 internal slot；
- module namespace、iterator helper、Temporal object 保持 Realm 归属。

## 7. Async Contract

- 所有 continuation 进入统一 JobQueue；
- builtin 不主动 drain；
- dynamic import、TLA、async iterator、AsyncDisposableStack 共用 Promise/Job 协议；
- rejected job 必须能传播到 Test262 async harness。

## 8. Cross-layer Feature Rule

Feature owner 可跨层修改，但必须登记锁。

例：dynamic import 归 B，B 对以下全部负责：

```text
parse import()
lower opcode
module load/link/evaluate
promise result
error rejection
TLA interaction
```

A 不承担 dynamic import 的子任务，只审查 shared parser/compiler 兼容性。

## 9. Shared-file Lock

相关接口冻结历史现汇总于 `docs/runtime-evolution.md`：

```markdown
| file/function | owner | feature | start SHA | merge order | released |
```

必须登记：

```text
src/runtime/context.rs
src/runtime/object.rs
src/builtins/mod.rs
src/vm/interpreter.rs
src/bytecode/compiler.rs
src/parser/expression.rs
src/parser/statement.rs
```

## 10. PR Evidence

每个 PR 必须提供：

```text
focused before JSON
focused after JSON
new passes
new failures
unit tests
failure root cause
shared-interface impact
known partial behavior
```

禁止：

```text
扩大 skip
修改 Test262 预期
按文件名硬编码 pass
为单测试复制规范算法
无报告修改共享文件
```

## 11. Full-suite Gate

95% 最终门槛：

```text
total = 53,379
passed >= 50,711
skip <= 2
failed <= 2,666（skip=2 时）
conformance >= 95.00%
```
