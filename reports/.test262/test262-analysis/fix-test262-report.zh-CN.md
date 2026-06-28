# Fix Test262 测试报告

## 范围

本报告记录当前 fix 后 Native 后端的最新全量 Test262 诊断结果。

报告结构参考 `reports/native-v7-test262-report.md`：区分宽口径诊断扫描与零回归门禁，给出总体结果、失败区域分布、失败类型分类、结论解释和后续建议。与 V7 pinned gate 不同，本次运行覆盖整个 `test262/test`，并通过自研 Native lexer、parser、bytecode compiler、VM、runtime、builtin、object model 与 Test262 host skeleton 执行。测试口径下没有把 Boa 作为 Native 后端 fallback。

因此，本报告应理解为全量诊断基线，而不是零失败验收门禁。

## 测试命令

日志中捕获到的命令为：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

日志中的构建 / 运行信息：

- 从 `D:\00_OS\CSCC` 编译 `agentjs v0.1.0`。
- release profile 构建成功，用时 `33.45s`。
- 实际执行命令为 `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`。

## 数据完整性说明

上传日志是 PowerShell 捕获的 progress 日志，不是最终 JSON summary。最后一条捕获到的进度为：

```text
[53377/53379 100.0%] pass=24950 fail=28425 skip=2
```

runner 选择的总测试数是 53,379，但最后可见处理数是 53,377。上传日志中没有最后 2 条记录，因此本报告使用“最后捕获状态”，不虚构缺失的最终 summary。

## 总体结果

| 选择测试总数 | 最后捕获处理数 | 通过 | 失败 | 跳过 | 通过率 |
| --- | --- | --- | --- | --- | --- |
| 53,379 | 53,377 | 24,950 | 28,425 | 2 | 46.74% |

跳过项不计入通过。通过率按照 `passed / selected total` 计算，与 V7 报告中的统计口径一致。

当前 60% 目标至少需要 `32,028` 个通过测试。当前通过 `24,950` 个，还差 `7,078` 个通过点。

## 相比上一版 V11 fix 基线的变化

上一版 V11 fix 基线停在 `[53377/53379] pass=19027 fail=34348 skip=2`。本次结果相比上一版有明显提升：

| 运行 | 选择测试总数 | 最后捕获处理数 | 通过 | 失败 | 跳过 | 通过率 |
| --- | --- | --- | --- | --- | --- | --- |
| 上一版 V11 fix 基线 | 53,379 | 53,377 | 19,027 | 34,348 | 2 | 35.65% |
| 当前 fix 运行 | 53,379 | 53,377 | 24,950 | 28,425 | 2 | 46.74% |
| 变化 | 0 | 0 | +5,923 | -5,923 | +0 | +11.10 pp |

解读：

1. 通过数增加 `5,923`。
2. 失败数减少 `5,923`。
3. 全量通过率提升 `11.10` 个百分点。
4. 本轮收益不再只集中在 RegExp property escapes，而是明显扩展到了 class、expression、statement、TypedArray、Object、Array、DataView 等区域。

## 失败数下降最大的区域

二级路径下降最明显的区域：

| 区域 | 失败数变化 | 上一版失败数 | 当前失败数 |
| --- | --- | --- | --- |
| `language/expressions` | -1,934 | 7,458 | 5,524 |
| `language/statements` | -1,806 | 7,378 | 5,572 |
| `built-ins/TypedArray` | -424 | 1,155 | 731 |
| `built-ins/Object` | -303 | 711 | 408 |
| `built-ins/Array` | -259 | 1,109 | 850 |
| `built-ins/DataView` | -129 | 359 | 230 |
| `annexB/language` | -107 | 481 | 374 |
| `built-ins/TypedArrayConstructors` | -106 | 462 | 356 |
| `staging/sm` | -95 | 1,003 | 908 |
| `built-ins/RegExp` | -81 | 817 | 736 |
| `language/arguments-object` | -70 | 192 | 122 |
| `built-ins/ArrayBuffer` | -68 | 112 | 44 |
| `built-ins/BigInt` | -65 | 77 | 12 |
| `built-ins/Iterator` | -52 | 418 | 366 |
| `built-ins/String` | -48 | 217 | 169 |
| `language/literals` | -46 | 60 | 14 |
| `language/statementList` | -44 | 48 | 4 |
| `language/eval-code` | -29 | 304 | 275 |
| `language/block-scope` | -25 | 32 | 7 |
| `built-ins/GeneratorPrototype` | -21 | 61 | 40 |

三级路径下降最明显的区域：

| 子区域 | 失败数变化 | 上一版失败数 | 当前失败数 |
| --- | --- | --- | --- |
| `language/statements/class` | -1,122 | 3,741 | 2,619 |
| `language/expressions/class` | -766 | 3,165 | 2,399 |
| `built-ins/TypedArray/prototype` | -425 | 1,136 | 711 |
| `built-ins/Array/prototype` | -250 | 950 | 700 |
| `language/expressions/object` | -210 | 758 | 548 |
| `language/statements/for` | -183 | 306 | 123 |
| `language/expressions/compound-assignment` | -172 | 317 | 145 |
| `language/expressions/generators` | -148 | 253 | 105 |
| `language/statements/generators` | -132 | 226 | 94 |
| `built-ins/DataView/prototype` | -126 | 322 | 196 |
| `annexB/language/eval-code` | -104 | 313 | 209 |
| `language/expressions/arrow-function` | -94 | 183 | 89 |
| `language/expressions/assignment` | -85 | 322 | 237 |
| `built-ins/Object/seal` | -81 | 94 | 13 |
| `built-ins/RegExp/prototype` | -73 | 274 | 201 |
| `language/statements/for-of` | -68 | 595 | 527 |
| `language/statements/function` | -62 | 197 | 135 |
| `language/statements/variable` | -58 | 104 | 46 |
| `built-ins/ArrayBuffer/prototype` | -57 | 91 | 34 |
| `built-ins/Object/hasOwn` | -56 | 59 | 3 |
| `language/expressions/logical-assignment` | -43 | 66 | 23 |
| `built-ins/Iterator/prototype` | -42 | 293 | 251 |
| `built-ins/String/prototype` | -38 | 191 | 153 |
| `built-ins/Object/preventExtensions` | -36 | 40 | 4 |
| `language/expressions/left-shift` | -35 | 45 | 10 |

小规模新增 / 回退区域：

| 区域 | 失败数变化 | 上一版失败数 | 当前失败数 |
| --- | --- | --- | --- |
| `language/module-code` | +12 | 398 | 410 |
| `language/reserved-words` | +3 | 1 | 4 |
| `language/global-code` | +2 | 32 | 34 |
| `built-ins/eval` | +1 | 3 | 4 |
| `language/function-code` | +1 | 22 | 23 |
| `built-ins/Date` | +1 | 258 | 259 |

新增 / 回退规模远小于修复收益，但下一轮合并前仍应纳入 pinned regression gate。

## 当前剩余失败热点

二级路径分布：

| 区域 | 剩余失败数 | 占失败比例 |
| --- | --- | --- |
| `language/statements` | 5,572 | 19.60% |
| `language/expressions` | 5,524 | 19.43% |
| `built-ins/Temporal` | 4,196 | 14.76% |
| `intl402/Temporal` | 2,026 | 7.13% |
| `staging/sm` | 908 | 3.19% |
| `built-ins/Array` | 850 | 2.99% |
| `built-ins/RegExp` | 736 | 2.59% |
| `built-ins/TypedArray` | 731 | 2.57% |
| `built-ins/Promise` | 703 | 2.47% |
| `language/module-code` | 410 | 1.44% |
| `built-ins/Object` | 408 | 1.44% |
| `built-ins/Atomics` | 388 | 1.36% |
| `annexB/language` | 374 | 1.32% |
| `built-ins/Iterator` | 366 | 1.29% |
| `built-ins/TypedArrayConstructors` | 356 | 1.25% |
| `built-ins/Proxy` | 311 | 1.09% |
| `language/eval-code` | 275 | 0.97% |
| `built-ins/Date` | 259 | 0.91% |
| `built-ins/Function` | 245 | 0.86% |
| `built-ins/DataView` | 230 | 0.81% |

更集中的三级路径热点：

| 子区域 | 剩余失败数 | 占失败比例 |
| --- | --- | --- |
| `language/statements/class` | 2,619 | 9.21% |
| `language/expressions/class` | 2,399 | 8.44% |
| `language/statements/for-await-of` | 1,146 | 4.03% |
| `built-ins/Temporal/ZonedDateTime` | 899 | 3.16% |
| `built-ins/TypedArray/prototype` | 711 | 2.50% |
| `built-ins/Array/prototype` | 700 | 2.46% |
| `built-ins/Temporal/PlainDateTime` | 677 | 2.38% |
| `language/expressions/dynamic-import` | 628 | 2.21% |
| `intl402/Temporal/ZonedDateTime` | 583 | 2.05% |
| `built-ins/Temporal/PlainDate` | 578 | 2.03% |
| `language/expressions/object` | 548 | 1.93% |
| `language/expressions/async-generator` | 543 | 1.91% |
| `language/statements/for-of` | 527 | 1.85% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 1.79% |
| `intl402/Temporal/PlainDate` | 491 | 1.73% |
| `intl402/Temporal/PlainDateTime` | 483 | 1.70% |
| `built-ins/Temporal/Duration` | 454 | 1.60% |
| `built-ins/Temporal/PlainTime` | 431 | 1.52% |
| `built-ins/Temporal/Instant` | 414 | 1.46% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.15% |
| `language/statements/async-generator` | 267 | 0.94% |
| `built-ins/Iterator/prototype` | 251 | 0.88% |
| `language/eval-code/direct` | 248 | 0.87% |
| `language/module-code/top-level-await` | 244 | 0.86% |
| `language/expressions/assignment` | 237 | 0.83% |

当前最大剩余问题仍集中在语言语法 / 静态语义、class 语义、Temporal、async/generator/for-await、Array/TypedArray、Object/Promise/Iterator、module-code 和 RegExp 语义等方向。

## 失败分类

下面分类由失败信息和路径名启发式归类得到，用于工程诊断，不是严格的 ECMAScript 规范分类。

| 失败类型 | 数量 | 占失败比例 |
| --- | --- | --- |
| Temporal / Intl Temporal 未实现 | 6,301 | 22.17% |
| 前端语法 / 静态语义缺口 | 4,756 | 16.73% |
| 断言 / 运行语义不匹配 | 3,047 | 10.72% |
| 其它运行时失败 | 2,592 | 9.12% |
| 内建对象形状缺失 / undefined 调用目标 | 2,575 | 9.06% |
| 绑定 / 环境记录语义缺口 | 2,289 | 8.05% |
| yield / generator 缺口 | 1,399 | 4.92% |
| 属性描述符 / builtin 形状缺口 | 1,299 | 4.57% |
| 模块 import/export / 顶层 await 缺口 | 1,200 | 4.22% |
| 暂未支持的特性缺口 | 1,193 | 4.20% |
| BigInt 未支持或不完整 | 983 | 3.46% |
| RegExp 语法 / 解析器缺口 | 495 | 1.74% |
| 宿主对象 / 跨 Realm 缺口 | 282 | 0.99% |
| Annex B HTML 注释 / 动态源码解析缺口 | 14 | 0.05% |

## 报告中的错误类型

| 错误类型 | 数量 | 占失败比例 |
| --- | --- | --- |
| TypeError | 8,197 | 28.84% |
| SyntaxError | 8,071 | 28.39% |
| Test262Error | 7,539 | 26.52% |
| ReferenceError | 2,506 | 8.82% |
| Unsupported | 1,317 | 4.63% |
| Error | 586 | 2.06% |
| RangeError | 115 | 0.40% |
| Other | 91 | 0.32% |
| EvalError | 3 | 0.01% |

## 特性关键词热点

这些统计是非互斥的。一个失败测试可能同时计入多个关键词。

| 特性关键词 | 相关失败数 | 占失败比例 |
| --- | --- | --- |
| Temporal | 6,318 | 22.23% |
| async | 5,634 | 19.82% |
| class | 5,506 | 19.37% |
| Intl | 3,055 | 10.75% |
| private | 2,514 | 8.84% |
| Iterator | 1,895 | 6.67% |
| await | 1,798 | 6.33% |
| generator | 1,357 | 4.77% |
| TypedArray | 1,200 | 4.22% |
| BigInt | 1,164 | 4.09% |
| yield | 1,053 | 3.70% |
| Promise | 1,011 | 3.56% |
| RegExp | 930 | 3.27% |
| import | 886 | 3.12% |
| module | 648 | 2.28% |
| Proxy | 593 | 2.09% |
| ArrayBuffer | 404 | 1.42% |
| DataView | 237 | 0.83% |
| ShadowRealm | 64 | 0.23% |

## 各失败类型代表性文件

### Temporal / Intl Temporal 未实现

代表性文件：
- `test262\test\built-ins\Date\prototype\toTemporalInstant\name.js`
- `test262\test\built-ins\Date\prototype\toTemporalInstant\length.js`
- `test262\test\built-ins\Date\prototype\toTemporalInstant\this-value-invalid-date.js`

### 前端语法 / 静态语义缺口

代表性文件：
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err-try.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-skip-early-err.js`
- `test262\test\annexB\language\eval-code\direct\func-if-decl-else-decl-a-eval-func-skip-early-err-try.js`

### 断言 / 运行语义不匹配

代表性文件：
- `test262\test\annexB\built-ins\Array\from\iterator-method-emulates-undefined.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\this-not-date.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\this-not-date.js`

### 其它运行时失败

代表性文件：
- `test262\test\annexB\built-ins\Date\prototype\getYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\Date\prototype\setYear\not-a-constructor.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-control-escape-russian-letter.js`

### 内建对象形状缺失 / undefined 调用目标

代表性文件：
- `test262\test\annexB\built-ins\Date\prototype\getYear\B.2.4.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\name.js`
- `test262\test\annexB\built-ins\Date\prototype\getYear\length.js`

### 绑定 / 环境记录语义缺口

代表性文件：
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-block-scoping.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-existing-block-fn-no-init.js`
- `test262\test\annexB\language\eval-code\direct\func-block-decl-eval-func-existing-block-fn-update.js`

### yield / generator 缺口

代表性文件：
- `test262\test\annexB\language\expressions\yield\star-iterable-return-emulates-undefined-throws-when-called.js`
- `test262\test\annexB\language\expressions\yield\star-iterable-throw-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\AsyncGeneratorFunction\instance-await-expr-in-param.js`

### 属性描述符 / builtin 形状缺口

代表性文件：
- `test262\test\annexB\built-ins\String\prototype\substr\length-to-int-err.js`
- `test262\test\annexB\built-ins\String\prototype\trimLeft\name.js`
- `test262\test\annexB\built-ins\String\prototype\trimRight\name.js`

### 模块 import/export / 顶层 await 缺口

代表性文件：
- `test262\test\built-ins\AbstractModuleSource\proto.js`
- `test262\test\built-ins\AbstractModuleSource\length.js`
- `test262\test\built-ins\AbstractModuleSource\name.js`

### 暂未支持的特性缺口

代表性文件：
- `test262\test\annexB\language\statements\for-await-of\iterator-close-return-emulates-undefined-throws-when-called.j`
- `test262\test\annexB\language\statements\for-of\iterator-close-return-emulates-undefined-throws-when-called.js`
- `test262\test\built-ins\AsyncFromSyncIteratorPrototype\next\absent-value-not-passed.js`

### BigInt 未支持或不完整

代表性文件：
- `test262\test\built-ins\Array\fromAsync\asyncitems-bigint.js`
- `test262\test\built-ins\Array\prototype\entries\resizable-buffer-grow-mid-iteration.js`
- `test262\test\built-ins\Array\prototype\every\resizable-buffer-grow-mid-iteration.js`

### RegExp 语法 / 解析器缺口

代表性文件：
- `test262\test\annexB\built-ins\RegExp\RegExp-decimal-escape-class-range.js`
- `test262\test\annexB\built-ins\RegExp\RegExp-decimal-escape-not-capturing.js`
- `test262\test\annexB\built-ins\RegExp\incomplete_hex_unicode_escape.js`

### 宿主对象 / 跨 Realm 缺口

代表性文件：
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\index\this-cross-realm-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\input\this-cross-realm-constructor.js`
- `test262\test\annexB\built-ins\RegExp\legacy-accessors\lastMatch\this-cross-realm-constructor.js`

### Annex B HTML 注释 / 动态源码解析缺口

代表性文件：
- `test262\test\annexB\built-ins\Function\createdynfn-html-close-comment-body.js`
- `test262\test\annexB\built-ins\Function\createdynfn-html-close-comment-params.js`
- `test262\test\annexB\built-ins\Function\createdynfn-html-open-comment-body.js`

## 跳过测试

日志中记录了 2 个显式 skip：

- `test262\test\built-ins\Atomics\wait\bigint\cannot-suspend-throws.js` — non-blocking agent tests are not enabled
- `test262\test\built-ins\Atomics\wait\cannot-suspend-throws.js` — non-blocking agent tests are not enabled

这两个都是 Atomics wait 相关测试，需要宿主侧可挂起行为。它们应继续视为显式跳过项，而不是隐藏通过项。

## 结论解读

本轮 fix 相比上一版 V11 fix 基线是一次明显进步：

1. 通过数从 19,027 增加到 24,950。
2. 全量通过率从 35.65% 提升到 46.74%。
3. 距离 60% 目标还差 7,078 个通过点，而上一版 V11 fix 后还差约 13,001 个。
4. 本轮最大收益来自 `language/statements/class`、`language/expressions/class`、`TypedArray.prototype`、`Array.prototype`、object expression、`Object.seal`、`Object.hasOwn`、DataView、RegExp prototype，以及若干表达式 / 语句运算族。
5. 剩余失败规模仍然很大，继续随机修零散失败不足以冲到 60%。下一阶段应继续打高密度功能族：class/private/static 语义、async/generator/iterator、Array/TypedArray/DataView、Object/Reflect 描述符精度、module/TLA parsing、BigInt 与 TypedArray 集成。
6. Temporal 仍是最大的单个宽口径 builtin 家族，但实现成本很高。除非已有轻量实现方案，否则不建议把它作为下一轮短期冲分主线。

## 建议后续顺序

1. 建立 `fix` pinned regression gate，把本轮从失败变通过的文件纳入门禁，尤其是 class、object-expression、TypedArray、Array、Object、DataView、RegExp 相关文件。
2. 继续推进前端 / class 轨道：`language/statements/class` 和 `language/expressions/class` 合计仍有 5,000+ 失败。
3. 继续推进对象模型轨道：descriptor 精度、`Object` / `Reflect`、`Array.prototype`、`TypedArray.prototype`、`ArrayBuffer`、`DataView` 已经证明有大收益，且仍有密集失败。
4. 继续推进 runtime protocol 轨道：generator/yield、iterator protocol、async function、Promise job queue、for-await-of。
5. Temporal/Intl402、完整 module loading、ShadowRealm、跨 Realm `$262.createRealm` 应作为更大的后续里程碑，除非团队明确切换阶段目标。
6. 全量扫描继续作为诊断基线，不要替代小规模 merge gate。失败、跳过、崩溃、超时或未捕获测试都不能计为通过。

## 质量门禁

下一轮合并前建议执行：

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features
```

专项诊断扫描：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress
```

全量诊断扫描：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress *> reports\fix-test262-output.txt
```

如果当前 runner 支持 JSON 输出，建议同时生成机器可读 summary，便于后续自动对比。
