# Native V4 对象模型与数组语义里程碑

本文档冻结 AgentJS Native V4 的功能范围、协作分工和验收标准。V3
已经支持函数、闭包、基础对象/数组字面量和自有属性读写；V4 不重复这些
能力，而是补齐 Test262 大量依赖的对象语义层：属性描述符、原型链、访问器、
通用构造调用和数组空洞/`length`。

共享类型和字节码契约见
[Native V4 共享接口规格](native-v4-interface.md)。

## 1. 完成标准

V4 完成时必须满足：

- V1、V2、V3 固定 Test262 清单零回退、零跳过；
- 支持数据属性与访问器属性的完整 V4 描述符；
- 属性读取、写入、删除和 `in` 沿统一对象模型执行；
- 支持普通函数作为构造器、实例原型连接和 `instanceof`；
- 支持对象字面量 getter/setter；
- 支持数组空洞、索引写入和最小 `length` 截断/扩展语义；
- 提供最小 `Object`、`Array`、`Function` 构造器和原型对象；
- 新增 `--native-v4`，覆盖 V4 相关目录并如实报告 passed/failed/skipped；
- 不支持的 Proxy、Symbol、class、解构、spread、迭代器不得由 Boa 兜底。

V4 分三批开发：

1. **V4.1**：属性描述符、原型链、`delete`、`in`；
2. **V4.2**：用户构造器、`prototype`、`instanceof`、getter/setter；
3. **V4.3**：稀疏数组、`length`、基础 Object/Array/Function builtins。

## 1.1 当前实现审计

状态定义：

- **接口完成**：共享类型或 API 已存在，但不能作为版本完成依据；
- **端到端完成**：可从 JavaScript 源码通过 Native 链路执行；
- **Test262 固定门**：已加入零失败、零跳过的固定清单；
- **未完成**：仍为空函数、占位实现、缺少全局安装或缺少必要语义。

| 功能 | 当前状态 | 主要证据/缺口 |
| --- | --- | --- |
| 描述符、属性顺序、原型链 | 端到端完成 | Runtime API、VM 和 V4 测试已连接 |
| `delete`、`in` | Test262 固定门 | 已覆盖删除、缺失属性和数组元素 |
| 用户构造器、`instanceof` | 端到端完成 | 自定义构造器可运行；官方目录仍依赖标准构造器 |
| getter/setter 字面量 | 端到端完成 | 访问器调用和 `this` 已连接 |
| 稀疏数组、空洞、`length` | 端到端完成 | 空洞、扩展、截断和 RangeError 已有测试 |
| `Object` 构造器与静态方法 | **未完成** | `src/builtins/object.rs` 仍为空安装函数 |
| `Array` 构造器与原型方法 | **未完成** | `src/builtins/array.rs` 仍为空安装函数 |
| `Function` 构造器与 `call` | **未完成** | `src/builtins/function.rs` 仍为空安装函数 |
| Builtin 注册表与 Intrinsics | **未完成** | 当前只有封闭 `NativeFunction`，Context 无 Intrinsics |
| V4 Test262 固定门 | 初始完成 | 当前 11 项只是回归门，不代表 Object/Array 目录完成 |

因此，当前版本不得标记为“V4 完成”。扩大开发以完成 V4.3 和标准内建层为
核心，不重复已经通过端到端测试的 V4.1/V4.2 代码。

## 1.2 扩大开发阶段

### V4E.0：Builtin 基础设施

- 将封闭的 `NativeFunction` 分发迁移为可扩展 Builtin 注册表；
- 为 Builtin 同时记录 `call`、可选 `construct`、`name`、`length` 和函数对象；
- 在 `NativeContext` 中建立每个 Realm 独立的 `Intrinsics`；
- 统一用户函数与 Builtin 的调用/构造入口；
- 安装 `Object`、`Array`、`Function` 全局绑定和原型关系。

### V4E.1：Object

实现并端到端验证：

```text
Object
Object.create
Object.defineProperty
Object.getOwnPropertyDescriptor
Object.getPrototypeOf
Object.setPrototypeOf
Object.keys
```

### V4E.2：Array

实现并端到端验证：

```text
Array
Array.isArray
Array.prototype.push
Array.prototype.pop
```

数组构造器、数组字面量和稀疏数组必须共享同一 `ObjectKind::Array` 语义。

### V4E.3：Function

实现并端到端验证：

```text
Function
Function.prototype
Function.prototype.call
```

同时修正普通函数对象的 `"prototype"`、`"constructor"` 和 `instanceof`
连接，不允许通过旁路表形成第二套可观察对象模型。

### V4E.4：Test262 扩容与验收

- 扫描 `test/built-ins/Object`、`test/built-ins/Array` 和
  `test/built-ins/Function/prototype/call`；
- 重跑 `expressions/object`、`expressions/array` 和
  `expressions/instanceof`；
- 逐文件排除超范围语法与 harness 依赖；
- 扩大 `--native-v4` 覆盖目录，并记录完整扫描中的通过、失败和跳过数；
- 只有 V4E.0–V4E.4 全部完成后才允许标记 V4 完成。

## 2. 语言范围

### 2.1 前端新增

新增保留字/运算符：

```text
delete in instanceof
```

`get` 和 `set` 不是全局保留字，只在对象字面量属性定义位置作为上下文关键字识别。

新增或扩展语法：

```javascript
delete object.property
delete object[key]
key in object
value instanceof Constructor
({ get value() { return 1; } })
({ set value(v) { this.saved = v; } })
({ __proto__: base })
[1, , 3]
new Constructor(a, b)
```

V4 不包含：

- `let`、`const`、TDZ；
- `try/catch/finally`；
- class、`super`、arrow function；
- 解构、spread/rest、默认参数；
- Symbol 属性键、Proxy；
- `for-in`、`for-of`、迭代器协议。

### 2.2 对象语义

- 每个对象都有 `prototype: Option<ObjectId>`；
- `[[Get]]` 先查自有属性，再沿原型链查找；
- 数据属性写入遵守 `writable`；
- 访问器读取调用 getter，写入调用 setter，并传递接收者作为 `this`；
- 删除仅删除自有属性，并遵守 `configurable`；
- `in` 检查自有属性和原型链；
- 原型链循环必须被拒绝；
- 属性查找不得递归至宿主栈失控。
- 对象字面量的单个 `__proto__: expression` 设置对象原型；重复的原型 setter
  是早期语法错误，普通计算属性不触发该规则。

### 2.3 构造与原型

- `new F(args)` 创建以 `F.prototype` 为原型的新对象；
- 调用 `F` 时传入新对象作为 `this`；
- 构造器显式返回对象时使用该对象，否则返回新对象；
- 普通函数创建时拥有可写的 `"prototype"` 对象；
- prototype 对象的 `"constructor"` 指回函数；
- `instanceof` 沿左值对象的原型链查找右值的 `"prototype"`；
- 非可调用/不可构造右值返回 TypeError。

### 2.4 数组语义

- 数组元素槽使用 `Option<JsValue>` 区分空洞和 `undefined`；
- `[1, , 3].length === 3`，但索引 `1` 不是自有属性；
- 写入超出当前长度的数组索引会扩展 `length`；
- 写入较小 `length` 删除越界元素；
- 增大 `length` 只产生空洞；
- 非法数组长度返回 RangeError；
- V4 暂不实现迭代器和全部 Array.prototype 方法。

### 2.5 最小 Builtins

优先实现：

```text
Object
Object.create
Object.defineProperty
Object.getOwnPropertyDescriptor
Object.getPrototypeOf
Object.setPrototypeOf
Object.keys

Array
Array.isArray
Array.prototype.push
Array.prototype.pop

Function
Function.prototype.call
```

所有 builtin 必须通过共享对象/描述符 API 实现，禁止维护第二套属性表。

## 3. 四组分工

扩大 V4 的文件所有权、C0–C3 子任务、分支建议和合并依赖见
[Native V4 扩大开发分工](native-v4-team-plan.md)。本节保留各组职责摘要；
发生分工冲突时以该协作文档为准。

### A. 前端组：`lexer/`、`ast/`、`parser/`

- 增加上述 Token 和运算符优先级；
- 在对象属性位置上下文识别 `get`、`set` 和 `__proto__`；
- 将数组字面量元素改为可表示空洞的结构；
- 区分数据属性、getter 和 setter；
- 解析 `delete`、`in`、`instanceof`；
- 添加非法访问器参数、重复逗号和构造表达式负向测试；
- 不依赖 Compiler 或 Runtime。
- 扩大阶段只修复 Object/Array/Function Test262 暴露的范围内语法问题。

### B. 编译器组：`bytecode/`

- 实现 V4 Opcode 和固定栈效果；
- 将对象字面量降级为“创建对象 + 定义属性”；
- 将稀疏数组降级为“按长度创建 + 写入存在元素”；
- 编译访问器函数并保留源码求值顺序；
- 编译 `delete`、`in`、`instanceof`；
- 继续使用手工 AST 测试，所有 Chunk 必须通过验证。
- 扩大阶段验证通用 `Call`、`CallWithThis`、`Construct` 可以承载 Builtin，
  不为具体标准方法增加专用 Opcode。

### C. VM/Runtime/Builtins 组

- 扩展 PropertyDescriptor；
- 实现统一 `get/set/define/delete/has` 内部方法；
- 实现原型链检查与循环保护；
- 实现用户函数构造调用和 `instanceof`；
- 实现访问器调用及异常清理；
- 改造数组存储与 `length`；
- 注册 Object/Array/Function 最小 builtins；
- 建立 Builtin 注册表与 Realm 内 Intrinsics；
- 保证标准构造器、字面量和用户函数共享同一对象/调用模型；
- 使用手工 Chunk 和直接 Runtime API 测试。
- 内部分为 C0 Builtin Core、C1 Object、C2 Array、C3 Function，按协作文档
  顺序合并。

### D. 集成测试组

- 增加 `--native-v4` 完整 V4 目录扫描；
- 保留 V1–V3 固定门；
- 扫描 `expressions/object`、`expressions/array`、
  `expressions/delete`、`expressions/in`、`expressions/instanceof` 和相关 built-ins；
- 区分功能失败、harness 缺失、超范围语法和超时；
- 更新 Native Test262 报告，不把 skipped 计入 passed。
- 独占 `src/test262.rs`、V4 集成测试、报告和 CI 的最终写入。

## 4. 自有端到端验收

```javascript
function Point(x) { this.x = x; }
var p = new Point(3);
p.x;                                                   // 3
```

```javascript
function Point() {}
var p = new Point();
p instanceof Point;                                    // true
```

```javascript
var base = { x: 1 };
var child = Object.create(base);
child.x;                                                // 1
"x" in child;                                          // true
```

```javascript
var object = {
  get x() { return 7; },
  set x(v) { this.saved = v; }
};
object.x = 4;
object.x + object.saved;                                // 11
```

```javascript
var array = [1, , 3];
array.length === 3 && !(1 in array);                    // true
array[5] = 6;
array.length;                                           // 6
array.length = 2;
array[3];                                               // undefined
```

还必须覆盖：

```text
delete nonConfigurableProperty -> false 或 strict TypeError
Object.setPrototypeOf(a, a)    -> TypeError
1 instanceof 2                 -> TypeError
new 1()                        -> TypeError
array.length = -1              -> RangeError
getter/setter 抛错后            -> 栈、环境和调用帧恢复
```

## 5. Test262 策略

V4 不预先宣称整个 Object/Array 目录通过。D 组应按以下顺序建立清单：

1. 从 V3 报告中的失败目录生成候选；
2. 排除明确依赖 class、Symbol、Proxy、解构、spread 和迭代器的文件；
3. 每个候选单独运行 default/strict；
4. `--native-v4` 必须直接运行 V4 相关目录，不使用挑选后的通过清单；
5. 同步记录 Object/Array/Function 目录基线增量。

优先候选区域：

```text
test/language/expressions/object
test/language/expressions/array
test/language/expressions/delete
test/language/expressions/in
test/language/expressions/instanceof
test/built-ins/Object
test/built-ins/Array
test/built-ins/Function/prototype/call
```

## 6. 合并顺序

V4.1/V4.2 的 AST、Descriptor 和 Opcode 契约已经合并。扩大开发按以下顺序：

1. 单独合并 V4E.0 的 BuiltinId、Builtin 注册表和 Intrinsics 契约；
2. 合并 Object Builtins；
3. 合并 Array Builtins；
4. 合并 Function Builtins 与统一调用/构造入口；
5. 合并 Test262 扩容、报告和 CI；
6. 每批运行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
cargo run --release -- test262 --native-v1 --jobs 1
cargo run --release -- test262 --native-v2 --jobs 1
cargo run --release -- test262 --native-v3 --jobs 1
cargo run --release -- test262 --native-v4 --jobs 1
```

当前 `--native-v4` 是回归门。最终验收还必须运行相关目录基线：

```sh
cargo run --release -- test262 --native-v4 --jobs 1 --verbose
cargo run --release -- test262 --backend native --suite test/built-ins/Object --jobs 1
cargo run --release -- test262 --backend native --suite test/built-ins/Array --jobs 1
cargo run --release -- test262 --backend native \
  --suite test/built-ins/Function/prototype/call --jobs 1
```
