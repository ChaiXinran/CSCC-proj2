# 前端 V1 实现说明

本文档记录 AgentJS 自研引擎 **前端部分**(`src/lexer/`、`src/ast/`、`src/parser/`)
在 Native V1 表达式里程碑中的实现成果、设计取舍与后续计划。

- 模块边界见 [interface-spec.md](interface-spec.md)。
- V1 语言范围见 [native-v1-scope.md](native-v1-scope.md)。

## 1. 职责边界

前端是接口规格中的 **A 部分**:

| 项 | 内容 |
|---|---|
| 所有目录 | `src/lexer/`、`src/ast/`、`src/parser/` |
| 输入 | UTF-8 源码 `&str` |
| 输出 | `Program`(AST) |
| 公共接口 | `SourceParser::parse_source` |
| 错误类别 | `NativeError::Lex`、`NativeError::Parse`(均携带 `Span`) |

前端不生成字节码,不引用 VM、Heap、Boa 或 QuickJS 类型;输出的 AST 拥有自己的数据,
不持有调用者传入的 `&str`。

## 2. 已实现内容

### 2.1 词法分析器(Lexer)

`src/lexer/mod.rs`、`src/lexer/cursor.rs`、`src/lexer/token.rs`

- 空白与行终止符;V1 不做 ASI,行终止符视为普通空白。
- `//` 行注释与 `/* ... */` 块注释(块注释未闭合报 `LexError`)。
- ASCII 标识符(允许 `$`、`_`)。
- 十进制数字:整数、小数(含 `.5`)、指数(`1e3`、`2.0e-2`)。
- 单/双引号字符串及基础转义(`\n \t \r \b \f \v \0 \\ \' \"`、行续接、其余为恒等转义)。
- 关键字:`var`、`true`、`false`、`null`。
- 分隔符:`( ) { } ; , .`。
- 运算符:`+ - * / % ! = === !== < <= > >= && ||`。

实现要点:

- **最长匹配(maximal munch)**:运算符按"长在前、短在后"的固定表线性扫描,
  保证 `===` 不会被切成 `==`+`=`。参考了 QuickJS 的扫描思路。
- **除法与注释的区分**:注释在词法 trivia 阶段优先消费,因此 `a/b` 的 `/`
  一定是除法运算符,`a//b` 的 `//` 一定是注释。
- 所有 Token 携带半开字节区间 `Span`,EOF 满足 `start == end == source.len()`。

### 2.2 抽象语法树(AST)

`src/ast/expression.rs`、`src/ast/statement.rs`、`src/ast/mod.rs`

V1 涉及的节点:`Program`、空语句、表达式语句、`var` 声明、字面量、标识符、
分组、一元(`+ - !`)、二元算术/比较/严格相等、短路逻辑、简单赋值、成员访问、调用。

**本次对共享契约的修改**(详见 §4):

- `BinaryOperator` 新增 `StrictNotEqual`、`LessThanOrEqual`、`GreaterThanOrEqual`。
- 新增 `Expression::Logical` 节点与 `LogicalOperator { And, Or }`,
  专门表示短路的 `&&` / `||`。

### 2.3 语法分析器(Parser)

`src/parser/mod.rs`、`src/parser/statement.rs`、`src/parser/expression.rs`

- 递归下降驱动语句,**Pratt 优先级爬升**处理表达式(参考 Boa)。
- 优先级阶梯(由低到高):

  ```text
  赋值 =            右结合,单独处理
  ||                level 1
  &&                level 2
  === !==           level 3
  < <= > >=         level 4
  + -               level 5
  * / %             level 6
  一元 + - !         前缀,右结合
  调用 / 成员         后缀,最高
  primary
  ```

- 二元/逻辑运算左结合(递归时 `precedence + 1`),一元与赋值右结合。
- 赋值目标仅允许标识符与成员表达式,否则报 `ParseError`。
- 语句终止符为显式 `;` 或 EOF;V1 不做换行 ASI。

## 3. 验证

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```

均通过。测试覆盖:

- **单元测试**:词法(注释、数字、字符串、最长匹配、除法 vs 注释、Span)、
  语法(优先级、结合性、逻辑节点、成员/调用、赋值、负向用例)。
- **集成测试** `tests/frontend.rs`:
  - 真实解析 V1 必过的 6 个 Test262 文件:
    `multiplication/division/modulus` 的 `line-terminator.js`、
    `division/no-magic-asi.js`、`unary-plus/11.4.6-2-1.js`、
    `unary-minus/11.4.7-4-1.js`;
  - 解析 scope §5 的 10 个端到端表达式;
  - 校验 `unterminated` → `Lex`、`1 +` → `Parse` 的错误类别。

按接口规格,编译器未完成前,前端测试只断言 `Program` 与错误类别,不验证运行时数值。

## 4. 契约变更说明(需其他组知晓)

`src/ast/` 属于 §10 列出的共享契约。本次扩展不可避免:`&&`、`||`、`!==`、`<=`、`>=`
是 V1 范围内的运算符,但原 AST 没有对应表示。

影响:

- 新增的 AST 变体经 `ast/mod.rs` 与 `contracts.rs` 重新导出。
- 当前 `bytecode/compiler.rs` 仍是 stub(对非空程序统一报错),未做 AST 穷尽匹配,
  因此本次扩展不破坏编译。
- **编译器组接入时必须显式处理** `Logical` 节点及三个新增的 `BinaryOperator` 变体。

## 5. 后续语言特性(前端待办)

以下为 V1 明确延期、且属于前端职责的工作,需与编译器/VM 组对齐后再排期
(因为涉及共享 AST):

- **紧接 V1**:`if`、`throw` 语句(`If`/`While` 的 AST 壳已存在,`throw` 节点待加)。
- `let` / `const` 声明;`while`、`for` 循环。
- 用户函数 `function`、`return`、调用形参;`new`、`this`。
- 对象字面量 `{a: 1}`、数组字面量 `[1, 2]`(AST 节点已存在,parser 未接)。
- 更多运算符:宽松相等 `== !=`、位运算、`**`、`??`、`++ --`、复合赋值、三元 `?:`。
- 模板字符串、正则字面量、Unicode 标识符、BigInt 等词法扩展。

每一批新增都对应一组新的 Test262 目录,应按"先词法 Token、再 AST 节点、
再 parser 产生式"的顺序推进,并补充对应的正向/负向测试。
