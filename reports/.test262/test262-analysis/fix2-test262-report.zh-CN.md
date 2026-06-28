# Native Fix2 Test262 报告

## 范围

本文总结 `ChaiXinran/CSCC-proj2` 最新一次 Native 后端 Test262 全量诊断运行结果。

报告结构参考 `reports/native-v7-test262-report.md`：范围、诊断扫描、结果汇总、失败热点、失败分类、跳过测试、结果解释、后续建议和质量门禁。本次命令走自研 Native 路径，即 lexer、parser/AST、bytecode compiler、VM、runtime、builtins、heap 和 Test262 runner；不把 Boa 作为兜底通过路径。

输入日志：`test262-fixbug2-output.txt`。

## Fix2 诊断扫描

命令：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

构建与运行证据：

```text
Finished `release` profile [optimized] target(s) in 1m 45s
Running `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress`
```

本次运行**没有到达正常完整全量扫描终点**。最后捕获到的进度行为：

```text
[52748/53379  98.8%] pass=26803 fail=25943 skip=2
```

随后进程结束于：

```text
memory allocation of 34359738368 bytes failed
error: process didn't exit successfully: `target\release\agentjs.exe test262 --backend native --root test262 --suite test --jobs 4 --progress` (exit code: 0xc0000409, STATUS_STACK_BUFFER_OVERRUN)
```

因此，本报告把 Fix2 视为**最后捕获状态下的诊断结果**，而不是干净完成的完整全量结果。

## 结果汇总

| 指标 | 数值 |
| --- | --- |
| 选中测试总数 | 53,379 |
| 最后捕获到的已处理数量 | 52,748 |
| 通过数 | 26,803 |
| 失败数 | 25,943 |
| 跳过数 | 2 |
| 相对选中总数的通过率 | 50.21% |
| 相对已处理部分的通过率 | 50.81% |
| 60% 目标通过数 | 32,028 |
| 距离 60% 目标还差 | 5,225 |
| 即使未处理尾部全通过，仍至少还差 | 4,594 |

跳过测试不计为通过。

按 53,379 个选中测试计算，60% 目标是 32,028 个通过点。最后捕获状态下，Fix2 通过 26,803 个，距离 60% 还差 5,225 个。由于崩溃后还有 631 个选中测试没有捕获，即使极端假设这些尾部测试全部通过，仍至少还差 4,594 个通过点。

## 历史进展

| 运行 | 已处理 | 选中总数 | 通过 | 失败 | 跳过 | 通过/选中总数 |
| --- | --- | --- | --- | --- | --- | --- |
| native-v11 baseline | 53,377 | 53,379 | 18,050 | 35,325 | 2 | 33.81% |
| native-v11-fixbug | 53,377 | 53,379 | 19,027 | 34,348 | 2 | 35.65% |
| fix | 53,377 | 53,379 | 24,950 | 28,425 | 2 | 46.74% |
| fix2 last captured | 52,748 | 53,379 | 26,803 | 25,943 | 2 | 50.21% |

说明：

- `fix2` 需要谨慎比较，因为它停在 52,748 个已处理测试，而不是通常的 53,377 个捕获终点。
- 在相同的处理位置，上一版 `fix` 的结果是 `pass=24728 fail=28018 skip=2`；Fix2 是 `pass=26803 fail=25943 skip=2`，说明可比前缀上 **通过数 +2,075 / 失败数 -2,075**。
- 即使 Fix2 提前 629 个已处理测试崩溃，相比上一版 `fix` 的完整-ish 终点，它已经多出 **1,853 个通过点**。

## 按区域统计的失败热点

进度日志记录了失败路径和总计数，但不记录通过文件路径。因此下表是以失败为中心的热点统计，不是每个目录的完整通过率表。

| 区域 | 失败数 | 占已捕获失败比例 |
| --- | --- | --- |
| `language/expressions` | 4,799 | 18.5% |
| `language/statements` | 4,776 | 18.4% |
| `built-ins/Temporal` | 4,194 | 16.2% |
| `intl402/Temporal` | 2,022 | 7.8% |
| `built-ins/RegExp` | 857 | 3.3% |
| `built-ins/Array` | 742 | 2.9% |
| `built-ins/TypedArray` | 687 | 2.6% |
| `built-ins/Promise` | 597 | 2.3% |
| `staging/sm` | 452 | 1.7% |
| `language/module-code` | 410 | 1.6% |
| `built-ins/Atomics` | 388 | 1.5% |
| `built-ins/Object` | 371 | 1.4% |
| `built-ins/TypedArrayConstructors` | 334 | 1.3% |
| `annexB/language` | 326 | 1.3% |
| `built-ins/Iterator` | 326 | 1.3% |
| `built-ins/Proxy` | 311 | 1.2% |
| `language/eval-code` | 276 | 1.1% |
| `built-ins/Date` | 246 | 0.9% |
| `built-ins/Function` | 240 | 0.9% |
| `built-ins/DataView` | 213 | 0.8% |
| `intl402/NumberFormat` | 207 | 0.8% |
| `intl402/DateTimeFormat` | 191 | 0.7% |
| `built-ins/Set` | 174 | 0.7% |
| `annexB/built-ins` | 165 | 0.6% |
| `built-ins/String` | 129 | 0.5% |

## 按子区域统计的失败热点

| 子区域 | 失败数 | 占已捕获失败比例 |
| --- | --- | --- |
| `language/statements/class` | 2,111 | 8.1% |
| `language/expressions/class` | 1,915 | 7.4% |
| `language/statements/for-await-of` | 1,146 | 4.4% |
| `built-ins/Temporal/ZonedDateTime` | 899 | 3.5% |
| `built-ins/Temporal/PlainDateTime` | 677 | 2.6% |
| `built-ins/TypedArray/prototype` | 671 | 2.6% |
| `language/expressions/dynamic-import` | 628 | 2.4% |
| `built-ins/Array/prototype` | 614 | 2.4% |
| `intl402/Temporal/ZonedDateTime` | 583 | 2.2% |
| `built-ins/Temporal/PlainDate` | 577 | 2.2% |
| `language/expressions/async-generator` | 542 | 2.1% |
| `built-ins/Temporal/PlainYearMonth` | 508 | 2.0% |
| `language/expressions/object` | 506 | 2.0% |
| `intl402/Temporal/PlainDate` | 489 | 1.9% |
| `intl402/Temporal/PlainDateTime` | 481 | 1.9% |
| `built-ins/Temporal/Duration` | 453 | 1.7% |
| `built-ins/Temporal/PlainTime` | 431 | 1.7% |
| `built-ins/Temporal/Instant` | 414 | 1.6% |
| `built-ins/RegExp/property-escapes` | 345 | 1.3% |
| `intl402/Temporal/PlainYearMonth` | 327 | 1.3% |
| `language/statements/for-of` | 271 | 1.0% |
| `language/statements/async-generator` | 267 | 1.0% |
| `language/eval-code/direct` | 249 | 1.0% |
| `language/module-code/top-level-await` | 244 | 0.9% |
| `built-ins/Iterator/prototype` | 239 | 0.9% |

剩余失败主要集中在：

1. `language/expressions` 和 `language/statements`，尤其是 class、async-generator、for-await-of、dynamic import、object expressions 和 for-of。
2. `built-ins/Temporal` 和 `intl402/Temporal`，数量很大，但实现成本也很高。
3. Array、TypedArray、RegExp、Promise、Object、Iterator 等区域，这些更依赖对象模型和协议精度，短期仍有现实收益。

## 失败分类

以下类别由失败路径和错误信息启发式聚合而来，是诊断分组，不是正式 ECMAScript 规范分类。

| 失败类别 | 数量 | 占已捕获失败比例 |
| --- | --- | --- |
| Temporal / Intl Temporal not implemented or incomplete | 6,294 | 24.3% |
| Async / Promise semantics gap | 5,042 | 19.4% |
| Assertion / semantic mismatch | 3,409 | 13.1% |
| Other runtime failure | 2,884 | 11.1% |
| Unsupported feature | 1,427 | 5.5% |
| Frontend syntax / static-semantics gap | 984 | 3.8% |
| Missing builtin method / undefined target | 977 | 3.8% |
| Property descriptor / builtin shape gap | 835 | 3.2% |
| BigInt semantics gap | 807 | 3.1% |
| Iterator / for-of protocol gap | 674 | 2.6% |
| Binding / environment semantics gap | 673 | 2.6% |
| Module syntax / module-loader gap | 583 | 2.2% |
| Unsupported Proxy / ShadowRealm | 536 | 2.1% |
| Generator / yield semantics gap | 388 | 1.5% |
| RegExp syntax / regexp-engine gap | 315 | 1.2% |
| Unsupported BigInt | 101 | 0.4% |
| Annex B HTML comment syntax gap | 11 | 0.0% |
| TypeError / runtime object-model gap | 3 | 0.0% |

## 首个可见错误类型

| 首个可见错误类型 | 数量 | 占比 |
| --- | --- | --- |
| TypeError | 8,082 | 31.2% |
| Test262Error | 7,511 | 29.0% |
| SyntaxError | 5,163 | 19.9% |
| ReferenceError | 1,765 | 6.8% |
| Unsupported | 1,436 | 5.5% |
| Other | 1,139 | 4.4% |
| Error | 725 | 2.8% |
| RangeError | 119 | 0.5% |
| EvalError | 3 | 0.0% |

## 代表性失败类别

| 失败类别 | 数量 | 代表性文件 |
| --- | --- | --- |
| Temporal / Intl Temporal not implemented or incomplete | 6,294 | `built-ins/Date/prototype/toTemporalInstant/length.js`<br>`built-ins/Date/prototype/toTemporalInstant/name.js` |
| Async / Promise semantics gap | 5,042 | `built-ins/Array/fromAsync/async-iterable-async-mapped-awaits-once.js`<br>`built-ins/Array/fromAsync/async-iterable-input-does-not-await-input.js` |
| Assertion / semantic mismatch | 3,409 | `annexB/built-ins/Date/prototype/getYear/this-not-date.js`<br>`annexB/built-ins/Date/prototype/setYear/this-not-date.js` |
| Other runtime failure | 2,884 | `annexB/built-ins/Date/prototype/getYear/not-a-constructor.js`<br>`annexB/built-ins/Date/prototype/setYear/not-a-constructor.js` |
| Unsupported feature | 1,427 | `annexB/language/statements/for-await-of/iterator-close-return-emulates-undefined-throws-when-called.j`<br>`annexB/language/statements/for-of/iterator-close-return-emulates-undefined-throws-when-called.js` |
| Frontend syntax / static-semantics gap | 984 | `annexB/language/expressions/assignmenttargettype/callexpression-as-for-in-lhs.js`<br>`annexB/language/expressions/assignmenttargettype/callexpression-as-for-of-lhs.js` |
| Missing builtin method / undefined target | 977 | `annexB/built-ins/Date/prototype/getYear/nan.js`<br>`annexB/built-ins/Date/prototype/getYear/return-value.js` |
| Property descriptor / builtin shape gap | 835 | `annexB/built-ins/Date/prototype/getYear/length.js`<br>`annexB/built-ins/Date/prototype/getYear/name.js` |
| BigInt semantics gap | 807 | `built-ins/Atomics/add/bigint/bad-range.js`<br>`built-ins/Atomics/add/bigint/good-views.js` |
| Iterator / for-of protocol gap | 674 | `annexB/built-ins/Array/from/iterator-method-emulates-undefined.js`<br>`annexB/built-ins/TypedArrayConstructors/from/iterator-method-emulates-undefined.js` |
| Binding / environment semantics gap | 673 | `annexB/language/eval-code/direct/func-block-decl-eval-func-block-scoping.js`<br>`annexB/language/eval-code/direct/func-block-decl-eval-func-existing-block-fn-no-init.js` |
| Module syntax / module-loader gap | 583 | `language/expressions/dynamic-import/always-create-new-promise.js`<br>`language/expressions/dynamic-import/assign-expr-get-value-abrupt-throws.js` |

## Fix2 相对上一版 Fix 的同进度对比

上一版 `fix` 在 `[52748/53379]` 位置的结果是 `pass=24728 fail=28018 skip=2`。Fix2 在相同位置的结果是 `pass=26803 fail=25943 skip=2`。

### 失败数减少最多的区域

| 区域 | fix 在 52,748 处失败数 | fix2 失败数 | 减少量 |
| --- | --- | --- | --- |
| `language/statements` | 5,572 | 4,776 | +796 |
| `language/expressions` | 5,524 | 4,799 | +725 |
| `built-ins/Array` | 850 | 742 | +108 |
| `built-ins/Promise` | 703 | 597 | +106 |
| `staging/sm` | 503 | 452 | +51 |
| `annexB/language` | 374 | 326 | +48 |
| `built-ins/TypedArray` | 731 | 687 | +44 |
| `built-ins/String` | 169 | 129 | +40 |
| `built-ins/Iterator` | 366 | 326 | +40 |
| `built-ins/Object` | 408 | 371 | +37 |
| `built-ins/TypedArrayConstructors` | 356 | 334 | +22 |
| `built-ins/DataView` | 230 | 213 | +17 |
| `built-ins/Symbol` | 42 | 26 | +16 |
| `built-ins/Reflect` | 24 | 10 | +14 |
| `language/identifiers` | 82 | 68 | +14 |

### 失败数回升的区域

| 区域 | fix 在 52,748 处失败数 | fix2 失败数 | 变化 |
| --- | --- | --- | --- |
| `built-ins/RegExp` | 736 | 857 | -121 |
| `language/directive-prologue` | 14 | 17 | -3 |
| `language/eval-code` | 275 | 276 | -1 |

### 失败数减少最多的子区域

| 子区域 | fix 在 52,748 处失败数 | fix2 失败数 | 减少量 |
| --- | --- | --- | --- |
| `language/statements/class` | 2,619 | 2,111 | +508 |
| `language/expressions/class` | 2,399 | 1,915 | +484 |
| `language/statements/for-of` | 527 | 271 | +256 |
| `language/expressions/assignment` | 237 | 136 | +101 |
| `built-ins/Array/prototype` | 700 | 614 | +86 |
| `language/expressions/object` | 548 | 506 | +42 |
| `built-ins/TypedArray/prototype` | 711 | 671 | +40 |
| `built-ins/String/prototype` | 153 | 117 | +36 |
| `annexB/language/eval-code` | 209 | 177 | +32 |
| `built-ins/Promise/prototype` | 124 | 93 | +31 |
| `language/statements/function` | 135 | 112 | +23 |
| `built-ins/Array/from` | 18 | 0 | +18 |
| `annexB/language/global-code` | 57 | 41 | +16 |
| `staging/sm/Iterator` | 100 | 84 | +16 |
| `language/expressions/arrow-function` | 89 | 73 | +16 |

### 失败数回升的子区域

| 子区域 | fix 在 52,748 处失败数 | fix2 失败数 | 变化 |
| --- | --- | --- | --- |
| `built-ins/RegExp/property-escapes` | 213 | 345 | -132 |
| `language/statements/for` | 123 | 144 | -21 |
| `language/directive-prologue/func-expr-no-semi-runtime.js` | 0 | 1 | -1 |
| `built-ins/DataView/custom-proto-if-not-object-fallbacks-to-default-prototype.js` | 0 | 1 | -1 |
| `language/eval-code/direct` | 248 | 249 | -1 |
| `language/directive-prologue/get-accsr-runtime.js` | 0 | 1 | -1 |
| `built-ins/DataView/custom-proto-access-resizes-buffer-valid-by-length.js` | 0 | 1 | -1 |
| `language/directive-prologue/func-decl-no-semi-runtime.js` | 0 | 1 | -1 |
| `language/directive-prologue/set-accsr-not-first-runtime.js` | 0 | 1 | -1 |
| `language/directive-prologue/func-expr-inside-func-decl-runtime.js` | 0 | 1 | -1 |

最大的收益来自 `language/statements/class`、`language/expressions/class`、`language/statements/for-of`、assignment/destructuring、Array prototype、Promise prototype 和 TypedArray prototype。最明显的回退是 `built-ins/RegExp/property-escapes`；继续合并 frontend 和 RegExp 相关修改前，应该把这部分加入 pinned regression gate。

## 跳过测试

| 跳过文件 |
| --- |
| `test262/test/built-ins/Atomics/wait/bigint/cannot-suspend-throws.js` |
| `test262/test/built-ins/Atomics/wait/cannot-suspend-throws.js` |

两个跳过项仍然是 Atomics wait 相关的显式未支持 host/runtime 场景，不计为通过。

## 结果解释

Fix2 相比上一版 fix 有明显提升，但当前扫描稳定性还不足以作为最终 release gate。核心结论如下：

1. 最后捕获状态下，相对选中总数的通过率约为 **50.21%**。
2. 与上一版 fix 的同进度前缀相比，Fix2 增加了 **2,075 个通过点**。
3. 距离 60% 至少还差 **5,225 个通过点**；即使未处理尾部全部通过，仍至少还差 **4,594 个通过点**。
4. 剩余最大可落地收益仍在 language/class/async/iterator 相关方向，而不是 GC/cache。
5. Temporal/Intl Temporal 是最大的单独 builtin 族，但实现成本太高；除非只做 descriptor 级别骨架，否则不应作为短期冲 60% 的主线。
6. 必须修复最后的崩溃，否则后续全量扫描数据的可信度会低于实际 conformance 进展。

## 后续建议顺序

1. **先稳定全量扫描报告。** runner 不应该在收尾阶段分配 32 GiB 内存，也不应该以 `STATUS_STACK_BUFFER_OVERRUN` 退出。建议流式写失败记录、截断单条错误信息、避免把所有失败一次性格式化成巨大字符串。
2. **把 Fix2 已获得的收益加入 pinned gate。** 至少覆盖 class、for-of、Array、Promise、TypedArray 和 descriptor 中已经从 fail 翻转为 pass 的代表测试。
3. **修复 RegExp property-escape 回退。** `built-ins/RegExp/property-escapes` 相比上一版同进度前缀多了 132 个失败。
4. **继续推进 language/class/destructuring。** `language/statements/class` 和 `language/expressions/class` 仍是最大的非 Temporal 子区域，同时也是 Fix2 收益最大的部分。
5. **优先补 async / Promise / iterator。** for-await-of、async generators、Promise、Iterator、Array.fromAsync 仍是最现实的高收益方向。
6. **提高对象模型和 descriptor 精度。** 这会同时影响 Object、Array、TypedArray、String/Date Annex B、RegExp legacy accessors，以及 builtin 的 `name` / `length` / descriptor 测试。
7. **Temporal / Intl Temporal 单独作为后续里程碑。** 时间紧时，只建议做低成本 constructor/property descriptor 骨架，不建议在达到 60% 前硬啃完整 Temporal 语义。

## 质量门禁

下一轮 broad Fix2 follow-up 合并前建议先跑：

```powershell
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features
```

推荐的高收益 Test262 专项门禁：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-await-of --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress
```

修复报告崩溃后再跑最终全量门禁：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress *> reports\fix2-test262-output.txt
```

失败、跳过、崩溃、超时或未处理的测试都不能计为通过。
