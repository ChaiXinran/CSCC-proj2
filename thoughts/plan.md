比赛的核心目标不是“功能看起来很多”，而是形成可证明的自研引擎：

> Native 路径完全不依赖 Boa，Test262 真实通过率超过 60%，并有可复现的性能、稳定性和架构证据。

当前 Boa 的 87.31% 报告不能作为最终成绩；Native 目前只有固定 6 个测试。因此后续要围绕 Native Test262 增长规划。

# 一、最终交付目标

比赛版本必须同时满足：

- Native 成为默认执行后端。
- 最终二进制不链接 Boa 或 QuickJS。
- Native Test262 通过率超过 60%，跳过不能算通过。
- Windows、Linux 至少稳定构建。
- 有完整 Lexer、Parser、AST、Bytecode、VM、Runtime、GC、Builtins。
- 有超时、栈深、递归和内存限制。
- 有与 Boa、QuickJS 的性能对比。
- 有架构、接口、测试、性能、依赖说明和答辩材料。

建议把内部目标定为 65%，为环境差异留余量。

# 二、开发里程碑

| 阶段 | 核心能力 | 主要解锁 |
|---|---|---|
| V1 已完成 | 基础表达式、全局变量、调用 | 6 个 Test262 |
| V2 | 块、`if`、循环、`throw` | 基础控制流测试 |
| V3 | 函数、局部作用域、闭包 | 大量 language/harness 测试 |
| V4 | 对象、属性描述符、原型、数组 | Object/Array 测试 |
| V5 | 异常、类、完整作用域语义 | 现代语言测试 |
| V6 | 核心 Builtins | 提升总体通过率 |
| V7 | GC、限制、优化 | 比赛稳定性与性能 |
| Release | 全量 Test262、基准、文档 | 最终交付 |

## V2：控制流与异常

实现：

- 块语句、`if/else`
- `while`、`do-while`、`for`
- `break`、`continue`
- `throw`
- 条件表达式 `?:`
- `typeof`、`void`
- `++`、`--`、复合赋值

字节码增加：

```text
Jump
Throw
```

此阶段重点建立 Completion/异常传播模型，避免以后实现 `try/finally` 时推翻 VM。

## V3：函数与作用域

实现：

- 函数声明和函数表达式
- 参数、局部变量、调用帧
- `return`
- `this`
- 词法环境和闭包
- `let`、`const`、块作用域、TDZ
- 递归限制

这一阶段是整个项目最重要的架构节点。完成后才能运行正式 `assert.js`，逐步取消最小 Harness。

## V4：对象与数组

实现：

- 对象、数组字面量
- 计算属性访问
- 属性读写
- Property Descriptor
- 原型链
- getter/setter
- `new`
- 方法调用的 `this` 绑定
- 数组 `length` 语义

优先完成 `Object` 和 `Array` 的基础内建方法。

## V5：异常与现代语法

具体范围、共享接口和并行开发规则已经冻结在：

- `docs/native-v5-scope.md`
- `docs/native-v5-interface.md`
- `docs/native-v5-team-plan.md`

第一批优先完成 Completion、`try/catch/finally`、`switch`、`let/const` 与
TDZ。箭头函数、解构、展开/剩余参数和 class 作为后续 V5 扩展，避免与
尚在修复的 V4 Runtime/Builtins 同时修改高冲突文件。

实现：

- `try/catch/finally`
- Error 对象体系
- `switch`
- `for-in`、`for-of`
- 展开、剩余参数
- 解构
- 箭头函数
- 类、继承、`super`

模块、异步函数和 Generator 可以后置，先根据 Test262 增量决定投入。

## V6：Builtins

建议顺序：

1. Object、Function
2. Array
3. String
4. Number、Math、Boolean
5. Error、JSON
6. Map、Set
7. RegExp
8. Date
9. Promise

Temporal、Intl、TypedArray、Atomics、Module 成本很高，应根据距离 60% 的差额决定是否实现。

# 三、四组并行分工

- A 前端组：Token、Parser、AST、语法错误与 Span。
- B 编译器组：Opcode、控制流回填、函数 Chunk、静态栈分析。
- C VM/Runtime：调用帧、作用域、对象、异常、GC、Builtins。
- D 集成组：Test262、CLI、差分测试、CI、性能和报告。

共享类型变更必须先单独合并：

```text
AST → Opcode/Chunk → JsValue/Context → 各组实现
```

每组尽量只修改自己的目录，减少再次出现 rebase 冲突。

# 四、Test262 增长机制

每个功能都必须遵循：

1. 先选对应 Test262 文件。
2. 添加固定回归清单。
3. 完成模块单测。
4. 完成 Native 端到端测试。
5. 运行对应目录。
6. 记录新增通过和回退数量。

建议增加测试分层：

```text
native-smoke      每次提交，几十项
native-v2         每个里程碑固定清单
native-language   每日运行
native-builtins   每日分片
native-full       每周运行
```

报告必须分别记录：

- 总数
- Passed
- Failed
- Skipped
- Panic/Timeout
- 相比上次新增通过和回退

# 五、性能与工程计划

语义稳定后再优化：

- 字符串驻留，名称不重复存入常量池。
- 紧凑字节码编码。
- VM 栈和调用帧预分配。
- 属性访问 inline cache。
- 数组密集元素快速路径。
- 数字算术快速路径。
- 编译后 Chunk 缓存。
- Mark-Sweep GC 与分配额度。
- Runtime 重置和 isolate 池。

性能报告至少包含：

- 冷启动时间
- 热执行吞吐量
- 二进制大小
- 峰值内存
- JetStream 子集
- 与 Boa、QuickJS 的同机比较

# 六、比赛前质量门槛

每次合并：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
```

最终发布前还要：

- `rg "boa_" src/` 审计 Native 依赖。
- 将 Boa 改为可选开发/基线功能，默认构建不链接 Boa。
- Linux、Windows CI 全通过。
- 全量 Native Test262 固定版本运行。
- 所有失败、跳过和超时如实报告。
- 清理过期文档和“Native Unsupported”描述。

# 七、优先级原则

必须优先：

1. 函数与作用域
2. 对象与属性
3. 异常
4. Array/Object/String 等核心 Builtins
5. Test262 自动统计

暂缓：

- JIT
- WebAssembly
- Intl
- Temporal
- Atomics
- 完整 Module Loader
- 高级 GC 优化

当前下一步是先合并 V5 共享契约，再由 A、B、D 组分别开展前端、字节码和
Test262 准备；C 组等待 V4 修复基线合并后实现 Completion、异常处理和词法环境。
