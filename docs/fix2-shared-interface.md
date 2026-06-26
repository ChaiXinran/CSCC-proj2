# Fix2 共享接口协议

本文档根据 `reports/fix/fix2.md` 冻结下一阶段冲 60% Test262 的跨组协作协议。它补充
`docs/interface-spec.md` 和现有 Native V1–V11 接口文档；若 Fix2 范围内出现冲突，以本文对
“新增改动”的约束为准。目标是让 A/B/C 三组并行开发时只在明确接口点交汇，避免再次出现
半截函数、重复分支、重复 Runtime helper 或互相覆盖的问题。

## 1. 总体边界

Fix2 的三条主线如下：

| 组别 | 主攻方向 | 主要收益目录 |
|---|---|---|
| A | class、destructuring、binding/early error | `test/language/statements/class`、`test/language/expressions/class`、`test/language/*/dstr` |
| B | generator、yield、iterator、Promise、async | `test/language/*/generators`、`test/language/expressions/yield`、`test/built-ins/Iterator`、`test/built-ins/Promise` |
| C | descriptor、Object/Reflect、Array/TypedArray、RegExp/Annex B | `test/built-ins/Object`、`Reflect`、`Array`、`TypedArray`、`RegExp`、`test/annexB/built-ins` |

任何跨组共享类型必须先记录在本文，再落到代码。实现代码不得写入
`src/contracts.rs`；该文件只导出稳定类型、trait 和少量无状态辅助契约。

## 2. 共享文件所有权

| 文件/目录 | 默认负责人 | 修改规则 |
|---|---|---|
| `src/lexer/`、`src/parser/`、`src/ast/` | A | B/C 需要新语法时先在本文登记 AST 形状，由 A 或契约 PR 落地 |
| `src/bytecode/opcode.rs`、`src/bytecode/chunk.rs` | B 主导，A/C 评审 | 新 opcode 必须说明栈效果、操作数格式和错误传播 |
| `src/bytecode/compiler.rs` | A/B 交汇 | A 只负责前端语义 lowering，B 负责 generator/async lowering；同一函数不得各自整段替换 |
| `src/vm/` | B | Iterator、Generator、Promise job queue 的执行协议由 B 维护 |
| `src/runtime/` | B/C 共享 | 对象模型、descriptor、iterator helper、Promise job queue 必须走共享 helper |
| `src/builtins/` | C，B 对 Promise/Iterator 协作 | Builtin 不得绕过 runtime 直接维护第二套属性表或迭代协议 |
| `tests/`、`reports/fix/` | 各组 | 每个功能 PR 必须附专项测试和 Test262 增量摘要 |

## 3. A 组接口：class 与 destructuring

A 组负责把语法和 binding 语义统一到稳定 AST。建议共享形状如下，名称可贴合现有代码，但语义必须一致：

```rust
pub enum BindingPattern {
    Identifier(String),
    Array(ArrayBindingPattern),
    Object(ObjectBindingPattern),
}

pub struct BindingElement {
    pub target: BindingPattern,
    pub default: Option<Expression>,
    pub rest: bool,
}

pub enum ClassElementKind {
    Method,
    Getter,
    Setter,
    Field,
    StaticBlock,
    PrivateMethod,
    PrivateField,
}
```

协议：

- destructuring 的 getter 调用、默认值求值、rest 收集和 `null`/`undefined` 抛错必须只实现一套共享 lowering，不得在 `let`、参数、`for-of`、class 参数中各写一套。
- computed property name 必须保留求值顺序，交给 compiler/VM 执行，不在 parser 中提前求值。
- private name 的解析结果要能被 class runtime 检查复用，不能只作为字符串普通属性处理。
- early error 可以分阶段补，但必须统一返回 `SyntaxError`，不能落成 panic 或 `RuntimeError`。

## 4. B 组接口：generator、iterator、Promise

B 组负责所有“可暂停执行”和“异步调度”的公共协议。最低共享契约：

```rust
pub enum FunctionKind {
    Normal,
    Generator,
    Async,
    AsyncGenerator,
}

pub struct IteratorRecord {
    pub iterator: JsValue,
    pub next_method: JsValue,
    pub done: bool,
}

pub struct IteratorResult {
    pub value: JsValue,
    pub done: bool,
}
```

需要提供或冻结的 Runtime/VM helper：

```rust
get_iterator(value, hint) -> IteratorRecord
iterator_next(record, value?) -> IteratorResult
iterator_close(record, completion) -> Completion
create_iterator_result_object(value, done) -> JsValue
enqueue_promise_job(job)
drain_promise_jobs()
```

协议：

- `for-of`、`yield*`、array/string iterator、destructuring rest、`Array.from` 必须共用同一套 iterator helper。
- generator 的 `next`、`return`、`throw` 必须统一返回 `{ value, done }`，不得由 builtin 手写不同对象形状。
- Promise 第一阶段允许简化微任务模型，但所有 `then/catch/finally/await` 必须通过同一个 job queue。
- B 如需新增 opcode，必须先补 `Instruction::stack_effect()` 和至少一个 bytecode contract test。

## 5. C 组接口：descriptor、Object/Reflect、Array/TypedArray

C 组负责对象模型精度和 builtin 可观察属性。最低共享 descriptor 契约：

```rust
pub struct PropertyDescriptor {
    pub value: Option<JsValue>,
    pub get: Option<JsValue>,
    pub set: Option<JsValue>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}
```

Runtime 必须集中提供：

```rust
define_own_property(object, key, descriptor) -> Result<bool, VmError>
get_own_property(object, key) -> Option<PropertyDescriptor>
get_property(receiver, key) -> Result<JsValue, VmError>
set_property(receiver, key, value) -> Result<bool, VmError>
own_property_keys(object) -> Vec<PropertyKey>
ordinary_to_property_descriptor(value) -> PropertyDescriptor
from_property_descriptor(descriptor) -> JsValue
```

协议：

- `Object.*`、`Reflect.*`、Array、TypedArray、Date/String Annex B 和 RegExp legacy accessors 必须走上述 descriptor/property helper。
- `name`、`length`、`prototype`、`constructor` 的属性描述符必须由安装函数统一设置，不得在每个 builtin 中手写不一致 flags。
- Array hole、integer index 顺序、string key 顺序和 symbol key 顺序要由 `own_property_keys` 统一保证。
- TypedArray/DataView 可以先实现最小语义，但不得伪装成普通 Array；必须保留后续接入 ArrayBuffer byte store 的接口位置。

## 6. 合并顺序

推荐顺序：

```text
Fix2 shared interface
 -> runner 稳定性修复
 -> A/B/C 各自最小 contract PR
 -> A destructuring/class、B generator/iterator、C descriptor/Object 并行
 -> Promise/async、Array/TypedArray、RegExp/Annex B 扩展
 -> 小全量目录扫描
 -> 全量 Test262 扫描
```

如果某组需要修改非本组文件，先在本文新增“接口变更记录”，再提交小型契约 PR。不要在大功能 PR 中顺手修改共享文件。

## 7. 合并前门禁

每个 PR 至少运行：

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

专项 Test262：

```powershell
# A
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for/dstr --jobs 4 --progress

# B
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/yield --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress

# C
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
```

每天至少跑一次 Fix2 小全量目录组；完整 53k 全量只在大功能合并后、每日收尾和最终报告前运行。

## 8. 接口变更记录

新增或修改共享接口时，在这里追加记录：

| 日期 | 负责人 | 变更 | 影响文件 | 评审人 |
|---|---|---|---|---|
| 2026-06-26 | 全组 | 建立 Fix2 共享接口协议 | `docs/fix2-shared-interface.md` | 待定 |

