你们现在**不要推倒重来**，而是把当前 Boa 套壳项目改造成“可对照、可替换、可证明自研”的工程。赛题明确要求 Rust 轻量 JS 引擎、通过 ECMAScript Test Suite 60% 以上，并且“创新性/非套壳实现”占 20%；功能测试和 benchmark 也各占 30%，所以不能只追求“把 Boa 跑通”。 你给的仓库页面能确认这是 `project2-Panicking`，并提供了 HTTPS clone 地址；但公开网页没有展开具体文件树，所以我下面按“当前主体是 Boa 包装层”来给路线。([GitLab][1])

## 总体策略：保留工程壳，替换执行内核

你们现在的 Boa 套壳可以继续保留，但只能作为：

1. **开发期 oracle**：对比你们自研引擎和 Boa 的输出差异。
2. **测试基准线**：记录“原来靠 Boa 能过多少”。
3. **工程框架来源**：保留 CLI、参数解析、测试脚本、benchmark 脚本、报告生成、CI、目录结构。

最终提交时必须做到：

```text
最终 release 路径：
main.rs / CLI
  -> 你们自己的 lexer
  -> 你们自己的 parser
  -> 你们自己的 AST / bytecode compiler
  -> 你们自己的 runtime / object model / builtin
  -> 你们自己的 interpreter / VM
```

而不是：

```text
main.rs
  -> boa_engine::Context::eval(...)
```

也不建议最终保留：

```text
boa_parser
boa_ast
boa_engine
QuickJS FFI
Node 子进程
Deno 子进程
```

否则很容易被认定为“套壳”。

---

# 第一阶段：先把项目拆成“可替换架构”

先做一层抽象，把 Boa 从主流程里隔离出去。

建议改成 Cargo workspace：

```text
agentjs/
  crates/
    agentjs_cli/          # 命令行入口
    agentjs_lexer/        # 自研词法分析
    agentjs_parser/       # 自研语法分析
    agentjs_ast/          # 自研 AST
    agentjs_runtime/      # Value/Object/Function/Scope
    agentjs_vm/           # AST解释器或字节码VM
    agentjs_test262/      # test262 runner / 结果统计
    boa_oracle/           # 只在 dev/test 中使用，最终 release 不依赖
```

核心接口先定下来：

```rust
pub trait JsEngine {
    fn eval_script(&mut self, source: &str) -> Result<JsValue, JsError>;
}
```

然后有两个实现：

```rust
BoaEngine      // 仅开发期使用
AgentEngine    // 你们自己的实现
```

这样当前框架还能用，但你们每天都能逐步把 `BoaEngine` 的测试迁移到 `AgentEngine`。

验收标准：

```bash
cargo tree | grep boa
```

最终 release 分支里不应该出现 `boa_engine`、`boa_parser` 等核心依赖。

---

# 第二阶段：不要一上来追完整 JS，先做“能跑 Test262 的核心子集”

JavaScript 引擎不是“把 JS 翻译成 Rust”，而是：

```text
JS 源码
  -> Token
  -> AST
  -> 内部指令 / AST执行
  -> Runtime Value
  -> 输出结果
```

你们三个人做比赛项目，推荐路线是：

```text
先 AST 解释器保正确性
再局部改成 bytecode VM 提性能和创新性
```

不要一开始就做 JIT，不现实，也没必要。

## 2.1 词法分析 Lexer

先支持这些 token：

```text
标识符
数字字面量
字符串字面量
关键字：var let const function return if else while for true false null undefined
运算符：+ - * / % == === != !== < <= > >= && || ! = += -=
分隔符：() {} [] ; , .
注释：// 和 /* */
```

先别急着支持完整 Unicode 标识符、正则字面量、模板字符串。

## 2.2 语法分析 Parser

用手写递归下降 + Pratt parser 最合适。

先支持：

```text
Program
BlockStatement
ExpressionStatement
VariableDeclaration
FunctionDeclaration
ReturnStatement
IfStatement
WhileStatement
ForStatement

Literal
Identifier
BinaryExpression
UnaryExpression
AssignmentExpression
CallExpression
MemberExpression
ObjectExpression
ArrayExpression
FunctionExpression
```

这已经能跑大量基础语法、表达式、函数、对象、数组测试。

## 2.3 Runtime Value

先设计自己的 `JsValue`：

```rust
enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object(ObjectId),
}
```

不要一开始做 BigInt、Symbol、Date、RegExp、Proxy、Promise、Module、Intl。

这些可以放后面。

## 2.4 环境模型 Scope / Environment

必须做，否则函数、变量、闭包会很难继续。

至少实现：

```text
GlobalEnvironment
FunctionEnvironment
LexicalEnvironment
var / let / const 的基本区别
作用域链查找
函数调用时的新环境
return 控制流
```

初期可以简化 TDZ，但要在文档里说明。

---

# 第三阶段：真正决定 Test262 通过率的是 Object Model

很多人以为 JS 引擎难点是 parser，其实后面会发现难点是：

```text
对象
属性
原型链
this
函数对象
内建对象
类型转换
```

所以你们要尽早实现对象系统。

## 必须优先做的对象能力

```text
Object
Array
Function
prototype chain
property get / set
this binding
new 调用
constructor
Object.prototype
Array.prototype
Function.prototype
```

属性至少要支持：

```text
value
writable
enumerable
configurable
```

也就是类似：

```rust
struct PropertyDescriptor {
    value: JsValue,
    writable: bool,
    enumerable: bool,
    configurable: bool,
}
```

否则 `Object.defineProperty`、数组 length、原型链测试会大量失败。

---

# 第四阶段：内建对象按“收益”排序实现

不要平均用力。为了 Test262 通过率，建议顺序是：

## 第一批：最高收益

```text
Object
Function
Array
String
Number
Boolean
Math
JSON
Error
TypeError
ReferenceError
SyntaxError
```

这些基础对象覆盖面很大。

## 第二批：中等收益

```text
Date
RegExp
Map
Set
Symbol
Promise
```

其中 `RegExp` 和 `Promise` 很难，不要太早碰。`Date` 可以先做部分实现。

## 第三批：比赛后期再考虑

```text
Module
async / await
generator
Proxy
Reflect
Intl
Temporal
WeakMap
WeakSet
Atomics
SharedArrayBuffer
```

这些对 3 人团队来说成本很高，不适合作为前期目标。

---

# 第五阶段：Test262 不能盲跑，要建“筛选 + 统计”体系

Test262 是 TC39 的官方 ECMAScript 一致性测试套件，用来检查实现是否符合 ECMAScript 规范；它不是普通单元测试集合，里面包含 negative、strict、module、features、includes 等元信息。([GitHub][2])

你们要自己做一个 runner，至少解析文件头：

```text
/*---
description:
features:
flags:
includes:
negative:
---*/
```

先筛掉这些：

```text
module
async
generator
Intl
Temporal
Atomics
SharedArrayBuffer
WeakRef
FinalizationRegistry
```

然后分组跑：

```text
language/expressions
language/statements
language/types
built-ins/Object
built-ins/Array
built-ins/Function
built-ins/String
built-ins/Number
built-ins/Math
built-ins/JSON
```

每次输出：

```text
总数
通过数
失败数
跳过数
按目录统计
按 feature 统计
失败原因 Top 20
```

你们之前 Boa 套壳跑出很高通过率，这个数据可以作为“参考基线”，但最终报告要清楚区分：

```text
Boa wrapper baseline：用于早期工程验证
AgentJS self engine：最终有效成绩
```

---

# 第六阶段：三个人怎么分工

你们只有 3 位开发者，不能所有人都写一坨 runtime。建议这样分：

## 开发者 A：Parser / AST / Compiler

负责：

```text
lexer
parser
AST
语法错误
AST pretty print
后期 bytecode compiler
```

短期目标：

```text
能解析基础 JS 程序
能把 AST 交给 runtime 执行
能处理 negative parse tests
```

## 开发者 B：Runtime / Object / Builtin

负责：

```text
JsValue
Object heap
scope / environment
function call
this
prototype chain
Object / Array / Function / String / Number / Math / JSON
```

这是最关键的人，决定 Test262 通过率上限。

## 开发者 C：Test262 / Benchmark / 工程化 / 文档

负责：

```text
test262 runner
CI
结果统计
失败归因
benchmark 脚本
和 Boa/Node/QuickJS 对照
文档和评审材料
```

这个人不是“打杂”，而是保证你们不跑偏。赛题里文档、功能测试、benchmark 加起来占 80%，不能只写代码。

---

# 推荐里程碑

## Milestone 0：冻结 Boa 套壳基线

目标：

```text
当前项目能稳定 build
当前 Boa 版本能跑 test262 子集
当前 benchmark 结果保存
```

产物：

```text
docs/baseline_boa.md
docs/current_architecture.md
scripts/run_test262.sh
scripts/run_benchmark.sh
```

并明确写：

```text
Boa baseline is for comparison only.
Final engine does not depend on Boa.
```

---

## Milestone 1：自研引擎能跑最小程序

目标：

```js
1 + 2 * 3;
```

```js
var x = 1;
x + 2;
```

```js
function add(a, b) {
  return a + b;
}
add(1, 2);
```

完成后，你们就已经不是纯套壳了。

---

## Milestone 2：基础语句和函数

支持：

```js
if / else
while
for
return
function declaration
function expression
作用域链
闭包初步
```

测试：

```js
function makeAdder(x) {
  return function(y) {
    return x + y;
  };
}
var add1 = makeAdder(1);
add1(2);
```

这一步做完，语言核心开始成型。

---

## Milestone 3：对象、数组、原型链

支持：

```js
var obj = { a: 1 };
obj.a;
obj.b = 2;

var arr = [1, 2, 3];
arr.length;
arr[0];

function Foo() {}
Foo.prototype.x = 1;
var f = new Foo();
f.x;
```

这是从“玩具解释器”到“JS 引擎”的分水岭。

---

## Milestone 4：内建对象冲 Test262

优先做：

```text
Object
Array
Function
String
Number
Boolean
Math
JSON
Error
```

每个 builtin 不要追完整，先做高频 API。

例如 Array 先做：

```text
push
pop
shift
unshift
slice
join
map
forEach
filter
reduce
isArray
```

String 先做：

```text
charAt
slice
substring
indexOf
includes
split
trim
toLowerCase
toUpperCase
```

Object 先做：

```text
keys
values
entries
assign
create
defineProperty
getOwnPropertyDescriptor
getPrototypeOf
setPrototypeOf
hasOwn
```

---

## Milestone 5：从 AST 解释器升级到 bytecode VM

为了 benchmark 和创新性，建议后期加一层 bytecode。

内部指令可以很简单：

```text
LoadConst
LoadName
StoreName
Add
Sub
Mul
Div
Eq
StrictEq
Jump
JumpIfFalse
Call
Return
GetProp
SetProp
NewObject
NewArray
CreateFunction
Pop
```

流程变成：

```text
JS source
  -> lexer
  -> parser
  -> AST
  -> bytecode
  -> VM execute
```

这比单纯 AST 解释器更容易写进报告：

```text
我们实现了自研词法分析、语法分析、字节码编译、栈式虚拟机、对象模型与部分 ECMAScript 内建对象。
```

QuickJS 也是小型可嵌入 JS 引擎，官方文档强调其低启动时间、小体积和解释执行特征；你们可以参考它的设计思想，但不能 FFI 调 QuickJS。([Fabrice Bellard's Home Page][3]) Boa 也可以作为 Rust JS 引擎参考，但最终不能依赖 Boa 的 parser/runtime/eval。([GitHub][4])

---

# 当前 Boa 框架哪些能留，哪些要删

## 可以保留

```text
Cargo 工程结构
CLI 参数解析
文件读取
REPL 外壳
测试脚本
benchmark 脚本
CI
README
报告生成工具
错误输出格式
```

## 可以暂时保留，但最终要隔离

```text
BoaEngine
boa_oracle
和 Boa 对比的测试代码
```

这些只能放在：

```text
dev-dependencies
features = ["oracle"]
```

不能在最终默认 build 里出现。

## 必须替换

```text
boa_engine::Context
boa_engine::Source
boa_parser
Boa AST
Boa Value
Boa Object
Boa builtin
```

最终答辩时评委最可能问：

> 你们这个和 Boa 的区别是什么？

你们要能回答：

```text
Boa 只作为早期参考和 oracle。
最终执行路径不经过 Boa。
我们自研了 lexer/parser/runtime/object model/VM。
```

---

# 最现实的技术取舍

你们目标不是做完整 V8，而是比赛交付。

所以建议明确不做或后做：

```text
JIT：不做
完整 GC：先用 arena + ObjectId，后期再做 mark-sweep
完整 RegExp：后做
Intl：不做
Temporal：不做
完整 Module：后做
async/await：后做
Proxy：后做
```

优先做：

```text
表达式
语句
函数
对象
数组
原型链
类型转换
基础 builtin
test262 runner
benchmark
文档
```

---

# 最后报告怎么写，才能证明“非套壳”

报告里建议专门放一章：

## “从 Boa Wrapper 到自研 AgentJS Engine 的迁移”

内容包括：

```text
1. 初始版本问题：依赖 boa_engine::Context::eval，属于套壳风险。
2. 改造方式：引入 JsEngine trait，将 Boa 隔离为 oracle。
3. 自研模块：
   - Lexer
   - Parser
   - AST
   - Runtime Value
   - Object Model
   - Environment
   - Builtin Objects
   - Bytecode VM
4. 最终依赖检查：
   cargo tree 中无 boa_engine / boa_parser。
5. 测试结果：
   - Test262 总通过率
   - 分模块通过率
   - benchmark 对比
6. 与 Boa/QuickJS 的关系：
   - 仅参考设计与对照测试
   - 不链接、不调用、不包装
```

这比单纯说“我们不是套壳”有说服力得多。

---

# 你们现在最应该立刻做的 5 件事

1. **把 Boa 调用封装进 `BoaEngine`，不要散落在代码里。**
2. **新增 `AgentEngine`，哪怕一开始只能算 `1 + 2`。**
3. **建立 test262 分类统计，不要只给总通过率。**
4. **把最终 release 分支设为无 Boa 依赖。**
5. **尽快实现对象模型和内建对象，不要长期停留在 parser。**

最推荐的路线一句话概括：

> 用当前 Boa 套壳项目保留工程能力和测试能力，用 Boa 当 oracle；然后按 lexer → parser → AST解释器 → runtime/object model → builtin → bytecode VM 的顺序逐步替换核心执行链，最终提交无 Boa 依赖的自研轻量 JS 引擎。

[1]: https://gitlab.eduxiji.net/T2026100069911071/project3136859-388770/-/tree/main/ "Files · main · PanicKing / project2-Panicking · GitLab"
[2]: https://github.com/tc39/test262?utm_source=chatgpt.com "tc39/test262: Official ECMAScript Conformance Test Suite"
[3]: https://bellard.org/quickjs/?utm_source=chatgpt.com "QuickJS Javascript Engine"
[4]: https://github.com/boa-dev/boa?utm_source=chatgpt.com "boa-dev/boa: Boa is an embeddable Javascript engine ..."
