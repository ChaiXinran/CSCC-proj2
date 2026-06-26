# Post-Fix2 三线并行继续修改方案

日期：2026-06-26
依据：原始 `part.md`、用户提供的 `fix2.md`、当前 Fix2 C 修复结果

## 1. 本方案的边界

本方案不是重新规划 `fix2.md`。`fix2.md` 中的内容按“已经基本完成或正在由对应组收尾”处理，因此从本轮后续方案中移除。

明确移除的 Fix2 范围：

| Fix2 内容 | 原负责人 | 本方案处理 |
|---|---|---|
| runner 末尾内存崩溃、失败详情截断、报告稳定性 | runner / 全组 | 不再作为三线功能主线，只保留最终门禁要求 |
| class + destructuring + binding/early error 主攻 | A | 不重复规划基础修复，只规划后续前端硬尾 |
| generator / yield / iterator / Promise / async / for-await-of 主攻 | B | 不重复规划基础闭环，只规划 runtime 硬尾 |
| PropertyDescriptor / Object / Reflect 基础桥接 | C | 不重复规划基础描述符，只规划 Proxy/realm/exotic 等后续 |
| Array.from / Array iterator / callback / hole / TypedArray/DataView 基础 | C | 不重复规划基础内建，只规划高阶尾部 |
| Date/String Annex B、RegExp legacy、RegExp exec/test 基础补分 | C | 不重复规划基础补分，只规划高级 RegExp/descriptor 尾部 |
| Temporal / Intl402 作为主线 | 无 | 继续不作为主线，最多保留 descriptor stub 备选 |

本方案的目标：在 Fix2 之后，继续沿原始 `part.md` 的 A/B/C 三线并行方式推进，但只打剩余高收益硬尾，并通过统一接口减少合并冲突。

## 2. 新三线分工概览

| 组别 | Post-Fix2 主方向 | 主要收益来源 | 冲突风险 |
|---|---|---|---|
| A | 前端硬尾：module、剩余 function/object 语法、剩余 early error、parser 稳定性 | `language/*` 中非 Fix2 class/dstr 的剩余块 | parser/AST 与 compiler 边界 |
| B | Runtime 硬尾：BigInt 完整化、Promise/job queue 深化、async hard tail、runtime error/Completion 统一 | `BigInt`、Promise combinators、async 尾部、运行时公共语义 | VM、JsValue、job queue 与 C 的 TypedArray/Function |
| C | 对象模型硬尾：Proxy、realms、Function/constructor、advanced TypedArray/DataView、advanced RegExp | `Proxy` 相关 Object/Reflect、cross-realm、TypedArray/DataView/RegExp 尾部 | runtime/object、builtins、VM construct path |

建议收益预期：

```text
A: +1000 ~ +2000
B: +1500 ~ +2500
C: +1500 ~ +2500
合计：+4000 ~ +7000
```

该估计低于原始 `part.md` 和 `fix2.md` 的短期收益，因为 Fix2 已经吃掉了一批最大块。后续更依赖基础设施质量。

## 3. A 线：前端语法与静态语义硬尾

### A1. Module / import-export / global-code hard tail

Fix2 没有把 module 作为主线。A 可以从 parser/AST/compiler 统一处理剩余 module 与 global-code 失败。

任务：

- `import` / `export` 语法剩余形态。
- module source 自动 strict mode。
- module top-level binding 与重复声明 early error。
- import/export name、string export name、namespace export。
- module evaluation 与 existing module registry 的边界对齐。

建议文件：

- `src/lexer/`
- `src/parser/`
- `src/ast/`
- `src/bytecode/compiler.rs`
- `src/runtime/module.rs`
- `tests/native_modules.rs`
- `tests/parser_*`

验收：

```powershell
cargo test --no-default-features --test native_modules
cargo test --no-default-features --test parser_basics
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --json reports/post-fix2-a-module.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/global-code --jobs 4 --json reports/post-fix2-a-global-code.json
```

### A2. Function / arrow / object expression syntax tail

Fix2 重点在 class/dstr，剩余 function/object/arrow 仍应由 A 收尾。

任务：

- arrow 参数 early error 与 cover grammar。
- function parameter list hard tail：duplicate、default、rest、strict directive。
- object literal method hard tail：async/generator method、computed method、accessor error。
- private-name parse 只限 class 合法位置。
- template literal、regexp literal、numeric literal 的 parser 稳定性尾部。

验收：

```powershell
cargo test --no-default-features --test parser_closures
cargo test --no-default-features --test parser_objects_bytecode
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/expressions/function --jobs 4 --json reports/post-fix2-a-function-expr.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/expressions/arrow-function --jobs 4 --json reports/post-fix2-a-arrow.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/expressions/object --jobs 4 --json reports/post-fix2-a-object-expr.json
```

### A3. Parser/Compiler 安全收尾

任务：

- 所有新增 AST 形状先登记到统一接口。
- 新 opcode 必须补 `Instruction::stack_effect()` 和 bytecode contract test。
- parser 错误必须返回 `SyntaxError` 路径，不得 panic。
- 不在 parser 中提前求值 computed key。

合并门禁：

```powershell
cargo test --no-default-features --test bytecode_objects
cargo test --no-default-features --test native_control_flow
cargo check --no-default-features --all-targets
```

## 4. B 线：Runtime / Value / Async 硬尾

### B1. BigInt 完整化

`fix2.md` 说明 BigInt 不再是第一主线，但 Post-Fix2 它仍然是 B 的清晰剩余块。

任务：

- BigInt parser literal、`JsValue::BigInt`、`typeof`、`BigInt()`、禁止 `new BigInt()`。
- BigInt arithmetic、comparison、bitwise、unary。
- Number/BigInt 混合运算 TypeError。
- `BigInt.asIntN`、`BigInt.asUintN`。
- `BigInt.prototype.toString/valueOf/toLocaleString`。
- 给 C 暴露 BigInt typed-array conversion helper。

建议文件：

- `src/runtime/value.rs`
- `src/vm/interpreter.rs`
- `src/builtins/std_primitives.rs`
- `src/builtins/binary_data.rs` 仅通过接口消费
- `tests/native_stdlib.rs`
- `tests/parser_bigint.rs`

验收：

```powershell
cargo test --no-default-features --test parser_bigint
cargo test --no-default-features --test native_stdlib bigint
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/BigInt --jobs 4 --json reports/post-fix2-b-bigint.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/types/bigint --jobs 4 --json reports/post-fix2-b-bigint-language.json
```

### B2. Promise / job queue / async hard tail

Fix2 做基础闭环后，B 继续处理复杂 combinators 和调度顺序。

任务：

- `Promise.all/allSettled/any/race`。
- thenable assimilation。
- rejection tracking 可先简化，但不能破坏 job queue 顺序。
- `async function` await 链、try/catch/finally 中 await。
- async generator hard tail 与 `for-await-of` 的 IteratorClose。

验收：

```powershell
cargo test --no-default-features --test native_iteration
cargo test --no-default-features --test native_collections
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --json reports/post-fix2-b-promise.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/expressions/async-function --jobs 4 --json reports/post-fix2-b-async-function.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --json reports/post-fix2-b-for-await-of.json
```

### B3. Completion / error propagation 统一

任务：

- VM 内部统一 `Normal/Return/Throw/Yield/Await` 语义。
- JS 抛错必须可被 JS catch 捕获。
- Rust `VmError::runtime` 只保留给引擎 bug 或资源限制。
- C 的 Proxy trap、getter/setter、constructor 调用必须复用该机制。

合并门禁：

```powershell
cargo test --no-default-features --test native_closures
cargo test --no-default-features --test native_function_bind
cargo test --no-default-features --test native_test262
```

## 5. C 线：Object Model / Builtins 硬尾

### C1. Proxy 与 internal methods

Fix2 的 Object/Reflect 基础已经基本收束，当前 Reflect 和 Object.assign 剩余 focused 失败已经是 Proxy-facing。

任务：

- `Proxy` constructor。
- `ObjectKind::Proxy` / `ProxyRecord`。
- internal method dispatch：
  `getPrototypeOf`、`setPrototypeOf`、`isExtensible`、`preventExtensions`、
  `getOwnProperty`、`defineOwnProperty`、`has`、`get`、`set`、`delete`、
  `ownKeys`。
- Proxy trap abrupt completion 和基础 invariants。
- Object/Reflect/Object.assign 统一走 internal methods。

验收：

```powershell
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_proxy
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --verbose --json reports/post-fix2-c-proxy-reflect.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Object/assign --jobs 4 --verbose --json reports/post-fix2-c-proxy-assign.json
```

### C2. Realm / constructor / Function hard tail

任务：

- 真正 `$262.createRealm()`，替换同 realm fallback。
- per-realm intrinsic prototypes。
- builtin/user function realm。
- `GetFunctionRealm`、`GetPrototypeFromConstructor`。
- bound function construct/newTarget 保真。
- dynamic `Function` 使用目标 realm global scope。

验收：

```powershell
cargo test --no-default-features --test native_function_bind
cargo test --no-default-features --test native_realms
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Function --jobs 4 --json reports/post-fix2-c-function.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Array/from --jobs 4 --verbose --json reports/post-fix2-c-realm-array-from.json
```

### C3. Advanced TypedArray/DataView/RegExp hard tail

任务：

- BigInt typed arrays 在 B BigInt helper 稳定后接入。
- resizable ArrayBuffer 完整 fixed-length / length-tracking 语义。
- detached/OOB/range error 顺序集中化。
- RegExp named groups、lookbehind、indices、unicode sets。
- String RegExp-facing methods 消费同一 RegExp match record。

验收：

```powershell
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_regexp
cargo build --release --no-default-features
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --json reports/post-fix2-c-typedarray.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --json reports/post-fix2-c-dataview.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --json reports/post-fix2-c-regexp.json
```

## 6. 并行合并顺序

推荐按“小接口 PR + 功能 PR”的节奏推进：

1. 全组先合入统一接口文档。
2. A/B/C 各自只改本组文件，若需要共享文件，先提 contract patch。
3. B 先稳定 BigInt conversion / Completion / job queue 接口。
4. C 再接 BigInt typed arrays、Proxy traps、realms。
5. A 的 parser/AST 改动必须在 B/C 消费前完成 AST 形状登记。
6. 每天固定跑小全量目录组，完整 53k 只在大块合并后跑。

## 7. 文件所有权

| 文件区域 | 主负责人 | 合并规则 |
|---|---|---|
| `src/lexer/`, `src/parser/`, `src/ast/` | A | B/C 不直接改语法，除非是已登记的小 unblocker |
| `src/bytecode/opcode.rs`, `src/bytecode/compiler.rs` | A/B | 新 opcode 先补 stack effect 和 bytecode test |
| `src/vm/` | B | C 需要 constructor/proxy/realm call path 时先走接口 patch |
| `src/runtime/value.rs`, BigInt conversion | B | C 只消费转换 helper |
| `src/runtime/object.rs`, descriptor/internal methods | C | A/B 不绕过 descriptor/internal methods |
| `src/builtins/` | C，B owns Promise/Iterator/BigInt | 新 builtin shape 必须走 descriptor install helper |
| `src/test262.rs` / runner | 轮值 | 不混在功能 PR 里做大改 |
| `tests/` / `reports/` | 各组 | 每个功能 PR 必须有 local test + focused Test262 JSON |

## 8. 通用门禁

每次合并前：

```powershell
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo test --no-default-features --test native_object_keys
cargo test --no-default-features --test native_array_methods
cargo test --no-default-features --test native_typed_arrays
cargo test --no-default-features --test native_stdlib
cargo build --release --no-default-features
```

每日小全量：

```powershell
target/release/agentjs.exe test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --json reports/post-fix2-daily-module.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/BigInt --jobs 4 --json reports/post-fix2-daily-bigint.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --json reports/post-fix2-daily-promise.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/Reflect --jobs 4 --json reports/post-fix2-daily-reflect.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --json reports/post-fix2-daily-typedarray.json
target/release/agentjs.exe test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --json reports/post-fix2-daily-regexp.json
```

完整全量：

```text
只在大功能合并后、每日收尾、最终报告前运行。
如果 runner 仍有内存/栈崩溃，先修 runner，再相信全量数据。
```
---

## Post-Fix2 B1 当前状态补记

日期：2026-06-26

B1 BigInt 小闭环已完成一轮 VM/runtime 精度修复：

- BigInt 混合 Number 算术收紧为 TypeError；
- BigInt 位运算与移位基础路径已接入；
- VM 数值运算错误已在本轮相关路径中转为 JS 可捕获 Completion；
- `to_numeric` / `to_numeric_operands` 成为 B 组后续 Promise/async/TypedArray
  conversion 前的共享数值语义入口之一。

验收结果：

- `cargo test --no-default-features --test native_stdlib bigint`：5/5 passed。
- `cargo test --no-default-features --test parser_bigint`：7/7 passed。
- `cargo check --no-default-features --all-targets`：通过。
- `cargo test --no-default-features --test native_test262`：15/15 passed。
- `test/built-ins/BigInt`：68/77 passed。
- `test/language/literals/bigint`：59/59 passed。
- `test/language/expressions/bitwise-and`：29/30 passed。
- `test/language/expressions/bitwise-or`：29/30 passed。
- `test/language/expressions/bitwise-xor`：29/30 passed。
- `test/language/expressions/left-shift`：44/45 passed。
- `test/language/expressions/right-shift`：36/37 passed。
- `test/language/expressions/unsigned-right-shift`：44/45 passed。

注意：原计划中的 `test/language/types/bigint` 在当前 test262 树不存在；
当前 BigInt 字面量目录为 `test/language/literals/bigint`。

后续 B 组建议：

1. 若继续 B1，应先决定是否引入任意精度 BigInt 存储；当前 `i128` 是剩余大整数
   Test262 的主要硬上限。
2. 若不做任意精度，下一步更适合转入 B2 Promise combinator / thenable
   assimilation，避免在 BigInt 上继续做收益很小的边角补丁。
