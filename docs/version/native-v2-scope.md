# Native V2 控制流里程碑

本文档冻结 AgentJS Native V2 的功能范围、协作分工与验收标准。V2 在
V1 表达式流水线之上增加基础控制流和主动异常，使引擎能够执行真实分支与
循环程序，并扩大不经过 Boa 的 Test262 固定回归集。

共享类型和字节码契约见
[Native V2 共享接口规格](native-v2-interface.md)。

## 1. V2 完成标准

V2 必须做到：

- V1 的 6 个 Test262 文件继续全部通过；
- Native 路径支持块、`if/else`、条件表达式、`while`、`break`、
  `continue` 和 `throw`；
- `new Test262Error(message?)` 可用于 Test262 失败路径；
- 循环受 `RuntimeConfig::loop_limit` 限制；
- 非法的顶层 `break`/`continue` 返回语法或编译错误；
- 新增固定 Test262 清单在 default 和 strict 模式下通过；
- 不支持的 `for`、`do-while`、标签、`try/catch` 不得由 Boa 兜底。

V2 分三批合并：

1. **V2.1**：块、`if/else`、条件表达式；
2. **V2.2**：`while`、`break`、`continue`、循环预算；
3. **V2.3**：`throw`、最小 `new Test262Error(...)`、运行时负向测试。

## 2. 语言功能

### 2.1 Lexer

新增关键字：

```text
if else while break continue throw new typeof
```

新增分隔符：

```text
? :
```

Lexer 必须保留行终止符信息，使 `throw` 后出现换行时能够报告语法错误。

### 2.2 Parser 与 AST

新增或启用：

- `{ StatementList }`；
- `if (expression) statement`；
- `if (expression) statement else statement`，`else` 绑定最近的 `if`；
- `condition ? consequent : alternate`，右结合；
- `while (expression) statement`；
- 无标签 `break;` 与 `continue;`；
- `throw expression;`，`throw` 与表达式之间禁止行终止符；
- `new Test262Error(arguments)`；
- 一元 `typeof`；
- `var a, b = 1;` 多声明符形式。

V2 暂不实现标签、`do-while`、`for`、`switch`、`try/catch/finally`、
`let`/`const`、用户函数和通用构造器。

### 2.3 Bytecode

新增指令：

```text
Jump(target)
Throw
Construct(argument_count)
TypeOf
TypeOfGlobal(name)
```

控制流必须通过跳转实现，不能由 VM 直接解释 AST。所有跳转路径继续接受
`Chunk::validate()` 的栈深分析。

### 2.4 Runtime 与 VM

- `Jump` 无条件设置指令指针；
- 向后跳转消耗一次循环预算，耗尽后返回 `RuntimeLimit`；
- `Throw` 弹出一个值并结束本次执行；
- 抛出最小 Test262 Error 值时映射为 `FailureKind::Test262`；
- 其他值暂映射为普通运行时异常，并保留可显示的值；
- `typeof` 至少覆盖 V2 已有类型和未声明标识符；
- 每次执行和错误退出后都必须清理临时操作数栈。

## 3. 四组分工

### A. 前端组：`lexer/`、`ast/`、`parser/`

- 先提交 AST/Token 契约变更；
- 实现块、分支、循环、跳转语句和条件表达式；
- 跟踪 `break`/`continue` 是否处于循环内；
- 为 dangling-`else`、缺少括号、`throw\nvalue` 添加负向测试；
- 只验证 AST 和错误 Span，不依赖编译器。

### B. 编译器组：`bytecode/`

- 扩展 Opcode、跳转回填和栈效果；
- 编译 `if`、条件表达式与 `while`；
- 使用循环上下文记录 `break` 目标与 `continue` 目标；
- 编译 `throw` 和最小构造调用；
- 扩展 CFG 分析：无条件跳转没有 fallthrough，`Throw` 是终止指令；
- 测试只使用手工 AST。

### C. VM/Runtime 组：`vm/`、`runtime/`、`builtins/`

- 执行 `Jump`、`Throw`、`Construct`、`TypeOf`；
- 接入循环预算与错误清理；
- 提供最小 Test262 Error 值和构造器；
- 用手工 Chunk 测试正常跳转、死循环限制、异常传播；
- 不调用 Parser 或 Boa。

### D. 集成测试组：`backend/`、CLI、Test262

- 增加 `NATIVE_V2_TESTS` 和 `--native-v2`；
- 保留 `--native-v1` 回归门；
- 输出 V1/V2 分开的通过、失败、跳过和超时数量；
- 增加 Boa 差分测试，但规范与 Test262 仍是最终依据；
- 每批合并后更新 Native conformance 报告。

## 4. 自有端到端测试

以下脚本必须在加入正式 Test262 前通过：

```javascript
var x = 0; if (true) { x = 1; } else { x = 2; } x; // 1
var x = false ? 1 : true ? 2 : 3; x;               // 2
var i = 0; while (i < 5) { i = i + 1; } i;         // 5
var i = 0; while (true) { i = 1; break; } i;        // 1
var i = 0; while (i < 3) {
  i = i + 1;
  if (i === 2) continue;
}
i;                                                   // 3
throw new Test262Error("expected");                  // Test262 failure
```

还必须覆盖：

```text
break;             -> ParseError 或 CompileError
continue;          -> ParseError 或 CompileError
throw
1;                 -> ParseError（禁止换行）
while (true) {}     -> RuntimeLimit
```

## 5. 固定 Test262 对象

### 5.1 V2 核心门

这些文件只依赖 V1、V2 和最小 Test262 Error 构造：

```text
test/language/statements/if/empty-statement.js
test/language/statements/if/S12.5_A1_T1.js
test/language/statements/if/S12.5_A1.1_T1.js
test/language/statements/if/S12.5_A12_T1.js
test/language/statements/if/S12.5_A12_T2.js
test/language/statements/if/S12.5_A12_T3.js
test/language/statements/if/S12.5_A12_T4.js
test/language/expressions/conditional/S11.12_A3_T4.js
test/language/expressions/conditional/S11.12_A4_T4.js
test/language/statements/while/S12.6.2_A1.js
test/language/statements/while/S12.6.2_A4_T1.js
test/language/statements/if/S12.5_A6_T1.js
test/language/statements/if/S12.5_A6_T2.js
test/language/statements/if/S12.5_A8.js
test/language/statements/while/S12.6.2_A6_T1.js
```

其中 `while/S12.6.2_A1.js` 需要多 `var` 声明符，
`while/S12.6.2_A4_T1.js` 还需要 `typeof` 与 `break`。

### 5.2 Throw 运行时负向门

以下文件通过“预期抛出 Test262Error”验证 `throw` 和注释后的源码继续执行：

```text
test/language/line-terminators/comment-single-lf.js
test/language/line-terminators/comment-single-cr.js
test/language/line-terminators/comment-single-ls.js
test/language/line-terminators/comment-single-ps.js
test/language/line-terminators/comment-multi-lf.js
test/language/line-terminators/comment-multi-ls.js
test/language/line-terminators/comment-multi-ps.js
```

### 5.3 暂不纳入 V2 的相邻测试

- `cptn-*.js` 多数依赖 `eval` 和完整 Completion Value；
- `throw/S12.13_*.js` 多数依赖 `try/catch`；
- labeled `break`/`continue` 测试依赖标签；
- 包含 `new Boolean/Number/String` 的 conditional 测试依赖包装对象；
- `while` 的复杂 continue 测试依赖 `eval`、字符串方法或自增运算。

这些文件只能在依赖功能真实完成后加入，不能标记为跳过后计入通过。

## 6. 合并与验收顺序

1. A 组提交共享 AST/Token 契约；
2. B/C 组基于同一契约并行开发；
3. D 组先准备 ignored 的固定测试清单；
4. V2.1、V2.2、V2.3 分别合并，不使用一个超大 PR；
5. 每批运行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
cargo run --release -- test262 --native-v2 --jobs 1 --verbose
```

V2 完成时，V1 与 V2 固定清单都必须为零回退、零跳过。
