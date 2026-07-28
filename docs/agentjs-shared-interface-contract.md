# AgentJS 并行开发共享接口契约

> 文件建议位置：`docs/shared-interface-contract.md`  
> 适用范围：Parser/AST/Bytecode、VM/Runtime、Builtins/Temporal 三条并行开发线。  
> 本文档描述稳定边界；实现细节由各模块负责人维护。

# 1. 总原则

AgentJS 当前已经存在三阶段边界：

```text
source → parser/AST → bytecode Chunk → VM/Runtime → JsValue
```

共享接口必须遵守：

1. Parser 不依赖 Runtime。
2. Bytecode 不依赖 Lexer、Parser、VM、Runtime。
3. Builtins 不绕过 Runtime Semantic Kernel。
4. 可调用 JavaScript 的操作必须走 VM-mediated path。
5. pure algorithm 与 observable algorithm 分离。
6. 跨团队只传稳定数据结构，不传内部 heap 引用。
7. `ObjectId`、`EnvironmentId` 只能在同一个 NativeContext/isolate 中使用。

---

# 2. 文件所有权

| 接口/模块 | 负责人 |
|---|---|
| `src/lexer/`, `src/parser/`, `src/ast/`, `src/bytecode/compiler.rs` | A |
| `src/runtime/`, `src/vm/`, `src/backend/` | B |
| `src/builtins/object.rs`, `array.rs`, `function.rs`, `promise.rs`, `proxy.rs` | B |
| `src/builtins/date_intl/` | C |
| `src/contracts.rs` | B 维护，A/C review |
| `src/bytecode/opcode.rs`, `chunk.rs` | interface PR，A/B 共同 review |
| `src/builtins/mod.rs`, `src/runtime/mod.rs`, `src/lib.rs` | interface PR |

任何普通功能 PR 不得修改不属于自己的独占文件。

---

# 3. Frontend 接口

当前仅有“脚本源码 → Program”不足以支持 Module 和 Eval 的上下文语义。建议增加以下稳定类型。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseGoal {
    Script,
    Module,
    DirectEval,
    IndirectEval,
    FunctionBody,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ParseContext {
    pub strict: bool,
    pub allow_return: bool,
    pub allow_await: bool,
    pub allow_yield: bool,
    pub allow_new_target: bool,
    pub allow_super_property: bool,
    pub allow_super_call: bool,
    pub in_class_field: bool,
    pub in_static_block: bool,
}

pub struct ParseRequest<'a> {
    pub source: &'a str,
    pub goal: ParseGoal,
    pub context: ParseContext,
}

pub trait SourceParser {
    fn parse_source(
        &mut self,
        request: ParseRequest<'_>,
    ) -> Result<Program, NativeError>;
}
```

## 3.1 约束

- strictness 和 grammar goal 必须通过 request 传入。
- B 不得在 `eval_call` 中直接 import `Lexer` 和 `Parser`。
- Module、Eval 和 Function constructor 必须使用同一 Frontend 接口。
- parse-phase negative 必须在 Frontend 返回 `NativeError::Parse`。
- Runtime 不得事后模拟本应由 Parser 报出的 SyntaxError。

---

# 4. Compiler 接口

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileGoal {
    Script,
    Module,
    DirectEval,
    IndirectEval,
    FunctionBody,
}

pub struct CompileRequest<'a> {
    pub program: &'a Program,
    pub goal: CompileGoal,
    pub strict: bool,
}

pub trait ProgramCompiler {
    fn compile(
        &mut self,
        request: CompileRequest<'_>,
    ) -> Result<Chunk, NativeError>;
}
```

## 4.1 Bytecode 边界

A 只能输出：

```text
Chunk
Instruction
FunctionTemplate
ExceptionHandler
stable metadata
```

不得把以下类型写入 Chunk：

```text
ObjectId
EnvironmentId
PromiseId
ModuleId
NativeContext reference
VM callback
```

这些身份只能在执行时由 B 创建。

---

# 5. 通用参数列表接口

现有 spread opcode 只覆盖单个末尾 spread。接口统一为“先构造参数列表，再调用”。

建议新增：

```rust
pub enum Instruction {
    // existing ...

    /// Stack: [callee, arguments_array] -> [result]
    CallArgumentList,

    /// Stack: [callee, this_value, arguments_array] -> [result]
    CallWithThisArgumentList,

    /// Stack: [constructor, arguments_array] -> [result]
    ConstructArgumentList,
}
```

A 使用现有：

```text
ArrayCreateSparse
ArrayPush
SpreadIntoArray
```

构造 arguments array。

B 实现：

```rust
pub fn call_with_argument_list(
    &mut self,
    callee: JsValue,
    this_value: JsValue,
    arguments: JsValue,
    context: &mut NativeContext,
) -> Result<JsValue, VmError>;

pub fn construct_with_argument_list(
    &mut self,
    constructor: JsValue,
    arguments: JsValue,
    new_target: JsValue,
    context: &mut NativeContext,
) -> Result<JsValue, VmError>;
```

## 5.1 语义要求

- argument expression 从左到右求值；
- spread iterator 从左到右消费；
- abrupt completion 必须执行 IteratorClose；
- method call 保留原始 receiver；
- constructor 保留 newTarget；
- Proxy call/construct trap 不得绕过；
- 不允许把 arguments array 提前复制成不可观察快照。

---

# 6. Runtime Semantic Kernel

以下接口是 builtins 和 VM 的唯一可观察操作入口：

```rust
pub fn get(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<JsValue, VmError>;

pub fn set(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
    value: JsValue,
    throw: bool,
) -> Result<bool, VmError>;

pub fn has_property(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<bool, VmError>;

pub fn get_method(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<Option<JsValue>, VmError>;

pub fn call(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    callee: JsValue,
    this_value: JsValue,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError>;

pub fn construct(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    constructor: JsValue,
    arguments: Vec<JsValue>,
    new_target: JsValue,
) -> Result<JsValue, VmError>;

pub fn to_primitive(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    value: JsValue,
    preferred: PreferredType,
) -> Result<JsValue, VmError>;

pub fn to_string(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError>;

pub fn to_property_key(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyKey, VmError>;
```

## 6.1 禁止行为

可观察属性上禁止直接使用：

```rust
ctx.get_property(...)
ctx.set_property(...)
ctx.has_property(...)
ctx.get_own_property_descriptor(...)
```

允许直接读取的情况仅限：

1. engine-owned internal slot；
2. `__agentjs_*` 隐藏字段；
3. 明确要求 own data property 且规范不允许执行 getter；
4. GC/heap 内部维护。

所有例外必须在代码旁注释规范原因。

## 6.2 IsCallable / IsConstructor

禁止只通过 `JsValue` enum variant 推断。

```rust
pub fn is_callable(ctx: &NativeContext, value: &JsValue) -> bool;
pub fn is_constructor(ctx: &NativeContext, value: &JsValue) -> bool;
```

必须覆盖：

- interpreted function；
- builtin function；
- arrow/async/generator 非 constructable；
- bound function；
- callable/constructable Proxy。

---

# 7. Property Descriptor 接口

```rust
pub struct PropertyDescriptorUpdate {
    pub value: Option<JsValue>,
    pub writable: Option<bool>,
    pub get: Option<Option<JsValue>>,
    pub set: Option<Option<JsValue>>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

pub fn to_property_descriptor(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyDescriptorUpdate, VmError>;

pub fn from_property_descriptor(
    ctx: &mut NativeContext,
    descriptor: Option<PropertyDescriptor>,
) -> Result<JsValue, VmError>;

pub fn validate_and_apply_property_descriptor(
    ctx: &mut NativeContext,
    object: Option<ObjectId>,
    key: PropertyKey,
    extensible: bool,
    descriptor: &PropertyDescriptorUpdate,
    current: Option<&PropertyDescriptor>,
) -> Result<bool, VmError>;
```

要求：

- `None` 表示字段缺失；
- `Some(false)` 与缺失不能混淆；
- `get: Some(None)` 表示显式 undefined；
- getter/setter callable 校验走 Runtime callable protocol；
- 读取 descriptor object 字段的顺序必须可观察；
- abrupt completion 立即传播。

---

# 8. Eval 接口

把 eval 从 Function builtin 中分离。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalKind {
    Direct,
    Indirect,
}

pub struct EvalRequest<'a> {
    pub source: &'a str,
    pub kind: EvalKind,
    pub strict_caller: bool,
    pub lexical_environment: EnvironmentId,
    pub variable_environment: EnvironmentId,
}

pub trait EvalExecutor {
    fn execute_eval(
        &mut self,
        request: EvalRequest<'_>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError>;
}
```

## 8.1 声明实例化

A 负责从 AST/Compiler 发出声明信息，B 负责运行期环境操作。

禁止：

- B 在 Runtime 中重新递归扫描 AST；
- A 直接操作 Environment；
- eval 通过字符串前加 `"use strict"` 模拟所有上下文语义。

建议增加稳定计划：

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationPlan {
    pub var_names: Vec<String>,
    pub lexical_names: Vec<String>,
    pub function_names: Vec<String>,
    pub annex_b_function_names: Vec<String>,
    pub strict: bool,
}
```

计划可作为 `Chunk` metadata 或独立编译结果的一部分，但不能包含 heap ID。

---

# 9. Module 与 JobQueue 接口

## 9.1 Host loader

```rust
pub trait ModuleLoader {
    fn resolve(
        &mut self,
        referrer: Option<&Path>,
        specifier: &str,
        attributes: &[(String, JsValue)],
    ) -> Result<PathBuf, NativeError>;

    fn load(&mut self, path: &Path) -> Result<String, NativeError>;
}
```

## 9.2 Dynamic import job

```rust
pub struct DynamicImportJob {
    pub promise: PromiseId,
    pub request: DynamicImportRequest,
}

pub enum Job {
    PromiseReaction(PromiseJob),
    PromiseCallback(PromiseCallbackJob),
    PromiseResolveThenable(ResolveThenableJob),
    DynamicImport(DynamicImportJob),
    ModuleEvaluation(ModuleId),
    HostCallback(NativeJob),
}
```

## 9.3 Queue 执行权

只有 VM/Runtime 可以 drain JobQueue：

```rust
pub fn enqueue_job(&mut self, job: Job);

pub fn run_one_job(
    &mut self,
    context: &mut NativeContext,
) -> Result<bool, VmError>;

pub fn drain_jobs(
    &mut self,
    context: &mut NativeContext,
) -> Result<(), VmError>;
```

Builtin 只能 enqueue，不得自行循环 drain。

## 9.4 Module 状态机

所有状态变化必须通过：

```rust
ModuleRegistry::transition_to(...)
```

禁止直接 `set_status` 跳过中间状态。

有效主路径：

```text
Unlinked
→ Linking
→ Linked
→ Evaluating
→ Evaluated
```

任意阶段可进入 `Failed`，并保存 rejection reason。

dynamic import 必须返回 pending Promise，不能同步完成后伪装成异步。

---

# 10. Temporal/Intl 接口

## 10.1 纯记录

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub microsecond: u16,
    pub nanosecond: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoDateTime {
    pub date: IsoDate,
    pub time: IsoTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Constrain,
    Reject,
}
```

这些类型：

- 不包含 `JsValue`；
- 不包含 `ObjectId`；
- 不访问 VM/Context；
- 可以单元测试。

## 10.2 Observable field preparation

```rust
pub fn prepare_temporal_fields(
    vm: &mut Vm,
    ctx: &mut NativeContext,
    input: JsValue,
    fields: &[TemporalField],
) -> Result<TemporalFields, VmError>;
```

要求：

- 严格按规范顺序调用 `abstract_ops::get`；
- getter 只调用一次；
- abrupt completion 立即传播；
- month/monthCode 走统一 ResolveISOMonth；
- PlainDate、PlainDateTime、PlainYearMonth、PlainMonthDay、ZonedDateTime 共用。

## 10.3 Intl bridge

Intl 接收 canonical Temporal record：

```rust
pub enum TemporalDisplayValue {
    Date(IsoDate),
    DateTime(IsoDateTime),
    YearMonth { year: i32, month: u8 },
    MonthDay { month: u8, day: u8 },
    ZonedDateTime {
        epoch_nanoseconds: i128,
        time_zone: String,
        calendar: String,
    },
}
```

Intl 不再自行读取 Temporal object 的用户可观察字段。

---

# 11. Error 与 Realm 契约

所有错误通过 VM/Context 创建：

```rust
pub fn type_error(
    ctx: &mut NativeContext,
    message: impl Into<String>,
) -> VmError;

pub fn range_error(...);
pub fn reference_error(...);
pub fn syntax_error(...);
```

要求：

- 错误 prototype 来自当前 Realm；
- 跨 Realm builtin 使用被调用函数所属 Realm；
- 不允许仅按错误名称字符串比较或替换构造器；
- abrupt completion 不被包装成无关 TypeError。

---

# 12. 接口变更流程

接口变更 PR 标题：

```text
interface: <contract name>
```

PR 必须包含：

1. 接口动机；
2. 当前失败簇；
3. 新类型/函数签名；
4. 所有调用方；
5. 向后兼容策略；
6. stub 或默认实现；
7. 单元测试；
8. stack effect（若新增 opcode）；
9. A/B/C 三方 review 结论。

接口冻结后，功能 PR 不得修改签名。

---

# 13. 最低测试要求

## Frontend/Bytecode

- AST snapshot；
- parse negative；
- opcode sequence；
- stack analysis；
- multiple spread；
- computed key single evaluation。

## Runtime/VM

- getter/setter exactly once；
- Proxy trap；
- abrupt completion；
- cross-Realm error；
- descriptor missing-vs-false；
- direct/indirect eval；
- JobQueue FIFO；
- module state transition。

## Temporal/Intl

- pure ISO round-trip；
- month/monthCode；
- field getter order；
- Duration balance/round；
- Temporal → Intl record bridge；
- overflow reject/constrain。

---

# 14. 完成定义

一个功能只有同时满足以下条件才算完成：

1. targeted Test262 通过数增加；
2. 高频错误签名减少；
3. 无新的跨目录回归；
4. 所有可观察操作走共享接口；
5. 没有复制另一模块的规范 helper；
6. 单元测试覆盖接口契约；
7. 文档更新；
8. 全量 Test262 diff 已记录。
