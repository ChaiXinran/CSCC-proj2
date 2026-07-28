# AgentJS JetStream 三线并行共享接口规范

> 仓库：`ChaiXinran/CSCC-proj2`  
> 基线提交：`69dbabeb181e013e4ac1f7d109b537cdc6bba041`  
> 适用工作：Class / `super`、调用栈、`Function` 构造器  
> 状态：**并行开发前冻结草案（Interface Freeze v1）**  
> 原则：先合并“无语义变化”的接口层，再由三条分支并行实现功能。

---

## 1. 目的

本规范用于解决三条开发线共享以下内部链路时的冲突：

```text
源码 / 动态源码
    ↓
Lexer → Parser → AST → Compiler → FunctionTemplate
    ↓
函数实例化
    ↓
Call / Construct / super()
    ↓
CallFrame + Environment + this + new.target
    ↓
VM 执行与 Completion
```

三条开发线分别负责：

| 开发线 | 主要目标 |
|---|---|
| A：Class / `super` | 派生类构造、`super()`、`super.method()`、`this` 初始化、`new.target` 透传 |
| B：调用栈 | 调用深度、CallFrame 生命周期、深递归稳定性、调用/构造统一调度 |
| C：`Function` 构造器 | 动态源码参数与函数体、全局环境、函数对象元数据、构造行为 |

三条线可以并行，但必须共享同一套“调用、构造、函数实例化”接口，禁止各自增加旁路。

---

## 2. 当前代码事实

### 2.1 函数运行时表示

当前 `JsFunction` 已包含：

- `chunk` 与参数信息；
- `environment`；
- `is_arrow`、`is_async`、`is_generator`；
- `is_derived_constructor`；
- `is_constructable`；
- `lexical_this`、`lexical_new_target`；
- `home_object`；
- 函数 `prototype` 属性策略。

因此，不应再为 Class 或动态 `Function` 新建第二种函数对象。

### 2.2 调用帧

当前 `CallFrame` 已包含：

```rust
pub struct CallFrame {
    pub function: Option<FunctionId>,
    pub return_ip: usize,
    pub environment: EnvironmentId,
    pub this_value: JsValue,
    pub new_target: JsValue,
    pub stack_base: usize,
}
```

三条开发线不得再单独维护另一套 `this`、`new.target` 或栈基址状态。

### 2.3 VM 调用现状

当前解释器内部已经存在：

- `call_value(...)`；
- `construct_value(...)`；
- `construct_value_with_new_target(...)`；
- `call_super_constructor(...)`；
- `call_value_from_builtin(...)`。

普通调用、Proxy、用户函数、内建函数、bound function、`call`、`apply` 已汇入 `call_value`。`super()` 会保留当前派生构造器的 `new.target`。

问题在于：

1. 这些接口大多是 `interpreter.rs` 私有实现，其他模块容易增加旁路；
2. 动态 `Function` 当前直接构造 `JsFunction`，与 VM 的 `create_function` 重复；
3. JavaScript 抛出值与 VM 内部错误仍依赖私有 `OperationResult` 和 `pending_exception` 转换；
4. 调用深度、CallFrame、环境、Realm、派生 `this` 的清理责任分散。

---

## 3. 总体设计决策

### 决策 D1：只有 VM Invocation Layer 能执行调用和构造

禁止以下模块直接执行函数体：

- Class 编译/运行代码；
- `builtins/function.rs`；
- Array、Promise、Iterator 等 builtin；
- 测试适配器。

这些模块只能调用本规范定义的 VM 接口。

### 决策 D2：`new.target` 在构造请求中始终显式传递

构造接口不得在内部猜测 `new.target`：

- 普通 `new C()`：`new_target = C`；
- `super()`：`new_target = 当前 CallFrame.new_target`；
- Reflect/Proxy/Bound construct：按各自语义传递或替换。

### 决策 D3：JavaScript Throw 与引擎错误严格分离

```text
InvocationOutcome::Throw(value)
```

表示可被 JavaScript `try/catch` 捕获的异常。

```text
Err(VmError)
```

只表示：

- RuntimeLimit；
- VM 状态损坏；
- 无效 bytecode；
- 堆/环境/函数 arena 耗尽；
- 不可恢复的内部错误。

`TypeError`、`ReferenceError`、`SyntaxError`、`RangeError` 等 ECMAScript 异常进入调用层后，应转换为 `InvocationOutcome::Throw`。

### 决策 D4：`JsFunction` 只能通过统一函数实例化入口创建

完成接口迁移后，除函数实例化模块外，仓库中不得出现新的：

```rust
JsFunction { ... }
```

Class、普通函数和动态 `Function` 必须共享：

- 函数对象 `name` / `length` / `prototype` 创建；
- strict function 标记；
- legacy `caller` / `arguments` 处理；
- Realm 归属；
- constructability；
- generator prototype 后处理。

---

## 4. 新增模块与稳定类型

建议新增：

```text
src/vm/invocation.rs
```

并在 `src/vm/mod.rs` 中：

```rust
mod invocation;

pub(crate) use invocation::{
    CallRequest,
    ConstructRequest,
    FunctionEnvironmentMode,
    FunctionInstantiationRequest,
    InvocationOutcome,
};
```

### 4.1 `InvocationOutcome`

```rust
use crate::runtime::JsValue;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InvocationOutcome {
    Value(JsValue),
    Throw(JsValue),
}

impl InvocationOutcome {
    pub(crate) fn into_value(self) -> Result<JsValue, JsValue> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Throw(value) => Err(value),
        }
    }
}
```

约束：

- 不允许用 `JsValue::Error` 是否存在来判断成功或失败；
- JavaScript 可以正常返回一个 Error 对象；
- `Throw` 表示 completion 类型，不表示值类型。

### 4.2 `CallRequest`

```rust
#[derive(Debug, Clone)]
pub(crate) struct CallRequest {
    pub callee: JsValue,
    pub this_value: JsValue,
    pub arguments: Vec<JsValue>,
}

impl CallRequest {
    pub(crate) fn new(
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self {
            callee,
            this_value,
            arguments,
        }
    }
}
```

调用方职责：

- 普通 `f()` 传入 `JsValue::Undefined`；
- `obj.f()` 传入 `obj`；
- `super.f()` 传入当前 frame 的 `this`；
- bound function 的 `this` 与参数拼接由 Invocation Layer 处理，而非调用方处理。

### 4.3 `ConstructRequest`

```rust
#[derive(Debug, Clone)]
pub(crate) struct ConstructRequest {
    pub constructor: JsValue,
    pub arguments: Vec<JsValue>,
    pub new_target: JsValue,
}

impl ConstructRequest {
    pub(crate) fn ordinary(
        constructor: JsValue,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self {
            new_target: constructor.clone(),
            constructor,
            arguments,
        }
    }

    pub(crate) fn with_new_target(
        constructor: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
    ) -> Self {
        Self {
            constructor,
            arguments,
            new_target,
        }
    }
}
```

禁止提供 `Option<JsValue>` 形式的 `new_target`。普通调用不进入 Construct API，构造调用始终有明确的 `new_target`。

---

## 5. VM 稳定调用接口

### 5.1 VM 内部语义接口

由 B 线实现并拥有：

```rust
impl Vm {
    pub(crate) fn invoke_call(
        &mut self,
        request: CallRequest,
        context: &mut NativeContext,
    ) -> Result<InvocationOutcome, VmError>;

    pub(crate) fn invoke_construct(
        &mut self,
        request: ConstructRequest,
        context: &mut NativeContext,
    ) -> Result<InvocationOutcome, VmError>;
}
```

第一版只做无语义变化迁移：

```text
call_value                        → invoke_call
construct_value_with_new_target   → invoke_construct
OperationResult                   → InvocationOutcome
```

旧私有函数可暂时保留为内部 helper，但新代码不得直接调用。

### 5.2 Builtin 桥接接口

当前 `NativeCall` / `NativeConstruct` 返回 `Result<JsValue, VmError>`，无法直接返回 JavaScript Throw completion。因此保留统一桥接：

```rust
impl Vm {
    pub(crate) fn invoke_call_from_builtin(
        &mut self,
        request: CallRequest,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError>;

    pub(crate) fn invoke_construct_from_builtin(
        &mut self,
        request: ConstructRequest,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError>;
}
```

转换规则固定为：

```rust
match self.invoke_call(request, context)? {
    InvocationOutcome::Value(value) => Ok(value),
    InvocationOutcome::Throw(value) => {
        self.set_pending_exception(value);
        Err(VmError::runtime("JavaScript callback threw"))
    }
}
```

要求：

1. `pending_exception` 只能由桥接层设置；
2. builtin 不得自行构造 `"JavaScript callback threw"`；
3. 外层 Invocation Layer 发现 `pending_exception` 后恢复为 `InvocationOutcome::Throw`；
4. 桥接层必须有测试验证嵌套 callback 的原始 throw 值不会丢失。

### 5.3 Bytecode 对接

| Bytecode | 调用方式 |
|---|---|
| `Call(n)` | `CallRequest { this_value: Undefined }` |
| `CallWithThis(n)` | 显式 `this_value` |
| `SpreadCall(n)` | 展开后进入 `invoke_call` |
| `Construct(n)` | `ConstructRequest::ordinary(...)` |
| `SpreadConstruct(n)` | 展开后进入 `invoke_construct` |
| `SuperCall(n)` | `ConstructRequest::with_new_target(super_ctor, args, current_new_target)` |
| `SuperSpreadCall(n)` | 同上 |
| `SuperForwardCall` | 内部 rest 参数直接转发，同上 |

Class 线不得新增第二套 `SuperConstruct` 执行函数。

---

## 6. 统一函数实例化接口

### 6.1 环境选择

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionEnvironmentMode {
    /// 按 FunctionTemplate.environment_policy 处理。
    FollowTemplate,
    /// 动态 Function：固定绑定当前 Realm 的 global environment。
    Global,
}
```

### 6.2 请求结构

```rust
use crate::{
    bytecode::FunctionTemplate,
    runtime::ObjectId,
};

#[derive(Debug, Clone)]
pub(crate) struct FunctionInstantiationRequest {
    pub template: FunctionTemplate,
    pub environment_mode: FunctionEnvironmentMode,

    /// 动态 Function 使用 "anonymous"；None 表示沿用 template。
    pub name_override: Option<String>,

    /// Function 子类构造时可覆盖函数对象的 [[Prototype]]。
    /// 普通函数和 JetStream 第一阶段传 None。
    pub function_object_prototype: Option<ObjectId>,
}
```

### 6.3 稳定入口

```rust
impl Vm {
    pub(crate) fn instantiate_function(
        &mut self,
        request: FunctionInstantiationRequest,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError>;
}
```

该函数是唯一允许构造 `JsFunction` 的入口，并负责：

1. 根据 `FunctionTemplate` 复制参数、chunk 和函数标志；
2. 根据环境模式选择 closure environment；
3. arrow function 捕获 `this` 与 `new.target`；
4. arrow function继承外层 `home_object`；
5. 调用 `NativeContext::allocate_function`；
6. strict function 标记及 legacy 属性清理；
7. generator/async generator prototype 后处理；
8. 可选修改函数对象的 `[[Prototype]]`；
9. 返回 `JsValue::Function(FunctionId)`。

### 6.4 普通函数使用方式

`Instruction::CreateFunction`：

```rust
let template = chunk.functions[index].clone();

let value = self.instantiate_function(
    FunctionInstantiationRequest {
        template,
        environment_mode: FunctionEnvironmentMode::FollowTemplate,
        name_override: None,
        function_object_prototype: None,
    },
    context,
)?;
```

### 6.5 动态 `Function` 使用方式

`builtins/function.rs` 只负责：

1. 参数依次执行 `ToString`；
2. 构造动态函数源码；
3. Lexer / Parser / Compiler；
4. 取出 `FunctionTemplate`；
5. 调用 `instantiate_function`。

禁止继续直接写 `JsFunction { ... }`。

```rust
let value = vm.instantiate_function(
    FunctionInstantiationRequest {
        template,
        environment_mode: FunctionEnvironmentMode::Global,
        name_override: Some("anonymous".into()),
        function_object_prototype,
    },
    context,
)?;
```

### 6.6 `Function` 的 `new_target`

`function_construct` 不得丢弃 `new_target`。

最低要求：

- `Function(...)`：函数对象的 `[[Prototype]]` 使用当前 Realm 的 `%Function.prototype%`；
- `new Function(...)`：同上；
- `Reflect.construct(Function, args, NewTarget)` 或 Function 子类：通过 `new_target.prototype` 解析函数对象的 `[[Prototype]]`，无效时回退 `%Function.prototype%`。

JetStream 第一阶段可先保证普通 `Function` 和 `new Function`，但接口必须保留 `new_target`，避免之后再次改签名。

---

## 7. CallFrame 与调用深度契约

### 7.1 单一所有权

只有 B 线的 Invocation Layer 可以：

- `push_call_frame`；
- `pop_call_frame`；
- `consume_call_depth`；
- `release_call_depth`；
- 恢复 caller operand stack；
- 恢复 environment state；
- 进入/退出 Realm。

A、C 线不得直接调用这些方法。

### 7.2 深度计数不变量

一次逻辑调用层级必须满足：

```text
进入前 depth = N
成功压帧后 depth = N + 1
任何 Value / Throw / Err 路径退出后 depth = N
```

需要覆盖：

- 用户函数正常返回；
- 用户函数抛异常；
- builtin 正常返回；
- builtin 抛 ECMAScript 异常；
- bound function 转发；
- Proxy call/construct；
- `call` / `apply` 转发；
- `super()`；
- 构造器显式返回对象；
- RuntimeLimit；
- Realm 切换失败；
- 参数绑定失败。

### 7.3 调用栈与 Rust 栈

短期允许解释器递归调用 Rust 函数，但 B 线必须保证：

- 所有 JS 调用都先经过 `ExecutionBudget::check_call_depth`；
- JetStream 配置和默认配置使用同一套预算；
- 不通过无限提高 OS 线程栈掩盖错误；
- 若 `crypto-simple` 在 8192 深度配置下仍触发 Rust stack overflow，则进入显式 VM frame 重构。

显式 frame 重构不得改变 A/C 使用的 `invoke_call` / `invoke_construct` 签名。

---

## 8. Class / `super` 专用契约

### 8.1 `super()` 调用

A 线只提交：

```rust
ConstructRequest::with_new_target(
    super_constructor,
    arguments,
    context.current_new_target(),
)
```

实际构造、frame、Realm、Proxy、bound construct 由 B 线处理。

### 8.2 派生 `this` 状态

当前 `NativeContext` 使用派生 `this` 状态栈。冻结以下语义：

1. 进入派生构造器时，`this` 为未初始化状态；
2. `this`、`super.prop` 需要 receiver 时，在初始化前抛 `ReferenceError`；
3. 第一次成功 `super()` 用返回对象初始化当前 frame 的 `this_value`；
4. 第二次 `super()` 抛 `ReferenceError`；
5. `super()` 抛异常时不得标记已初始化；
6. 派生构造器隐式返回前未初始化 `this`：`ReferenceError`；
7. 显式返回对象：允许，即使未调用 `super()`；
8. 显式返回 `undefined`：按隐式返回处理；
9. 显式返回其他 primitive：`TypeError`；
10. 离开构造器时派生状态栈必须恢复。

A 线可以修改 class lowering 和这些语义的判定，但不得直接修改调用深度、frame 生命周期或 Realm 恢复。

### 8.3 `super.method()`

必须通过普通调用接口：

```rust
CallRequest::new(
    resolved_super_method,
    context.current_this(),
    arguments,
)
```

`GetSuperMethod` / `GetSuperElementMethod` 负责解析方法和 receiver；执行仍由 `invoke_call` 完成。

---

## 9. 文件所有权

### 9.1 B 线独占或主审

```text
src/vm/invocation.rs
src/vm/interpreter.rs
src/vm/frame.rs
src/vm/mod.rs
src/runtime/context.rs
src/runtime/function.rs
src/engine.rs                    # 仅调用/栈预算配置部分
tests/jetstream_call_stack.rs
```

规则：

- A/C 需要 VM 新能力时，先提交接口需求，不直接修改 B 独占文件；
- `interpreter.rs` 的 class opcode 小改可由 A 提交，但必须由 B review；
- C 不应直接修改 `interpreter.rs`。

### 9.2 A 线主责

```text
src/ast/expression.rs            # class/super 节点
src/parser/expression.rs         # class/super 语法
src/parser/statement.rs          # class declaration/early errors
src/bytecode/compiler.rs         # 仅 class/super lowering
tests/native_classes.rs
tests/jetstream_class_super.rs
```

### 9.3 C 线主责

```text
src/builtins/function.rs
tests/native_function_constructor.rs
tests/jetstream_function_constructor.rs
```

### 9.4 共同冻结文件

```text
src/bytecode/opcode.rs
src/bytecode/chunk.rs
src/contracts.rs
```

修改这些文件必须：

1. 三人确认；
2. 单独一个 interface commit；
3. 不与功能修复混在同一 commit；
4. 所有分支立即 rebase。

---

## 10. 禁止事项

### A 线禁止

- 在 class 代码中直接执行 `JsFunction.chunk`；
- 手工 push/pop CallFrame；
- 手工增减 call depth；
- 为 `super()` 创建单独的函数调用实现；
- 直接构造 `JsFunction`。

### B 线禁止

- 为解决栈问题修改 class parser/early error；
- 改变动态 `Function` 的源码拼接语义；
- 仅通过无限提高递归限制宣称完成；
- 把 `InvocationOutcome::Throw` 改成普通 `VmError::runtime` 丢失原始值。

### C 线禁止

- 直接调用 `vm.eval_execute` 执行新生成函数体；
- 直接构造 `JsFunction`；
- 捕获调用者局部 environment；
- 绕过 `invoke_construct` 实现 `new Function`；
- 修改 CallFrame 字段或调用深度算法。

---

## 11. 测试门禁

## 11.1 接口层提交门禁

接口层必须是“无语义变化”提交，并通过：

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

附加检查：

```bash
rg "JsFunction \{" src
```

迁移完成后，只允许统一实例化实现中出现一次。

### 11.2 A：Class / `super`

最低单元测试：

```javascript
class B { constructor(x) { this.x = x; } }
class D extends B { constructor(x) { super(x); this.y = x + 1; } }
new D(2).x + new D(2).y
```

还必须覆盖：

- 默认派生构造器参数转发；
- `super(...args)`；
- `new.target` 保留；
- `this` before `super()`；
- 重复 `super()`；
- super constructor throw；
- 返回 object / undefined / primitive；
- `super.method()`；
- `super[computed]()`；
- static `super.method()`；
- getter/setter；
- Proxy superclass；
- bound superclass。

聚焦 Test262：

```text
test/language/statements/class
test/language/expressions/class
```

JetStream：

```text
ai-astar
richards
stanford-crypto-sha256
```

### 11.3 B：调用栈

最低测试：

- 100、1000、5000 层递归；
- 递归中 throw/catch；
- 达到限制后的 RuntimeLimit 类型；
- 失败后再次执行普通脚本；
- 深层 builtin → JS callback → builtin → JS callback；
- bound function 链；
- Proxy call/construct；
- 构造器递归；
- `super()` 嵌套构造；
- call depth 与 `call_frames.len()` 一致；
- operand stack、environment、Realm 均恢复。

JetStream：

```text
crypto-simple
crypto
richards-simple
```

### 11.4 C：`Function` 构造器

最低测试：

```javascript
Function("a", "b", "return a + b")(1, 2)
new Function("a", "return a * 2")(3)
new Function("return this")() === globalThis
```

还必须覆盖：

- 0 个参数；
- 多参数字符串拼接顺序；
- 参数 `ToString` 的 getter/side effect 顺序；
- 参数语法错误；
- body 语法错误；
- `"use strict"`；
- 不捕获调用者局部变量；
- 可作为构造器；
- `name === "anonymous"`；
- `length`；
- own `prototype`；
- `call` / `apply` / `bind`；
- `new.target`；
- Function 子类/Reflect.construct（可作为第二阶段）。

聚焦 Test262：

```text
test/built-ins/Function
test/language/expressions/function
```

JetStream：

```text
ai-astar
richards
stanford-crypto-sha256
```

### 11.5 共同 JetStream 门禁

每条分支至少运行 2 次迭代的快速门禁：

```powershell
scripts/run-jetstream2.ps1 `
  -Tests ai-astar,crypto,richards,stanford-crypto-sha256 `
  -Iterations 2
```

合并候选再运行官方或较高迭代次数。

---

## 12. 分支与合并顺序

### Phase 0：接口冻结分支

```text
feature/jetstream-invocation-interface
```

只完成：

1. `InvocationOutcome`；
2. `CallRequest` / `ConstructRequest`；
3. `invoke_call` / `invoke_construct` wrapper；
4. builtin bridge；
5. `instantiate_function`；
6. 将动态 Function 和 `CreateFunction` 迁移到统一实例化入口；
7. 无语义变化测试。

该分支先合入 `main`。

### Phase 1：三线并行

```text
feature/jetstream-class-super
feature/jetstream-call-stack
feature/jetstream-function-constructor
```

三条分支都从 Phase 0 合并后的同一提交创建。

### Phase 2：推荐合并顺序

1. `Function` 构造器；
2. Class / `super`；
3. 调用栈小修；
4. 若调用栈需要显式 frame 重构，最后单独合入。

原因：

- C 的改动最集中；
- A 依赖构造接口但不应改其底层；
- B 若重写调度循环，冲突面最大，放在功能语义稳定后更安全。

---

## 13. Commit 规范

接口提交：

```text
refactor(vm): add stable invocation request and outcome types
refactor(runtime): centralize FunctionTemplate instantiation
```

A：

```text
test(class): add JetStream derived-super regressions
fix(class): complete super call and derived-this semantics
```

B：

```text
test(vm): diagnose deep recursive call limits
fix(vm): restore call state on every invocation exit
refactor(vm): execute JS calls with explicit frame stack
```

C：

```text
test(function): add dynamic Function constructor regressions
fix(function): use global function instantiation path
```

每个 PR 描述必须包含：

- 修改的接口；
- 修改的所有共享文件；
- 新增测试；
- Test262 聚焦结果；
- JetStream runner 结果；
- 是否改变 call depth / frame / environment / `new.target` 语义。

---

## 14. 接口验收清单

合并 Phase 0 前逐项确认：

- [ ] `CallRequest` 是所有普通调用的统一入口；
- [ ] `ConstructRequest` 始终显式携带 `new_target`；
- [ ] `super()` 不再有旁路构造实现；
- [ ] JavaScript Throw 与 VM Error 已分离；
- [ ] builtin callback 统一经过桥接层；
- [ ] 动态 `Function` 不再直接构造 `JsFunction`；
- [ ] 普通函数与动态函数共享实例化逻辑；
- [ ] A/C 不直接 push/pop CallFrame；
- [ ] 调用深度在所有退出路径恢复；
- [ ] 派生 `this` 在所有退出路径恢复；
- [ ] Realm 和 environment 在所有退出路径恢复；
- [ ] `cargo test`、Clippy、聚焦 Test262 全部通过；
- [ ] 四个 JetStream 快速 runner 可重复执行并保存日志。

---

## 15. 最终边界图

```text
A: Class / super
  Parser + Compiler
        │
        ├── CallRequest ───────────────┐
        └── ConstructRequest           │
                                       ▼
B: VM Invocation Layer
  invoke_call / invoke_construct
  CallFrame / depth / Realm / env
        │                              ▲
        │                              │
        └── FunctionInstantiation ─────┤
                                       │
C: Function constructor                │
  ToString + parse + compile           │
        └── FunctionInstantiation ─────┘
```

该边界的核心是：

> A 决定“class/super 应该调用谁、传什么”；  
> C 决定“动态 Function 应该编译成什么”；  
> B 决定“调用和构造如何安全执行”。  

任何一条线都不同时拥有以上三项权力。
