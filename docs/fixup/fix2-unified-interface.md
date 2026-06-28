# Post-Fix2 A/B/C 统一接口规范

日期：2026-06-26
状态：Post-Fix2 三线并行开发接口草案

## 1. 目的

本规范服务于 `reports/post-fix2-three-track-plan.md`。它基于原始
`part.md` 的 A/B/C 三线分工，但排除 `fix2.md` 已经覆盖或基本完成的内容。

核心目标：

- A/B/C 并行推进时不互相覆盖共享文件。
- 所有跨组能力先冻结接口，再落实现。
- 后续合并不再出现同一 helper 被不同组重复实现的问题。
- Object/descriptor/iterator/job queue/realm/proxy 等核心语义只保留一套。

## 2. Fix2 内容排除规则

以下接口已经由 `docs/fixup/fix2-interface.md` 或 Fix2 实现承担，本文件不重复定义基础语义：

- class/destructuring/binding 基础语义；
- generator/yield/iterator/Promise/async 基础闭环；
- Object/Reflect descriptor 基础 helper；
- Array.from、Array iterator、TypedArray/DataView 基础 helper；
- Annex B / RegExp 基础补分；
- runner 稳定性修复。

本文件只定义 Post-Fix2 仍需新增或深化的交汇接口。

## 3. 文件所有权和改动协议

| 区域 | 主负责人 | 其他组规则 |
|---|---|---|
| `src/lexer/`, `src/parser/`, `src/ast/` | A | B/C 需要新语法时先登记 AST 形状 |
| `src/bytecode/opcode.rs`, `src/bytecode/chunk.rs` | B 主导，A 参与 | 新 opcode 必须补 stack effect 和 bytecode test |
| `src/bytecode/compiler.rs` | A/B 共享 | A 负责语法 lowering，B 负责 runtime-control lowering |
| `src/vm/` | B | C 需要 constructor/proxy/realm call path 时先加接口 |
| `src/runtime/value.rs` | B | C 消费 BigInt/primitive conversion helper，不直接分叉 |
| `src/runtime/object.rs`, `src/runtime/context.rs` | C | A/B 不绕过 descriptor/internal methods |
| `src/builtins/` | C，B owns Promise/Iterator/BigInt | 新 builtin 必须走统一 descriptor install |
| `src/test262.rs` | 轮值 | runner 改动不得混入大型功能 PR |

任何跨组改动必须满足：

```text
接口文档记录 -> local contract test -> 小 PR 合入 -> 功能 PR 消费
```

## 4. A 线对外接口：AST 与前端语义

A 输出稳定 AST，不输出运行时语义。B/C 只能消费 AST，不应在 parser 旁路求值。

### 4.1 Module 接口

建议 AST 形状：

```rust
pub enum ModuleItem {
    Statement(Statement),
    ImportDeclaration(ImportDeclaration),
    ExportDeclaration(ExportDeclaration),
}

pub struct ImportDeclaration {
    pub specifier: String,
    pub entries: Vec<ImportEntry>,
}

pub struct ExportDeclaration {
    pub entries: Vec<ExportEntry>,
    pub source: Option<String>,
    pub declaration: Option<Statement>,
}
```

规则：

- module code 统一 strict mode。
- import/export early error 在 parser 或 module linker 中统一返回 `SyntaxError`。
- A 不负责模块实例化的 runtime 细节，只保证 AST 和 binding 信息完整。

### 4.2 Function/Object/Arrow Hard Tail 接口

建议继续使用现有 `FunctionLiteral`、`FunctionParam`、`ObjectProperty`，但新增形态前必须登记。

规则：

- object literal computed key 保持求值顺序，不在 parser 中提前求值。
- function/arrow 参数 early error 必须用统一参数检查 helper。
- parser 不直接制造 runtime object ID。
- private name 只能以 AST private-name 形态传递，不能降级成普通字符串属性。

### 4.3 Compiler 消费约束

新 lowering 若需要 opcode：

```rust
Instruction::stack_effect() 必须同步更新
Chunk::validate() 必须能发现栈深错误
tests/bytecode_* 必须包含 contract test
```

## 5. B 线对外接口：Completion、BigInt、Job Queue

B 负责 VM 可执行语义和 runtime value 硬尾。A/C 通过 helper 消费，不直接复制逻辑。

### 5.1 Completion 接口

建议统一形态：

```rust
pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
    Yield { value: JsValue, next_ip: usize },
    Await { value: JsValue, next_ip: usize },
}
```

规则：

- getter/setter、Proxy trap、Promise job、callback、constructor 中抛出的 JS 值必须保持 catchable。
- `VmError::runtime` 只用于引擎错误或资源限制。
- C 的 internal methods 如果调用 JS，必须返回可携带 pending exception 的 `Result`。

### 5.2 BigInt 接口

B 提供 BigInt 值和转换，C 只消费。

Required helpers:

```rust
is_bigint(value) -> bool
to_bigint(value) -> Result<BigIntValue, VmError>
to_bigint64(value) -> Result<i64, VmError>
to_biguint64(value) -> Result<u64, VmError>
bigint_to_string(value, radix) -> Result<String, VmError>
bigint_binary_op(op, left, right) -> Result<JsValue, VmError>
bigint_compare(op, left, right) -> Result<JsValue, VmError>
```

规则：

- Number/BigInt 混合算术 TypeError 由 B helper 统一抛。
- C 的 BigInt typed arrays 不得自行解析 BigInt 字面量或复制 BigInt 算术。
- BigInt 转换错误必须是 JS-visible TypeError/RangeError。

### 5.3 Job Queue / Promise 接口

Required helpers:

```rust
enqueue_promise_job(job) -> Result<(), VmError>
drain_promise_jobs() -> Result<(), VmError>
promise_resolve(constructor, value) -> Result<JsValue, VmError>
new_promise_capability(constructor) -> Result<PromiseCapability, VmError>
perform_then(promise, on_fulfilled, on_rejected, capability?) -> Result<JsValue, VmError>
```

规则：

- async function、Promise combinators、for-await-of 都必须消费同一 job queue。
- C 的 builtins 不创建第二套 microtask queue。
- Test262 async completion 输出由 runner/host 统一处理。

## 6. C 线对外接口：Object Internal Methods

C 负责对象模型硬尾。Object/Reflect/Array/TypedArray/JSON/Proxy 必须共享 internal method dispatch。

### 6.1 PropertyKey

```rust
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}
```

规则：

- symbol key 不得被隐式转成 string。
- ownKeys 顺序由 C 的 shared helper 统一保证。

### 6.2 Internal Methods

Required helpers:

```rust
internal_get_prototype_of(value) -> Result<Option<ObjectId>, VmError>
internal_set_prototype_of(value, prototype) -> Result<bool, VmError>
internal_is_extensible(value) -> Result<bool, VmError>
internal_prevent_extensions(value) -> Result<bool, VmError>
internal_get_own_property(value, key: PropertyKey) -> Result<Option<PropertyDescriptor>, VmError>
internal_define_own_property(value, key: PropertyKey, descriptor) -> Result<bool, VmError>
internal_has_property(value, key: PropertyKey) -> Result<bool, VmError>
internal_get(value, key: PropertyKey, receiver) -> Result<JsValue, VmError>
internal_set(value, key: PropertyKey, value_to_set, receiver) -> Result<bool, VmError>
internal_delete(value, key: PropertyKey) -> Result<bool, VmError>
internal_own_property_keys(value) -> Result<Vec<PropertyKey>, VmError>
```

规则：

- Ordinary object、Proxy、TypedArray integer-indexed exotic、String wrapper virtual index、future realm exotic 都走这个 dispatch。
- Builtins 不直接读取 object property map，除非对象尚未暴露给 JS。
- C 修改 internal methods 时必须跑 Object/Reflect/Array.from guard。

## 7. Proxy 接口

Suggested shape:

```rust
pub struct ProxyRecord {
    pub target: JsValue,
    pub handler: JsValue,
}
```

Required helpers:

```rust
proxy_create(target, handler) -> Result<JsValue, VmError>
proxy_get_trap(handler, name: &str) -> Result<Option<JsValue>, VmError>
proxy_call_trap(trap, this_arg, args) -> Result<JsValue, VmError>
proxy_validate_own_keys(target, trap_result) -> Result<Vec<PropertyKey>, VmError>
proxy_validate_define_own_property(target, key, descriptor, trap_result) -> Result<bool, VmError>
proxy_validate_get_own_property(target, key, trap_result) -> Result<Option<PropertyDescriptor>, VmError>
```

第一批 traps：

```text
getPrototypeOf
setPrototypeOf
isExtensible
preventExtensions
getOwnPropertyDescriptor
defineProperty
has
get
set
deleteProperty
ownKeys
```

规则：

- trap abrupt completion 走 B 的 Completion/pending exception 机制。
- invariants 在返回给 Object/Reflect 前校验。
- revocable Proxy 可后置，但 record 形状不能阻塞后续扩展。

## 8. Realm / Function / Constructor 接口

### 8.1 Realm

Suggested shape:

```rust
pub struct RealmId(u32);

pub struct RealmRecord {
    pub global_object: ObjectId,
    pub global_environment: EnvironmentId,
    pub intrinsics: Intrinsics,
}
```

Required helpers:

```rust
current_realm() -> RealmId
create_realm(install_test262_host: bool) -> Result<RealmId, VmError>
realm_global(realm) -> JsValue
function_realm(function_value) -> Result<RealmId, VmError>
get_function_realm(constructor) -> Result<RealmId, VmError>
get_prototype_from_constructor(constructor, default_intrinsic) -> Result<Option<ObjectId>, VmError>
```

规则：

- user function 和 builtin function 都携带 realm。
- constructor 必须使用 `newTarget` 选择默认 prototype。
- `$262.createRealm().global` 必须返回真实新 realm global object。

### 8.2 Function / Constructor

Required helpers:

```rust
is_callable(value) -> bool
is_constructor(value) -> bool
call_function(function, this_arg, args) -> Result<JsValue, VmError>
construct_function(constructor, new_target, args) -> Result<JsValue, VmError>
ordinary_has_instance(constructor, value) -> Result<bool, VmError>
function_bind(target, bound_this, bound_args) -> Result<JsValue, VmError>
```

规则：

- bound constructor 必须保留 `newTarget`。
- Function descriptor 安装走统一 builtin descriptor helper。
- dynamic `Function` 在 realm 接口完成后必须绑定目标 realm global scope。

## 9. TypedArray / DataView / ArrayBuffer 接口

C 拥有 byte store，B 提供 BigInt conversion。

Required helpers:

```rust
is_detached_buffer(buffer) -> bool
detach_array_buffer(buffer) -> Result<(), VmError>
array_buffer_byte_length(buffer) -> Result<usize, VmError>
resize_array_buffer(buffer, new_length) -> Result<(), VmError>
transfer_array_buffer(buffer, new_length?, fixed_length) -> Result<JsValue, VmError>

validate_typed_array(value) -> Result<TypedArrayViewId, VmError>
typed_array_is_out_of_bounds(view) -> Result<bool, VmError>
typed_array_length(view) -> Result<usize, VmError>
typed_array_byte_offset(view) -> Result<usize, VmError>
typed_array_byte_length(view) -> Result<usize, VmError>
get_value_from_buffer(buffer, byte_index, kind, little_endian) -> Result<JsValue, VmError>
set_value_in_buffer(buffer, byte_index, kind, value, little_endian) -> Result<(), VmError>
```

规则：

- detached / OOB / range check 顺序集中在 helper 内。
- integer-indexed exotic property 使用 internal methods。
- BigInt typed arrays 等 B 的 BigInt helper 稳定后接入。

## 10. RegExp 高级接口

Fix2 基础 RegExp 不重复定义。Post-Fix2 只处理高级 match record。

Suggested shape:

```rust
pub struct RegExpMatch {
    pub start: usize,
    pub end: usize,
    pub captures: Vec<Option<String>>,
    pub named_captures: Vec<(String, Option<String>)>,
    pub indices: Option<Vec<Option<(usize, usize)>>>,
}
```

Required helpers:

```rust
regexp_exec_advanced(rx, string) -> Result<Option<RegExpMatch>, VmError>
regexp_create_match_array(match, input, groups?) -> Result<JsValue, VmError>
regexp_create_indices_array(match) -> Result<JsValue, VmError>
advance_string_index(string, index, unicode) -> usize
```

规则：

- RegExp 和 String methods 消费同一 match record。
- `lastIndex` 读写走 VM property path。
- named groups、indices、unicode sets 不用 ad-hoc 字符串补丁伪造。

## 11. Builtin Descriptor Install 接口

Required helpers:

```rust
define_builtin_function(target, name, length, call, construct?, flags) -> Result<JsValue, VmError>
define_builtin_accessor(target, name, get, set, enumerable, configurable) -> Result<(), VmError>
define_intrinsic_constructor(realm, name, ctor, prototype, length) -> Result<(), VmError>
define_well_known_symbol_method(target, symbol, name, length, call) -> Result<(), VmError>
```

规则：

- `name`、`length`、`prototype`、`constructor`、`Symbol.toStringTag`、`@@iterator` 统一安装。
- 新 builtin descriptor 问题必须补 `native_object_keys` 或 `native_stdlib` 测试。

## 12. 测试与报告接口

每个功能 PR 至少包含：

```text
1 个 local regression test
1 个 focused Test262 JSON
1 条对应 report 更新
```

推荐测试映射：

| 改动 | local test |
|---|---|
| A parser/module | `tests/parser_*`, `tests/native_modules.rs` |
| B BigInt | `tests/parser_bigint.rs`, `tests/native_stdlib.rs` |
| B Promise/job queue | `tests/native_iteration.rs`, `tests/native_collections.rs` |
| C Proxy/internal methods | `tests/native_proxy.rs`, `tests/native_object_keys.rs` |
| C Realm/Function | `tests/native_realms.rs`, `tests/native_function_bind.rs` |
| C TypedArray/DataView | `tests/native_typed_arrays.rs`, `tests/native_binary_data.rs` |
| C RegExp | `tests/native_regexp.rs`, `tests/native_string.rs` |

## 13. 接口变更记录

| 日期 | 负责人 | 变更 | 文件 |
|---|---|---|---|
| 2026-06-26 | A/B/C | 建立 Post-Fix2 三线并行统一接口，排除已完成 Fix2 内容 | `docs/fixup/fix2-unified-interface.md` |
---

## Post-Fix2 B 状态补记：BigInt / ToNumeric / Completion

日期：2026-06-26

B 组已完成一轮 BigInt/ToNumeric 小闭环：

- VM 新增统一 `to_numeric` / `to_numeric_operands` 消费路径；
- Number/BigInt 混合算术统一抛 JS 可见 `TypeError`；
- BigInt `& | ^ ~ << >>` 已接入基础运行时路径；
- BigInt `>>>` 统一抛 `TypeError`；
- BigInt `/ 0n` 与 `% 0n` 抛 JS 可捕获 `RangeError`；
- 数值运算中的 TypeError/RangeError 走 `Completion::Throw`；
- `ToPrimitive` 期间 JS callback 抛出的 pending exception 优先传给 JS catch，
  不再被本轮数值运算路径包装成普通 `Error`。

协作约束：

- C 组后续 BigInt typed-array conversion 应消费 B 组 BigInt/ToNumeric 语义，
  不建议在 TypedArray/DataView 内复制 BigInt 解析或混合运算判断。
- 当前 BigInt 存储仍是 `i128`，超大整数 Test262 仍需后续任意精度 BigInt
  存储方案。

相关文件：

- `src/vm/interpreter.rs`
- `tests/native_stdlib.rs`
- `reports/fix/partb.md`
