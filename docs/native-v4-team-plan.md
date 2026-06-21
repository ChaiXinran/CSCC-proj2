# Native V4 扩大开发分工

本文档将 [V4 扩大范围](native-v4-scope.md) 的 V4E.0–V4E.4 映射到 A/B/C/D
四组。目标是让各组独立开发、独立测试，并降低共享文件冲突。

## 1. 总体原则

- A/B/C/D 继续遵守既有目录所有权，不为了平均工作量跨层实现功能。
- V4 扩大开发是 Runtime/Builtins 密集阶段，C 组工作量天然较大，因此拆为
  C0–C3 四个连续子任务。
- `src/contracts.rs`、`runtime/value.rs`、`runtime/context.rs`、
  `vm/interpreter.rs` 和 `builtins/mod.rs` 是高冲突文件，同一时间只允许一个
  子任务修改。
- V4E.0 契约分支必须先合并；其他组不得自行改变 `BuiltinId`、调用签名、
  `Intrinsics` 或函数对象表示。
- Boa 只用于差分参考，不得作为 Native 实现或测试兜底。

## 2. A 组：前端兼容与语法补缺

负责目录：

```text
src/lexer/
src/ast/
src/parser/
```

扩大 V4 不新增核心语法，A 组主要负责：

- 保持 `delete`、`in`、`instanceof`、getter/setter、`__proto__` 和数组空洞；
- 修复 Object/Array/Function Test262 暴露的范围内解析问题；
- 正确处理 identifier/string/number 属性名和重复 `__proto__` 早期错误；
- 为每个修复增加 Parser 单元测试；
- 不修改 Compiler、VM、Runtime 或 Builtins。

交付测试：

```text
source -> AST
合法 Builtin 调用语法 -> Parse 成功
范围内 Early Error -> ParseError
```

分支建议：`feat/v4e-frontend-compat`。

当前状态：**已完成**。

已交付：

- 关键字可作为点号后的 IdentifierName，例如 `object.delete`；
- 关键字属性名使用稳定源码拼写，不依赖 Debug 格式；
- 调用参数支持单个尾逗号；
- `tests/frontend_v4.rs` 独立覆盖 Builtin 调用形状、V4 正向 Test262 文件和
  范围内 Early Error；
- 未修改 Compiler、VM、Runtime 或 Builtins。

## 3. B 组：字节码兼容与调用契约

负责目录：

```text
src/bytecode/
tests/bytecode_v4_contract.rs
tests/frontend_bytecode_v4.rs
```

B 组任务：

- 保证普通调用和构造指令可同时承载用户函数与 Builtin；
- 确认 `Object.create(...)`、`Array(...)`、`array.push(...)`、
  `Function.prototype.call(...)` 不需要专用 Opcode；
- 保持 `Call`、`CallWithThis`、`Construct` 的求值顺序和栈布局；
- 如 V4E.0 调整可调用值表示，只修改编译器所需的最小接缝；
- 增加手工 AST/Chunk 测试和 `Chunk::validate()` 覆盖；
- 不在 Compiler 中实现具体 Object/Array/Function 行为。

交付测试：

```text
Object.create(base) -> LoadGlobal/GetMethod/CallWithThis
Array(1, 2)         -> LoadGlobal/Call
new Array(3)        -> LoadGlobal/Construct
array.push(1)       -> receiver 保留
Function.prototype.call.call(...) -> 调用顺序不变
```

若现有 Opcode 已满足全部契约，B 组允许只提交测试证明，不强制增加新指令。

分支建议：`feat/v4e-bytecode-contract`。

当前状态：**已完成**。

审计结论与交付：

- 现有 `Call`、`CallWithThis`、`Construct` 足以承载用户函数和 Builtin；
- `Object.create`、`Array(...)`、`new Array(...)`、`array.push(...)` 和嵌套
  `Function.prototype.call.call(...)` 均使用通用指令；
- 手工 Chunk 已验证固定栈效果、最大栈深度和 `Chunk::validate()`；
- A→B 源码集成测试已验证 receiver 与参数顺序；
- 未增加具体 Builtin Opcode，未修改 VM、Runtime 或 Builtins。

`obj[key]()` 的 receiver 保留需要新的通用计算成员调用契约，不属于本轮冻结
交付；后续若 Test262 要求，应先更新共享接口再由 B/C 联合实现。

## 4. C 组：VM、Runtime 与 Builtins

负责目录：

```text
src/vm/
src/runtime/
src/builtins/
src/contracts.rs
tests/native_v4_runtime.rs
```

### C0：Builtin 基础设施

对应 V4E.0，必须最先完成：

- `BuiltinId`、`BuiltinFunction`、`NativeCall`、`NativeConstruct`；
- Builtin 注册表和 `NativeContext::register_builtin`；
- `Intrinsics` 及初始化顺序；
- 用户函数与 Builtin 的统一 `call`/`construct` 分发；
- 将 Test262 `assert`、`Test262Error` 迁移到注册表；
- 只建立 Object/Array/Function 构造器和原型骨架，不实现具体静态方法。

主要共享文件：

```text
src/runtime/value.rs
src/runtime/function.rs
src/runtime/context.rs
src/vm/interpreter.rs
src/builtins/mod.rs
src/contracts.rs
```

分支建议：`feat/v4e-builtin-core`。

### C1：Object Builtins

必须基于已合并的 C0：

```text
src/builtins/object.rs
src/runtime/property.rs
src/runtime/property_map.rs
```

实现 `Object`、`create`、`defineProperty`、`getOwnPropertyDescriptor`、
`getPrototypeOf`、`setPrototypeOf`、`keys`。除非接口确有缺口，不修改
`vm/interpreter.rs`。

分支建议：`feat/v4e-object-builtins`。

### C2：Array Builtins

必须基于已合并的 C0，可与 C1 并行，但避免同时修改 `context.rs`：

```text
src/builtins/array.rs
src/runtime/object.rs
```

实现 `Array`、`Array.isArray`、`push`、`pop`，并复用现有稀疏数组与 `length`
语义。禁止建立第二套数组元素存储。

分支建议：`feat/v4e-array-builtins`。

### C3：Function Builtins

在 C0、C1、C2 合并后进行：

```text
src/builtins/function.rs
src/runtime/function.rs
src/vm/interpreter.rs
```

实现 `Function` 全局和 `Function.prototype.call`，修正函数对象、
`"prototype"`、`"constructor"` 与 `instanceof` 的可观察连接。动态源码
构造可明确返回 Unsupported。

分支建议：`feat/v4e-function-builtins`。

## 5. D 组：集成、Test262、CI 与报告

负责文件：

```text
src/test262.rs
src/main.rs
tests/native_test262.rs
tests/native_v4.rs
reports/native-v4-test262-report.md
.github/workflows/ci.yml
readme.md
```

D 组任务：

- 在 C0 合并前准备 ignored/候选测试，不伪造通过结果；
- 为 C1/C2/C3 分别添加源码端到端测试；
- 扫描 Object、Array、Function/`call` 和 `instanceof` 官方目录；
- 逐文件排除 harness、超范围语法和假通过；
- 扩大 `NATIVE_V4_TESTS`，保留 V1–V3 零回归；
- 更新目录基线、报告和 CI；
- 不修改 Parser、Compiler 或 Runtime 来“让测试通过”。

分支建议：`test/v4e-test262-expansion`。

## 6. 共享文件写入规则

| 文件 | 首要所有者 | 允许修改时机 |
| --- | --- | --- |
| `src/contracts.rs` | C0 | 仅 V4E.0 契约提交 |
| `runtime/value.rs` | C0 | Builtin 值表示冻结前 |
| `runtime/context.rs` | C0 | C1/C2 需要扩展时先评审接口 |
| `runtime/function.rs` | C0/C3 | C0 建结构，C3 完成语义 |
| `vm/interpreter.rs` | C0/C3 | C1/C2 不直接添加专用 Builtin 分支 |
| `builtins/mod.rs` | C0 | C1/C2/C3 只注册所属模块导出的安装函数 |
| `src/test262.rs` | D | 其他组只提供候选文件清单 |
| `readme.md`、报告、CI | D | 功能合并后统一更新 |

需要修改非本组文件时，先在接口文档记录原因，由该文件所有者提交接缝修改。

## 7. 合并顺序

```text
V4E.0 契约与 C0 Builtin Core
        ↓
   B 调用契约验证
        ↓
  C1 Object ─┐
             ├→ C3 Function → D Test262/CI
  C2 Array ──┘
        ↑
 A 按 Test262 结果补范围内语法
```

每次合并后必须运行：

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
```

## 8. 各组完成定义

- A：范围内语法测试通过，未跨层实现 Builtin。
- B：所有调用/构造 Chunk 合法，未增加具体 Builtin Opcode。
- C0：三个全局构造器和 Intrinsics 骨架存在，注册表可调用和构造。
- C1：Object 自有端到端样例与直接 Runtime 测试通过。
- C2：Array 自有端到端样例与稀疏数组回归通过。
- C3：Function.call、函数原型关系和 `instanceof` 通过。
- D：V4 固定清单扩大、零失败、零跳过，报告如实记录目录基线。
