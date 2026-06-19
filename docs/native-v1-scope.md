# Native V1 表达式里程碑

本文档定义 AgentJS 自研引擎第一版可运行范围。V1 的目标不是追求 Test262
目录通过率，而是完成第一条不经过 Boa 的端到端执行链，并真实通过一组精选
Test262 文件。

## 1. 完成标准

V1 必须实现：

```text
JavaScript 源码
  -> Native Lexer
  -> Native Parser / AST
  -> Native Bytecode Compiler
  -> Native VM / Runtime
  -> JsValue / Test262 结果
```

验收时必须确认：

- Native 路径没有调用 Boa；
- CLI 和 Test262 Runner 可以显式选择 `native`；
- 同一脚本在独立 Runtime 中没有状态泄漏；
- 精选 Test262 用例在 default 和 strict 两种模式下通过；
- 不支持的语法返回明确错误，而不是 panic 或 Boa 兜底。

## 2. V1 语言范围

### 2.1 Lexer

必须支持：

- ECMAScript 常用空白和行终止符；
- `//` 与 `/* ... */` 注释；
- ASCII 标识符；
- 十进制整数和小数；
- 单引号、双引号字符串及基础转义；
- 关键字：`var`、`true`、`false`、`null`；
- 分隔符：`(){};,.`;
- 运算符：`+ - * / % ! = === !== < <= > >= && ||`。

暂不支持 Unicode 标识符、BigInt、模板字符串、正则字面量和数字分隔符。

### 2.2 AST 与 Parser

必须支持：

- `Program`；
- 空语句和表达式语句；
- `var name;` 与 `var name = expression;`；
- 数字、字符串、布尔、`null` 和标识符；
- 分组表达式；
- 一元 `+`、`-`、`!`；
- 算术 `+`、`-`、`*`、`/`、`%`；
- 严格相等 `===`、`!==`；
- 比较 `<`、`<=`、`>`、`>=`；
- 短路逻辑 `&&`、`||`；
- 简单赋值 `identifier = expression`；
- 成员访问 `object.property`；
- 调用 `callee(arguments)`。

Parser 必须使用明确优先级：

```text
调用/成员
  > 一元
  > 乘除取模
  > 加减
  > 比较
  > 严格相等
  > &&
  > ||
  > 赋值
```

V1 暂不实现 `if`、循环、函数、数组/对象字面量、`new`、`throw`、宽松相等、
自增、自减、复合赋值和条件运算符。

### 2.3 Bytecode

建议第一版指令：

```text
Constant
Pop
DeclareGlobal
LoadGlobal
StoreGlobal
UnaryPlus
Negate
LogicalNot
Add
Subtract
Multiply
Divide
Remainder
StrictEqual
StrictNotEqual
LessThan
LessThanOrEqual
GreaterThan
GreaterThanOrEqual
JumpIfFalse
JumpIfTrue
GetProperty
Call
Return
ReturnUndefined
```

`&&` 和 `||` 必须通过跳转实现短路，不能提前计算右操作数。每条执行路径必须
保持确定的栈高度。

### 2.4 Runtime 与 VM 语义

必须实现以下基础抽象操作：

- `ToBoolean`：`undefined`、`null`、`false`、`0`、`-0`、`NaN`、空字符串为假；
- `ToNumber`：支持 Number、Boolean、Null、Undefined 和基础字符串转换；
- `StrictEqualityComparison`，包括 `NaN !== NaN` 和 `+0 === -0`；
- Number 的 IEEE-754 算术，包括 `NaN`、Infinity、除零和负零；
- `+` 的数字加法和字符串连接；
- 全局 `var` 绑定、读取和赋值；
- 全局基础值 `undefined`、`NaN` 和 `Infinity`；
- VM 每次执行前清理临时操作数栈。

对象到原始值转换、Symbol、BigInt 和完整属性算法不属于 V1。

## 3. 最小 Test262 Harness

完整 `assert.js` 和 `sta.js` 本身依赖函数、对象模型、异常、JSON 等大量能力，
不适合作为 V1 前置条件。Native Test262 模式应由宿主直接注册：

```text
assert.sameValue(actual, expected, message?)
assert.notSameValue(actual, unexpected, message?)
Test262Error
```

`assert` 可以是一个带 Native Function 属性的普通对象。断言失败必须产生
`FailureKind::Test262`。该最小 Harness 只用于尚未支持完整 Harness 的 Native
阶段，报告中必须明确标记。

不得修改 Test262 测试源码，也不得把断言结果交给 Boa。

## 4. 首批真实 Test262 目标

V1 必须通过以下文件：

```text
test/language/expressions/multiplication/line-terminator.js
test/language/expressions/division/line-terminator.js
test/language/expressions/division/no-magic-asi.js
test/language/expressions/modulus/line-terminator.js
test/language/expressions/unary-plus/11.4.6-2-1.js
test/language/expressions/unary-minus/11.4.7-4-1.js
```

这些用例覆盖：

- 注释、换行和分号；
- `var` 声明与全局变量；
- 运算符优先级和左结合；
- `* / %` 跨行解析；
- 一元正负号；
- 空字符串到数字转换；
- `-0` 的 SameValue 语义；
- `assert.sameValue` 的成员访问和调用。

通过这 6 个文件比“自己写一个 `1 + 2` 测试”更有价值，因为它同时验证了
Runner、Harness、Parser、Compiler、VM 和 Runtime 的链接。

## 5. 自有端到端测试

在运行 Test262 前，以下脚本必须通过 NativePipeline：

```javascript
1 + 2 * 3;                       // 7
(1 + 2) * 3;                     // 9
var x = 18; x / 2 / 3;           // 3
+"";                             // 0
-"";                             // -0
1 === 1;                         // true
NaN === NaN;                     // false
false && missingName;            // false，不读取右侧
true || missingName;             // true，不读取右侧
"agent" + 262;                   // "agent262"
```

还必须覆盖错误：

```text
"unterminated          -> LexError
1 +                    -> ParseError
missingName            -> ReferenceError/Execute error
无效常量索引            -> VmError
```

## 6. 四部分任务分解

### A. 前端组

- 完成 Token 扫描和 Span；
- 用 Pratt Parser 实现上述优先级；
- 输出稳定的 AST 快照；
- 负责词法/语法负向测试。

### B. 编译器组

- 扩展 Opcode 和常量池；
- 编译变量、运算符、短路跳转、成员访问和调用；
- 编译器测试只使用手工 AST；
- 校验常量索引和栈效果。

### C. VM/Runtime 组

- 实现基础类型转换与运算语义；
- 实现全局 Environment；
- 提供最小普通对象和 Native Function；
- 注册 Native `assert.sameValue`；
- 使用手工 Chunk 独立测试每条指令。

### D. 集成与测试组

- 为 CLI 增加 `--backend boa|native`；
- 为 `RunnerOptions` 增加后端选择；
- 增加 Native 最小 Harness 模式；
- 建立上述 6 个文件的固定回归清单；
- 输出 Boa 与 Native 分开的结果。

## 7. 明确延期

以下功能不应阻塞 V1：

- `let`、`const`、TDZ 和块级作用域；
- `if`、循环和异常语句；
- 用户定义函数和闭包；
- 对象/数组字面量和原型链；
- `new`、`this` 和构造器；
- 宽松相等、位运算、指数、空值合并；
- BigInt、Symbol、RegExp、Promise、Module；
- 完整 `assert.js`、`sta.js` 和附加 Harness。

完成 V1 后，下一阶段应增加 `if`、`throw`、`Test262Error` 构造和字符串连接，
从而开始运行旧 Sputnik 风格的算术目录测试。
