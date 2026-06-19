# 前端 V2 实现说明

本文档记录 AgentJS 自研引擎 **前端部分**(`src/lexer/`、`src/ast/`、`src/parser/`)
在 Native V2 控制流里程碑中的实现成果、设计取舍与后续计划。

- 模块边界见 [interface-spec.md](interface-spec.md) 与 [native-v2-interface.md](native-v2-interface.md)。
- V2 语言范围与分工见 [native-v2-scope.md](native-v2-scope.md)。
- V1 实现见 [frontend-v1.md](frontend-v1.md)。

## 1. 职责边界

前端是 V2 分工中的 **A 组**:`src/lexer/`、`src/ast/`、`src/parser/`。

- 输入 UTF-8 源码 `&str`,输出 `Program`(AST)。
- 公共接口 `SourceParser::parse_source`;错误类别 `NativeError::Lex` / `NativeError::Parse`。
- 不生成字节码,不引用 VM、Heap、Boa。
- 按接口规格,**只验证 AST 与错误 Span,不依赖编译器**。

## 2. 已实现内容

### 2.1 词法分析器(Lexer)

`src/lexer/token.rs`、`src/lexer/mod.rs`

- 新增关键字:`if else while break continue throw new typeof`。
- 新增分隔符:`?` `:`(条件运算符)。
- **行终止符记录**:每个 `Token` 新增 `line_terminator_before: bool`,表示从上一个
  Token 结束到当前 Token 开始之间是否出现 ECMAScript 行终止符,**包括注释内部的换行**。
  词法的 trivia 跳过阶段统一计算该标志,parser 直接读取,不二次扫描源码或比较行号。

### 2.2 抽象语法树(AST)

`src/ast/statement.rs`、`src/ast/expression.rs`、`src/ast/mod.rs`

按 [native-v2-interface.md](native-v2-interface.md) §1 冻结契约:

- 新增结构体 `VariableDeclarator { name, initializer }`。
- `Statement::VariableDeclaration` 由原来的 `{ kind, name, initializer }` 迁移为
  `{ kind, declarations: Vec<VariableDeclarator> }`,**至少含一个声明符**;V1 的单声明
  自然变成长度 1 的列表。
- 新增语句:`Break`、`Continue`、`Throw(Expression)`。
- 新增表达式:`Conditional { test, consequent, alternate }`、
  `Construct { callee, arguments }`。
- `Block`、`If`、`While` 节点 V1 时已存在,本次由 parser **首次真正产生**。

### 2.3 语法分析器(Parser)

`src/parser/mod.rs`、`src/parser/statement.rs`、`src/parser/expression.rs`

新增语句产生式:

- `{ StatementList }` 块语句。
- `if (test) stmt` 与 `if (test) stmt else stmt`;`else` 在消费完 consequent 后**立即贪婪
  匹配**,因此总是绑定到最近一个未匹配的 `if`(dangling-else)。
- `while (test) body`。
- 无标签 `break;` / `continue;`。Parser 维护 `loop_depth` 计数器,进入 `while` 体 +1、
  退出 -1;在 `loop_depth == 0` 时遇到 `break`/`continue` 直接报 `ParseError`。
- `throw expression;`。`throw` 与表达式之间禁止行终止符:消费 `throw` 后若下一个 Token 的
  `line_terminator_before` 为真则报错。
- `var a, b = 1;` 多声明符;每个初始化器在 **assignment 级别**解析,使顶层逗号正确地分隔
  声明符而非被当作运算符。

新增表达式产生式:

- 条件运算符 `test ? consequent : alternate`,位于赋值与二元之间;两个分支按 assignment 级
  解析,因此**右结合**(`a ? b : c ? d : e` 解析为 `a ? b : (c ? d : e)`)。
- 一元 `typeof`(右结合,复用 `UnaryOperator::TypeOf`)。
- `new callee` 与 `new callee(args)`:callee 解析为成员表达式(只吃 `.`,不吃调用),
  其后的第一个括号归构造调用所有;结果仍流经后缀循环,故 `new X().y` 合法。

更新后的优先级阶梯(由低到高):

```text
赋值 =            右结合
条件 ?:           右结合
||                level 1
&&                level 2
=== !==           level 3
< <= > >=         level 4
+ -               level 5
* / %             level 6
一元 + - ! typeof  前缀,右结合
new / 调用 / 成员  后缀,最高
primary
```

## 3. 验证

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```

测试覆盖(均为前端隔离测试,不依赖编译器/VM):

- **词法单元测试**:V2 关键字与 `? :`、`throw\nx` 的换行标志、注释内换行的计入。
- **语法单元测试**(`src/parser/statement.rs`、`src/parser/expression.rs`):块、if/else、
  dangling-else、while+break/continue、循环外 break/continue 报错、throw 及其换行报错、
  条件表达式右结合、条件比二元更松、`typeof`、`new`(带/不带实参)、多声明符。
- **Token 驱动测试**(`src/parser/token_tests.rs`):全部手工构造 `Vec<Token>` 直接喂给
  `Parser`,完全绕过词法,对上述 V2 产生式逐条断言 AST 形状;换行报错用 `tok_nl` 注入
  `line_terminator_before`。
- **集成测试**(`tests/frontend.rs`):解析 native-v2-scope §4 的端到端控制流脚本;
  校验 `break;`、`continue;`、`throw\n1;`、孤立 `else` 均归 `NativeError::Parse`。

## 4. 契约变更说明(需其他组知晓)

本次改动属于 V2 共享契约(`src/lexer/token.rs`、`src/ast/*`、`src/contracts.rs`):

- `Token` 新增 `line_terminator_before`。`Token::new` 保持双参签名(标志默认 `false`),
  另提供 `Token::with_line_terminator_before`,因此既有手工构造 Token 的测试无需改动,
  仅词法负责记录真实换行。
- `Statement::VariableDeclaration` 改为 `Vec<VariableDeclarator>`。**这是破坏性变更**:
  编译器组(B)的 `src/bytecode/compiler.rs` 与 `tests/bytecode_contract.rs` 需相应适配。
  为保持仓库可编译、各组测试可运行,本次已对 B 组做**最小机械适配**(遍历 `declarations`
  调用既有的单声明逻辑),其语义归属仍属编译器组,后续由 B 组按需重写。
- 新增的 `Break`/`Continue`/`Throw`/`Conditional`/`Construct` 节点目前会落入编译器的
  `unsupported` 兜底分支(返回 `CompileError`),不破坏编译;**B 组接入 V2 时必须显式处理**
  这些节点及 `Jump`/`Throw`/`Construct`/`TypeOf`/`TypeOfGlobal` 指令。

## 5. 后续语言特性(前端待办)

V2 明确延期、且属前端职责的工作(均涉及共享 AST,需与 B/C 组对齐后排期):

- **V3**:函数声明/表达式、形参、`return`、`this`、`let`/`const` 与块作用域/TDZ。
- 循环扩展:`do-while`、`for`、`for-in`/`for-of`、标签语句与带标签 `break`/`continue`。
- `switch`、`try/catch/finally`。
- 对象/数组字面量(AST 壳已存在,parser 未接)、计算成员访问、展开/剩余、解构、箭头函数、类。
- 更多运算符:宽松相等 `== !=`、位运算、`**`、`??`、`++ --`、复合赋值、`void`。

推进顺序仍为"先词法 Token → 再 AST 节点 → 再 parser 产生式",每批补充正向/负向测试并对齐
对应 Test262 目录。
