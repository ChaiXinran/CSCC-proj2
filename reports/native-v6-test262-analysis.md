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
