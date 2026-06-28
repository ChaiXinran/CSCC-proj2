# Fix4 Team Plan and Shared Interface

## 0. 当前目标

Fix4 的目标不是重构整个 Native backend，而是在当前全量 Test262 通过率 53.33% 的基础上，尽快冲到 60% 以上。

当前状态：

```text
total = 53379
passed = 28468
failed = 24909
skipped = 2
conformance = 53.33%
```

60% 目标：

```text
ceil(53379 * 0.6) = 32028
```

仍需新增通过：

```text
32028 - 28468 = 3560
```

因此 Fix4 的策略是：优先选择能成片转 pass 的高收益区域，不再平均修所有失败。

---

## 1. Fix4 总体路线

Fix4 继续沿用三轨并行模型：

| 组别 | 主攻方向                                                 |          目标收益 | 主要收益目录                                                                                                                                      |
| -- | ---------------------------------------------------- | ------------: | ------------------------------------------------------------------------------------------------------------------------------------------- |
| A  | class elements + destructuring + binding/early error | +1000 ~ +2000 | `language/statements/class`、`language/expressions/class`、`language/*/dstr`                                                                  |
| B  | generator + iterator + Promise + async/for-await-of  | +1500 ~ +3000 | `language/expressions/yield`、`language/*/generators`、`language/statements/for-await-of`、`built-ins/Promise`、`built-ins/Iterator`            |
| C  | descriptor + TypedArray/DataView 收尾 + RegExp/AnnexB  |  +800 ~ +1500 | `built-ins/Object`、`built-ins/Function`、`built-ins/Array`、`built-ins/TypedArray`、`built-ins/DataView`、`built-ins/RegExp`、`annexB/built-ins` |

推荐总收益组合：

```text
B 组 async/generator/Promise/Iterator：+2000
A 组 class/destructuring：+1000
C 组 TypedArray/descriptor/RegExp：+700
----------------------------------------
合计：+3700
```

这组收益足够越过 60% 线，同时比完整 Temporal / Intl402 更可控。

---

## 2. 推荐分支

```text
docs/fix4-contracts
fix4-a-class-dstr
fix4-b-generator-async-promise
fix4-c-descriptor-typedarray-regexp
test/fix4-integration
```

推荐合并顺序：

```text
docs/fix4-contracts
  -> fix4-b-generator-async-promise 的最小 iterator/promise contract
  -> fix4-a-class-dstr 的 class/dstr lowering
  -> fix4-c-descriptor-typedarray-regexp 的 descriptor/builtin 收尾
  -> test/fix4-integration
  -> full Test262 scan and report update
```

如果某组需要改非本组文件，必须先在接口文档里新增接口记录，再提交小型 contract PR，不能在大功能 PR 中顺手大改共享文件。

---

## 3. 文件所有权

| 文件 / 目录                         | 默认负责人                     | 修改规则                                                          |
| ------------------------------- | ------------------------- | ------------------------------------------------------------- |
| `src/lexer/`                    | A                         | B/C 不直接改词法规则；需要新 token 先登记接口                                  |
| `src/parser/`                   | A                         | B/C 需要新语法时由 A 落地 AST 形状                                       |
| `src/ast/`                      | A                         | BindingPattern、ClassElement、FunctionKind 等共享 AST 先冻结再实现       |
| `src/bytecode/opcode.rs`        | B 主导，A/C 评审               | 新 opcode 必须说明栈效果、操作数格式、错误传播                                   |
| `src/bytecode/chunk.rs`         | B                         | 新 chunk 元信息由 B 统一维护                                           |
| `src/bytecode/compiler.rs`      | A/B 交汇                    | A 负责 class/dstr lowering，B 负责 generator/async lowering；不得整段覆盖 |
| `src/vm/`                       | B                         | generator resume、iterator、Promise job queue、await 执行协议由 B 维护  |
| `src/runtime/`                  | B/C 共享                    | object model、descriptor、iterator helper、Promise queue 必须集中实现  |
| `src/builtins/`                 | C，B 对 Promise/Iterator 协作 | builtin 不得绕过 runtime 直接维护第二套属性表或迭代协议                          |
| `src/test262.rs`                | D/集成，三组评审                 | Fix4 scan selector、manifest、JSON summary 保持稳定                 |
| `tests/`                        | 各组                        | 每组必须补专项 contract test                                         |
| `reports/.version-report/fix4-part*.md`         | 各组                        | 每个 PR 更新对应报告                                                  |
| `docs/fixup/fix4-interface.md` | 全组                        | 所有共享接口变化先写文档再改代码                                              |

---

## 4. A 组方案：class + destructuring

### 4.1 A 组目标

A 组负责解决剩余 language 失败中的 class 与 destructuring 高密度部分。

重点目录：

```text
test/language/statements/class
test/language/expressions/class
test/language/statements/class/dstr
test/language/expressions/class/dstr
test/language/expressions/assignment/dstr
test/language/statements/for/dstr
```

A 组不负责 Promise job queue，不负责 generator resume，不负责 builtin descriptor sweep。

---

### 4.2 A 组任务拆分

#### A1. class elements 收尾

优先补：

```text
class field
static field
private field
private method
private getter / setter
static block
computed property name
method / getter / setter descriptor
class name binding
derived constructor this 初始化
super() 调用顺序
```

最小目标不是完整覆盖所有 class 特性，而是先让以下测试族成片下降：

```text
language/statements/class/elements
language/expressions/class/elements
language/statements/class/dstr
language/expressions/class/dstr
```

#### A2. destructuring 统一 lowering

目前 destructuring 不能分散在 let/const/参数/for-of/class method 中各写一套，必须抽成共享 lowering。

需要统一支持：

```text
array binding pattern
object binding pattern
default initializer
rest element
rest property
nested pattern
elision
iterator destructuring
getter 调用顺序
null / undefined 解构抛 TypeError
abrupt completion 传播
IteratorClose
```

A 组只负责生成正确 lowering；如果需要 iterator close，调用 B 组接口，不自己实现 iterator 协议。

#### A3. early error 与 binding

优先处理：

```text
duplicate lexical declaration
private name 未声明
private name 重复声明
invalid assignment target
class 中默认 strict mode
yield / await 在不同函数上下文中的 identifier 限制
```

A 组要保证错误类型统一为 `SyntaxError`，不能让 parser panic，也不能落成普通 runtime error。

---

### 4.3 A 组共享 AST 接口

建议冻结以下 AST 形状。名称可以贴合现有代码，但语义必须一致：

```rust
pub enum BindingPattern {
    Identifier(String),
    Array(ArrayBindingPattern),
    Object(ObjectBindingPattern),
}

pub struct ArrayBindingPattern {
    pub elements: Vec<Option<BindingElement>>,
    pub rest: Option<Box<BindingElement>>,
}

pub struct ObjectBindingPattern {
    pub properties: Vec<ObjectBindingProperty>,
    pub rest: Option<Box<BindingPattern>>,
}

pub struct BindingElement {
    pub target: BindingPattern,
    pub default: Option<Expression>,
}

pub enum ObjectBindingProperty {
    SingleName {
        name: String,
        default: Option<Expression>,
    },
    Property {
        key: PropertyName,
        target: BindingPattern,
        default: Option<Expression>,
    },
}

pub enum ClassElementKind {
    Method,
    Getter,
    Setter,
    Field,
    StaticBlock,
    PrivateMethod,
    PrivateField,
    PrivateGetter,
    PrivateSetter,
}

pub struct ClassElement {
    pub kind: ClassElementKind,
    pub is_static: bool,
    pub name: ClassElementName,
    pub value: Option<Expression>,
    pub params: Vec<BindingPattern>,
    pub body: Option<Vec<Statement>>,
}
```

---

### 4.4 A 组对 B/C 的接口需求

A 组需要 B 提供：

```rust
compile_binding_pattern(pattern, mode) -> Result<(), CompileError>
compile_iterator_destructuring(pattern, iterator_record) -> Result<(), CompileError>
emit_iterator_close_on_abrupt(iterator_record)
```

A 组需要 C 提供：

```rust
copy_data_properties_excluding(
    target: JsValue,
    source: JsValue,
    excluded_keys: Vec<PropertyKey>,
) -> Result<JsValue, VmError>
```

用于 object rest：

```js
let { a, ...rest } = obj;
```

A 组不得自己在 compiler 中直接枚举对象属性；object rest 必须走 C/B 共享 runtime helper。

---

### 4.5 A 组验证命令

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class/dstr --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class/dstr --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/assignment/dstr --jobs 4 --progress
```

---

## 5. B 组方案：generator + iterator + Promise + async

### 5.1 B 组目标

B 组是 Fix4 冲 60% 的主力。当前剩余失败中，generator / yield / async iterator / Promise / for-await-of 的数量刚好接近冲线缺口。

重点目录：

```text
test/language/expressions/yield
test/language/expressions/generators
test/language/statements/generators
test/language/expressions/async-generator
test/language/statements/async-generator
test/language/statements/for-await-of
test/built-ins/Iterator
test/built-ins/Promise
```

B 组负责所有“可暂停执行”和“异步调度”公共协议。

---

### 5.2 B 组任务拆分

#### B1. 普通 generator 最小闭环

优先实现：

```text
function* f() {}
yield expr
yield*
generator object
generator.next()
generator.return()
generator.throw()
IteratorResult: { value, done }
```

最小可接受语义：

```js
function* g() {
  yield 1;
  return 2;
}

let it = g();
it.next(); // { value: 1, done: false }
it.next(); // { value: 2, done: true }
```

#### B2. Iterator protocol

统一提供：

```text
GetIterator
IteratorNext
IteratorComplete
IteratorValue
IteratorClose
CreateIterResultObject
Array iterator
String iterator
```

必须保证以下路径共用同一套 iterator helper：

```text
for-of
yield*
array destructuring
destructuring rest
Array.from
TypedArray.from
Promise combinators 后续扩展
```

#### B3. Promise 最小闭环

第一阶段不追求完整微任务模型，但必须统一 job queue。

最低实现：

```text
Promise constructor
Promise.resolve
Promise.reject
Promise.prototype.then
Promise.prototype.catch
Promise.prototype.finally
enqueue_promise_job
drain_promise_jobs
```

#### B4. async / await / for-await-of

在普通 generator、iterator 和 Promise 基础上继续补：

```text
async function 返回 Promise
await 表达式
for-await-of
async generator
AsyncIteratorResult
```

优先做 `for-await-of`，因为它同时影响 class async generator method、module top-level await、async generator 等多块失败。

---

### 5.3 B 组共享接口

建议冻结：

```rust
pub enum FunctionKind {
    Normal,
    Generator,
    Async,
    AsyncGenerator,
}

pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
}

pub enum IteratorHint {
    Sync,
    Async,
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

Runtime helper：

```rust
pub fn get_iterator(
    ctx: &mut NativeContext,
    value: JsValue,
    hint: IteratorHint,
) -> Result<IteratorRecord, VmError>;

pub fn iterator_next(
    ctx: &mut NativeContext,
    record: &mut IteratorRecord,
    value: Option<JsValue>,
) -> Result<IteratorResult, VmError>;

pub fn iterator_complete(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<bool, VmError>;

pub fn iterator_value(
    ctx: &mut NativeContext,
    result: JsValue,
) -> Result<JsValue, VmError>;

pub fn iterator_close(
    ctx: &mut NativeContext,
    record: &mut IteratorRecord,
    completion: Completion,
) -> Result<Completion, VmError>;

pub fn create_iterator_result_object(
    ctx: &mut NativeContext,
    value: JsValue,
    done: bool,
) -> Result<JsValue, VmError>;
```

Promise job queue：

```rust
pub type PromiseJob = Box<dyn FnOnce(&mut NativeContext) -> Result<(), VmError>>;

pub fn enqueue_promise_job(
    ctx: &mut NativeContext,
    job: PromiseJob,
);

pub fn drain_promise_jobs(
    ctx: &mut NativeContext,
) -> Result<(), VmError>;
```

---

### 5.4 B 组 opcode 接口

如果现有 VM 不足以实现 generator/await，需要新增 opcode。每个 opcode 必须记录：

```text
opcode 名称
操作数格式
入栈 / 出栈效果
是否可能抛 VmError
是否改变当前 frame 状态
是否需要保存 instruction pointer
```

建议候选 opcode：

```rust
Yield
GeneratorResume
GeneratorReturn
Await
GetIterator
IteratorNext
IteratorClose
CreateIterResult
EnqueuePromiseJob
DrainPromiseJobs
```

示例栈效果：

```text
Yield:
  before: [..., value]
  after:  suspend current frame, return IteratorResult(value, false)

CreateIterResult:
  before: [..., value, done]
  after:  [..., iter_result_object]
```

B 组新增 opcode 时，必须同步补：

```text
Instruction::stack_effect()
bytecode dump / debug display
至少一个 tests/native_bytecode_*.rs contract test
```

---

### 5.5 B 组对 A/C 的接口需求

B 需要 A 提供：

```rust
FunctionKind 标记
yield / await 的 AST 节点
for-await-of 的 AST 形状
generator method / async generator method 的 class element 标记
```

B 需要 C 配合：

```rust
Array.prototype[Symbol.iterator]
String.prototype[Symbol.iterator]
Iterator.prototype
Promise constructor/prototype 安装
builtin 函数 name/length/descriptor
```

C 不得自己实现第二套 iterator result object；必须调用 B 的 `create_iterator_result_object`。

---

### 5.6 B 组验证命令

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/yield --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/generators --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/generators --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress
```

---

## 6. C 组方案：descriptor + TypedArray/DataView + RegExp/AnnexB

### 6.1 C 组目标

C 组负责“JS 可见对象形状”和 builtin 行为。Fix4 阶段 C 组不建议大规模实现 Temporal，而是继续收尾已经验证有效的 TypedArray/DataView，并补 descriptor 精度、RegExp/AnnexB 小块收益。

重点目录：

```text
test/built-ins/Object
test/built-ins/Function
test/built-ins/Array
test/built-ins/TypedArray
test/built-ins/TypedArrayConstructors
test/built-ins/DataView
test/built-ins/RegExp
test/built-ins/String
test/built-ins/Date
test/annexB/built-ins
```

---

### 6.2 C 组任务拆分

#### C1. descriptor / builtin shape 统一安装

重点修：

```text
name
length
prototype
constructor
writable
enumerable
configurable
own property
not-a-constructor
accessor get/set
```

当前日志中还有大量类似错误：

```text
getYear should be an own property
Cannot convert undefined or null to object
undefined is not callable
descriptor should not be enumerable
name descriptor value should be trimStart
isConstructor invoked with a non-function value
```

这些问题不能在每个 builtin 里手写修补，必须抽成统一安装 helper。

建议提供：

```rust
pub fn install_builtin_function(
    ctx: &mut NativeContext,
    target: JsValue,
    name: &str,
    length: usize,
    func: NativeFunction,
    attrs: BuiltinAttrs,
) -> Result<JsValue, VmError>;

pub fn install_builtin_method(
    ctx: &mut NativeContext,
    prototype: JsValue,
    name: &str,
    length: usize,
    func: NativeFunction,
) -> Result<(), VmError>;

pub fn install_builtin_accessor(
    ctx: &mut NativeContext,
    target: JsValue,
    name: &str,
    get: Option<NativeFunction>,
    set: Option<NativeFunction>,
    attrs: BuiltinAttrs,
) -> Result<(), VmError>;
```

默认属性约定：

```text
普通 builtin method:
  writable: true
  enumerable: false
  configurable: true

prototype 上的 constructor:
  writable: true
  enumerable: false
  configurable: true

全局函数 escape/unescape:
  writable: true
  enumerable: false
  configurable: true
```

#### C2. TypedArray / DataView 收尾

当前 TypedArray / DataView 已经明显下降，Fix4 要继续收尾。

优先补：

```text
TypedArray.prototype 剩余方法
TypedArrayConstructors internals
BigInt typed array constructor 部分
DataView.prototype get/set 精度
ArrayBuffer detached / range check / byteOffset / byteLength
TypedArray.from 和 iterator 协议联动
```

这部分必须接入 B 的 iterator helper，不能在 TypedArray.from 里单独写一套迭代逻辑。

#### C3. Array / Object / Function

优先补：

```text
Array.from
Array.of
Array.prototype.values / keys / entries
Array.prototype[Symbol.iterator]
Array holes
Array species 初步行为
Object.getOwnPropertyDescriptor
Object.defineProperty
Object.keys
Object.getOwnPropertyNames
Function.prototype
Function constructor AnnexB HTML comment parsing 协作
```

#### C4. RegExp / AnnexB 补分

优先补：

```text
RegExp legacy accessors
RegExp.prototype.compile
RegExp.prototype.exec/test
RegExp.prototype[@@split]
String.prototype.match/search/replace/split
String AnnexB HTML methods
Date.prototype.getYear / setYear
String.prototype.trimLeft === trimStart
String.prototype.trimRight === trimEnd
escape / unescape descriptor
```

RegExp parser 静态错误如果涉及 lexer/parser，由 A 负责；C 只负责 JS-visible RegExp builtin 行为。

---

### 6.3 C 组共享 descriptor 接口

建议冻结：

```rust
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}

pub struct PropertyDescriptor {
    pub value: Option<JsValue>,
    pub get: Option<JsValue>,
    pub set: Option<JsValue>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

pub struct BuiltinAttrs {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}
```

Runtime 必须集中提供：

```rust
pub fn define_own_property(
    ctx: &mut NativeContext,
    object: JsValue,
    key: PropertyKey,
    descriptor: PropertyDescriptor,
) -> Result<bool, VmError>;

pub fn get_own_property(
    ctx: &mut NativeContext,
    object: JsValue,
    key: PropertyKey,
) -> Result<Option<PropertyDescriptor>, VmError>;

pub fn get_property(
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<JsValue, VmError>;

pub fn set_property(
    ctx: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
    value: JsValue,
) -> Result<bool, VmError>;

pub fn own_property_keys(
    ctx: &mut NativeContext,
    object: JsValue,
) -> Result<Vec<PropertyKey>, VmError>;

pub fn ordinary_to_property_descriptor(
    ctx: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyDescriptor, VmError>;

pub fn from_property_descriptor(
    ctx: &mut NativeContext,
    descriptor: PropertyDescriptor,
) -> Result<JsValue, VmError>;
```

---

### 6.4 C 组对 A/B 的接口需求

C 需要 A 配合：

```text
RegExp literal static metadata
AnnexB HTML comment tokenization / Function constructor parser behavior
computed property name AST 形状
```

C 需要 B 配合：

```text
Array.from / TypedArray.from 使用 get_iterator
IteratorClose 行为
Promise job queue 用于 Promise builtin
```

C 不得直接修改 parser 的 RegExp 语法规则；也不得在 builtins 中绕过 B 的 iterator helper。

---

### 6.5 C 组验证命令

```powershell
cargo test --release --no-default-features --all-targets

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Function --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArrayConstructors --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/built-ins --jobs 4 --progress
```

---

## 7. 集成负责人 D：Test262、报告与门禁

虽然实现仍然是 A/B/C 三组，但 Fix4 建议明确设置一个 D 角色，可以由一人兼任。

D 负责：

```text
维护 Fix4 scan manifest
维护 full test262 日志
统计通过率
更新 reports
检查新增失败和回归
处理 runner 稳定性问题
```

D 不直接做大功能实现。

---

### 7.1 D 组任务

#### D1. 建立 Fix4 小全量扫描

建议添加：

```text
reports/.test262/test262-scan-failure/native-fix4-scan-failures.txt
reports/.native-test262-tmp/native-fix4-scan-summary.json
```

扫描 manifest 选 5000 个当前未通过测试，覆盖：

```text
language/statements/class
language/expressions/class
language/expressions/yield
language/statements/for-await-of
built-ins/Promise
built-ins/Iterator
built-ins/TypedArray
built-ins/DataView
built-ins/Object
built-ins/Array
built-ins/RegExp
annexB/built-ins
```

如果时间不够实现新的 `--native-fix4-scan` CLI，可以先用 PowerShell 脚本顺序跑这些目录，并汇总结果。

#### D2. 每组报告

新增：

```text
reports/.version-report/fix4-partA-report.md
reports/.version-report/fix4-partB-report.md
reports/.version-report/fix4-partC-report.md
reports/fix4-integration-report.md
```

每份报告必须包含：

```text
owner and scope
locked baseline source
baseline totals and relevant failure classes
change log, newest first
implemented functionality
tests run
result deltas against baseline
newly exposed failures and regressions
cross-group coordination notes
```

#### D3. 全量扫描

只在以下情况跑完整 53k：

```text
大功能合并后
每天收尾
最终提交前
```

完整命令：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

如果 runner 支持 JSON summary，必须同时保存 JSON：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/.native-test262-tmp/fix4-full-test262-summary.json
```

---

## 8. 合并前门禁

每个 PR 至少跑：

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

如果 clippy 由于历史问题无法立即全过，PR 报告中必须说明：

```text
clippy was not run / did not pass because ...
```

不能空着不写。

---

## 9. 每组专项门禁

### A 组

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/assignment/dstr --jobs 4 --progress
```

### B 组

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/yield --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress
```

### C 组

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress
```

---

## 10. 每日小全量门禁

每天至少跑一次：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress

cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress
```

---

## 11. 集成顺序

### 第 1 阶段：冻结接口

目标文件：

```text
docs/fixup/fix4-teamplan.md
docs/fixup/fix4-interface.md
reports/.version-report/fix4-partA-report.md
reports/.version-report/fix4-partB-report.md
reports/.version-report/fix4-partC-report.md
```

完成标准：

```text
A/B/C 文件所有权明确
共享 AST / iterator / descriptor 接口冻结
每组专项命令明确
每组 baseline 记录完成
```

---

### 第 2 阶段：B 组先落最小公共接口

B 先落地：

```text
FunctionKind
IteratorRecord
IteratorResult
get_iterator
iterator_next
iterator_close
create_iterator_result_object
Promise job queue skeleton
```

原因：A 的 destructuring、C 的 Array.from/TypedArray.from 都需要 iterator helper。

完成标准：

```text
tests/native_iteration.rs 通过
tests/native_promise_jobs.rs 通过
test/language/expressions/yield 有下降
test/built-ins/Iterator 有下降
```

---

### 第 3 阶段：A/C 并行接入

A 接入：

```text
class/dstr lowering
private method / field parser + runtime metadata
object/array binding pattern lowering
```

C 接入：

```text
builtin descriptor installer
TypedArray/DataView 收尾
Array.from / TypedArray.from 使用 B 的 iterator helper
String/Date AnnexB descriptor sweep
```

完成标准：

```text
A: class/dstr 目录下降
B: yield/for-await/Promise/Iterator 目录下降
C: TypedArray/DataView/Object/Array 目录下降
```

---

### 第 4 阶段：集成扫描

合并到：

```text
test/fix4-integration
```

跑：

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
```

然后跑小全量目录组。

最后跑完整 Test262：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

完成标准：

```text
passed >= 32028
conformance >= 60.00%
skipped 仍然如实记录，不能计入 passed
不得出现 runner crash / panic / memory allocation failure
```

---

## 12. 不建议在 Fix4 主攻的内容

### 12.1 不主攻完整 Temporal

Temporal + Intl Temporal 数量很大，但实现成本过高。Fix4 可以做 skeleton，但不要把完整 Temporal 作为冲 60% 主线。

可选 skeleton：

```text
Temporal object exists
Temporal.Xxx constructor exists
prototype exists
name / length / descriptor correct
not-a-constructor 行为正确
```

但不承诺完整日期时间语义。

### 12.2 不主攻完整 Intl402

Intl402 依赖大量 locale、calendar、number/date formatting 行为，短期不适合作为主线。

### 12.3 不做大规模重构

Fix4 的目标是冲线，不是重写 VM。任何重构必须证明：

```text
不会降低当前 28468 passed
能带来明确 Test262 delta
能在一到两天内合并
```

---

## 13. 接口变更记录模板

每次改共享接口，都在文档末尾追加：

| 日期         | 负责人 | 变更                                                     | 影响文件                                                      | 评审人 | 测试                          |
| ---------- | --- | ------------------------------------------------------ | --------------------------------------------------------- | --- | --------------------------- |
| 2026-06-27 | 全组  | 建立 Fix4 分工和共享接口                                        | `docs/fixup/fix4-teamplan.md`, `docs/fixup/fix4-interface.md` | 待定  | 文档变更                        |
| 2026-06-27 | B   | 冻结 IteratorRecord / IteratorResult / Promise job queue | `src/runtime/`, `src/vm/`, `src/contracts.rs`             | A/C | `tests/native_iteration.rs` |
| 2026-06-27 | A   | 冻结 BindingPattern / ClassElementKind                   | `src/ast/`, `src/parser/`, `src/bytecode/compiler.rs`     | B   | class/dstr focused tests    |
| 2026-06-27 | C   | 冻结 PropertyDescriptor / builtin installer              | `src/runtime/`, `src/builtins/`                           | B   | Object/Array focused tests  |

---

## 14. 最终验收标准

Fix4 完成需要同时满足：

```text
1. 全量 Test262 passed >= 32028
2. conformance >= 60.00%
3. full scan 能正常完成，不崩溃
4. cargo fmt/check/test 通过
5. 每组 part report 更新
6. docs/status.md 更新当前通过率和剩余缺口
7. readme.md 或 AGENTS.md 更新 Fix4 运行命令
8. 不把 skipped / timeout / crashed 计入 passed
```

---

## 15. 一句话分工

A 组：把 class 和 destructuring 的语法、binding、lowering 做统一，目标吃掉 1000～2000 个 language 失败。

B 组：把 generator、iterator、Promise、async/for-await-of 做成公共执行协议，目标吃掉 1500～3000 个失败，是 Fix4 冲 60% 的主力。

C 组：把 descriptor、TypedArray/DataView、Array/Object/RegExp/AnnexB 做收尾，目标吃掉 800～1500 个失败，并保证 builtin 行为不绕过 runtime object model。

D 组：维护扫描、报告、回归门禁和 full Test262 可信结果，保证最后的 60% 是可复现、可提交、可解释的结果。
