## 结论

这是 TC39 官方的 **Test262 ECMAScript 一致性测试套件**。它不是测试某个特定 JS 引擎，而是验证 V8、SpiderMonkey、JavaScriptCore、QuickJS 等引擎是否按照 ECMAScript 标准实现语言行为。

当前检出版本包含约：

- 53,658 个 JS 文件
- 其中约 53,379 个独立测试，另有 279 个模块依赖文件
- 语言测试：23,979
- 内建对象测试：23,646
- 国际化测试：3,341
- Annex B 兼容测试：1,086
- 暂存/新提案测试：1,490

仓库说明见 [README.md](D:/00_OS/CSCC/test262/README.md:4)。

## 主要测试功能

### 1. 词法分析和语法解析

测试 JS 引擎前端是否正确识别：

- 标识符、关键字、保留字
- 数字、字符串、模板字符串、正则表达式字面量
- 注释、空白符、Unicode、行终止符
- 自动分号插入 ASI
- 合法与非法语法
- Early Error 静态语义错误

例如非法参数、重复声明、错误的 `super`、私有字段语法，应当在解析阶段抛出 `SyntaxError`。

### 2. 表达式与运算符

这是语言测试中最大的一部分，约 11,101 个测试，包括：

- 算术、比较、位运算
- `typeof`、`delete`、`instanceof`、`in`
- 赋值与逻辑赋值
- 可选链 `?.`
- 空值合并 `??`
- 展开语法、剩余参数
- 解构赋值
- `new`、函数调用和属性访问
- `yield`、`await`
- 运算顺序、副作用和类型转换

例如 [optional-call-preserves-this.js](D:/00_OS/CSCC/test262/test/language/expressions/optional-chaining/optional-call-preserves-this.js) 验证可选调用不会错误丢失 `this`。

### 3. 语句和控制流

约 9,337 个测试，覆盖：

- `if`、`switch`
- `for`、`for-in`、`for-of`、`for-await-of`
- `while`、`do-while`
- `try/catch/finally`
- `break`、`continue`、`return`、`throw`
- 标签语句
- 类声明与函数声明
- `using` 和显式资源管理等新功能

重点测试完成记录、异常传播、迭代器关闭和求值顺序。

### 4. 作用域与执行上下文

覆盖引擎内部非常核心的语义：

- `var`、`let`、`const`
- 块级作用域和暂时性死区
- 闭包与名称解析
- 全局、函数、模块、`eval` 执行上下文
- 严格模式与非严格模式
- `this`、`arguments`、`new.target`
- Realm 跨上下文行为

普通测试默认需要分别以严格和非严格模式运行，具体规则见 [INTERPRETING.md](D:/00_OS/CSCC/test262/INTERPRETING.md:128)。

### 5. 函数、类和对象模型

测试：

- 普通函数、箭头函数
- Generator、Async Function、Async Generator
- 类、继承、`super`
- 公有字段、私有字段和私有方法
- 静态字段和静态初始化块
- 属性描述符
- 原型链
- getter/setter
- `Proxy` 和 `Reflect`
- 对象可扩展性、密封和冻结

这些测试会验证属性是否可写、可枚举、可配置，以及内部抽象操作是否严格遵守规范。

### 6. 标准内建对象和 API

规模最大的几组包括：

| 内建功能 | 测试数量 |
|---|---:|
| Temporal | 4,603 |
| Object | 3,411 |
| Array | 3,081 |
| RegExp | 1,879 |
| TypedArray | 1,446 |
| String | 1,223 |
| Promise | 703 |
| Date | 594 |
| DataView | 561 |
| Iterator | 514 |
| Function | 509 |
| Atomics | 390 |
| Set | 383 |
| Proxy | 311 |

此外还包括：

- `Map`、`WeakMap`、`WeakSet`
- `Symbol`、`BigInt`
- `JSON`
- `Math`、`Number`
- `ArrayBuffer`、`SharedArrayBuffer`
- `Error`、`AggregateError`
- `WeakRef`、`FinalizationRegistry`
- `DisposableStack`
- `ShadowRealm`

### 7. Promise、异步任务与模块

测试：

- Promise 状态和 thenable 解析
- 微任务执行顺序
- `async/await`
- 异步迭代
- 动态 `import()`
- ES Module 解析、链接和求值
- 循环模块依赖
- Top-level await
- JSON Module、import attributes
- 模块解析错误和运行时错误

例如 Promise 测试会明确检查微任务回调顺序，而不只是检查最终值。

### 8. 二进制数据和并发内存模型

覆盖：

- `ArrayBuffer`
- 可调整及可转移 ArrayBuffer
- `DataView`
- 所有 TypedArray
- `SharedArrayBuffer`
- `Atomics`
- 多 agent 并发
- 内存读写、等待和唤醒语义

测试运行器需要实现 `$262.agent` 等宿主接口，相关要求见 [INTERPRETING.md](D:/00_OS/CSCC/test262/INTERPRETING.md:38)。

### 9. 国际化 ECMA-402

`test/intl402` 测试：

- `Intl.NumberFormat`
- `Intl.DateTimeFormat`
- `Intl.Collator`
- `Intl.Locale`
- `Intl.Segmenter`
- `Intl.PluralRules`
- `Intl.RelativeTimeFormat`
- `Intl.ListFormat`
- `Intl.DisplayNames`
- `Intl.DurationFormat`
- Temporal 的国际化格式

如果引擎不实现 ECMA-402，可以跳过这部分，见 [INTERPRETING.md](D:/00_OS/CSCC/test262/INTERPRETING.md:176)。

### 10. 兼容行为与未来特性

- `test/annexB`：浏览器历史遗留兼容行为，例如旧式函数声明和 RegExp 特性。
- `test/staging`：尚在快速推进或等待正式归类的功能。
- `features.txt`：Stage 3 及后续提案，例如装饰器、Temporal、ShadowRealm、资源管理、延迟模块导入等。

## 它如何判定引擎正确

每个测试带 YAML 元数据，可以要求：

- 在解析阶段抛出错误
- 在模块解析阶段抛出错误
- 在运行阶段抛出指定异常
- 以模块方式运行
- 仅严格或仅非严格模式运行
- 等待异步测试完成
- 加载指定辅助脚本
- 仅在引擎支持某项 feature 时运行

每个测试需要在独立 Realm 中执行，避免测试之间相互污染，见 [INTERPRETING.md](D:/00_OS/CSCC/test262/INTERPRETING.md:21)。

## 不属于它的测试范围

Test262主要测试“规范一致性”，通常不负责：

- JIT 编译性能和性能回归
- GC 性能、内存占用
- 引擎崩溃和内存安全漏洞
- Debugger/Profiler 接口
- DOM、CSS、WebGL 等浏览器 API
- Node.js 的 `fs`、`http`、`Buffer` 等平台 API
- 引擎命令行和嵌入 API

所以更准确地说，它测试的是：

> JS 引擎从解析器、执行语义、对象模型、标准库、模块系统、异步机制到并发内存模型的 ECMAScript 标准符合度，而不是引擎的性能或宿主平台功能。