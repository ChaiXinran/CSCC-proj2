# AgentJS Native 后续功能开发计划（三组并行批次版）

更新时间：2026-06-24

## 当前基线

最新 full Test262 直跑命令：

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

结果见：

- `reports/test262-report.md`
- `reports/test262-analysis.md`

本轮结果：

| Total | Passed | Failed | Skipped | Pass rate |
| ---: | ---: | ---: | ---: | ---: |
| 53,379 | 14,035 | 38,507 | 837 | 26.29% |

主要失败结构：

| Failure class | Count | Share |
| --- | ---: | ---: |
| Parser syntax gap | 16,259 | 42.22% |
| Missing global / builtin / harness helper | 9,219 | 23.94% |
| Template literal substitutions unsupported | 5,307 | 13.78% |
| Assertion / semantic mismatch | 2,131 | 5.53% |
| Runtime call/property/object-model gap | 789 | 2.05% |
| Expected-error / early-error mismatch | 789 | 2.05% |
| Lexer/static syntax gap | 530 | 1.38% |
| RegExp engine feature gap | 528 | 1.37% |

注意：

- 该命令扫描整个 `test/`，包含 `test/staging`，因此它是压力/全域诊断视角。
- 本轮已经完整跑完，没有复现早期 stack overflow。
- `reports/native-full-test262-output.txt` 由 PowerShell 写成 UTF-16LE，脚本解析时要按 UTF-16 读取。

## 核心原则

我们仍然保持 3 组，但每个版本里 3 组可以并行开发不同功能。

正确节奏是：

```text
V8：A/B/C 三条功能线并行
 -> V8 集成测试
 -> V9：A/B/C 三条功能线并行
 -> V9 集成测试
 -> V10 ...
```

不要变成：

```text
A 组已经做 V9，B 组还在 V8，C 组突然开 V11。
```

每个版本的规则：

1. 版本开始前冻结三组任务边界。
2. 三组并行开发不同功能，尽量少改同一批文件。
3. 中途只允许做当前版本范围内的接口协调。
4. 三组都完成后统一合并测试。
5. 更新 `reports/test262-report.md`、新建的 dated/versioned analysis、`AGENTS.md` 和本文件。
6. 再进入下一版本。

## 三组长期定位

### A 组：Frontend / Syntax / Bytecode

主要热区：

- `src/lexer/`
- `src/parser/`
- `src/ast/`
- `src/bytecode/compiler.rs`

长期职责：

- 语法解析。
- early error。
- AST lowering。
- 字节码生成。
- parser/lexer 导致的大面积 SyntaxError。

### B 组：Runtime / VM / Object Model / Module

主要热区：

- `src/vm/`
- `src/runtime/`
- `src/contracts.rs`
- module environment / loader 相关文件。

长期职责：

- 调用帧、作用域、环境记录。
- object model、prototype、descriptor。
- iterator protocol runtime。
- module runner、module registry、linking。
- VM 指令和栈效果。

### C 组：Builtins / Host / Test262 Integration

主要热区：

- `src/builtins/`
- `src/test262.rs`
- Test262 selector/report/host helper。
- docs/reports。

长期职责：

- builtin family。
- `$262` host helper。
- Test262 分片、报告、失败分类。
- 文档同步。

## V8：最大解锁批次

目标：让大量测试从“无法解析 / 找不到对象”进入可执行状态。

V8 三组并行开发不同功能：

| 组别 | 功能线 | 主要收益 | 主要避免冲突 |
| --- | --- | --- | --- |
| A 组 | Frontend unlockers | 降低 parser/template/class/spread 失败 | 不碰大型 builtin |
| B 组 | Module runner 基础设施 | 降低 821 module skipped | 不实现 Intl/TypedArray |
| C 组 | Builtin skeletons 第一批 | 降低 missing global | 不改 parser/compiler |

### V8-A：Frontend unlockers

负责人：A 组。

任务：

1. Template literal substitutions
   - 支持 `` `${expr}` ``。
   - 支持多段 template。
   - 普通 template 先 lower 成字符串拼接。
   - tagged template 暂缓，但要给出清楚错误。
2. Class syntax 第一阶段
   - class declaration / expression。
   - constructor。
   - prototype method。
   - static method。
   - `extends` 先 parse；复杂 `super` 可保守报错。
3. Spread/rest/destructuring 第一阶段
   - call spread：`f(...args)`。
   - array spread：`[a, ...b]`。
   - rest parameter：`function f(...args)`。
   - 简单 array/object destructuring binding。

验收：

- `Template literal substitutions unsupported` 从 5,307 大幅下降。
- `Parser syntax gap` 从 16,259 至少下降 30%。
- class 相关目录不再以大面积 parse failure 为主。

推荐命令：

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language --jobs 4 --progress --json reports/native-v8-a-language-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String --jobs 4 --progress --json reports/native-v8-a-string-summary.json
```

### V8-B：Module runner 基础设施

负责人：B 组。

任务：

1. Runner / runtime 基础设施
   - 在 Test262 执行路径中区分 script 和 module。（已完成第一阶段）
   - module 顶层默认 strict。（已完成第一阶段）
   - module 顶层 `this === undefined`。（已完成第一阶段）
   - module 独立错误分类。（已完成第一阶段，runner 使用 `module mode` 标签）
2. Module environment
   - module scope。（基础执行入口已完成；完整 import/export binding 待 A 组 AST）
   - import/export binding 的 runtime 数据结构。
   - module registry，避免重复执行。（已完成第一阶段）
3. Loader 最小路径
   - 相对路径解析。（已完成第一阶段，支持 `./` / `../`）
   - 依赖图加载。（Rust-side loader helper 已完成；AST 接线待 A/B connector）
   - 先支持 acyclic graph；循环依赖后置。（循环依赖目前明确 Unsupported）

需要和 A 组对齐的接口：

- import/export AST 形状。
- module source 编译入口。
- module 顶层 strict 标志传递。

验收：

- module runner 相关代码可以独立编译和单测。（`native_v8_module` 5/5 passed）
- focused `test/language/module-code`：201/599 passed，398 failed，0 skipped。
- standard `--native-v8-scan`：205/5000 passed，4795 failed，0 skipped。
- 后续等 A 组 import/export parser 接上后，继续把剩余 parse/linking failure 转为真实 module semantic coverage。

推荐命令：

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/module-code --jobs 4 --progress --json reports/native-v8-b-module-summary.json
```

### V8-C：Builtin skeletons 第一批

负责人：C 组。

任务：

1. TypedArray / ArrayBuffer skeleton
   - 先补 constructor/prototype/name/length/descriptor。
   - 优先覆盖 `Float64Array`、`Uint8Array`、`Int32Array` 等高频名字。
   - 底层存储可先接 B 组 runtime helper，算法后续补。
2. Intl skeleton
   - `Intl` namespace。
   - `Intl.DateTimeFormat`。
   - `Intl.NumberFormat`。
   - `Intl.Collator`。
   - `resolvedOptions()` / `supportedLocalesOf()` 最小确定性实现。
3. `$262` host helper 第一批
   - 只补 full analysis 中实际高频触发的 helper。
   - 不实现无关 host API。

验收：

- `Float64Array is not defined` 明显下降。
- `Intl is not defined` 明显下降。
- `$262 is not defined` 明显下降。
- 失败从 ReferenceError 转为 descriptor/semantic mismatch。

推荐命令：

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v8-c-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/ArrayBuffer --jobs 4 --progress --json reports/native-v8-c-arraybuffer-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v8-c-intl402-summary.json
```

### V8 集成测试

三组完成后统一运行：

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
cargo run --release --no-default-features -- test262 --native-v8-scan --jobs 4 --json reports/native-v8-scan-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

`--native-v8-scan` 是 V8 标准轻量集成测试，固定运行
`reports/native-v8-scan-failures.txt` 中的 5000 个之前未通过样例。V8 相关
AI/协作者在完成 focused tests 后应默认运行该命令，并把结果写入对应
`reports/v8-part*-report.md`。

V8 完成后再启动 V9。

## V9：执行语义扩展批次

目标：在 V8 已经解锁 parser/module/builtin skeleton 后，补运行时语义。

V9 三组并行开发不同功能：

| 组别 | 功能线 | 主要收益 |
| --- | --- | --- |
| A 组 | async/generator/for-of syntax lowering | 降低 language 现代语法失败 |
| B 组 | Promise / job queue / iterator runtime | 支撑 async、for-await、Promise tests |
| C 组 | Map / Set / Iterator builtins | 降低 collection 和 iterator builtin 失败 |

### V9-A：async/generator/for-of lowering

任务：

- generator function / `yield`。
- async function / `await`。
- async generator parser。
- `for...of` lowering。
- `for await...of` parser + 最小 lowering。

### V9-B：Promise / job queue / iterator runtime

任务：

- Promise runtime substrate。（第一阶段已完成：`PromiseId` / `PromiseState` /
  单次 settle helper）
- microtask/job queue 最小模型。（第一阶段已完成：FIFO `JobQueue` +
  native backend `run_jobs()`）
- iterator protocol runtime 完整化。（第一阶段已完成：数组/字符串 fallback；
  generic `Symbol.iterator` dispatch 待后续 connector）
- iterator close。（第一阶段已完成：`iterator_close` 标记 done）
- async harness completion 支撑。

当前 B 组边界提醒：不要在 B 组直接安装 JS-visible `Promise`、`Map`、`Set`
或 `Iterator` 全局；这些属于 C 组 builtin/integration。B 组后续更适合继续
补 Promise reaction list / thenable resolution / async completion 与 iterator
runtime connector。

### V9-C：Map / Set / Iterator builtins

任务：

- `Map`。
- `Set`。
- `WeakMap` / `WeakSet` skeleton。
- `Iterator` constructor/prototype/helper skeleton。
- Iterator helper 方法按 Test262 热点补。

### V9 集成测试

```sh
cargo run --release --no-default-features -- test262 --native-v9-scan --jobs 4 --json reports/native-v9-scan-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/for-of --jobs 4 --progress --json reports/native-v9-forof-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress --json reports/native-v9-promise-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress --json reports/native-v9-iterator-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

V9 标准轻量 scan 已锁定为 `reports/native-v9-scan-failures.txt` 中的
5000 个 V9 热点非通过用例。初始结果：0/5000 passed，5000 failed，0 skipped。
V9 开发时每组完成 focused tests 后，应运行 `--native-v9-scan` 并更新对应
`reports/v9-part*-report.md`。

V9 完成后再启动 V10。

## V10：大型 Builtin 语义批次

目标：从 skeleton 进入主要算法实现。

状态：V10 第一阶段 setup 已完成。已创建 `docs/native-v10-scope.md`、
`docs/native-v10-interface.md`、`docs/native-v10-team-plan.md`、
`reports/v10-partA-report.md`、`reports/v10-partB-report.md`、
`reports/v10-partC-report.md`，并锁定 `reports/native-v10-scan-failures.txt`。
标准命令：

```sh
cargo run --release --no-default-features -- test262 --native-v10-scan --jobs 4 --json reports/native-v10-scan-summary.json
```

初始结果：645/5,000 passed，4,355 failed，0 skipped。V9-A 仍在进行时，
V10-A 不要覆盖 V9-A 热区改动；如需修改 parser/AST/bytecode，应先确认
当前分支是否已经合入 V9-A。

V10 三组并行开发不同功能：

| 组别 | 功能线 | 主要收益 |
| --- | --- | --- |
| A 组 | BigInt / numeric / unicode syntax tail | 降低 lexer/static syntax gap |
| B 组 | TypedArray / ArrayBuffer runtime substrate | 支撑 TypedArray 主算法 |
| C 组 | Temporal / Intl / Date builtin semantics | 降低 Temporal 和 intl402 大块失败 |

### V10-A：syntax tail

任务：

- BigInt literal edge cases。
- numeric literal edge cases。
- unicode identifier escape。
- static syntax residual。

### V10-B：TypedArray / ArrayBuffer runtime

任务：

- buffer storage。（第一阶段已完成：`ArrayBufferId` / `ArrayBufferRecord`
  共享 byte storage）
- element type conversion。（第一阶段已完成：Number-backed Int/Uint/Float
  load/store；BigInt element kind 显式 TypeError）
- bounds check。（第一阶段已完成：view range、element index、DataView offset
  检查）
- detach 最小可观测模型。（第一阶段已完成：detached flag + 拒绝 view access）
- DataView 共用底层 helper。（第一阶段已完成：与 TypedArray 共享
  `ArrayBufferRecord`，支持 endian-aware get/set）

当前 B 组边界提醒：不要在 B 组直接安装 JS-visible `ArrayBuffer`、
`TypedArray` 或 `DataView` 构造器；这些属于 C 组 builtin/integration。
V10-C 后续应将 V8-C skeleton 的 hidden-property 临时槽迁移到 B 组 runtime
substrate。

### V10-C：Temporal / Intl / Date semantics

任务：

- `Temporal` 主要类型算法分批补。
- `Date` prototype 尾部。
- `Intl` 格式化确定性 fallback。
- `resolvedOptions()` 精度提升。

### V10 集成测试

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/TypedArray --jobs 4 --progress --json reports/native-v10-typedarray-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/DataView --jobs 4 --progress --json reports/native-v10-dataview-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress --json reports/native-v10-temporal-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/intl402 --jobs 4 --progress --json reports/native-v10-intl402-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

V10 完成后再启动 V11。

## V11：精度与 RegExp 批次

目标：清理已经能执行但行为不符合 spec 的测试。

状态：V11 第一阶段 setup 已完成。已创建 `docs/native-v11-scope.md`、
`docs/native-v11-interface.md`、`docs/native-v11-team-plan.md`、
`reports/v11-partA-report.md`、`reports/v11-partB-report.md`、
`reports/v11-partC-report.md`，并锁定 `reports/native-v11-scan-failures.txt`。
标准命令：

```sh
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

当前状态：selector 已接入，`native_test262` 15/15 passed；
第一次本地 `--native-v11-scan` 在 300s 工具超时内未完成，未生成
`reports/native-v11-scan-summary.json`。后续 V11 工作者需要用更长外部
timeout 重跑，或先替换 manifest 中疑似长耗时样本，再记录正式 baseline。

V11 三组并行开发不同功能：

| 组别 | 功能线 | 主要收益 |
| --- | --- | --- |
| A 组 | RegExp parser/static errors | 降低 RegExp SyntaxError 和 error kind mismatch |
| B 组 | object model / descriptor precision | 降低 semantic mismatch |
| C 组 | RegExp builtins / Annex B / descriptor sweep | 降低 RegExp 和 builtin 尾部失败 |

### V11-A：RegExp parser/static

任务：

- property escapes token/parse。
- regexp literal error kind。
- unicode escape residual。
- early error residual。

### V11-B：object model precision

任务：

- descriptor exactness。
- receiver handling。
- getter/setter。
- property lookup order。
- expected error ordering。

### V11-C：RegExp builtins and descriptor sweep

任务：

- backreferences。
- Unicode property escapes。
- Annex B legacy accessors。
- `RegExp.prototype.compile`。
- `Object` / `Function` / `Array` / `String` / `Iterator` descriptor sweep。

### V11 集成测试

```sh
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress --json reports/native-v11-regexp-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress --json reports/native-v11-object-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB --jobs 4 --progress --json reports/native-v11-annexb-summary.json
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

## 跨组协作规则

- 每个版本开始时，三组先对齐公共接口；接口没定，不开工。
- A 组改 AST/bytecode 结构时，必须通知 B/C。
- B 组改 runtime/object model 时，必须通知 C。
- C 组新增 builtin skeleton 时，不能绕过 B 组的 object model。
- 任何组都不要在当前版本范围外开大功能。
- 小 bugfix 可以穿插，但不能改动其他版本的主线文件。

## 通用合入门槛

每个 PR 至少运行：

```sh
cargo fmt --all -- --check
cargo check --no-default-features --all-targets
cargo test --no-default-features --test native_test262
```

每个版本合并后必须运行：

```sh
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

每次版本合并后同步更新：

- `reports/test262-report.md`
- 新建 dated/versioned analysis 文件，不覆盖 `reports/test262-analysis.md`
- `reports/v8-partA-report.md`、`reports/v8-partB-report.md`、`reports/v8-partC-report.md`
- `AGENTS.md`
- 本文件

`reports/test262-analysis.md` 是 2026-06-24 基线分析，后续不要覆盖；新的
full-suite analysis 新建 dated/versioned 文件。不要把 skipped 计为 passed；
不要把 `test/staging` 和正式 non-staging 成绩混为同一个指标。
