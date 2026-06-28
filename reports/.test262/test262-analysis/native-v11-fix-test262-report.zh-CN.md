# Native V11 Fix Test262 报告

## 范围

本报告记录 V11 fixbug 后 Native 后端的 Test262 测试结果。

报告结构参考 `reports/native-v7-test262-report.md`，但本次运行不是 V7 中那种小规模 pinned gate，而是更宽的全量诊断扫描：测试入口覆盖 `test262/test`，并通过自研 Native lexer、parser、bytecode compiler、VM、runtime、builtin、object model 与 Test262 host skeleton 执行。测试口径下没有把 Boa 作为 Native 后端的 fallback。

因此，本报告应理解为全量诊断基线，而不是零失败验收门禁。

## 测试命令

日志中捕获到的命令为：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

日志中的构建/运行信息：

- 从 `D:\00_OS\CSCC` 编译 `agentjs v0.1.0`。
- release profile 构建成功，用时 `33.87s`。
- 实际执行命令为 `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`。

## 总体结果

上传日志最后停在 `[53377/53379 100.0%] pass=19027 fail=34348 skip=2`。也就是说，runner 的总分母是 53,379，但最后一条可见进度只统计到 53,377 个处理结果。因此，本报告将下面的数据视为“最后捕获状态”，不虚构缺失的最终 summary。

| 选择测试总数 | 最后捕获处理数 | 通过 | 失败 | 跳过 | 通过率 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |

跳过项不计入通过。通过率按照 `passed / total selected` 计算，与 V7 报告中的统计口径一致。

## 相比上一版 V11 全量基线的变化

上一版 V11 全量日志最后停在 `[53377/53379] pass=18050 fail=35325 skip=2`。本次 fixbug 后，通过数净增 977。

| 运行 | 选择测试总数 | 最后捕获处理数 | 通过 | 失败 | 跳过 | 通过率 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 上一版 V11 全量基线 | 53,379 | 53,377 | 18,050 | 35,325 | 2 | 33.81% |
| V11 fixbug 本次运行 | 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |
| 变化 | 0 | 0 | +977 | -977 | +0 | +1.83 pp |

按照失败记录对比，旧失败中有 1,151 条消失，同时新增 174 条失败记录，因此净减少 977 条失败。

## Fix 影响最大的区域

失败数下降最明显的区域集中在语言语法/语义与 RegExp property escapes。

| 区域 | 失败数变化 |
| --- | ---: |
| `language/expressions` | -409 |
| `built-ins/RegExp` | -261 |
| `language/statements` | -253 |
| `language/literals` | -13 |
| `language/destructuring` | -12 |
| `built-ins/Reflect` | -9 |
| `staging/sm` | -8 |
| `built-ins/Temporal` | -7 |
| `built-ins/TypedArrayConstructors` | -2 |
| `annexB/built-ins` | -2 |

更细的三级路径下降如下：

| 子区域 | 失败数变化 |
| --- | ---: |
| `built-ins/RegExp/property-escapes` | -261 |
| `language/expressions/class` | -210 |
| `language/statements/function` | -100 |
| `language/expressions/function` | -92 |
| `language/expressions/object` | -63 |
| `language/statements/for-of` | -45 |
| `language/expressions/arrow-function` | -36 |
| `language/statements/variable` | -33 |
| `language/statements/const` | -32 |
| `language/statements/let` | -32 |
| `language/literals/regexp` | -13 |
| `language/destructuring/binding` | -12 |

新增失败区域规模明显小得多：

| 区域 | 失败数变化 |
| --- | ---: |
| `built-ins/Object` | +3 |
| `built-ins/Error` | +2 |
| `language/module-code` | +1 |

解读：

1. 最明显的收益来自 `built-ins/RegExp/property-escapes`，该子区域减少 261 个失败。
2. Native 前端在 expression 与 statement 相关路径中也有较多收益，尤其是 class/function/object-expression 附近的失败减少。
3. 新增失败主要集中在少量 early-error 精度和 object/error builtin 语义上，规模较小，不影响本次净改进结论。

## 当前剩余失败最多的区域

下面按 `test262/test` 下前两级路径统计剩余失败数量。

| 区域 | 剩余失败数 | 占失败比例 |
| --- | ---: | ---: |
| `language/expressions` | 7,458 | 21.7% |
| `language/statements` | 7,378 | 21.5% |
| `built-ins/Temporal` | 4,214 | 12.3% |
| `intl402/Temporal` | 2,026 | 5.9% |
| `built-ins/TypedArray` | 1,155 | 3.4% |
| `built-ins/Array` | 1,109 | 3.2% |
| `staging/sm` | 1,003 | 2.9% |
| `built-ins/RegExp` | 817 | 2.4% |
| `built-ins/Object` | 711 | 2.1% |
| `built-ins/Promise` | 703 | 2.0% |
| `annexB/language` | 481 | 1.4% |
| `built-ins/TypedArrayConstructors` | 462 | 1.3% |
| `built-ins/Iterator` | 418 | 1.2% |
| `language/module-code` | 398 | 1.2% |
| `built-ins/Atomics` | 388 | 1.1% |
| `built-ins/DataView` | 359 | 1.0% |
| `built-ins/Proxy` | 311 | 0.9% |
| `language/eval-code` | 304 | 0.9% |
| `built-ins/Function` | 260 | 0.8% |
| `built-ins/Date` | 258 | 0.8% |

更集中的三级路径如下：

| 子区域 | 剩余失败数 | 占失败比例 |
| --- | ---: | ---: |
| `language/statements/class` | 3,741 | 10.9% |
| `language/expressions/class` | 3,165 | 9.2% |
| `language/statements/for-await-of` | 1,149 | 3.3% |
| `built-ins/TypedArray/prototype` | 1,136 | 3.3% |
| `built-ins/Array/prototype` | 950 | 2.8% |
| `built-ins/Temporal/ZonedDateTime` | 900 | 2.6% |
| `language/expressions/object` | 758 | 2.2% |
| `built-ins/Temporal/PlainDateTime` | 683 | 2.0% |
| `language/expressions/dynamic-import` | 628 | 1.8% |
| `language/statements/for-of` | 595 | 1.7% |
| `intl402/Temporal/ZonedDateTime` | 583 | 1.7% |
| `built-ins/Temporal/PlainDate` | 579 | 1.7% |
| `language/expressions/async-generator` | 554 | 1.6% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 1.5% |
| `intl402/Temporal/PlainDate` | 491 | 1.4% |
| `intl402/Temporal/PlainDateTime` | 483 | 1.4% |
| `built-ins/Temporal/Duration` | 458 | 1.3% |
| `built-ins/Temporal/PlainTime` | 432 | 1.3% |
| `built-ins/Temporal/Instant` | 417 | 1.2% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.0% |

当前主要剩余失败仍分散在语言语法/静态语义、Temporal、TypedArray、Array、RegExp、Object、Promise、module-code 与 staging 测试中。

## 失败分类

下面的分类由失败信息启发式归类得到，用于工程诊断，不是严格的 ECMAScript 规范分类。

| 失败类型 | 数量 | 占失败比例 |
| --- | ---: | ---: |
| 前端语法 / 静态语义缺口 | 12,251 | 35.7% |
| 断言 / 运行语义不匹配 | 6,734 | 19.6% |
| 绑定 / 环境记录语义缺口 | 4,482 | 13.0% |
| 内建方法缺失 / 调用目标为 undefined | 3,043 | 8.9% |
| yield / generator 暂未支持 | 2,013 | 5.9% |
| 暂未支持的语言或运行时特性 | 1,944 | 5.7% |
| BigInt 暂未支持 | 1,494 | 4.3% |
| 模块语法 / source-phase import 缺口 | 1,030 | 3.0% |
| 属性描述符 / 内建对象形状缺口 | 598 | 1.7% |
| RegExp 语法 / 解析器缺口 | 500 | 1.5% |
| 宿主对象 / 跨 Realm 支持缺失 | 222 | 0.6% |
| 其它运行时失败 | 24 | 0.1% |
| Annex B HTML 注释语法缺口 | 13 | 0.0% |

### 前端语法 / 静态语义缺口
代表性文件：
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-a-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-b-eval-func-skip-early-err-try.js`

### 断言 / 运行语义不匹配
代表性文件：
- `test262\test\annexB\built-ins\Array\from\iterator-method-emulates-undefined.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\this-not-date.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\this-not-date.js`

### 绑定 / 环境记录语义缺口
代表性文件：
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\index\this-subclass-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\input\this-subclass-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\lastMatch\this-subclass-constructor.js`

### 内建方法缺失 / 调用目标为 undefined
代表性文件：
- `test262\test\annexB\built-ins\Date\prototype\getYear\nan.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\return-value.js`

### yield / generator 暂未支持
代表性文件：
- `test262\test\annexB\built-ins\RegExp\RegExp-control-escape-russian-letter.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-invalid-control-escape-character-class.js`
- `test262\test\annexB\language\expressions\yield\star-iterable-return-emulates-undefined-throws-when-called.js`

### 暂未支持的语言或运行时特性
代表性文件：
- `test262\test\annexB\language\statements\for-of\iterator-close-return-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\Array\fromAsync\async-iterable-async-mapped-awaits-once.js`
- `test262\test\built-ins\Array\fromAsync\async-iterable-input-does-not-await-input.js`

### BigInt 暂未支持
代表性文件：
- `test262\test\annexB\built-ins\escape\argument_bigint.js`
- `test262\test\annexB\built-ins\unescape\argument_bigint.js`
- `test262\test\built-ins\Array\fromAsync\asyncitems-arraylike-length-accessor-throws.js`

### 模块语法 / source-phase import 缺口
代表性文件：
- `test262\test\built-ins\Proxy\preventExtensions\trap-is-undefined-target-is-proxy.js`
- `test262\test\language\comments\hashbang\module.js`
- `test262\test\language\eval-code\indirect\export.js`



## 跳过测试

日志中记录了 2 个显式 skip：

- `test262\test\built-ins\Atomics\wait\bigint\cannot-suspend-throws.js`
- `test262\test\built-ins\Atomics\wait\cannot-suspend-throws.js`

这两个都是 Atomics wait 相关测试，需要支持可挂起的宿主行为。它们应继续视为显式跳过项，而不是隐藏通过项。

## 结论解读

V11 fixbug 后，相比上一版 V11 诊断基线有明确进步：

1. 通过数从 18,050 增加到 19,027。
2. 失败数从 35,325 降低到 34,348。
3. 按全量 Test262 分母计算，通过率净提升 +1.83 pp。
4. 最明显的收益来自 RegExp property escapes，以及较大范围的前端语法/静态语义路径。
5. 剩余最大阻塞不集中在单个 builtin，而是分布在现代语法、class 语义、Temporal、module 语法、async/generator、BigInt、跨 Realm 宿主支持、对象模型和 builtin 属性描述符精度上。

因此，本次结果适合作为宽口径工程基线。它能说明整体方向变好，但不应替代更小、更稳定的 V11 fix pinned gate。

## 建议后续顺序

1. 建立 V11-fix pinned regression gate，把本次已经修好的文件纳入门禁，尤其是 RegExp property-escape 相关文件和从失败列表中消失的 expression/statement 文件。
2. 继续优先稳定前端静态语义：class/function/object expression、严格模式 early error、destructuring、for-of / for-await 路径仍然占据大量失败。
3. 对 BigInt、async function、generator/yield、module import/export、Temporal、跨 Realm `$262.createRealm` 等大功能族，要么实现，要么明确标记为后续里程碑。
4. 继续补齐 Date/String Annex B 方法、Object/Reflect 描述符、RegExp legacy accessors、builtin 的 `name` / `length` / property descriptor 精度。
5. 全量扫描继续作为诊断用基线，并与历史日志对比；失败、跳过、崩溃或超时的测试都不能计作通过。
