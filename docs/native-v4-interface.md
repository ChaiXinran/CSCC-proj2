# Native V4 共享接口规格

本文档定义 V4 对象模型开发期间 A/B/C/D 四组共同遵守的 AST、字节码、
Runtime 和 Builtin 契约。它补充
[基础接口规格](interface-spec.md)与
[V3 接口规格](native-v3-interface.md)；V4 新增部分发生冲突时以本文为准。

## 0. 契约状态

当前 AST、V4 Opcode、PropertyDescriptor、PropertyMap、稀疏数组和原型链 API
已经落地。以下扩大开发契约仍是必需实现项：

```text
BuiltinId / BuiltinFunction 注册表
JsFunction 的函数对象与 constructable 标记
NativeContext::Intrinsics
统一 call_value / construct_value
Object / Array / Function 全局安装
```

在这些接口落地前，`src/builtins/object.rs`、`array.rs`、`function.rs` 的空安装
函数不得视为 V4 实现。契约变更应先单独提交，再由 Builtins 和 VM 实现跟进。

## 1. 共享 AST

### 1.1 运算符

```rust
pub enum UnaryOperator {
    // existing variants...
    Delete,
}

pub enum BinaryOperator {
    // existing variants...
    In,
    InstanceOf,
}
```

`in` 与 `instanceof` 使用关系运算符优先级。`delete` 使用一元运算符优先级。

### 1.2 对象属性

V3 的 `ObjectProperty { key, value }` 扩展为：

```rust
pub enum ObjectProperty {
    Data {
        key: PropertyName,
        value: Expression,
    },
    Getter {
        key: PropertyName,
        body: FunctionBody,
    },
    Setter {
        key: PropertyName,
        parameter: FunctionParam,
        body: FunctionBody,
    },
    PrototypeSetter {
        value: Expression,
    },
}
```

约束：

- getter 参数数量必须为 0；
- setter 参数数量必须为 1；
- V4 属性名仅支持 identifier/string/number；
- 计算属性名、method shorthand、spread 延后；
- 对象属性按源码顺序求值和定义。
- 单个非计算 `__proto__: value` 解析为 `PrototypeSetter`；
- 同一对象字面量出现两个 `PrototypeSetter` 时返回 ParseError。

### 1.3 稀疏数组

```rust
pub enum ArrayElement {
    Hole,
    Expression(Expression),
}

pub enum Expression {
    // existing variants...
    Array(Vec<ArrayElement>),
}
```

尾逗号不额外增加空洞：`[1,]` 长度为 1；`[1,,]` 长度为 2。

## 2. PropertyDescriptor

V3 的纯数据描述符替换为：

```rust
pub enum PropertyKind {
    Data {
        value: JsValue,
        writable: bool,
    },
    Accessor {
        get: Option<JsValue>,
        set: Option<JsValue>,
    },
}

pub struct PropertyDescriptor {
    pub kind: PropertyKind,
    pub enumerable: bool,
    pub configurable: bool,
}
```

`Object.defineProperty` 输入的是可省略字段的描述符对象，因此另定义：

```rust
pub struct PropertyDescriptorUpdate {
    pub value: Option<JsValue>,
    pub writable: Option<bool>,
    pub get: Option<Option<JsValue>>,
    pub set: Option<Option<JsValue>>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}
```

`PropertyDescriptor` 表示 Heap 中的完整属性；`PropertyDescriptorUpdate` 仅用于
解析和应用部分描述符。不得用 `undefined` 代替“字段未提供”。

必须提供稳定构造器：

```rust
impl PropertyDescriptor {
    pub fn data_with(
        value: JsValue,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    ) -> Self;

    pub fn accessor(
        get: Option<JsValue>,
        set: Option<JsValue>,
        enumerable: bool,
        configurable: bool,
    ) -> Self;
}
```

禁止通过公开字段假定所有属性都有 `value`。V3 调用点必须迁移到描述符方法。

## 3. 对象内部方法

`NativeContext` 统一提供：

```rust
pub fn get(
    &mut self,
    receiver: JsValue,
    key: &str,
) -> Result<JsValue, VmError>;

pub fn set(
    &mut self,
    receiver: JsValue,
    key: &str,
    value: JsValue,
    strict: bool,
) -> Result<bool, VmError>;

    pub fn define_own_property(
    &mut self,
    object: ObjectId,
    key: String,
    descriptor: PropertyDescriptor,
    ) -> Result<bool, VmError>;

    pub fn validate_and_apply_property_descriptor(
        &mut self,
        object: ObjectId,
        key: String,
        update: PropertyDescriptorUpdate,
    ) -> Result<bool, VmError>;

pub fn get_own_property(
    &self,
    object: ObjectId,
    key: &str,
) -> Option<&PropertyDescriptor>;

pub fn delete_property(
    &mut self,
    object: ObjectId,
    key: &str,
    strict: bool,
) -> Result<bool, VmError>;

pub fn has_property(
    &self,
    object: ObjectId,
    key: &str,
) -> Result<bool, VmError>;

pub fn get_prototype_of(&self, object: ObjectId) -> Option<ObjectId>;

pub fn set_prototype_of(
    &mut self,
    object: ObjectId,
    prototype: Option<ObjectId>,
) -> Result<bool, VmError>;
```

规则：

- VM、Builtins 和测试必须使用这些接口，不得直接操作属性 HashMap；
- `get`/`set` 必须保留最初 receiver，以便原型上的 accessor 得到正确 `this`；
- 原型遍历必须使用迭代和访问上限，不依赖无限宿主递归；
- `set_prototype_of` 必须检测循环；
- 对象 ID 只能属于当前 `NativeContext`。

### 3.1 属性顺序

V4 的属性存储必须保留创建顺序，不能继续只使用无序 `HashMap`。建议：

```rust
pub struct PropertyMap {
    entries: Vec<PropertyEntry>,
    index: HashMap<String, usize>,
}

pub struct PropertyEntry {
    pub key: String,
    pub descriptor: PropertyDescriptor,
}
```

枚举顺序最低要求：

1. 数组索引键按数值升序；
2. 其他字符串键按首次创建顺序；
3. 重定义属性不改变原有顺序。

这套顺序同时供 `Object.keys`、描述符测试和后续 `for-in` 使用。

## 4. 可调用与可构造值

V4 将“可调用”和“可构造”分开：

```rust
pub struct JsFunction {
    // V3 fields...
    pub object: ObjectId,
    pub constructable: bool,
}

pub struct BuiltinId(pub u16);

pub enum JsValue {
    // existing variants...
    BuiltinFunction(BuiltinId),
}
```

Builtin 注册表建议契约：

```rust
pub type NativeCall = fn(
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError>;

pub type NativeConstruct = fn(
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError>;

pub struct BuiltinFunction {
    pub name: &'static str,
    pub length: u8,
    pub call: NativeCall,
    pub construct: Option<NativeConstruct>,
    pub object: ObjectId,
}
```

现有封闭 `NativeFunction` 枚举应在 V4.0 契约提交中迁移，避免每增加一个
builtin 都修改 VM 的核心 match。

扩大开发冻结以下注册接口：

```rust
impl NativeContext {
    pub fn register_builtin(
        &mut self,
        name: &'static str,
        length: u8,
        call: NativeCall,
        construct: Option<NativeConstruct>,
    ) -> Result<JsValue, VmError>;

    pub fn builtin(&self, id: BuiltinId) -> Option<&BuiltinFunction>;
}
```

规则：

- Builtin 必须是可读取属性的函数对象，不能只是不可扩展的枚举标签；
- `call` 与 `construct` 独立，不能通过函数名判断是否可构造；
- VM 只分发 `BuiltinId`，具体 Object/Array/Function 行为留在 `builtins/`；
- Test262 的 `assert` 与 `Test262Error` 也迁移到同一注册机制。

## 5. 构造调用

VM 提供统一入口：

```rust
fn call_value(
    &mut self,
    callee: JsValue,
    this_value: JsValue,
    arguments: Vec<JsValue>,
    context: &mut NativeContext,
) -> Result<JsValue, VmError>;

fn construct_value(
    &mut self,
    constructor: JsValue,
    arguments: Vec<JsValue>,
    new_target: JsValue,
    context: &mut NativeContext,
) -> Result<JsValue, VmError>;
```

`Instruction::Construct(n)` 的栈布局保持：

```text
[constructor, arg0, ..., argN]
```

执行用户函数构造器：

1. 读取 `constructor.prototype`；
2. 若不是对象，使用 `Object.prototype`；
3. 创建 ordinary object；
4. 以该对象为 `this` 调用函数；
5. 函数返回对象则采用返回值，否则采用步骤 3 的对象。

用户函数和 Builtin 必须走同一个调用入口。VM 的指令循环不得为
`Object.create`、`Array.isArray` 等具体 Builtin 添加专用分支。

## 6. V4 Opcode

新增指令：

```rust
pub enum Instruction {
    // existing variants...
    ObjectCreateEmpty,
    ArrayCreateSparse(u32),

    DefineDataProperty(u16),
    DefineGetter(u16),
    DefineSetter(u16),
    SetObjectPrototype,
    DefineElement(u32),

    DeleteProperty(u16),
    DeleteElement,
    HasProperty,
    InstanceOf,
}
```

固定栈契约：

| Instruction | required | pops | pushes |
| --- | ---: | ---: | ---: |
| `ObjectCreateEmpty` | 0 | 0 | 1 |
| `ArrayCreateSparse(n)` | 0 | 0 | 1 |
| `DefineDataProperty(k)` | 2 | 1 | 0 |
| `DefineGetter(k)` | 2 | 1 | 0 |
| `DefineSetter(k)` | 2 | 1 | 0 |
| `SetObjectPrototype` | 2 | 1 | 0 |
| `DefineElement(i)` | 2 | 1 | 0 |
| `DeleteProperty(k)` | 1 | 1 | 1 |
| `DeleteElement` | 2 | 2 | 1 |
| `HasProperty` | 2 | 2 | 1 |
| `InstanceOf` | 2 | 2 | 1 |

`Define*` 指令保留栈中的 object，仅弹出待定义的值或访问器函数。示例：

```text
{ a: expr }:
  ObjectCreateEmpty
  expr
  DefineDataProperty("a")
```

```text
{ __proto__: expr }:
  ObjectCreateEmpty
  expr
  SetObjectPrototype
```

```text
[first, , third]:
  ArrayCreateSparse(3)
  first
  DefineElement(0)
  third
  DefineElement(2)
```

原有 `ObjectCreate(n)` 和密集 `ArrayCreate(n)` 在 V4 继续保留，以减少 V3
回归；新语义由新指令承担。

## 7. 对象和数组存储

```rust
pub enum ObjectKind {
    Ordinary,
    Array {
        elements: Vec<Option<JsValue>>,
        length_writable: bool,
    },
}
```

数组索引规则：

- 仅 canonical non-negative integer string 进入元素槽；
- 空洞返回 `undefined`，但 `has_property` 为 false；
- `"length"` 是特殊不可枚举、不可配置属性；
- 收缩长度时从高索引向低索引删除；
- 遇到不可配置元素导致收缩失败时恢复规范要求的最终长度；
- V4 可限制最大数组长度，但必须返回 RangeError，不能 panic/OOM。

## 8. Builtin 安装结果

基础安装必须产生稳定引用：

```rust
pub struct Intrinsics {
    pub object_prototype: ObjectId,
    pub function_prototype: ObjectId,
    pub array_prototype: ObjectId,
    pub object_constructor: JsValue,
    pub function_constructor: JsValue,
    pub array_constructor: JsValue,
}
```

`NativeContext` 持有 `Intrinsics`，禁止通过全局名称反查原型对象。不同 Context
的 Intrinsics 不得共享 ObjectId。

初始化顺序冻结为：

```text
创建 Object.prototype
→ 创建 Function.prototype
→ 创建 Object / Function 构造器
→ 创建 Array.prototype / Array 构造器
→ 安装静态方法和原型方法
→ 写入全局 Object / Array / Function
```

必须满足的关系：

```javascript
Object.prototype.constructor === Object
Array.prototype.constructor === Array
Function.prototype.constructor === Function
Object.getPrototypeOf([]) === Array.prototype
Object.getPrototypeOf(Array) === Function.prototype
```

### 8.1 Object Builtin 契约

- `Object(value)`：对象原样返回；`null`/`undefined` 创建普通对象；
- `Object.create(proto)`：只接受对象或 `null`；
- `Object.defineProperty`：通过 `PropertyDescriptorUpdate` 应用；
- `Object.getOwnPropertyDescriptor`：返回新的描述符对象或 `undefined`；
- `Object.getPrototypeOf` / `setPrototypeOf`：调用统一原型 API；
- `Object.keys`：只返回可枚举自有字符串键，顺序来自 `PropertyMap`。

### 8.2 Array Builtin 契约

- `Array()` 创建长度 0 数组；
- 单个数值参数表示长度，非法值返回 RangeError；
- 其他参数按元素创建密集数组；
- `Array.isArray` 根据 `ObjectKind::Array` 判断；
- `push`/`pop` 使用共享索引和 `length` 语义，不直接修改旁路容器。

### 8.3 Function Builtin 契约

- 普通用户函数和 Builtin 都有可观察函数对象；
- `Function.prototype.call` 将首参数作为 `thisArg`，其余参数转发；
- 动态源码形式 `Function("...")` 可在扩大 V4 中明确返回 Unsupported，但
  `Function` 全局、原型关系和 `call` 必须存在；
- `instanceof` 必须读取可观察的 `"prototype"` 属性，而不是只查询隐藏旁路表。

## 9. 错误与清理

- 非对象属性操作按具体操作返回 TypeError；
- 非法数组长度返回 RangeError；
- 不可配置属性删除在 strict 模式返回 TypeError；
- accessor 或构造器抛错后必须恢复 VM 栈、环境栈和调用帧；
- 原型链循环返回 TypeError；
- Builtin 不得 panic，也不得调用 Boa。

## 10. 各组独立测试

### A 组

```text
delete / in / instanceof -> AST 与优先级
getter/setter -> ObjectProperty
稀疏数组 -> Hole 位置
非法 accessor 参数 -> ParseError
```

### B 组

```text
手工 AST -> V4 指令序列
对象属性求值顺序
稀疏数组长度和元素索引
所有 V4 指令 stack_effect
Chunk::validate() 覆盖所有路径
```

### C 组

```text
直接 Descriptor API -> 属性重定义结果
手工原型链 -> get/set/has/delete
手工 Chunk -> getter/setter this
手工 Chunk -> new / instanceof
数组空洞、扩展、截断和 RangeError
异常后状态恢复
```

### D 组

```text
--native-v1/v2/v3 -> 零回退
--native-v4 -> 固定清单零失败、零跳过
Object/Array 目录基线增量
报告不把 skipped 计入 passed
```

### 扩大 V4 完成门

```text
typeof Object / Array / Function -> "function"
三个 builtins 文件不再包含空安装实现
Object/Array/Function 自有端到端样例全部通过
相关 Test262 目录相对基线有可解释的新增通过
NATIVE_V4_TESTS 相对初始 11 项扩大且零失败、零跳过
```

## 11. 共享文件和合并规则

V4 共享文件：

```text
src/ast/expression.rs
src/bytecode/opcode.rs
src/runtime/property.rs
src/runtime/property_map.rs
src/runtime/object.rs
src/runtime/function.rs
src/runtime/value.rs
src/runtime/context.rs
src/contracts.rs
docs/native-v4-scope.md
docs/native-v4-interface.md
```

这些文件的契约变更先单独合并。各实现 PR 不得自行改变栈布局、描述符结构、
数组槽位表示或 Builtin 调用签名；确需修改时，必须先更新本文档并完成评审。
