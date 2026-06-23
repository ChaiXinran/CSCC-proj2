V6 的 `34.97%` 并不代表核心 Builtins 只有三成正确。扫描覆盖了完整官方目录，其中大量测试依赖 V6 明确延期的语法、RegExp、Symbol、Proxy 和高级 JSON。

## 各目录结果

| 目录 | 通过 | 失败 | 跳过 | 通过率 |
|---|---:|---:|---:|---:|
| Number | 198 | 82 | 60 | 58.24% |
| Boolean | 23 | 18 | 10 | 45.10% |
| Math | 120 | 48 | 159 | 36.70% |
| String | 365 | 697 | 161 | 29.84% |
| JSON | 47 | 90 | 28 | 28.48% |
| Error | 16 | 32 | 45 | 17.20% |

String 占了 697 个失败，是总失败的主要来源。

## 967 个失败的原因

| 原因 | 数量 | 占失败比例 |
|---|---:|---:|
| Lexer/Parser 不支持相关语法 | 491 | 50.8% |
| Builtin 语义断言不一致 | 187 | 19.3% |
| 方法尚未实现 | 125 | 12.9% |
| 依赖其他全局对象 | 118 | 12.2% |
| 异常类型或触发时机错误 | 45 | 4.7% |
| 动态 Function 不支持 | 1 | 0.1% |

### 前端语法问题

主要包括：

- 正则表达式字面量
- 模板字符串和 `String.raw`
- `void`
- 箭头函数、解构、spread
- 部分立即调用函数表达式和旧式测试语法
- 更完整的数字和字符串字面量

其中 String 的 match、replace、search、split 测试大量依赖 RegExp，因此即使 String 基础方法正确也无法运行。

### 缺少的其他能力

常见未定义全局对象：

- `Symbol`：53
- `Proxy`：17
- `RegExp`：14
- `$262`：9
- `eval`：7
- `Date`：2

这些多数不属于 V6 核心范围。

### 尚未实现的方法

集中在：

- `String.prototype.split`
- `replace`、`replaceAll`
- `match`、`matchAll`、`search`
- locale 大小写转换
- `localeCompare`
- `normalize`
- `codePointAt`
- `String.raw`
- `Error.isError`
- `Math.sumPrecise`
- `JSON.rawJSON/isRawJSON`

## 已实现功能中的真实语义问题

这些值得优先修复：

- Number 字符串转换不支持完整十六进制、二进制、八进制和 ECMAScript 空白字符。
- Number 格式化的舍入规则不完全符合规范。
- `Object.prototype.toString` 缺少 Boolean、Number、Error 等内建标签。
- `Math.hypot()` 无参数结果错误。
- `Math.round` 的正负零边界仍有偏差。
- Math 对象原型链接不正确。
- JSON 尚未支持 reviver、replacer、space、`toJSON` 和完整属性顺序。
- Rust `String` 无法无损保存孤立 UTF-16 surrogate，部分 String/JSON Unicode 测试必然失败。
- 异常对象身份与异常构造器匹配还不完整。

## 463 个跳过的原因

几乎全部来自 Test262 harness 未接入：

- `propertyHelper.js`：307
- `isConstructor.js`：100
- `compareArray.js`：28
- `nativeErrors.js`：13

前三项占全部跳过的约 94%。接入这些 helper 后测试会真正执行，但不保证立即通过。

## 建议修复顺序

1. 修复 Number 字符串转换、Math 三个边界和内建对象标签。
2. 接入 `propertyHelper.js`、`isConstructor.js`、`compareArray.js`。
3. 完善 JSON reviver/replacer/space。
4. 实现 String `codePointAt`、`normalize` 和 locale 方法。
5. 再决定是否提前实现 RegExp；它能一次解锁大量 String 测试。
6. 最后处理 Symbol、Proxy、跨 Realm 和动态 Function。

当前 V6 固定门 `7/7` 是可靠的；目录扫描低主要是测试范围远大于 V6 核心范围，但也确实暴露了 Number、Math、对象标签和 JSON 的若干规范缺口。

# Native V6 Test262 修复后前端语法分析

## 当前结果

本轮优先修复 Lexer、Parser 以及可直接复用现有 VM 的基础语义。

| 阶段 | 通过 | 失败 | 跳过 | 通过率 |
|---|---:|---:|---:|---:|
| 修复前基线 | 769 | 967 | 463 | 34.97% |
| 第一批：ASI、`void`、字符串转义、Unicode 标识符 | 906 | 830 | 463 | 41.20% |
| 第二批：进制数字、计算属性、方法简写 | 922 | 814 | 463 | 41.93% |
| 第三批：基础箭头函数 | 941 | 795 | 463 | 42.79% |
| 第四批：`==` / `!=` 与抽象相等 | 951 | 785 | 463 | 43.25% |

累计新增 **182** 个通过用例，失败数减少 **182**，跳过数未变化。

## 已完成的前端能力

- 基础自动分号插入：换行处、EOF、右花括号前。
- `void` 一元表达式，保留操作数副作用并产生 `undefined`。
- `\xNN`、`\uNNNN`、Unicode 码点和代理对字符串转义。
- Unicode 标识符及标识符中的 Unicode 转义。
- `0x`、`0b`、`0o` 数字字面量。
- 对象方法简写和计算属性名。
- 基础箭头函数：单参数、参数列表、表达式体、块体。
- `==`、`!=` 的解析、字节码和基础抽象相等转换。

## 剩余主要前端阻塞

最新失败聚类中，较大的语法簇为：

- `for (var key in object)`：108 个。需要属性枚举和循环执行支持，不能只改 Parser。
- 正则字面量：67 个。需要 RegExp 值、构造器和 String 正则方法联动。
- 模板字符串：10 个。需要模板 AST、插值求值和 `String.raw`。
- BigInt 字面量（如 `1n`）：约 14 个。当前运行时没有 BigInt 值类型。
- 剩余反斜杠错误：24 个，主要涉及更完整的 Unicode/旧式转义规则。
- 部分计算属性仍失败：9 个，多与 Symbol、生成器或其他未实现语法组合出现。

## 结论与建议

低风险、可独立落地的前端修复已完成。下一步应优先实现
`for...in` 的枚举协议；它是当前最大的单一语法阻塞。随后再决定是否提前引入
RegExp。模板字符串和 BigInt 应各自作为独立里程碑，避免把新的值类型和现有
V6 Builtins 修复混在同一批提交中。

本轮验证命令：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run --release -- test262 --native-v6-scan --jobs 4 \
  --json reports/native-v6-frontend-summary.json
```

---

# Native V6 Test262 —— `for` / `for-in` 落地后的最新分析（2026-06-21）

> 数据来源：`reports/native-v6-frontend-summary.json`（本轮 `--native-v6-scan` 输出）。
> 说明：`--native-v6-scan` 走的是 native 后端，与 Boa 无关。本机 `target/` 中的
> `boa_engine` 是 Linux 目标的缓存（windows-msvc 链接会报 E0461），因此本轮用
> `--no-default-features` 绕开 Boa 编译；native 扫描结果与带 Boa 编译时**完全等价**。

## 当前结果

在前两版前端修复（ASI、`void`、字符串转义、Unicode 标识符、进制数字、计算属性、
方法简写、基础箭头函数、`==`/`!=`）的基础上，本版新增了 **`for` / `for-in` / `++` `--`**
的全链路支持（lexer → parser → bytecode → vm → runtime，新增 `ForInKeys` 指令与
`for_in_keys` 枚举），并接入了 Test262 harness includes（native 后端现在会 eval
`propertyHelper.js` 等）。

| 目录 | 通过 | 失败 | 通过率 |
|---|---:|---:|---:|
| Number | 251 | 89 | 73.82% |
| Boolean | 37 | 14 | 72.55% |
| String | 768 | 455 | 62.80% |
| Math | 197 | 130 | 60.24% |
| JSON | 74 | 91 | 44.85% |
| Error | 33 | 60 | 35.48% |
| **合计** | **1360** | **838** | **61.85%**（skipped=1） |

整体演进：基线 `769 (34.97%)` → 前端四批 `951 (43.25%)` → **本版 `1360 (61.85%)`**。
其中 `for`/`for-in` 让 `propertyHelper.js` 从「SyntaxError 解析失败」变为「能解析、能 eval」，
是本轮跳过归零、通过率跃升的关键前提。门禁 V1–V6 仍为 100%，零回归。

## 838 个失败的原因分类

| 原因 | 数量 | 占失败比 |
|---|---:|---:|
| harness include 运行时失败（几乎全是 `propertyHelper.js`） | 318 | 37.9% |
| 依赖未定义的全局对象 | 181 | 21.6% |
| Builtin 语义/断言不一致（真实 bug） | 171 | 20.4% |
| 剩余语法 parse error | 140 | 16.7% |
| 在原始值上访问属性等 | 20 | 2.4% |
| 方法缺失 / 不可调用 | 8 | 1.0% |

### ① `propertyHelper.js` 缺 `Function.prototype.bind`（316，最大单点）

`propertyHelper.js` 顶部即执行 `Function.prototype.call.bind(...)`、
`Function.prototype.call.bind(Array.prototype.join)` 等，而 **`Function.prototype.bind`
尚未实现**，于是 helper 加载阶段就抛 `TypeError: undefined is not callable`，
连带 **316 个** 依赖 `verifyProperty` 的用例全部失败。这是当前**收益最高的单点**。

### ② 未定义全局对象（181）

| 全局 | 数量 |
|---|---:|
| `Symbol` | 87 |
| `RegExp` | 29 |
| `Proxy` | 18 |
| `$262` | 10 |
| `eval` | 10 |
| `Date` | 3 |
| 其它（测试内未声明标识符等） | ~24 |

`Symbol` 是大头（`@@iterator`/`@@toPrimitive` 等 well-known symbol 在大量 builtin 测试里出现）；
`RegExp` 解锁 String 的 `match`/`replace`/`split`。多数不属于 V6 核心，应按独立里程碑推进；
其中 `$262`（10）属于 harness host 对象，成本低、可优先补。

### ③ Builtin 语义/断言不一致（171，真实 bug）

集中在两个低分目录 **Error（35.5%）** 和 **JSON（44.9%）**，Math 也有一批。这些是
**已实现方法**的边界/规范偏差，例如：Error 原型链与 `toString`、JSON 的
`reviver`/`replacer`/`space`/属性顺序、Math 特殊值（`±0`、`NaN`、空参）。属 C 组应优先修的真实缺口。

### ④ 剩余语法 parse error（140）

| 语法 | 数量 | 说明 |
|---|---:|---|
| 正则字面量 `/.../`（`unexpected /`） | 68 | 需 RegExp 值 + 词法层的正则识别 |
| `unexpected =` | 26 | 复合赋值 `+=`/`-=`/`*=` 等、解构默认值 |
| BigInt `1n`（`found identifier n`） | ~19 | 运行时无 BigInt 值类型 |
| 计算属性等 `[`（`expected property name, got [`） | 9 | 对象字面量部分计算键形式 |
| `function*` / `extends`（`*`、class） | ~3 | generator / class |

## 建议修复顺序（更新）

1. **（builtin，最高收益）实现 `Function.prototype.bind`**，并补齐它依赖的
   `Array.prototype.join`/`push`、`Object.prototype.hasOwnProperty`/`propertyIsEnumerable`。
   一次性可解锁约 **300** 个 propertyHelper 用例。
2. **（builtin 语义）修 Error 与 JSON 的真实缺口**：Error 原型链/`toString`、
   JSON `reviver`/`replacer`/`space`/属性顺序、Math 特殊值边界。
3. **（前端，低风险）复合赋值 `+=`/`-=` 等**；BigInt 字面量作为独立里程碑（需新值类型）。
4. **（大里程碑）RegExp**（解锁正则字面量 68 + String 正则方法）与 **Symbol**。
5. 最后处理 `$262`（低成本，先补）、`Proxy`、`eval`、`Date`。

## 本轮验证命令

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
# 因本机 target 中 boa 为 Linux 目标缓存，native 扫描用 --no-default-features 绕开 Boa：
cargo run --release --no-default-features -- test262 --native-v6-scan --jobs 4 \
  --json reports/native-v6-frontend-summary.json
```

---

# 三人并行修复方案（基于「建议修复顺序」，2026-06-21）

## 进度基线

- **已完成**：建议顺序 #1 `Function.prototype.bind`（及 `Array.join/push`、`Object.hasOwnProperty/propertyIsEnumerable` 经核实已存在）。`propertyHelper.js` 的 harness 加载失败从 **318 → 2**。
- **当前卡点**：`propertyHelper.js` 的 `verifyProperty` 第一行 `arguments.length > 2` 撞上引擎**不支持 `arguments` 对象**，**279 个**用例卡在 `arguments is not defined`。这是 bind 之后最大的单点。
- 当前扫描：`1360 / 2199 = 61.85%`。

下面把建议顺序 #2、#3 与紧接的 `arguments` 拆成**三条互不冲突**的并行轨道。

## 三条并行轨道

### 轨道 A —— `arguments` 对象（C 组：vm / runtime）｜最高收益

| 项 | 内容 |
|---|---|
| 目标 | 用户函数体内可用 `arguments`：类数组、`length`、按索引取实参 |
| 收益 | 解锁约 **280** 个 propertyHelper 用例（其中一批会进一步暴露真实断言，由轨道 B 接力） |
| 实现要点 | ① `vm/interpreter.rs::call_user_function` 进入函数时，用调用实参构造一个 `arguments` 对象并在函数环境声明绑定 `arguments`；② 简化实现用普通数组对象（不做 mapped arguments / `callee`），`length` = 实参个数；③ **无需改 lexer/parser** —— `arguments` 是普通标识符，函数内 `compile_identifier` 已 emit `LoadName "arguments"`；④ 箭头函数不应有自己的 `arguments`（当前箭头实现简单，先记 TODO） |
| 涉及文件 | `src/vm/interpreter.rs`、`src/runtime/context.rs`（如需 arguments 工厂）、`src/runtime/function.rs` |
| 验收 | 新增 `tests/native_arguments.rs`（`arguments.length`、`arguments[i]`、转发求和）；扫描中 `arguments is not defined` ≈ 0 |
| 边界 | 只动 VM call path 与 runtime，不碰 builtins / 前端 |

### 轨道 B —— Error / JSON / Math 语义修复（C 组：builtins）｜修真实 bug

| 项 | 内容 |
|---|---|
| 目标 | 提升三个低分目录：Error 35.5%、JSON 44.9%、Math 60.2% |
| 收益 | 188 个「断言/语义不一致」大多在此，预计可修 **80–120** |
| 实现要点 | **Error**：子类原型链（`TypeError.prototype → Error.prototype`）、`Error.prototype.toString`、`name`/`message` 描述符；**JSON**：`parse` 的 reviver、`stringify` 的 replacer / space / `toJSON` / 属性顺序 / 循环检测；**Math**：特殊值（`±0`、`NaN`、空参 `hypot/max/min`）、`round` 负零边界 |
| 涉及文件 | `src/builtins/{error.rs, json.rs, math.rs}`、`src/builtins/v6.rs`（对应 wiring 段） |
| 验收 | 各目录 `--suite test/built-ins/{Error,JSON,Math}` 通过率上升；扩展 `tests/native_v6_*` |
| 边界 | 只动 builtins，不碰 VM core / 前端。内部可再按 Error / JSON / Math 三小块细分 |

### 轨道 C —— 复合赋值 + 计算属性键（A 组：lexer/parser，+ B 组 compiler）｜低风险解锁

| 项 | 内容 |
|---|---|
| 目标 | `+=` `-=` `*=` `/=` `%=` 复合赋值；对象字面量计算属性键 `{[expr]: v}` |
| 收益 | parse error 中复合赋值 ~26 + 计算属性 ~9 ≈ **35**，且复合赋值在 builtin 测试里极常见，连带效应更大 |
| 实现要点 | ① `lexer`：`OPERATORS` 加 `+=`/`-=`/`*=`/`/=`/`%=`（置于单字符运算符之前，保证最长匹配）；② `ast`：`Assignment` 增加可选 `operator`（或新增 `CompoundAssignment`）；③ `parser::parse_assignment` 识别复合赋值；④ `compiler::compile_assignment` 复合赋值 = `load target → 运算 → store`（与 `compile_update` 同套路）；⑤ 计算属性键 `[expr]:` 的 parser + 字节码（沿用 `SetElement`/`DefineElement`） |
| 涉及文件 | `src/lexer/mod.rs`、`src/ast/expression.rs`、`src/parser/expression.rs`、`src/bytecode/compiler.rs` |
| 验收 | 新增复合赋值 / 计算属性测试；对应 parse error 降到 0 |
| 边界 | 前端 + compiler。**BigInt 不在本轨道**（需新值类型，独立里程碑） |

## 防冲突约定

| 维度 | 约定 |
|---|---|
| 文件所有权 | A=`vm/interpreter.rs` + `runtime/*`；B=`builtins/{error,json,math}.rs` + `v6.rs`；C=`lexer` + `ast` + `parser` + `bytecode/compiler.rs` |
| 唯一交汇点 | `bytecode/compiler.rs`：A（若给 `arguments` 加注入逻辑）集中在 `call`/`compile_function` 区；C 集中在 `compile_assignment`。**两人改不同函数**，合并时按函数粒度即可 |
| 契约先行 | 若 A 需新 VM 指令（如 `CreateArguments`）或 C 需新指令（计算属性多半可复用现有 `DefineElement`/`SetElement`，不必新增），**先合并 `opcode.rs`/`contracts.rs` 的契约 PR**，再并行实现（遵循 CLAUDE.md：契约变更先合并） |
| `runtime/context.rs` | 仅 A 可能新增 `arguments` 工厂方法；B/C 不动该文件，避免三方争用 |
| 验证（统一，每人提交前跑） | `cargo fmt --all -- --check`；`cargo clippy --no-default-features --all-targets -- -D warnings`；`cargo test --no-default-features --all-targets`；`cargo run --release --no-default-features -- test262 --native-v6-scan --jobs 4`。**门禁 V1–V6 必须 100% 零回归** |
| Boa 环境 | 本机 `target/` 的 `boa_engine` 是 Linux 目标缓存，统一用 `--no-default-features` 验证 native；**建议开工前由任一人跑一次 `cargo clean && cargo build --release`** 修复 Boa，让默认门禁恢复（与三轨道独立） |
| 合并顺序 | A 最独立、收益最高，建议先落地；B、C 并行；任何动 `opcode/contracts` 的契约 PR 最先合并 |

## A 与 B 的协同

轨道 A 解锁 `arguments` 后，propertyHelper 的 `verifyProperty` 会真正执行，**把一批用例从「arguments 未定义」推进到「描述符/语义断言」**——这些正是轨道 B 的修复目标。因此 **A+B 有叠加放大效应**：单看 A 解锁的部分用例可能仍因断言失败，需 B 接力才转为通过。建议 A 先合并、B 紧随。

## 不在本轮（更大里程碑，后续单独立项）

- **RegExp**：正则字面量 68 + String 的 `match/replace/split` 等，跨 lexer（正则识别）+ runtime（RegExp 值）+ builtins，是最大单块。
- **Symbol**（92）：新原始值类型，跨 runtime/vm/builtins。
- **BigInt**（19）：新值类型。
- **Proxy / eval / Date**；其中 **`$262`**（10）成本低，可顺带补。

## 预期收益（粗估）

| 轨道 | 解锁量级 | 说明 |
|---|---:|---|
| A `arguments` | ~280 | propertyHelper 主体放行（部分需 B 接力） |
| B Error/JSON/Math | ~80–120 | 真实语义修复 |
| C 复合赋值 + 计算属性 | ~35 | 低风险前端 |

三者叠加后，V6 扫描通过率有望从 **61.85%** 升至 **~75%+**（A 与 B 的协同会进一步放大）。
