# Native V3 共享接口规格

本文档定义 V3 函数与复合值开发期间 A/B/C/D 四组共同遵守的类型、字节码和 Runtime 契约。它补充 `interface-spec.md` 和 `native-v2-interface.md`；发生冲突时，V3 新增部分以本文为准。

## 1. 共享 AST

建议先单独提交以下契约变更。

```rust
pub struct FunctionParam {
    pub name: String,
}

pub struct FunctionBody {
    pub statements: Vec<Statement>,
}

pub struct FunctionLiteral {
    pub name: Option<String>,
    pub params: Vec<FunctionParam>,
    pub body: FunctionBody,
}

pub enum Statement {
    // V1/V2 variants...
    FunctionDeclaration {
        name: String,
        params: Vec<FunctionParam>,
        body: FunctionBody,
    },
    Return(Option<Expression>),
}

pub enum Expression {
    // V1/V2 variants...
    Function(FunctionLiteral),
    Array(Vec<Expression>),
    Object(Vec<ObjectProperty>),
}

pub struct ObjectProperty {
    pub key: PropertyName,
    pub value: Expression,
}

pub enum PropertyName {
    Identifier(String),
    String(String),
    Number(String),
}
```

规则：

* `FunctionDeclaration.name` 不得为空；
* `FunctionParam.name` 暂时只支持普通标识符；
* V3 暂不支持默认参数、剩余参数和解构参数；
* 非 strict 模式下重复参数可以暂时允许；
* strict 模式下重复参数必须返回 ParseError 或明确 Unsupported；
* `return` 只能出现在函数体内部；
* `FunctionLiteral.name` 用于具名函数表达式，V3 可以先只支持 `None`；
* `ObjectProperty.key` 暂不支持计算属性名；
* 计算成员访问由已有 `Expression::Member { computed: true }` 表示；
* 成员赋值继续使用 `Expression::Assignment`，但 target 可以是 `Member`。

## 2. 共享函数常量

V3 引入函数常量表，建议放入 `Chunk` 或单独的 `FunctionTable`。

```rust
pub struct FunctionTemplate {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: Chunk,
    pub environment_policy: EnvironmentCapturePolicy,
}

pub enum EnvironmentCapturePolicy {
    None,
    CaptureCurrent,
}
```

规则：

* 函数体编译为独立 `Chunk`；
* 函数体 `Chunk` 的参数名由 `FunctionTemplate.params` 提供；
* 函数声明和函数表达式都引用 `FunctionTemplate`；
* V3.1 可以只支持 `EnvironmentCapturePolicy::None`；
* V3.2 必须支持 `CaptureCurrent`，用于基础闭包；
* 函数模板不得包含 Boa 类型或 Parser 状态。

## 3. 共享 Opcode

V3 新增指令：

```rust
pub enum Instruction {
    // V1/V2 instructions...

    CreateFunction(u16),
    DeclareFunction {
        name: u16,
        function: u16,
    },

    DeclareLocal(u16),
    LoadName(u16),
    StoreName(u16),

    LoadThis,

    ArrayCreate(u16),
    ObjectCreate(u16),

    GetElement,
    SetProperty(u16),
    SetElement,

    CallWithThis(u16),
}
```

栈效果：

| 指令                       | required |    pops | pushes |
| ------------------------ | -------: | ------: | -----: |
| `CreateFunction(f)`      |        0 |       0 |      1 |
| `DeclareFunction { .. }` |        0 |       0 |      0 |
| `DeclareLocal(name)`     |        1 |       1 |      0 |
| `LoadName(name)`         |        0 |       0 |      1 |
| `StoreName(name)`        |        1 |       0 |      0 |
| `LoadThis`               |        0 |       0 |      1 |
| `ArrayCreate(n)`         |      `n` |     `n` |      1 |
| `ObjectCreate(n)`        |     `2n` |    `2n` |      1 |
| `GetElement`             |        2 |       2 |      1 |
| `SetProperty(name)`      |        2 |       1 |      1 |
| `SetElement`             |        3 |       2 |      1 |
| `CallWithThis(n)`        |  `n + 2` | `n + 2` |      1 |

说明：

* `CreateFunction(f)` 的操作数是函数常量表索引；
* `DeclareFunction` 在当前环境中创建函数绑定；
* `DeclareLocal(name)` 弹出初始化值并在当前函数环境创建绑定；
* `LoadName` 沿环境链查找，失败返回 ReferenceError；
* `StoreName` 沿环境链写入已有绑定，保留栈顶赋值结果；
* `LoadThis` 读取当前调用帧的 `this`；
* `ArrayCreate(n)` 按源码顺序构造数组；
* `ObjectCreate(n)` 要求栈布局为 `[key0, value0, key1, value1, ...]`；
* `GetElement` 使用 `ToPropertyKey` 的 V3 最小规则；
* `SetProperty(name)` 栈布局为 `[object, value]`，执行后保留 `value`；
* `SetElement` 栈布局为 `[object, key, value]`，执行后保留 `value`；
* `CallWithThis(n)` 栈布局为 `[callee, this_value, arg0, ..., argN]`；
* 普通 `Call(n)` 栈布局继续保持 `[callee, arg0, ..., argN]`，其 `this` 为 `undefined`。

## 4. Completion 契约

V3 引入共享 Completion 类型，供 Compiler、VM 和 Runtime 统一表达控制流。

```rust
pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
    Break,
    Continue,
}
```

规则：

* VM 执行函数体时可以内部使用 `Completion`；
* 对外 `ChunkExecutor::execute_chunk` 仍返回 `Result<JsValue, NativeError>`；
* 函数调用内部遇到 `Completion::Return(value)` 时转换为调用表达式结果；
* 顶层脚本不得产生 `Completion::Return`；
* `Completion::Throw(value)` 继续映射为 `NativeError::Execute`；
* `Break` 和 `Continue` 只允许被循环跳转编译消费，不应逃出最终字节码执行边界；
* 不允许 Parser、Compiler 和 VM 各自定义不兼容的 Return/Throw 类型。

## 5. Runtime 函数值

V3 扩展 `JsValue`：

```rust
pub enum JsValue {
    // V1/V2 variants...
    Function(FunctionId),
}
```

新增稳定句柄：

```rust
pub struct FunctionId(pub u32);
```

函数对象：

```rust
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: Chunk,
    pub environment: Option<EnvironmentId>,
}
```

规则：

* `environment` 表示函数创建时捕获的外层环境；
* V3.1 可以对函数声明使用当前全局环境；
* V3.2 起必须支持函数表达式闭包；
* `JsFunction` 不得持有 Parser AST 引用；
* 函数调用时创建新的 Environment，其 outer 指向捕获环境；
* 参数绑定创建在新函数 Environment 中；
* 函数体内 `var` 声明创建在当前函数 Environment 中。

## 6. NativeContext 接口

建议扩展 `NativeContext`：

```rust
impl NativeContext {
    pub fn current_environment(&self) -> EnvironmentId;
    pub fn push_environment(&mut self, outer: Option<EnvironmentId>) -> Result<EnvironmentId, VmError>;
    pub fn pop_environment(&mut self) -> Result<(), VmError>;

    pub fn declare_binding(
        &mut self,
        environment: EnvironmentId,
        name: impl Into<String>,
        value: JsValue,
        mutable: bool,
    ) -> Result<(), VmError>;

    pub fn resolve_binding(&self, name: &str) -> Option<(EnvironmentId, JsValue)>;

    pub fn set_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError>;

    pub fn allocate_function(&mut self, function: JsFunction) -> Result<FunctionId, VmError>;
    pub fn function(&self, id: FunctionId) -> Option<&JsFunction>;

    pub fn push_call_frame(&mut self, frame: CallFrame) -> Result<(), VmError>;
    pub fn pop_call_frame(&mut self) -> Result<CallFrame, VmError>;
    pub fn current_this(&self) -> JsValue;

    pub fn reset_call_depth(&mut self, call_stack_limit: u64);
    pub fn consume_call_depth(&mut self) -> Result<(), VmError>;
}
```

规则：

* 环境链属于当前 isolate；
* 不得用全局 `HashMap` 模拟局部变量；
* 每次 `NativeRuntime::eval` 开始时应重置调用深度预算；
* 错误退出后必须恢复调用帧、环境栈和操作数栈；
* 函数值、对象值和环境 ID 都只能引用当前 `NativeContext` 的 Heap；
* 不同 `NativeContext` 之间不得共享 `ObjectId`、`FunctionId` 或 `EnvironmentId`。

## 7. CallFrame 契约

V3 调用帧：

```rust
pub struct CallFrame {
    pub function: Option<FunctionId>,
    pub return_ip: usize,
    pub environment: EnvironmentId,
    pub this_value: JsValue,
    pub stack_base: usize,
}
```

规则：

* 顶层脚本可以使用 `function: None`；
* 函数调用前保存调用者 `ip`、`environment`、`this_value` 和 `stack_base`；
* 函数返回后恢复调用者状态；
* 返回值压回调用者操作数栈；
* 调用帧清理必须在正常返回、运行时错误和 Test262Error 抛出时都执行。

## 8. 对象与数组契约

V3 可以在现有 `JsObject` 基础上增加对象种类：

```rust
pub enum ObjectKind {
    Ordinary,
    Array {
        elements: Vec<JsValue>,
    },
}

pub struct JsObject {
    pub prototype: Option<ObjectId>,
    pub kind: ObjectKind,
    pub properties: HashMap<String, PropertyDescriptor>,
}
```

如果为了减少改动暂不引入 `ObjectKind`，也可以把数组元素直接编码为 `"0"`、`"1"`、`"2"` 等属性，并维护 `"length"` 属性。但必须在文档中固定一种实现，不允许不同模块各自假设不同数组布局。

数组最低契约：

* `[a, b, c]` 创建对象值；
* `arr[0]` 返回第一个元素；
* `arr.length` 返回元素数量；
* `arr[0] = value` 修改元素并保留 `value`；
* 非整数 key 暂时按普通对象属性处理；
* 不实现稀疏数组空洞；
* 不实现 `Array.prototype`。

对象最低契约：

* `{ a: 1 }` 创建普通对象；
* `obj.a` 读取自有属性；
* `obj["a"]` 读取自有属性；
* `obj.a = value` 写入自有属性；
* `obj["a"] = value` 写入自有属性；
* 暂不沿 prototype 查找属性；
* 暂不支持 getter/setter；
* 暂不支持 property attributes 的完整规范行为。

## 9. ToPropertyKey 最小规则

V3 增加最小属性键转换：

```rust
pub fn to_property_key(value: &JsValue) -> Result<String, VmError>;
```

最低规则：

| 输入        | 结果                   |
| --------- | -------------------- |
| String    | 字符串本身                |
| Number 整数 | 十进制字符串               |
| Boolean   | `"true"` / `"false"` |
| Null      | `"null"`             |
| Undefined | `"undefined"`        |

暂不支持 Symbol。对象到原始值转换暂不实现，遇到对象 key 返回明确 TypeError 或 Unsupported。

## 10. 编译降级规则

### 10.1 函数声明

```text
compile function body -> FunctionTemplate
DeclareFunction(name, function_index)
```

函数声明语句在栈上不留下值。

### 10.2 函数表达式

```text
CreateFunction(function_index)
```

函数表达式在栈上留下函数值。

### 10.3 return

```text
return expression:
  expression
  Return

return:
  ReturnUndefined 或 Constant(undefined) + Return
```

函数外 `return` 必须在 Parser 或 Compiler 阶段拒绝。

### 10.4 普通调用

```text
callee(a, b):
  callee
  a
  b
  Call(2)
```

### 10.5 成员调用

```text
obj.method(a, b):
  obj
  duplicate receiver or lower to receiver temporary
  GetProperty("method")
  receiver
  a
  b
  CallWithThis(2)
```

如果当前 VM 没有 `Dup` 指令，编译器可以选择更简单的专用降级策略，但必须保持最终 `CallWithThis` 的栈布局为：

```text
[callee, this_value, arg0, ..., argN]
```

### 10.6 对象字面量

```text
{ a: value }:
  Constant("a")
  value
  ObjectCreate(1)
```

### 10.7 数组字面量

```text
[a, b, c]:
  a
  b
  c
  ArrayCreate(3)
```

### 10.8 计算成员访问

```text
object[key]:
  object
  key
  GetElement
```

### 10.9 成员赋值

```text
object.name = value:
  object
  value
  SetProperty("name")

object[key] = value:
  object
  key
  value
  SetElement
```

赋值表达式必须保留 `value` 作为结果。

## 11. 各组独立测试

### A 组

```text
源码 -> 函数声明 AST
源码 -> 函数表达式 AST
源码 -> return AST
源码 -> 对象/数组字面量 AST
函数外 return -> ParseError
缺失函数体 -> ParseError
计算成员访问 -> computed = true
```

### B 组

```text
手工 FunctionDeclaration AST -> DeclareFunction
手工 FunctionExpression AST -> CreateFunction
手工 Return AST -> Return 指令
手工 Object AST -> ObjectCreate
手工 Array AST -> ArrayCreate
手工 Member Assignment AST -> SetProperty / SetElement
函数外 Return AST -> CompileError
函数体 Chunk -> 所有路径可终止
```

### C 组

```text
手工 Chunk -> 函数调用返回值
手工 Chunk -> 参数绑定
手工 Chunk -> 局部变量不泄漏
手工 Chunk -> 递归触发调用深度限制
手工 Chunk -> 对象属性读写
手工 Chunk -> 数组 length 和下标访问
错误退出后再次执行 -> 栈、环境、调用帧已恢复
```

### D 组

```text
--native-v1 -> 零回退
--native-v2 -> 零回退
--native-v3 -> 固定清单全部通过
每个文件 default + strict
报告中 skipped 不计 passed
Boa 差分结果只作参考
```

## 12. 共享文件与合并规则

V3 共享文件：

```text
src/lexer/token.rs
src/ast/expression.rs
src/ast/statement.rs
src/bytecode/opcode.rs
src/bytecode/chunk.rs
src/runtime/value.rs
src/runtime/object.rs
src/runtime/environment.rs
src/runtime/context.rs
src/vm/frame.rs
src/contracts.rs
docs/version/native-v3-scope.md
docs/version/native-v3-interface.md
```

合并规则：

* 共享契约先单独 PR；
* A/B/C/D 各组实现 PR 不得偷偷修改共享契约；
* 如果确实需要修改共享契约，必须先更新本文档；
* 不得在 `backend/native.rs` 实现 Parser、Compiler、VM 或对象模型细节；
* 不得在 Native 不支持时自动调用 Boa；
* 不得把 Boa 的 AST、Value 或 Context 暴露给 Native 模块。
