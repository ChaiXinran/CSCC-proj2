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
- 新增 `NATIVE_V4_TESTS` 与 `--native-v4`；
- 不支持的 Proxy、Symbol、class、解构、spread、迭代器不得由 Boa 兜底。

V4 分三批开发：

1. **V4.1**：属性描述符、原型链、`delete`、`in`；
2. **V4.2**：用户构造器、`prototype`、`instanceof`、getter/setter；
3. **V4.3**：稀疏数组、`length`、基础 Object/Array/Function builtins。

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

### A. 前端组：`lexer/`、`ast/`、`parser/`

- 增加上述 Token 和运算符优先级；
- 在对象属性位置上下文识别 `get`、`set` 和 `__proto__`；
- 将数组字面量元素改为可表示空洞的结构；
- 区分数据属性、getter 和 setter；
- 解析 `delete`、`in`、`instanceof`；
- 添加非法访问器参数、重复逗号和构造表达式负向测试；
- 不依赖 Compiler 或 Runtime。

### B. 编译器组：`bytecode/`

- 实现 V4 Opcode 和固定栈效果；
- 将对象字面量降级为“创建对象 + 定义属性”；
- 将稀疏数组降级为“按长度创建 + 写入存在元素”；
- 编译访问器函数并保留源码求值顺序；
- 编译 `delete`、`in`、`instanceof`；
- 继续使用手工 AST 测试，所有 Chunk 必须通过验证。

### C. VM/Runtime/Builtins 组

- 扩展 PropertyDescriptor；
- 实现统一 `get/set/define/delete/has` 内部方法；
- 实现原型链检查与循环保护；
- 实现用户函数构造调用和 `instanceof`；
- 实现访问器调用及异常清理；
- 改造数组存储与 `length`；
- 注册 Object/Array/Function 最小 builtins；
- 使用手工 Chunk 和直接 Runtime API 测试。

### D. 集成测试组

- 增加 `NATIVE_V4_TESTS`、`--native-v4`；
- 保留 V1–V3 固定门；
- 扫描 `expressions/object`、`expressions/array`、
  `expressions/delete`、`expressions/in`、`expressions/instanceof` 和相关 built-ins；
- 区分功能失败、harness 缺失、超范围语法和超时；
- 更新 Native Test262 报告，不把 skipped 计入 passed。

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
4. 只有零失败、零跳过的官方文件才能加入 `NATIVE_V4_TESTS`；
5. 同步记录 Object/Array 目录基线增量。

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

1. 单独合并 V4 AST、Descriptor、Opcode 契约；
2. A/B/C 基于同一契约并行开发；
3. 先合并 V4.1，再合并 V4.2、V4.3；
4. 每批运行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
cargo run --release -- test262 --native-v1 --jobs 1
cargo run --release -- test262 --native-v2 --jobs 1
cargo run --release -- test262 --native-v3 --jobs 1
```

V4 完成后再启用：

```sh
cargo run --release -- test262 --native-v4 --jobs 1 --verbose
```
