# Native V3 函数与复合值里程碑

本文档冻结 AgentJS Native V3 的功能范围、协作分工与验收标准。V3 在 V1 表达式流水线与 V2 控制流能力之上，增加用户定义函数、局部执行环境、`return` 控制流、对象字面量、数组字面量、计算成员访问和基础方法调用语义，使 Native 引擎从“可执行脚本片段”进入“可执行小型 JavaScript 程序”的阶段。

共享类型和字节码契约见 `native-v3-interface.md`。

## 1. V3 完成标准

V3 必须做到：

* V1 的固定 Test262 清单继续全部通过；
* V2 的固定 Test262 清单继续全部通过；
* Native 路径支持函数声明、函数表达式、参数绑定、局部变量、`return` 和嵌套函数调用；
* Native 路径支持对象字面量、数组字面量、计算成员访问、成员赋值和基础数组 `length`；
* 普通函数调用必须拥有独立调用帧，函数返回后不得泄漏局部变量和临时操作数栈；
* 递归调用受 `RuntimeConfig::call_stack_limit` 或等价调用深度限制；
* `return` 出现在函数外时必须返回语法错误或编译错误；
* 成员调用 `obj.method(args)` 必须正确传递最小 `this` 值；
* 不支持的 prototype、class、arrow function、generator、async、try/catch 不得由 Boa 兜底；
* 新增固定 Test262 清单在 default 和 strict 模式下通过。

V3 分三批合并：

1. **V3.1**：函数声明、函数调用、参数绑定、`return`；
2. **V3.2**：函数表达式、闭包基础、调用深度限制；
3. **V3.3**：对象/数组字面量、计算成员访问、成员赋值、基础 `this` 绑定。

## 2. 语言功能

### 2.1 Lexer

新增或正式启用关键字：

```text
function return
```

新增或正式启用分隔符：

```text
[ ]
```

V3 继续沿用 V1/V2 的字符串、数字、标识符和注释规则。暂不要求支持 Unicode 标识符、模板字符串、正则字面量和数字分隔符。

### 2.2 Parser 与 AST

新增或正式启用：

* `function name(param1, param2) { StatementList }`；
* `function (param1, param2) { StatementList }`；
* `return;`；
* `return expression;`；
* `return` 与表达式之间出现行终止符时按 `return;` 处理；
* 函数参数列表；
* 函数体块；
* 函数内 `var` 局部绑定；
* 函数内嵌套函数声明；
* 函数调用 `callee(arguments)`；
* 成员调用 `object.method(arguments)`；
* 对象字面量 `{ a: 1, b: expression }`；
* 数组字面量 `[a, b, c]`；
* 计算成员访问 `object[expression]`；
* 成员赋值 `object.property = value` 与 `object[expression] = value`。

V3 暂不实现：

* `let`、`const`、TDZ；
* 函数提升的完整规范语义；
* 默认参数、剩余参数、解构参数；
* arrow function；
* class；
* generator；
* async/await；
* `try/catch/finally`；
* 完整 prototype 链；
* getter/setter；
* 属性描述符完整算法；
* `delete`；
* `eval`；
* `arguments` 对象。

### 2.3 Runtime 与执行语义

V3 必须实现以下最小语义：

* 每次函数调用创建新的函数调用帧；
* 每个函数调用帧拥有当前执行环境；
* 参数按从左到右求值；
* 参数数量不足时，多余形参绑定为 `undefined`；
* 参数数量过多时，多余实参暂时忽略；
* `return expression` 立即结束当前函数；
* `return;` 返回 `undefined`；
* 函数体执行到末尾时返回 `undefined`；
* 函数声明在当前环境中绑定函数值；
* 函数表达式产生函数值；
* 函数值必须记录其声明时的外层环境，用于基础闭包；
* 读取变量时沿当前环境到外层环境链查找；
* 赋值时沿环境链查找已有绑定；
* 未声明变量读取仍返回 ReferenceError；
* 未声明变量赋值暂时返回 ReferenceError，不实现非严格模式隐式全局创建；
* 对象字面量创建普通对象；
* 数组字面量创建数组对象；
* `array.length` 返回数组元素数量；
* `object.property` 和 `object["property"]` 读取对象自有属性；
* `object.property = value` 写入对象自有属性并保留赋值结果；
* `obj.method()` 调用时传入 `this = obj`；
* 普通 `fn()` 调用时 `this = undefined`；
* Native Function 调用继续支持 V1/V2 Harness。

### 2.4 Bytecode

V3 新增指令建议：

```text
CreateFunction(function_index)
DeclareFunction(name_index, function_index)
LoadName(name_index)
StoreName(name_index)
DeclareLocal(name_index)
LoadThis
PushEnvironment
PopEnvironment
Return
ArrayCreate(element_count)
ObjectCreate(property_count)
GetElement
SetProperty(property_name_index)
SetElement
CallWithThis(argument_count)
```

说明：

* `CreateFunction` 从函数常量表创建函数值；
* `DeclareFunction` 在当前环境中绑定函数声明；
* `LoadName` 和 `StoreName` 沿环境链查找，不再只访问全局；
* `DeclareLocal` 在当前函数环境创建 `var` 绑定；
* `PushEnvironment` / `PopEnvironment` 用于函数调用和后续块级作用域扩展；
* `Return` 在函数内部表示返回，在顶层仍用于脚本最终完成值；
* `ArrayCreate(n)` 从栈上弹出 `n` 个元素，创建数组对象；
* `ObjectCreate(n)` 从栈上弹出 `n` 对属性名和值，创建普通对象；
* `GetElement` 支持计算成员访问；
* `SetProperty` 和 `SetElement` 保留赋值表达式结果；
* `CallWithThis(n)` 支持成员调用的最小 `this` 绑定。

V3 可以继续保留 V1/V2 的 `Call(n)`，但它只表示普通函数调用，不携带接收者。

### 2.5 Completion

V3 必须引入统一 Completion 模型，至少覆盖：

```text
Normal(value)
Return(value)
Throw(value)
Break
Continue
```

约束：

* 表达式产生 `Normal(value)`；
* 普通语句正常结束产生 `Normal(undefined)` 或保持脚本完成值；
* `return` 只能在函数体内被消费；
* 函数调用遇到 `Return(value)` 时转换为调用表达式的 `Normal(value)`；
* 顶层遇到 `Return` 必须是前端或编译阶段错误；
* `throw` 继续沿用 V2 的执行错误传播；
* `break` 和 `continue` 继续只由循环编译上下文消费；
* 不得在 VM、Compiler、Runtime 各自实现三套互不兼容的控制流返回类型。

## 3. 四组分工

### A. 前端组：`lexer/`、`ast/`、`parser/`

负责：

* 扩展 Token：`function`、`return`、`[`、`]`；
* 扩展 AST：函数声明、函数表达式、参数、函数体；
* 启用 `Return` 语句解析；
* 启用对象字面量和数组字面量解析；
* 启用计算成员访问；
* 区分普通调用和成员调用的 AST 结构；
* 检查函数外 `return`；
* 为参数列表、缺失右括号、函数体缺失、对象字面量语法错误添加负向测试。

A 组测试只验证 Token、AST 和错误 Span，不依赖编译器或 VM。

### B. 编译器组：`bytecode/`

负责：

* 扩展函数常量表；
* 编译函数声明、函数表达式和函数体；
* 编译参数绑定和局部 `var`；
* 编译 `return`；
* 编译对象/数组字面量；
* 编译计算成员访问和成员赋值；
* 编译成员调用的 `this` 绑定；
* 扩展静态栈分析，使函数体、`return`、`throw`、`break`、`continue` 都有明确控制流边界；
* 使用手工 AST 测试，不依赖 Parser 或 VM。

### C. VM/Runtime 组：`vm/`、`runtime/`、`builtins/`

负责：

* 实现函数值表示；
* 实现调用帧；
* 实现函数环境和外层环境捕获；
* 实现调用深度限制；
* 实现 `this` 传递；
* 实现对象创建、数组创建、属性读取、属性写入；
* 实现数组 `length`；
* 保证函数返回后操作数栈和环境栈恢复；
* 使用手工 Chunk 测试函数调用、递归限制、闭包、对象/数组操作。

### D. 集成测试组：`backend/`、CLI、Test262

负责：

* 增加 `NATIVE_V3_TESTS`；
* 增加 `--native-v3`；
* 保留 `--native-v1` 和 `--native-v2` 回归门；
* 输出 V1/V2/V3 分开的通过、失败、跳过和超时数量；
* 增加函数、对象、数组相关 Boa 差分测试；
* 更新 Native conformance 报告；
* 保证报告中 skipped 不计入 passed；
* 确认 Native 不支持功能时返回明确错误，而不是 Boa 回退。

## 4. 自有端到端测试

以下脚本必须在加入正式 Test262 前通过：

```javascript
function add(a, b) { return a + b; }
add(1, 2);                                           // 3
```

```javascript
function id(x) { return x; }
id("agent");                                         // "agent"
```

```javascript
function f() { return; }
f();                                                 // undefined
```

```javascript
function f() { var x = 1; return x + 2; }
f();                                                 // 3
```

```javascript
var x = 1;
function f() { var x = 2; return x; }
f() + x;                                             // 3
```

```javascript
function outer(x) {
  function inner(y) {
    return x + y;
  }
  return inner(2);
}
outer(1);                                            // 3
```

```javascript
var obj = { a: 1, b: 2 };
obj.a + obj["b"];                                    // 3
```

```javascript
var arr = [1, 2, 3];
arr[0] + arr.length;                                 // 4
```

```javascript
var obj = { x: 1 };
obj.x = 5;
obj.x;                                               // 5
```

```javascript
var obj = {
  value: 7,
  get: function () { return this.value; }
};
obj.get();                                           // 7
```

还必须覆盖错误：

```text
return 1;                         -> ParseError 或 CompileError
function f( { }                   -> ParseError
function f(a, a) {}               -> strict 模式 ParseError 或暂时明确 Unsupported
missing();                        -> ReferenceError / TypeError
var f = 1; f();                   -> TypeError
while (true) { function f(){} }   -> 仍受 loop_limit 约束
递归无出口                         -> RuntimeLimit / CallStackLimit
```

## 5. 固定 Test262 候选对象

V3 固定清单应优先选择只依赖函数、return、对象/数组字面量和基础成员访问的文件。建议先建立候选清单，逐个本地验证依赖后再冻结。

### 5.1 函数与 return 候选门

```text
test/language/statements/function/S13_A1.js
test/language/statements/function/S13_A4_T1.js
test/language/statements/function/S13_A5_T1.js
test/language/statements/return/S12.9_A1.js
test/language/statements/return/S12.9_A3.js
```

### 5.2 对象与数组候选门

```text
test/language/expressions/object/S11.1.5_A1.1_T1.js
test/language/expressions/object/S11.1.5_A2.1_T1.js
test/language/expressions/array/S11.1.4_A1.js
test/language/expressions/array/S11.1.4_A2.1.js
```

### 5.3 暂不纳入 V3 的相邻测试

* 依赖 `eval` 的函数测试；
* 依赖 `arguments` 对象的函数测试；
* 依赖完整函数提升规范的测试；
* 依赖 `Function.prototype` 的测试；
* 依赖 `new` 普通构造器和 prototype 的测试；
* 依赖 getter/setter 的对象字面量测试；
* 依赖稀疏数组、数组空洞和复杂 `length` 语义的测试；
* 依赖 `try/catch/finally` 的 `return` 与 `throw` 交互测试。

这些文件只能在依赖功能真实完成后加入，不能标记为跳过后计入通过。

## 6. 合并与验收顺序

1. A 组先提交 AST/Token 契约；
2. B 组提交 Opcode 与函数常量表契约；
3. C 组提交 Runtime 函数值、调用帧和环境 API；
4. D 组准备 ignored 的 V3 固定测试清单；
5. V3.1、V3.2、V3.3 分别合并；
6. 每批运行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
cargo run --release -- test262 --native-v1 --jobs 1 --verbose
cargo run --release -- test262 --native-v2 --jobs 1 --verbose
cargo run --release -- test262 --native-v3 --jobs 1 --verbose
```

V3 完成时，V1、V2、V3 固定清单都必须零回退、零跳过。
