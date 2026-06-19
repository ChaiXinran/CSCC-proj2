# AgentJS 模块接口规格

本文档定义 AgentJS 自研引擎四个协作部分之间的稳定边界。目标是让各部分能够独立实现、独立测试，并在不依赖 Boa 内部类型的情况下完成最终链接。

文档版本：`0.1`。本文中的“必须（MUST）”表示合并前必须满足，“建议（SHOULD）”表示若偏离需要在评审中说明。

第一批具体语言功能和 Test262 验收文件见
[Native V1 表达式里程碑](native-v1-scope.md)。

## 1. 四部分与所有权

| 部分 | 所有目录 | 输入 | 输出 | 公共接口 |
|---|---|---|---|---|
| A. 前端 | `lexer/`、`ast/`、`parser/` | UTF-8 源码 | `Program` | `SourceParser` |
| B. 编译器 | `bytecode/` | `&Program` | `Chunk` | `ProgramCompiler` |
| C. 执行内核 | `vm/`、`runtime/`、`builtins/` | `&Chunk`、`&mut NativeContext` | `JsValue` | `ChunkExecutor` |
| D. 集成层 | `backend/native.rs`、CLI、Test262 | 源码与执行配置 | `ExecutionReport` | `NativePipeline`、`RuntimeBackend` |

公共类型和 Trait 统一由 `src/contracts.rs` 导出。实现代码必须保留在所属目录，不得将具体解析、编译或执行逻辑写入 `contracts.rs` 或 `backend/native.rs`。

## 2. 总调用链

```text
&str
  │ SourceParser::parse_source
  ▼
Program
  │ ProgramCompiler::compile_program
  ▼
Chunk
  │ ChunkExecutor::execute_chunk（同时接收 NativeContext）
  ▼
JsValue
  │ NativeRuntime：格式化结果、收集输出、映射错误
  ▼
ExecutionReport
```

标准组装入口：

```rust
use agentjs::contracts::{NativeContext, NativePipeline};

let mut context = NativeContext::default();
let value = NativePipeline::default().evaluate(source, &mut context)?;
```

集成层必须按上述顺序调用，不得在某阶段失败后静默转交 Boa。Boa 只能用于独立的基线执行和差分测试。

## 3. A 部分：前端接口

```rust
pub trait SourceParser {
    fn parse_source(&mut self, source: &str)
        -> Result<Program, NativeError>;
}
```

### 输入约束

- `source` 必须按 UTF-8 处理。
- `Span` 使用原始源码的半开字节区间 `[start, end)`。
- EOF 的位置必须满足 `start == end == source.len()`。
- 前端不得持有调用者传入的 `&str`，输出 AST 必须拥有所需数据。

### 输出约束

- 成功时必须消费完整输入，不能忽略尾部 Token。
- 运算符优先级和结合性必须在 AST 中明确体现。
- 语法错误返回 `NativeError::Lex` 或 `NativeError::Parse`。
- 前端不得生成字节码，也不得引用 VM、Heap 或 Boa 类型。

### 独立验收

```text
源码 → Token 快照
源码 → Program/AST 快照
非法源码 → 错误类别与 Span
```

编译器未完成时，前端测试只检查 `Program`。

## 4. B 部分：编译器接口

```rust
pub trait ProgramCompiler {
    fn compile_program(&mut self, program: &Program)
        -> Result<Chunk, NativeError>;
}
```

### 输入约束

- 编译器只读取 `Program`，不得修改 AST。
- 所有 AST 变体必须被显式处理；未支持节点返回 `NativeError::Compile`。
- 编译器不得调用 Lexer、Parser、Boa 或宿主 API。

### 输出约束

- `Instruction::Constant(index)` 的索引必须存在于 `Chunk.constants`。
- 所有控制流目标必须落在合法指令边界。
- 每条可执行路径必须以 `Return`、`ReturnUndefined` 或后续定义的终止指令结束。
- 指令的栈效果必须确定；编译器不得依赖 VM 猜测缺失操作数。
- 常量池索引超过 `u16` 范围时必须返回错误，不得截断。

### 独立验收

编译器测试必须使用手工构造的 `Program`：

```text
Program → 指令序列
Program → 常量池
控制流 AST → 跳转结构
未支持 AST → CompileError
```

## 5. C 部分：执行内核接口

```rust
pub trait ChunkExecutor {
    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    )
        -> Result<JsValue, NativeError>;
}
```

### 输入约束

- VM 必须将 `Chunk` 视为只读。
- VM、Runtime 和 Builtins 必须通过传入的 `NativeContext` 共享当前
  isolate 的 Heap、严格模式和输出，不能维护与集成层断开的第二份状态。
- 执行前必须初始化或清理操作数栈；不同脚本不能泄漏临时栈值。
- 无效常量索引、栈下溢和非法指令状态必须返回 `NativeError::Execute`。
- VM 不得调用 Parser 或 Boa。

### Runtime 约束

- `JsValue::Object(ObjectId)` 只能引用当前 Heap 中的有效对象。
- 对象属性语义统一通过 `PropertyDescriptor` 表达。
- 词法绑定统一通过 `Environment` 表达。
- Builtins 通过 Runtime/Heap API 注册，不得绕过对象模型维护另一套值系统。
- GC 只能回收不可达对象，不得改变可观察的 JS 值。

### 独立验收

VM 测试必须使用手工构造的 `Chunk`：

```text
合法 Chunk → JsValue
非法索引 → VmError
栈下溢 → VmError
重复执行 → 无临时状态泄漏
```

Runtime 和 Builtins 应直接测试对象、属性、作用域和函数调用，不依赖 Parser。

## 6. D 部分：集成层接口

集成层负责：

- 持有 `NativePipeline`、`NativeContext` 和运行配置；
- 将 `ExecutionOptions` 传入 Native 执行语义；
- 将 `NativeError` 映射为 `EvalFailure`；
- 将 `JsValue` 转换为最终结果字符串；
- 实现隔离、资源限制、任务队列和 Test262 宿主接口。

集成层不得：

- 实现语法规则、Opcode 或对象属性算法；
- 在 Native 不支持某功能时自动调用 Boa；
- 将 Boa 的 Value、AST 或 Context 暴露给自研模块。

当前 `NativeRuntime` 在完整集成前返回 `Unsupported`。启用 Native 公共执行必须同时满足：

1. 非空源码可以通过完整流水线；
2. 错误能够稳定映射；
3. 独立 Runtime 之间状态隔离；
4. 至少一组目标 Test262 通过；
5. 不存在 Boa 兜底调用。

## 7. 错误契约

| 阶段 | `NativeError` | 示例 |
|---|---|---|
| Lexer | `Lex` | 非法字符、未终止字符串 |
| Parser | `Parse` | 缺少括号、无效语句 |
| Compiler | `Compile` | 未支持 AST、索引溢出 |
| VM/Runtime | `Execute` | 栈下溢、类型错误、非法字节码 |

错误必须包含可定位信息。前端错误优先提供 `Span`；编译器错误应指出 AST/功能；VM 错误应指出指令位置。不得使用 panic 表示用户输入错误。

## 8. 独立开发与 Mock

任意阶段都可替换：

```rust
let mut pipeline = NativePipeline::from_stages(
    fake_frontend,
    compiler_under_test,
    fake_executor,
);
let value = pipeline.evaluate("ignored", &mut NativeContext::default())?;
```

推荐替换方式：

| 被测试部分 | 上游替代 | 下游替代 |
|---|---|---|
| 前端 | 固定源码 | 直接断言 `Program` |
| 编译器 | 手工 `Program` | 直接断言 `Chunk` |
| VM | 手工 `Chunk` | 直接断言 `JsValue` |
| 集成层 | Fake 三阶段 | 断言报告、错误和隔离 |

Mock 必须只实现 `contracts.rs` 中的 Trait，不能依赖其他小组的私有结构。

## 9. 联调契约测试

每次公共接口变更后，至少运行以下固定链路：

```text
""                      → Undefined
"1 + 2 * 3"             → Number(7)
"let x = 1; x + 2"      → Number(3)
"function f(a){...}"     → 函数调用结果
"({a: 1}).a"            → Number(1)
非法语法                  → ParseError
非法运行操作              → 对应运行时错误
```

尚未实现的用例应明确标记 ignored/unsupported，不能伪造成功。Native 与 Boa 的差分测试比较：

- 最终值及 `NaN`、`-0`、`Infinity` 等特殊值；
- 控制台输出顺序；
- 错误类别；
- 多次调用后的状态；
- 独立 Runtime 的隔离行为。

## 10. 接口变更流程

以下文件视为共享契约：

```text
src/contracts.rs
src/lexer/token.rs
src/ast/
src/bytecode/chunk.rs
src/bytecode/opcode.rs
src/runtime/value.rs
```

修改共享契约必须：

1. 单独提交或在提交中明确标注 `contract`；
2. 说明兼容性影响和迁移方法；
3. 同时更新本文档和相关 Mock；
4. 通过所有模块测试后再合并；
5. 先合并契约变更，再 rebase 依赖分支。

实现模块可以自由重构私有结构，只要继续满足本规格。所有合并请求至少运行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```
