# Native V2 共享接口规格

本文档定义 V2 控制流开发期间 A/B/C/D 四组共同遵守的类型与字节码契约。
它补充 `interface-spec.md`；发生冲突时，V2 新增部分以本文为准。

## 1. 共享 AST

建议先单独提交以下契约变更：

```rust
pub struct VariableDeclarator {
    pub name: String,
    pub initializer: Option<Expression>,
}

pub enum Statement {
    Empty,
    Expression(Expression),
    Block(Vec<Statement>),
    VariableDeclaration {
        kind: VariableKind,
        declarations: Vec<VariableDeclarator>,
    },
    If {
        test: Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    },
    While {
        test: Expression,
        body: Box<Statement>,
    },
    Break,
    Continue,
    Throw(Expression),
    // Return 保留给 V3 函数里程碑。
}

pub enum Expression {
    // V1 variants...
    Conditional {
        test: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    Construct {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
}
```

规则：

- `VariableDeclaration.declarations` 不得为空；
- V1 的单声明 `var x = 1` 迁移为长度为 1 的列表；
- `else` 总是属于最近一个尚未匹配的 `if`；
- Parser 可以在循环外直接拒绝 `break`/`continue`；
- 编译器仍须防御手工 AST 中的非法跳转语句；
- `Construct` 在 V2 只保证 `Test262Error`，但 AST 不写死具体名字。

Token 需要增加：

```rust
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub line_terminator_before: bool,
}
```

该字段表示从前一个 Token 结束到当前 Token 开始之间是否出现 ECMAScript
行终止符，包括注释内部的行终止符。Parser 用它拒绝
`throw /* line terminator */ expression`。不得通过重新扫描源码或比较行号实现
另一套规则。

## 2. 共享 Opcode

```rust
pub enum Instruction {
    // V1 instructions...
    Jump(usize),
    TypeOf,
    TypeOfGlobal(u16),
    Construct(u16),
    Throw,
}
```

栈效果：

| 指令 | required | pops | pushes |
|---|---:|---:|---:|
| `Jump` | 0 | 0 | 0 |
| `TypeOf` | 1 | 1 | 1 |
| `TypeOfGlobal` | 0 | 0 | 1 |
| `Construct(n)` | `n + 1` | `n + 1` | 1 |
| `Throw` | 1 | 1 | 0 |

`Throw` 是终止指令。`Jump` 只有目标后继，没有顺序 fallthrough。
`JumpIfFalse`/`JumpIfTrue` 保持 V1 的“观察但不弹出”契约。

`Chunk::patch_jump` 必须支持三种跳转。`jump_target()` 对 `Jump` 和条件跳转都
返回目标。

## 3. 固定降级规则

### 3.1 If

```text
test
JumpIfFalse(else_or_false_cleanup)
Pop
consequent
Jump(end)
else_or_false_cleanup:
Pop
alternate? 
end:
```

V2 中普通语句在边界处必须栈中性。两个分支汇合时栈深必须相同。

### 3.2 条件表达式

```text
test
JumpIfFalse(else)
Pop
consequent
Jump(end)
else:
Pop
alternate
end:
```

两条路径在 `end` 处都必须留下恰好一个表达式结果。

### 3.3 While

```text
loop_start:
test
JumpIfFalse(false_cleanup)
Pop
body
Jump(loop_start)
false_cleanup:
Pop
loop_end:
```

循环上下文：

```rust
struct LoopContext {
    continue_target: usize,
    break_jumps: Vec<usize>,
}
```

- `continue` 编译为 `Jump(continue_target)`；
- `break` 先生成占位 `Jump`，循环结束后统一回填到 `loop_end`；
- `break` 不得跳到 `false_cleanup`，因为正常循环体路径上已经弹出了条件值。

### 3.4 Throw

```text
expression
Throw
```

`Throw` 后没有控制流后继。位于不可达区域的后续指令允许存在，但静态栈分析
只分析可达路径。

## 4. Completion 与栈约束

V2 暂不实现 `eval` 所需的完整 ECMAScript Completion Record，但必须满足：

- 表达式继续产生一个值；
- 普通语句在编译边界处栈中性；
- 条件表达式汇合后产生一个值；
- `if`、块和循环不会泄漏测试条件或临时值；
- `break`、`continue`、`throw` 之后的不可达代码不参与栈深合流；
- 程序最终值继续沿用 V1 规则。

完整的 `normal/break/continue/return/throw` Completion Record 语义将在 V3
函数与 `try` 设计时统一扩展，不应在 V2 用多套临时返回类型实现。

## 5. Runtime 接口

循环预算必须属于当前 isolate，而不是全局变量。建议由 `NativeContext` 提供：

```rust
pub fn reset_execution_budget(&mut self, loop_limit: u64);
pub fn consume_loop_iteration(&mut self) -> Result<(), VmError>;
```

每次 `RuntimeBackend::eval` 开始时重置预算。VM 执行向后 `Jump` 时消费预算。
错误退出后，下次执行不得继承旧预算或操作数栈。

V2 最小错误值建议使用明确变体，而不是用字符串猜测：

```rust
pub enum NativeErrorKind {
    Error,
    Test262,
}

pub struct NativeErrorValue {
    pub kind: NativeErrorKind,
    pub message: String,
}

pub enum JsValue {
    // V1 variants...
    Error(NativeErrorValue),
}
```

`new Test262Error(message)` 生成 `JsValue::Error(Test262, message)`；
`Throw` 根据错误值类型映射 `FailureKind`。以后实现普通 Error 对象时可以把该
内部值迁移为带 internal slot 的对象，而不改变 `Throw` 的字节码契约。

## 6. TypeOf 契约

V2 最低结果：

| 输入 | 结果 |
|---|---|
| Undefined | `"undefined"` |
| Null | `"object"` |
| Boolean | `"boolean"` |
| Number | `"number"` |
| String | `"string"` |
| NativeFunction | `"function"` |
| Object / Error | `"object"` |

`typeof missingName` 必须返回 `"undefined"`，不能产生 ReferenceError。因此
编译器不能简单地把它降级为 `LoadGlobal + TypeOf`；应使用专门的
`TypeOfGlobal(name)`，或让 `TypeOf` 接收一种不抛错的引用。两种方案只能选
一种并写入 Opcode，禁止在 VM 中按错误字符串补救。

推荐增加：

```rust
TypeOfGlobal(u16)
```

其栈效果为 `0 -> 1`，名称必须引用字符串常量。

## 7. Construct 契约

执行前栈布局：

```text
[callee, arg0, ..., argN]
```

参数按从左到右求值。V2 VM：

- `NativeFunction::Test262Error`：构造最小 Test262 Error 值；
- 其他 callee：返回明确 `TypeError`；
- 不得把普通 `Call` 自动当作构造调用；
- 不实现 `this`、prototype 或用户构造函数。

## 8. 各组独立测试

### A 组

```text
源码 -> AST
dangling else -> 正确嵌套
throw 换行 -> ParseError
循环外 break/continue -> ParseError
```

### B 组

```text
手工 If AST -> 跳转结构
手工 Conditional AST -> 汇合深度 1
手工 While AST -> 回边与退出清理
非法 Break AST -> CompileError
Throw AST -> 终止指令
```

### C 组

```text
手工 Chunk -> 分支只执行一侧
向后 Jump -> 消耗循环预算
Throw primitive -> 执行错误
Throw Test262 Error -> FailureKind::Test262
错误后再次执行 -> 栈与预算已重置
```

### D 组

```text
--native-v1 -> 零回退
--native-v2 -> 固定清单全部通过
每个文件 default + strict
报告中 skipped 不计 passed
```

## 9. 共享文件与合并规则

V2 共享文件：

```text
src/lexer/token.rs
src/ast/expression.rs
src/ast/statement.rs
src/bytecode/opcode.rs
src/bytecode/chunk.rs
src/runtime/value.rs
src/runtime/context.rs
src/contracts.rs
src/test262.rs
```

建议分支顺序：

```text
contract/v2-control-flow
  ├─ feat/frontend-v2
  ├─ feat/compiler-v2
  ├─ feat/runtime-v2
  └─ feat/test262-v2
```

共享契约分支先合并。其他分支 rebase 到该提交后，只修改所属目录和必要测试。
禁止多人分别新增含义不同的 `Jump`、`Throw` 或错误值类型。
