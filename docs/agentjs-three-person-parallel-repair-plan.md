# AgentJS 三人并行修复方案

> 适用基线：最新 Test262 全量结果 `45,990 / 53,379`，失败 `7,387`，通过率 `86.16%`。  
> 仓库：`ChaiXinran/CSCC-proj2`。  
> 开工前必须把产生该测试结果的**准确 commit SHA**记录到报告并建立基线 tag；不能只按当前 `main` 猜测。

## 1. 分工原则

本轮不按失败数量平均，而按以下原则划分：

1. **同一源码文件只有一个主负责人**，避免三个人同时修改 `compiler.rs`、`interpreter.rs`、`date_intl.rs`。
2. Parser/AST/Bytecode、VM/Runtime、Temporal/Intl 三条链路分别独立推进。
3. 跨组需求只能通过共享接口提出，不得在自己的功能 PR 中“顺手修改”别人的核心文件。
4. `staging/sm` 不单独分配；正式目录修复后自然回落。
5. Atomics Agent 暂停，不占本轮人力。

---

## 2. 当前失败池

| 重点目录 | 失败数 |
|---|---:|
| `intl402/Temporal` | 1,317 |
| `built-ins/Temporal` | 707 |
| `language/expressions` | 875 |
| `language/statements` | 622 |
| `staging/sm` | 503 |
| `built-ins/RegExp` | 376 |
| `annexB/language` | 193 |
| `language/module-code` | 193 |
| `built-ins/Object` | 187 |
| `intl402/NumberFormat` | 169 |
| `intl402/DateTimeFormat` | 168 |
| `language/eval-code` | 164 |
| `built-ins/Array` | 139 |
| `language/expressions/dynamic-import` | 138 |
| `built-ins/Atomics` | 118 |
| `language/import` | 116 |
| `intl402/DurationFormat` | 110 |
| `language/arguments-object` | 70 |
| `built-ins/Function` | 74 |

本轮明显回归集中在：

- `language/eval-code`：164；
- `built-ins/Object`：187；
- `language/expressions/compound-assignment`：89；
- `language/arguments-object`：70；
- getter 未走 VM 调用路径：约 50。

---

# 3. 人员 A：语言语义、Parser、AST 与 Bytecode

## 3.1 功能职责

负责 JavaScript 源码到字节码之间的全部静态语义：

- Parser early errors；
- strict / async / generator / class 上下文；
- binding 与声明名收集；
- class lowering；
- compound assignment lowering；
- 普通参数、多个 spread 参数和 construct spread；
- module/import 语法与 AST lowering；
- 为声明实例化生成稳定的字节码或元数据。

## 3.2 独占源码目录

```text
src/lexer/
src/parser/
src/ast/
src/bytecode/
```

人员 A 是以下文件的唯一功能修改者：

```text
src/parser/mod.rs
src/parser/expression.rs
src/parser/statement.rs
src/ast/expression.rs
src/ast/statement.rs
src/bytecode/compiler.rs
```

以下文件是共享接口文件，A 不得在普通功能 PR 中直接修改：

```text
src/bytecode/opcode.rs
src/bytecode/chunk.rs
src/contracts.rs
```

需要新 opcode 或 `Chunk` 元数据时，先提交独立的 `interface/*` PR。

## 3.3 主验收 Test262 目录

```text
test262/test/language/expressions/
  ├─ compound-assignment/
  ├─ assignment/
  ├─ class/
  ├─ call/
  ├─ new/
  └─ 其他非 dynamic-import 子目录

test262/test/language/statements/
  ├─ class/
  ├─ function/
  ├─ for/
  ├─ for-of/
  ├─ switch/
  └─ try/

test262/test/language/arguments-object/
test262/test/language/identifiers/
test262/test/language/block-scope/
```

联合验收但不独占的目录：

```text
test262/test/language/eval-code/
test262/test/annexB/language/
test262/test/language/module-code/
test262/test/language/import/
```

在这些联合目录中，A 只负责：

- parse-phase negative；
- compile-time Unsupported；
- 错误的 AST/lowering；
- 缺失或错误的 opcode 序列。

运行期环境、Promise、JobQueue 和模块状态由 B 负责。

## 3.4 第一阶段任务

### A1. 修复 spread 调用回归

当前 `SpreadCall`、`SpreadCallWithThis` 和 `SpreadConstruct` 只支持单个末尾 spread。

必须支持：

```js
f(1, ...a, 2, ...b);
obj.m(...a, 2);
new C(1, ...a, ...b);
```

建议不要继续扩充“第几个参数是 spread”的组合 opcode，而是统一生成 arguments list：

```text
ArrayCreateSparse(0)
ArrayPush / SpreadIntoArray
CallArgumentList
CallWithThisArgumentList
ConstructArgumentList
```

### A2. compound assignment 必须生成 Reference 语义

以下表达式必须只求值一次对象和属性键：

```js
obj[getKey()] += rhs;
obj.x ??= rhs;
super.x += rhs;
```

编译器不得把它们退化为两次 `GetProperty` 或绕过 getter/setter 的快捷路径。

### A3. ClassEvaluation lowering

把 statement class 与 expression class 统一到同一 lowering 路径，覆盖：

- computed property；
- private name；
- `super`；
- derived constructor；
- instance/static fields；
- static block；
- function `[[HomeObject]]`。

### A4. Parser early errors

优先处理当前新增的 “Expected SyntaxError but no exception”：

- direct eval 上下文；
- private identifiers；
- `new.target`；
- `super()` / `super.prop`；
- `await` / `yield`；
- lexical / var collision。

## 3.5 第一阶段目标

| 指标 | 目标 |
|---|---:|
| `compound-assignment` 失败 | 89 → 20 以下 |
| `arguments-object` 失败 | 70 → 20 以下 |
| class 两目录合计 | 357 → 260 以下 |
| parse-phase SyntaxError 漏报 | 至少减少 50 |
| 预期净增通过 | 100–180 |

---

# 4. 人员 B：运行时语义内核、对象模型、Eval、Module 与 Async

## 4.1 功能职责

负责所有运行期可观察语义：

- `Get / Set / HasProperty`；
- getter/setter 与 Proxy trap；
- `Call / Construct / IsCallable / IsConstructor`；
- property descriptor；
- environment 与声明实例化；
- direct/indirect eval 执行；
- Annex B block function；
- Promise 与 JobQueue；
- module linking/evaluation；
- dynamic import；
- top-level await；
- Object/Array/Function/Promise/Proxy builtins。

人员 B 同时担任本轮**共享接口维护人**，只负责合并接口 PR，不得在接口 PR 中夹带功能修复。

## 4.2 独占源码目录

```text
src/runtime/
src/vm/
src/backend/
src/builtins/object.rs
src/builtins/array.rs
src/builtins/function.rs
src/builtins/promise.rs
src/builtins/proxy.rs
src/test262/
```

建议第一步把 eval 从 `src/builtins/function.rs` 抽出：

```text
src/builtins/eval.rs
```

随后：

- `function.rs` 只负责 Function 构造器和 prototype；
- `eval.rs` 负责 EvalRequest、声明实例化和 eval 执行。

## 4.3 主验收 Test262 目录

```text
test262/test/annexB/language/
test262/test/language/eval-code/
test262/test/language/module-code/
test262/test/language/import/
test262/test/language/expressions/dynamic-import/

test262/test/built-ins/Object/
test262/test/built-ins/Array/
test262/test/built-ins/Function/
test262/test/built-ins/Promise/
test262/test/built-ins/Proxy/
```

辅助验收：

```text
test262/test/language/expressions/compound-assignment/
test262/test/language/arguments-object/
test262/test/staging/sm/
```

A 负责正确发出字节码；B 负责字节码执行的可观察行为。

## 4.4 第一阶段任务

### B1. 收回 Object descriptor 回归

集中检查：

```text
ToPropertyDescriptor
FromPropertyDescriptor
ValidateAndApplyPropertyDescriptor
OrdinaryDefineOwnProperty
Object.defineProperty
Object.defineProperties
Object.getOwnPropertyDescriptor
```

必须保证：

- getter/setter 非 callable 时抛 TypeError；
-缺失字段与显式 `false` 区分；
- configurable/enumerable/writable 默认值正确；
- abrupt completion 顺序正确；
- Proxy invariant 正确。

### B2. 统一 Runtime Semantic Kernel

所有 builtin 和 VM 指令必须通过：

```rust
abstract_ops::get
abstract_ops::set
abstract_ops::get_method
abstract_ops::call
abstract_ops::construct
abstract_ops::has_property
abstract_ops::to_primitive
abstract_ops::to_property_key_value
abstract_ops::to_string
```

禁止在可观察路径直接调用：

```rust
NativeContext::get_property
NativeContext::set_property
NativeContext::has_property
```

内部 slot 和明确的 own data property 读取除外。

### B3. EvalDeclarationInstantiation

统一实现：

```text
GlobalDeclarationInstantiation
EvalDeclarationInstantiation
FunctionDeclarationInstantiation
Annex B B.3.2 / B.3.3 block function semantics
```

重点修复：

```text
f is not defined
binding 未提前创建
binding 未初始化为 undefined
binding 未重新初始化
global property descriptor 错误
Identifier already declared 的时机错误
```

### B4. Promise / JobQueue / Module

dynamic import 必须：

1. 立即返回 pending Promise；
2. 把加载/链接/执行工作入队；
3. 通过 JobQueue settle；
4. 正确传播 abrupt completion；
5. TLA 完成后再 settle module evaluation promise。

禁止在 `dynamic_import()` 中同步执行整个模块并立即返回 settled Promise。

### B5. Call/Construct 接口

实现 A 新增的通用 argument-list opcode，并保证：

- 多个 spread；
- iterator abrupt close；
- getter/iterator 调用顺序；
- `this`；
- construct/newTarget；
- Proxy callable/constructable。

## 4.5 第一阶段目标

| 指标 | 目标 |
|---|---:|
| getter VM-path 错误 | 50 → 5 以下 |
| `built-ins/Object` | 187 → 110 以下 |
| `language/eval-code` | 164 → 90 以下 |
| `annexB/language` | 193 → 120 以下 |
| async 未完成 | 106 → 40 以下 |
| module/import/dynamic-import 合计 | 447 → 320 以下 |
| 预期净增通过 | 120–220 |

---

# 5. 人员 C：Date、Temporal 与 Intl

## 5.1 功能职责

负责日期与国际化功能的完整垂直链路：

- Date；
- Temporal；
- Intl.DateTimeFormat；
- Intl.NumberFormat；
- Intl.DurationFormat；
- Intl.Segmenter；
- Temporal 与 Intl bridge；
- ISO date/time、duration、rounding 和 options 共享算法。

人员 C 不修改 VM、Runtime、Parser 和 Compiler；缺少能力时通过接口 issue 交给 A/B。

## 5.2 独占源码目录

当前：

```text
src/builtins/date_intl.rs
```

必须优先拆分为：

```text
src/builtins/date_intl/
├─ mod.rs
├─ date.rs
├─ shared.rs
├─ temporal/
│  ├─ mod.rs
│  ├─ records.rs
│  ├─ fields.rs
│  ├─ iso.rs
│  ├─ duration.rs
│  ├─ rounding.rs
│  ├─ time_zone.rs
│  └─ calendar.rs
└─ intl/
   ├─ mod.rs
   ├─ options.rs
   ├─ locale.rs
   ├─ date_time_format.rs
   ├─ number_format.rs
   ├─ duration_format.rs
   └─ segmenter.rs
```

拆分只移动代码，不改变行为，单独提交一个 refactor PR。

## 5.3 主验收 Test262 目录

```text
test262/test/built-ins/Date/
test262/test/built-ins/Temporal/

test262/test/intl402/Temporal/
test262/test/intl402/DateTimeFormat/
test262/test/intl402/NumberFormat/
test262/test/intl402/DurationFormat/
test262/test/intl402/Segmenter/
```

当前失败池：

| 目录 | 失败数 |
|---|---:|
| `built-ins/Temporal` | 707 |
| `intl402/Temporal` | 1,317 |
| `intl402/DateTimeFormat` | 168 |
| `intl402/NumberFormat` | 169 |
| `intl402/DurationFormat` | 110 |
| `intl402/Segmenter` | 77 |
| `built-ins/Date` | 36 |

## 5.4 第一阶段任务

### C1. 纯算法与可观察算法分离

纯算法不得访问 `Vm` 或 `NativeContext`：

```rust
IsoDate
IsoTime
IsoDateTime
DurationRecord
RoundingMode
Overflow
BalanceISODate
RegulateISODate
ISODateToEpochDays
EpochDaysToISODate
```

可观察算法必须显式接收 VM/Context：

```rust
prepare_temporal_fields(vm, ctx, ...)
get_options_object(vm, ctx, ...)
get_temporal_calendar_identifier_with_iso_default(vm, ctx, ...)
```

### C2. 所有 property bag 统一入口

PlainDate、PlainDateTime、PlainYearMonth、PlainMonthDay、ZonedDateTime 不得各自复制字段读取与 month/monthCode 校验。

统一读取顺序、转换和 abrupt completion。

### C3. Duration 内核

优先修复：

- sign consistency；
- balance/unbalance；
- total；
- round；
- relativeTo；
- largestUnit/smallestUnit；
- overflow；
- BigInt/Number 边界。

### C4. Intl bridge

Intl 不得重新实现 Temporal 校验，应消费 canonical Temporal record。

优先把已在 built-ins 生效的日期/月字段修复传导到：

```text
Intl.DateTimeFormat + Temporal
Temporal.ZonedDateTime formatting
PlainDate / PlainDateTime / PlainYearMonth formatting
```

## 5.5 第一阶段目标

| 指标 | 目标 |
|---|---:|
| `built-ins/Temporal` | 707 → 500 以下 |
| `intl402/Temporal` | 1,317 → 1,150 以下 |
| DateTimeFormat/NumberFormat/DurationFormat 合计 | 至少减少 60 |
| 预期净增通过 | 180–300 |

---

# 6. 暂停项

本轮三人都不主动投入：

```text
test262/test/built-ins/Atomics/
test262/test/built-ins/RegExp/unicodeSets/
test262/test/built-ins/RegExp/property-escapes/
test262/test/built-ins/RegExp/regexp-modifiers/
test262/test/staging/sm/
```

原因：

- Atomics 需要跨 context SharedArrayBuffer 和完整 Agent host；
- RegExp 大簇受后端能力限制；
- staging 是正式功能的重复表现。

---

# 7. 强制共享文件规则

以下文件为热点文件：

```text
src/contracts.rs
src/lib.rs
src/builtins/mod.rs
src/runtime/mod.rs
src/bytecode/opcode.rs
src/bytecode/chunk.rs
src/backend/mod.rs
```

规则：

1. 普通功能 PR 不得修改这些文件。
2. 需要改动时先开 `interface/*` 分支。
3. 接口 PR 只包含：
   - 类型；
   - trait；
   - opcode 声明；
   - stack effect；
   - stub；
   - 单元测试。
4. 接口 PR 必须由另外两人 review 后先合入 main。
5. 三条功能分支 rebase 到接口 commit 后继续开发。
6. 不允许在功能 PR 中同时改变接口定义和实现语义。

---

# 8. Git 分支与合并流程

## 8.1 建立准确基线

测试日志没有自动记录 commit SHA，因此先执行：

```bash
git status
git rev-parse HEAD
git tag test262-45990-8616 <产生本次日志的SHA>
```

三条分支必须来自同一 SHA：

```text
fix/lang-bytecode
fix/runtime-module
fix/temporal-intl
```

共享接口使用：

```text
interface/v18-contracts
```

## 8.2 合并顺序

第一轮：

```text
interface/v18-contracts
        ↓
A/B/C 全部 rebase
        ↓
fix/runtime-module
        ↓
fix/lang-bytecode
        ↓
fix/temporal-intl
```

如果 A 的 opcode 依赖 B 的 VM 实现：

1. 接口 PR 先添加 opcode 与 stack effect；
2. B 添加 VM stub/实现；
3. A 添加 compiler emission；
4. 两个 PR 均以 targeted test 为准；
5. 最后合并完整功能。

## 8.3 每个 PR 必须提交的数据

```text
Base SHA:
Head SHA:
Owned source files changed:
Shared files changed: none / interface PR number

Target suite before:
Target suite after:
Resolved:
Regressed:
Unchanged:
New error signatures:
cargo test:
cargo clippy:
```

合并条件：

- `cargo fmt --check`；
- `cargo test` 全过；
- `cargo clippy` 无新增 warning；
- 主验收目录没有新回归；
- 共享目录回归不超过 3 项且有明确原因；
- 不允许只报告“新增通过数”，必须报告 resolved/regressed。

---

# 9. 每日协作节奏

每天只进行一次主干集成：

1. 上午：各自 rebase 基线；
2. 白天：仅跑 targeted suite；
3. 提交 PR 前：跑自己主目录和相邻共享目录；
4. 晚上：合并后统一跑一次完整 Test262；
5. 第二天按全量 diff 调整任务。

完整测试报告至少输出：

```text
total / passed / failed / skipped
level-1 directory diff
level-2 directory diff
resolved tests
new regressions
error signature before/after
wall-clock timeout
async incomplete
```

---

# 10. 首轮任务拆分摘要

| 人员 | 第一主任务 | 第二主任务 | 禁止越界 |
|---|---|---|---|
| A | spread + compound assignment lowering | class + parser early error | 不直接修改 Runtime/VM |
| B | Object descriptor + semantic kernel | eval/Annex B + module/async | 不直接修改 Parser/Compiler |
| C | Temporal shared core + Duration | Intl Temporal bridge | 不直接修改 VM/Runtime/Parser |

首轮完成后，合理目标是净增约 **400–700** 项；这是工程预期区间，不是保证值。若某条路线连续两批净增低于 20 且错误签名没有显著下降，应暂停并重新聚类。
