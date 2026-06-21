# Native V5 共享接口规格

本文档定义 V5（异常处理与 for 循环）开发期间 A / B / C / D 四组共同遵守的类型、字节码和 Runtime 契约。它在 `native-v4-interface.md` 基础上追加 V5 新增部分；两者发生冲突时以本文为准。

**本文档在 V5 开发启动前须经所有组审阅确认，之后视为只读。变更须重新走审阅流程。**

---

## 1. 共享 AST

### 1.1 新增语句节点

在 `src/ast/statement.rs` 中扩展 `Statement` 枚举：

```rust
pub enum Statement {
    // … 已有 variants（V1–V4）…

    /// try { body } catch(binding) { handler.body } finally { finalizer }
    /// handler 与 finalizer 至少有一个 Some。
    TryCatch {
        body: Vec<Statement>,
        handler: Option<CatchClause>,
        finalizer: Option<Vec<Statement>>,
    },

    /// for (init; test; update) body
    For {
        init: Option<ForInit>,
        test: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    },

    /// for (binding in right) body
    ForIn {
        binding: ForInBinding,
        right: Expression,
        body: Box<Statement>,
    },
}
```

### 1.2 新增辅助类型

```rust
/// catch 子句：binding 为 None 表示 `catch {}` 无参数形式（ES2019）。
pub struct CatchClause {
    pub binding: Option<String>,
    pub body: Vec<Statement>,
}

/// for 语句 init 段
pub enum ForInit {
    Var(String, Option<Expression>),   // var name [= init_expr]
    Expression(Expression),
}

/// for-in 语句左侧绑定
pub enum ForInBinding {
    Var(String),         // for (var x in obj)
    Identifier(String),  // for (x in obj)
}
```

### 1.3 约束

- `TryCatch.handler` 与 `TryCatch.finalizer` 至少有一个 `Some`，否则为语法错误；
- `ForIn.binding` V5 只支持单个标识符；不支持解构、`let`/`const`；
- `For.init` 中的 `Var` 只绑定一个名字；不支持 `for (var a, b;;)`；
- `For.test` 为 `None` 时逻辑等同于常量 `true`；
- `break`/`continue` 仍使用已有 `Statement::Break` / `Statement::Continue`；V5 不引入标签。

---

## 2. 共享 Opcode

在 `src/bytecode/opcode.rs` 中追加：

```rust
pub enum Instruction {
    // … 已有 variants（V1–V4）…

    // ── for-in ──────────────────────────────────────────────────────────────
    /// 弹出栈顶对象，收集其可枚举键（含原型链，按枚举顺序去重），
    /// 将 JsValue::ForInIterator(id) 压栈。
    ForIn,

    /// 读取栈顶 ForInIterator（不弹出）：
    ///   迭代完毕 → 弹出迭代器，跳转 ip.wrapping_add_signed(offset)；
    ///   否则       → 将下一个键字符串压栈。
    ForInNext(i32),

    /// 弹出并销毁栈顶 ForInIterator（break 提前退出时使用）。
    ForInDispose,

    // ── 异常处理 ────────────────────────────────────────────────────────────
    /// 将 NativeContext::current_exception 压栈。
    /// 仅在 catch 块入口有意义；catch 块结束前须通过环境绑定消费此值。
    LoadException,
}
```

### 2.1 栈效果表

| 指令                  | 栈 before         | 栈 after           | 说明 |
| --------------------- | ----------------- | ------------------ | ---- |
| `ForIn`               | `[…, obj]`        | `[…, iter]`        | 收集全部枚举键，创建迭代器 |
| `ForInNext(offset)`   | `[…, iter]`       | `[…, iter, key]` 或跳转后 `[…]` | 有键时压入键字符串；迭代完毕时弹出 iter 并跳转 |
| `ForInDispose`        | `[…, iter]`       | `[…]`              | 提前销毁迭代器 |
| `LoadException`       | `[…]`             | `[…, exc]`         | 读取当前捕获异常；不弹出 context 中的异常槽 |

### 2.2 说明

- `ForIn` 在创建迭代器时快照当前可枚举键列表；迭代期间对象属性变化不影响已创建的迭代器；
- `ForInNext` 迭代完毕跳转时弹出 iter，使栈深度与跳转到的循环出口处一致；
- `LoadException` 不清除 `current_exception`，catch 块结束后由 VM 负责清除；
- 新增指令不影响 V1–V4 Chunk 的解码；旧 Chunk 的 `exception_handlers` 字段默认为空 Vec。

---

## 3. Chunk 异常处理器表

在 `src/bytecode/chunk.rs` 中扩展 `Chunk`：

```rust
pub struct Chunk {
    // … 已有字段 …
    pub exception_handlers: Vec<ExceptionHandler>,
}

/// 一个 try/catch/finally 块的 IP 范围和跳转目标。
/// catch_start 与 finally_start 至少有一个 Some。
#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    /// try 块第一条指令的 IP（含）
    pub try_start: usize,
    /// try 块最后一条指令之后的 IP（不含 catch/finally）
    pub try_end: usize,
    /// catch 块第一条指令的 IP（首指令必须是 LoadException）
    pub catch_start: Option<usize>,
    /// 常量池中 catch 绑定名的索引；binding 为 None 时为 u16::MAX
    pub catch_binding: u16,
    /// finally 块第一条指令的 IP
    pub finally_start: Option<usize>,
    /// finally 块最后一条指令之后的 IP
    pub finally_end: usize,
}
```

约束：

- `catch_binding == u16::MAX` 表示 `catch {}` 无绑定；
- 嵌套 try 在表中生成多条 `ExceptionHandler`，按 try_start 升序排列；
- VM 查表时从后向前遍历（内层优先）；
- Compiler 负责填写所有字段，VM 只读取。

---

## 4. 新增 JsValue 变体

在 `src/runtime/value.rs` 中扩展 `JsValue`：

```rust
pub enum JsValue {
    // … 已有 variants …
    ForInIterator(ForInIteratorId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForInIteratorId(pub u32);
```

规则：

- `ForInIterator` 是引擎内部值，不暴露给 JS 代码（不可通过变量传递、不可作属性值）；
- `typeof ForInIterator(…)` 视为 `"object"`（若调用到 type_of，则返回 `"object"`）；
- Compiler 保证 `ForInIterator` 仅存在于 for-in 循环的迭代器槽中；
- `ForInIteratorId` 引用 `NativeContext` 内的迭代器槽，不跨 isolate。

---

## 5. Error 对象契约

V5 安装以下全局构造器（由 `builtins/error.rs` 实现，`install_foundation` 调用）：

```
Error   TypeError   RangeError   SyntaxError   ReferenceError
```

### 5.1 原型链

```
Error constructor         ──[[Prototype]]──> Function.prototype
Error.prototype           ──[[Prototype]]──> Object.prototype
TypeError.prototype       ──[[Prototype]]──> Error.prototype
RangeError.prototype      ──[[Prototype]]──> Error.prototype
SyntaxError.prototype     ──[[Prototype]]──> Error.prototype
ReferenceError.prototype  ──[[Prototype]]──> Error.prototype
```

### 5.2 属性约定

| 属性 | 值 |
| ---- | -- |
| `Error.prototype.name` | `"Error"` |
| `TypeError.prototype.name` | `"TypeError"` |
| `RangeError.prototype.name` | `"RangeError"` |
| `SyntaxError.prototype.name` | `"SyntaxError"` |
| `ReferenceError.prototype.name` | `"ReferenceError"` |
| `Error.prototype.message` | `""` |
| `error.message` | 构造时传入的字符串 |

### 5.3 行为规则

- `new TypeError(msg)` 和 `TypeError(msg)` 行为相同（ES 规范允许）；
- 构造器不带参数时，`message` 为空字符串；
- Error 对象的 `[[Prototype]]` 由 `ordinary_object_with_prototype` 设置，不绕过属性描述符 API；
- `instanceof TypeError` / `instanceof Error` 复用 V4 的 `[[HasInstance]]`，无需特殊处理；
- V5 不实现 `.stack` 属性；
- `AggregateError`、`EvalError`、`URIError` 不在 V5 范围内。

---

## 6. NativeContext 新增接口

在 `src/runtime/context.rs` 中新增以下公开方法：

```rust
impl NativeContext {

    // ── for-in 迭代器 ────────────────────────────────────────────────────────

    /// 收集 object 的可枚举字符串键（含原型链，去重，插入顺序），
    /// 创建迭代器并返回其 ID。
    pub fn create_for_in_iterator(
        &mut self,
        object: ObjectId,
    ) -> Result<ForInIteratorId, VmError>;

    /// 取出下一个键；返回 None 表示迭代完毕。
    pub fn for_in_next(
        &mut self,
        id: ForInIteratorId,
    ) -> Option<String>;

    /// 销毁迭代器（break 退出时调用）。
    pub fn dispose_for_in_iterator(&mut self, id: ForInIteratorId);

    // ── 异常槽 ──────────────────────────────────────────────────────────────

    /// 读取当前捕获异常（LoadException 指令使用）。
    pub fn current_exception(&self) -> Option<&JsValue>;

    /// VM 跳入 catch 块前设置。
    pub fn set_current_exception(&mut self, value: JsValue);

    /// 离开 catch/finally 后清除。
    pub fn clear_current_exception(&mut self);

    // ── Error 对象工厂 ───────────────────────────────────────────────────────

    /// 创建一个 TypeError 对象（JsValue::Object，带正确原型）。
    pub fn create_type_error_object(&mut self, message: &str) -> Result<JsValue, VmError>;

    /// 创建一个 RangeError 对象。
    pub fn create_range_error_object(&mut self, message: &str) -> Result<JsValue, VmError>;

    /// 创建一个 ReferenceError 对象。
    pub fn create_reference_error_object(&mut self, message: &str) -> Result<JsValue, VmError>;

    /// 创建一个 SyntaxError 对象。
    pub fn create_syntax_error_object(&mut self, message: &str) -> Result<JsValue, VmError>;
}
```

规则：

- `create_for_in_iterator` 在创建时快照键列表，不持有 object 的可变引用；
- 枚举顺序：对象自有可枚举键（插入顺序），然后递归原型链（各层插入顺序，已出现的键跳过）；
- `for_in_next` 在迭代完毕后不 panic，返回 `None`；
- `create_*_error_object` 依赖 `Intrinsics` 中对应的 Error 原型；若 Intrinsics 未初始化返回 `VmError::runtime`；
- 工厂方法在 builtins 安装完成前不得调用；VM 在执行阶段调用，此时 builtins 已就绪。

---

## 7. VM 异常传播契约

VM 执行时产生 Throw completion（来自 `throw` 语句或内部运行时错误）时，按以下顺序处理：

```
1. 将异常值存入 context.set_current_exception(value)
2. 从当前调用帧的 Chunk.exception_handlers 从后向前查找第一个
   满足 try_start <= current_ip < try_end 的处理器 H
3a. 若 H.catch_start.is_some()：
      跳转到 H.catch_start
      若 H.catch_binding != u16::MAX，在当前环境声明绑定
        （值 = context.current_exception().cloned()）
      继续执行（catch 体首指令为 LoadException，可将异常值压栈）
3b. 若 H 只有 finally_start（catch_start 为 None）：
      执行 finally 块（IP 设为 H.finally_start）
      finally 正常结束后重新传播原有异常
4.  若无匹配处理器：
      弹出当前调用帧
      若调用者帧存在，在调用者 Chunk 中重复步骤 2–4
      若已无调用帧，将 Throw(value) 映射为 NativeError::Execute
5.  finally 块中若产生新的 Throw，替换 current_exception 并重新走步骤 2
```

内部 VmError 到 JS 异常的转换：

```
VmError::type_error(msg)      → context.create_type_error_object(msg)?
VmError::range(msg)           → context.create_range_error_object(msg)?
VmError::reference_error(msg) → context.create_reference_error_object(msg)?
其他 VmError                   → 作为 NativeError::Execute 直接终止（不经过 catch）
```

规则：

- 离开 catch 块后必须调用 `context.clear_current_exception()`；
- finally 块执行期间不得自动清除 current_exception（VM 在 finally 结束后根据 completion 类型决定是否清除）；
- 调用帧退出（return / throw）时，若当前 Chunk 有覆盖调用 IP 的 finally 处理器，必须先执行 finally。

---

## 8. 编译器降级规则

### 8.1 for(;;)

```
for (init; test; update) body
─────────────────────────────
[init]
Jump(test_label)
loop_label:
  [body]
  [update]
test_label:
  [test]           ← test 为 None 时不生成，直接 JumpIfTrue(loop_label)
  JumpIfTrue(loop_label)
exit_label:

break  → Jump(exit_label)
continue → Jump(update 段首，即 test_label 之前的 update)
```

### 8.2 for-in

```
for (var k in obj) body
────────────────────────
[obj]
ForIn                       ← 弹出 obj，压入 ForInIterator
loop_label:
  ForInNext(exit_label)     ← 若迭代完毕，弹出 iter 并跳转 exit_label
  DeclareVar(k) / StoreVar(k)  ← 将刚压栈的键赋给 k
  [body]
  Jump(loop_label)
exit_label:

break  → ForInDispose + Jump(exit_label)
continue → Jump(loop_label)
```

说明：`ForInNext` 迭代完毕时自动弹出 iter，所以 exit_label 处栈深与 loop_label 入口处相同（iter 已弹）。正常退出（迭代完毕）不需要 ForInDispose；break 路径须显式插入 `ForInDispose`。

### 8.3 try/catch

```
try { try_body } catch(e) { catch_body }
────────────────────────────────────────
try_start:
  [try_body]
try_end:
  Jump(after_catch)
catch_start:               ← 异常处理器表中登记的 catch_start
  LoadException            ← 将捕获值压栈
  StoreVar(e) / DeclareVar(e)  ← 绑定 e
  [catch_body]
after_catch:

ExceptionHandler {
  try_start, try_end,
  catch_start: Some(catch_start),
  catch_binding: index_of("e"),  ← 无绑定时为 u16::MAX
  finally_start: None,
  finally_end: 0,
}
```

### 8.4 try/finally

```
try { try_body } finally { finally_body }
─────────────────────────────────────────
try_start:
  [try_body]
try_end:
  Jump(finally_start)      ← 正常路径跳入 finally
finally_start:
  [finally_body]
finally_end:

ExceptionHandler {
  try_start, try_end,
  catch_start: None,
  catch_binding: u16::MAX,
  finally_start: Some(finally_start),
  finally_end,
}
```

### 8.5 try/catch/finally

```
try_start:
  [try_body]
try_end:
  Jump(finally_start)           ← 正常路径
catch_start:
  LoadException
  [bind e if needed]
  [catch_body]
  Jump(finally_start)           ← catch 正常完成路径
finally_start:
  [finally_body]
finally_end:

ExceptionHandler {
  try_start, try_end,
  catch_start: Some(catch_start),
  catch_binding: ...,
  finally_start: Some(finally_start),
  finally_end,
}
```

---

## 9. 各组独立测试规格

### A 组（Parser）

```
"try {} catch(e) {}"           → TryCatch { handler: Some(CatchClause{binding: Some("e"), ..}), finalizer: None }
"try {} catch {}"              → TryCatch { handler: Some(CatchClause{binding: None, ..}), .. }
"try {} finally {}"            → TryCatch { handler: None, finalizer: Some([]) }
"try {} catch(e) {} finally {}" → 两者均 Some
"try {}"                        → ParseError（缺少 catch/finally）
"for (;;) {}"                   → For { init: None, test: None, update: None, .. }
"for (var i = 0; i < 10; i++) {}" → For { init: Some(Var("i", Some(..))), .. }
"for (var k in obj) {}"         → ForIn { binding: Var("k"), .. }
"for (x in obj) {}"             → ForIn { binding: Identifier("x"), .. }
"for (let k in obj) {}"         → ParseError 或 Unsupported（V5 不支持）
```

### B 组（Compiler）

```
TryCatch(body, handler=Some, finalizer=None)
  → exception_handlers[0].catch_start == Some(LoadException 所在 IP)
  → exception_handlers[0].try_start < exception_handlers[0].try_end

For { test: Some(..) }
  → 生成 [init] Jump(test) [body] [update] [test] JumpIfTrue(loop)

ForIn
  → 生成 [obj] ForIn ForInNext(exit) [bind] [body] Jump(loop) exit
  → break 路径插入 ForInDispose

catch 块首指令为 LoadException
```

### C 组（VM / Runtime / Builtins）

```
手工 Chunk：throw 42 → catch 块 LoadException 压栈 → 得到 42
手工 Chunk：finally 在正常退出时执行
手工 Chunk：finally 在 throw 时执行，异常继续传播到外层
手工 Chunk：ForIn 枚举 {a:1, b:2, c:3} 依次压入 "a", "b", "c"
手工 Chunk：ForIn 含原型链，去重正确
new TypeError("msg").message === "msg"
new TypeError("msg").name === "TypeError"
TypeError.prototype instanceof Error → false（TypeError.prototype 是对象不是 Error 实例）
new TypeError() instanceof Error → true
VmError::type_error → create_type_error_object → 可被 catch 捕获
```

### D 组（集成）

```
--native-v1 → 零回退
--native-v2 → 零回退
--native-v3 → 零回退
--native-v4 → 零回退
--native-v5 → NATIVE_V5_TESTS 固定清单全部通过
test/language/statements/try default + strict 各自分开统计
test/language/statements/for default + strict
test/language/statements/for-in default + strict
skipped 不计 passed，报告中分列
```

---

## 10. 共享文件与合并规则

V5 新增或修改的共享文件（须先单独 PR，不与实现混合）：

```
src/ast/statement.rs          TryCatch / For / ForIn / CatchClause / ForInit / ForInBinding
src/bytecode/opcode.rs        ForIn / ForInNext / ForInDispose / LoadException
src/bytecode/chunk.rs         exception_handlers 字段 / ExceptionHandler 类型
src/runtime/value.rs          ForInIterator(ForInIteratorId) 变体
src/runtime/context.rs        迭代器存储 / 异常槽 / Error 工厂方法接口
src/contracts.rs              ForInIteratorId / ExceptionHandler 公开导出
```

合并规则：

- 共享契约 PR 先合并，不含任何实现逻辑；
- A 组只修改 `lexer/`、`ast/`、`parser/`；
- B 组只修改 `bytecode/`；
- C 组只修改 `vm/`、`runtime/`、`builtins/`；
- D 组只修改 `src/test262.rs` 和 `tests/`；
- `Chunk.exception_handlers` 默认为空 Vec，V1–V4 Chunk 向前兼容，不破坏现有测试；
- Error 对象创建逻辑不得放在 `backend/native.rs`；
- 任何 Native 不支持的语法不得调用 Boa 兜底。
