# `detail_latest.txt` Test262 共性错误分析

## 基线

- 输入：`detail_latest.txt`
- 总数：53,379
- 通过：45,755
- 失败：7,622
- 跳过：2
- 正确率：85.72%

统计以每条 `FAIL` 记录的首个可见错误为准。分类是根因排查用的启发式聚类，
不是 ECMAScript 规范章节分类。

## 共性错误

| 错误簇 | 数量 | 主要所属文件夹 |
|---|---:|---|
| 可观察结果或执行顺序错误 | 2,178 | `intl402/Temporal/ZonedDateTime` (336)、`intl402/Temporal/PlainDateTime` (146)、`intl402/Temporal/PlainDate` (144) |
| Parser / RegExp 语法支持缺口 | 1,056 | `built-ins/RegExp/unicodeSets` (105)、`language/import/import-defer` (86)、`built-ins/RegExp` (75) |
| 应抛异常但未抛，或异常类型错误 | 876 | `built-ins/Temporal/Duration` (63)、`built-ins/Temporal/ZonedDateTime` (50)、`language/statements/class` (41) |
| 缺失或未定义运行时能力 | 464 | `intl402/DurationFormat/prototype` (62)、`built-ins/ShadowRealm/prototype` (51)、`built-ins/FinalizationRegistry/prototype` (30) |
| 缺少 parse phase early error | 186 | `language/block-scope/syntax` (40)、`language/expressions/object` (23)、`language/statements/switch` (15) |
| Temporal 日期范围/转换校验 | 174 | `intl402/Temporal/ZonedDateTime` (36)、`intl402/Temporal/PlainYearMonth` (30)、`intl402/Temporal/PlainDate` (20) |
| Temporal `monthCode` 校验 | 107 | `intl402/Temporal/PlainDate` (26)、`intl402/Temporal/PlainDateTime` (26)、`intl402/Temporal/ZonedDateTime` (26) |
| 异步测试未完成 | 106 | `language/expressions/dynamic-import` (66)、`language/module-code/top-level-await` (16) |
| 超时 | 68 | Temporal 的 PlainDate、PlainDateTime、ZonedDateTime 各 12 条最集中 |
| 属性定义失败 | 48 | `built-ins/Temporal/PlainDateTime` (11)、`built-ins/Temporal/PlainDate` (10) |
| 编译器明确不支持 | 23 | `language/expressions/assignment` (20) |
| 其他未归一化错误 | 2,336 | `built-ins/Function/prototype` (69)、`language/statements/class` (68)、`intl402/Temporal/ZonedDateTime` (66) |

## 失败最多的具体目录

| 文件夹 | 失败数 |
|---|---:|
| `intl402/Temporal/ZonedDateTime` | 505 |
| `intl402/Temporal/PlainDateTime` | 280 |
| `intl402/Temporal/PlainDate` | 268 |
| `language/statements/class` | 211 |
| `built-ins/Temporal/ZonedDateTime` | 183 |
| `built-ins/Temporal/Duration` | 173 |
| `language/expressions/class` | 164 |
| `intl402/Temporal/PlainYearMonth` | 148 |
| `intl402/NumberFormat/prototype` | 140 |
| `language/expressions/dynamic-import` | 137 |

## 修复顺序

1. Temporal 日期/时间转换和范围边界：共享实现、影响多个高失败目录，且可用
   小范围文件夹回归精确验证。
2. Temporal `monthCode`、property bag 读取顺序和异常优先级。
3. Parser early errors：按 `block-scope/syntax`、object、switch 分批处理。
4. RegExp `unicodeSets` 和 Annex B escape。
5. Dynamic import / top-level await 的异步完成协议。
6. ShadowRealm、FinalizationRegistry 等缺失运行时表面。

## 本轮已验证的第一批修复

| 文件夹 | 修复前 | 修复后 | 变化 |
|---|---:|---:|---:|
| `built-ins/Temporal/PlainDate` | 516/652 (79.14%) | 527/652 (80.83%) | +11，0 回退 |
| `built-ins/Temporal/PlainDateTime` | 641/773 (82.92%) | 642/773 (83.05%) | +1，0 回退 |
| `built-ins/Temporal/PlainTime` | 426/493 (86.41%) | 429/493 (87.02%) | +3，0 回退 |

修复内容：

- PlainDate 和 PlainDateTime 构造器的日期数字先向零截断。
- PlainDate 日期范围按规范边界
  `-271821-04-19` 到 `275760-09-13`（含端点）判断。
- PlainTime / PlainDateTime 的显式 `undefined` 时间字段按 0 处理。

## 第二批：Temporal `monthCode`

共享 property bag 读取现在要求 `monthCode` 必须是字符串，区分格式错误与
日历适用性错误，并在 ISO 日历中拒绝第 13 月及闰月代码。异常顺序保持为：
先校验 month code 语法，再转换 year，最后判断日历适用性。

| 文件夹 | 修复前 | 修复后 | 变化 |
|---|---:|---:|---:|
| `built-ins/Temporal/PlainDate` | 527/652 (80.83%) | 529/652 (81.13%) | +2，0 回退 |
| `built-ins/Temporal/PlainDateTime` | 642/773 (83.05%) | 644/773 (83.31%) | +2，0 回退 |
| `built-ins/Temporal/PlainYearMonth` | 385/509 (75.64%) | 386/509 (75.83%) | +1，0 回退 |
| `built-ins/Temporal/PlainMonthDay` | 152/199 (76.38%) | 152/199 (76.38%) | 持平，0 回退 |

`built-ins/Temporal/PlainDate/from/monthcode-invalid.js` 已整文件通过。

## 第三批：Parser early errors

本批将声明冲突和对象方法的静态语义集中到共享校验入口，避免普通方法、计算
属性方法、async 方法和 generator 方法走不同的 early-error 路径。

| 文件夹 | 修复前 | 修复后 | 变化 |
|---|---:|---:|---:|
| `language/block-scope/syntax` | 73/113 (64.60%) | 113/113 (100%) | +40，0 回退 |
| `language/block-scope` | 103/145（按旧日志 42 失败推算） | 143/145 (98.62%) | +40，剩余 2 |
| `language/expressions/object` | 1075/1170 (91.88%) | 1090/1170 (93.16%) | +15，0 回退 |
| `language/statements/switch` | 旧日志 40 失败 | 86/112，26 失败 | 失败减少 14 |

共享修复：

- Annex B sloppy-block 重复声明例外只适用于普通 FunctionDeclaration；
  async/generator 声明仍遵守词法重复规则。
- FunctionDeclaration 与同一 block 的 `var`（包括嵌套 block 中的 var）冲突
  始终是 parse-phase SyntaxError。
- 对象方法统一执行 UniqueFormalParameters、非简单参数加 `"use strict"`、
  参数与函数体词法声明冲突及直接 `super()` 校验。
- `yield` 与委托星号之间出现换行时立即产生 SyntaxError。

## 第四批：Async arrow 静态语义

async arrow 的参数解析过去发生在 async 上下文启用之前，但完成解析后没有执行
完整的静态语义检查。本批在确认 `async ... =>` 后统一验证参数 token 区间和
生成的参数/函数体 AST。

| 文件夹 | 修复前 | 修复后 | 变化 |
|---|---:|---:|---:|
| `language/expressions/async-arrow-function` | 42/60 (70.00%) | 51/60 (85.00%) | +9，0 回退 |
| `language/expressions/arrow-function` | 330/343 | 330/343 | 持平 |
| `language/expressions/async-function` | 69/93 | 69/93 | 持平 |
| `language/expressions/async-generator` | 旧日志 49 失败 | 575/623，48 失败 | 无回退 |

修复覆盖：

- async arrow 参数中直接或嵌套出现 `await`；
- async arrow 重复参数；
- async arrow 参数名与函数体 lexical declaration 冲突；
- async 函数体中嵌套普通 arrow 的参数默认值包含 `await`。
