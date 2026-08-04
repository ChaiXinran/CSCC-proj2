# AgentJS 三人并行修复方案

**基线固定为 commit `7bcd72a`**，对应本次 2026-08-04 JetStream 测试。当前不应立即三个人一起做“大范围性能优化”，而应按下面顺序推进：

> **先修测试基础设施 → 再修确定的语义错误 → 定性两个超时 → 最后根据剖析结果优化性能。**

这是因为赛题中功能测试和性能 benchmark 各占 30%，既不能为了性能破坏 Test262，也不能让资源缺失导致有效 benchmark 覆盖不足。

## 一、第一轮总体分工

| 人员 | 主线                       | 当前主要问题                                        | 独占文件                                                         |
| -- | ------------------------ | --------------------------------------------- | ------------------------------------------------------------ |
| A  | JetStream runner 与测试基础设施 | 3 个资源未嵌入、超时无法定性、报告口径不够细                       | `scripts/`、`benchmarks/generated/`、JetStream 报告脚本            |
| B  | RegExp 与调用链正确性           | `validatorjs` 编译错误、`regexp` checksum 错误       | `src/builtins/regexp.rs`、`src/unicode_set.rs`、相关 lexer/tests |
| C  | Intl 正确性与诊断入口            | NumberFormat 仍是 skeleton、`threejs/WSL` 缺少内部诊断 | `src/intl/`、Intl builtin、`src/main.rs` JetStream 配置          |

三个人分别建立分支：

```text
fix/jetstream-runner-resources
fix/jetstream-regexp
fix/jetstream-intl-diagnostics
```

建立集成分支：

```text
integration/jetstream-20260804
```

---

# 二、人员 A：修复 runner、资源嵌入和测试口径

## 目标

优先消除：

```text
jetstream2-jsdom-d3-startup  RESOURCE_MISSING
jetstream2-mobx              RESOURCE_MISSING
jetstream2-web-ssr           RESOURCE_MISSING
```

这三项目前不是 AgentJS 引擎失败，而是 generated runner 没有包含运行期 bundle。

## 根因

`prepare-jetstream2.mjs` 当前只收集：

```js
const benchmarkFiles =
    plan.files ?? benchmark._files ?? benchmark.files ?? [];
```

然后将这些文件写入 `__jetstreamResources`。运行时访问其他路径就直接抛出：

```js
throw new Error("JetStream resource not embedded: " + normalized);
```

以 MobX 为例，生成物中包含：

```text
./utils/StartupBenchmark.js
./mobx/benchmark.js
```

但 `StartupBenchmark.init()` 还会读取：

```text
JetStream.preload.BUNDLE
./mobx/dist/bundle.es6.min.js
```

当前资源表没有该 bundle，因此一定失败。

## A 的具体任务

### A1. 重写资源发现机制

修改：

```text
scripts/prepare-jetstream2.mjs
```

不要继续硬编码三个 bundle，应统一发现资源。

建议流程：

```js
const discoveredResources = new Set();

function discoveryReadFile(name) {
    const normalized = normalizeResourcePath(name);
    discoveredResources.add(normalized);

    const absolute = path.resolve(root, normalized);
    if (fs.existsSync(absolute))
        return fs.readFileSync(absolute, "utf8");

    return "";
}
```

资源来源至少包括：

```text
plan.files
benchmark.files / benchmark._files
preload 映射中的文件
discovery 阶段 readFile() 实际请求的文件
```

对 `.z` 文件需要明确选择：

* 嵌入解压后的文件；
* 或保留压缩资源并提供解压支持。

短期推荐直接嵌入已解压文本。

### A2. 增加生成物资源校验

每个 generated runner 额外输出 manifest：

```json
{
  "benchmark": "mobx",
  "entryFiles": [],
  "preloadFiles": [],
  "runtimeDiscoveredFiles": [],
  "embeddedFiles": [],
  "missingFiles": []
}
```

只要 `missingFiles` 非空，生成命令就以非零状态退出，不允许生成一个必然失败的 runner。

### A3. 增加 smoke test

新增：

```text
scripts/verify-generated-runner.mjs
```

检查：

1. runner 能解析；
2. `__jetstreamResources` 包含所有 manifest 路径；
3. 以最低迭代数运行时，不出现：

   * `JetStream resource not embedded`
   * `readFile path failed`
4. failure marker 能被正确识别。

### A4. 增加超时诊断矩阵

针对：

```text
jetstream2-threejs
jetstream2-WSL
```

自动生成迭代数：

```text
1 / 2 / 5 / 10
```

每次记录：

```text
是否开始 workload
是否完成 initialization
是否进入 runIteration
完成迭代数量
最后输出时间
进程 CPU 时间
峰值内存
```

A 暂时不改引擎，只通过 runner 插入阶段 marker：

```js
print("JETSTREAM_PHASE:init:start");
print("JETSTREAM_PHASE:init:end");
print("JETSTREAM_PHASE:iteration:0:start");
print("JETSTREAM_PHASE:iteration:0:end");
print("JETSTREAM_PHASE:validate:start");
```

## A 的验收标准

| 验收项              | 标准                                 |
| ---------------- | ---------------------------------- |
| 资源未嵌入            | 3 → 0                              |
| generated runner | 19 个全部能进入真实 workload               |
| manifest         | 每个 runner 都有完整资源清单                 |
| 重复生成             | 相同输入产生相同 runner                    |
| 超时分类             | 能区分初始化慢、单轮慢和疑似不终止                  |
| 生成物修改            | 禁止手工编辑 `benchmarks/generated/*.js` |

注意：资源问题解决后，这三个 workload **不保证立即 PASS**，但至少能够开始真正测试引擎。

---

# 三、人员 B：修复 RegExp 和错误调用链

## 目标

依次解决：

```text
jetstream2-validatorjs  REGEXP/CALL_ERROR
regexp                  CHECKSUM_MISMATCH
```

## 负责文件

```text
src/builtins/regexp.rs
src/unicode_set.rs
src/lexer/mod.rs
tests/parser_regexp_errors.rs
tests/jetstream_regexp.rs        # 新增
```

原则上不修改：

```text
src/intl/
scripts/prepare-jetstream2.mjs
src/runtime/property_map.rs
```

如果确认是 VM 异常传播问题，需要改 `src/vm/interpreter.rs`，必须单独提交一个小型 commit，不和 RegExp 大改混在一起。

## B 的具体任务

### B1. 先构造 validatorjs 最小复现

最先加入：

```js
const regexp = /[@_\- ]/g;

if (!regexp.test("@")) throw new Error("@");
regexp.lastIndex = 0;
if (!regexp.test("_")) throw new Error("_");
regexp.lastIndex = 0;
if (!regexp.test("-")) throw new Error("-");
regexp.lastIndex = 0;
if (!regexp.test(" ")) throw new Error("space");
```

再测试构造器形式：

```js
const regexp = new RegExp("[@_\\- ]", "g");
```

记录四个阶段：

```text
源码 pattern
Lexer 输出
AgentJS 翻译后的 pattern
传给 fancy-regex RegexBuilder 的 pattern
```

当前 RegExp 的执行方式是将 ECMAScript pattern 翻译后交给 `fancy-regex`，因此字符类转义差异必须由 AgentJS 转换层处理。

### B2. 修复字符类中的 `\-`

重点检查：

```text
\- 在字符类中
- 位于字符类首尾
普通 identity escape
u/v 模式与非 Unicode 模式区别
反斜杠是否被重复转义
```

不能只针对：

```js
/[@_\- ]/g
```

硬编码，应加入以下回归：

```js
/[\-]/.test("-")
/[a\-z]/.test("-")
/[-az]/.test("-")
/[az-]/.test("-")
new RegExp("[\\-]")
```

### B3. 修复异常传播

当前现象是：

```text
RegExp 编译失败
随后 undefined is not callable
```

说明可能存在错误级联。

正确行为应当是：

```text
CreateRegExp 失败
→ 立即产生 SyntaxError
→ 当前表达式停止执行
→ 不得继续调用 regexp 方法
```

需要检查：

```text
CreateRegExp opcode
RegExp constructor
GetMethod
CallWithThis
pending_exception
```

但只在最小测试证明调用链确实有问题后才改 VM。

### B4. 定位 regexp checksum 第一个分歧

不要直接通读整个 workload。采用分段 checksum：

```text
第 1 组用例 checksum
第 2 组用例 checksum
……
```

先找到第一个 AgentJS 与 Boa 不一致的组，再缩小到具体正则。

重点排查顺序：

1. `lastIndex`
2. global/sticky
3. 空匹配推进
4. capture group
5. replacement token
6. backreference
7. UTF-16 与 Unicode scalar 差异
8. ignoreCase/case folding

### B5. 守住现有 Test262 成果

当前提交报告显示：

```text
built-ins/RegExp      1757 / 1879
unicodeSets            114 / 114
property-escapes       611 / 613
```

因此 RegExp 修复不能以 JetStream 通过为唯一目标。

## B 的验收标准

| 验收项           | 标准                                |
| ------------- | --------------------------------- |
| `/[@_\- ]/g`  | 正确编译和匹配                           |
| validatorjs   | 不再出现 RegExp 编译错误                  |
| 错误传播          | 不再级联为 `undefined is not callable` |
| regexp        | checksum 与 runner 预期一致            |
| `unicodeSets` | 保持 114/114                        |
| RegExp 总通过数   | 不低于当前 1757/1879                   |
| 工程检查          | fmt、check、test、clippy 全通过         |

---

# 四、人员 C：修复 Intl，并提供引擎内部诊断入口

## 目标

解决：

```text
jetstream2-intl
NumberFormat-intl totalLength = 31112
runner 要求 >= 80000
```

同时为 `threejs` 和 `WSL` 提供内部限制与诊断开关。

## 负责文件

```text
src/intl/
src/builtins/binary_data.rs
src/main.rs
Cargo.toml
Cargo.lock
tests/intl_*.rs
```

第一轮期间 `Cargo.toml` 和 `Cargo.lock` 由 C 独占，其他人不要同时增加依赖。

## C 的具体任务

### C1. 确认 Intl 不是边界误差

当前模块说明明确写的是：

```text
Binary-data built-ins ... and Intl skeleton
```

虽然 `NumberFormatRecord` 保存了：

```text
style
currency
unit
minimumFractionDigits
maximumFractionDigits
useGrouping
notation
signDisplay
```

但当前 `format()` 实际只做：

```rust
let text = vm.to_string_coerce(value, context)?;
match text.as_str() {
    "Infinity" => "∞",
    "-Infinity" => "-∞",
    _ => text,
}
```

也就是除无穷大外，基本原样输出。

因此不能通过调整断言或人为补长字符串解决。

### C2. 实现最小但真实的 NumberFormat

优先实现：

```text
decimal
percent
currency
minimumIntegerDigits
minimumFractionDigits
maximumFractionDigits
useGrouping
signDisplay
Infinity / NaN
负零
```

格式化过程应使用数值：

```rust
let number = vm.to_number(value, context)?;
```

而不是先 `ToString`。

推荐将 Intl 从 `binary_data.rs` 中逐步抽出：

```text
src/builtins/intl/
├── mod.rs
├── number_format.rs
├── locale.rs
└── options.rs
```

但拆分与行为修复分成两个 commit：

```text
C1：仅移动代码，不改变行为
C2：实现真实格式化
```

### C3. 使用数据驱动后端

当前已经使用 ICU4X 的 calendar 和 property 组件。

NumberFormat 应优先选择：

* ICU4X 对应数字格式化组件；
* 或一个明确的数据驱动 formatting backend。

不建议自己硬编码 25 个 locale 的全部规则。

短期可以先保证 benchmark 涉及的选项都走统一 formatter，但禁止：

```text
为了达到 80000 人工增加空格
根据 benchmark 名称返回特殊结果
硬编码 totalLength
```

### C4. 修复 record 与真实行为脱节

构造器解析出的选项必须写入 `NumberFormatRecord`，format 时必须从 record 读取。

重点确认：

```text
minimumFractionDigits
maximumFractionDigits
style
currency
useGrouping
notation
signDisplay
```

目前代码中部分选项只进行了校验，却没有保存或实际使用，这会造成 `resolvedOptions()` 与 `format()` 行为不一致。

### C5. 为 JetStream 增加诊断参数

当前 `agentjs jetstream` 使用：

```rust
loop_limit: u64::MAX
wall_clock_limit: None
recursion_limit: 8192
```

并创建 256 MiB 专用线程栈。

增加：

```text
agentjs jetstream <runner>
    --loop-limit <n>
    --wall-clock-seconds <n>
    --gc-stats
    --heap-stats
```

或使用等价环境变量：

```text
AGENTJS_JS_LOOP_LIMIT
AGENTJS_JS_WALL_CLOCK_SECONDS
AGENTJS_JS_PRINT_GC_STATS
```

出现内部限制时至少输出：

```text
错误类型
最后执行阶段
loop budget
call depth
operand stack depth
heap object count
allocation count
collection count
```

第一轮不需要实现完整 profiler，只要能将 WSL 从“外部强杀”变成“内部可诊断终止”。

## C 的验收标准

| 验收项             | 标准                                  |
| --------------- | ----------------------------------- |
| NumberFormat    | 不再是 identity formatting             |
| intl runner     | `totalLength >= 80000`，且正常 validate |
| Infinity/NaN/负零 | 符合预期                                |
| options         | 构造、record、format 行为一致               |
| intl402         | `NumberFormat` focused suite 不回归    |
| WSL 诊断          | 能在内部 limit 下输出诊断                    |
| threejs 诊断      | 能确定停留阶段                             |
| 工程检查            | fmt、check、test、clippy 全通过           |

---

# 五、三人的依赖关系

三条线大部分可以真正并行：

```text
A runner/resources ───────────────┐
                                  ├─→ 重新生成 19 个 runner
B RegExp/validatorjs ─────────────┤
                                  ├─→ 集成测试
C Intl/diagnostics ───────────────┘
```

只有两个小依赖：

## 依赖 1：B 的 regexp checksum 调试

B 可以先独立完成 validatorjs 最小复现和字符类修复。

A 合并 runner 的 debug checkpoint 后，B 再利用新的 runner 做 checksum 二分。

## 依赖 2：C 的 Intl 依赖修改

C 独占：

```text
Cargo.toml
Cargo.lock
```

B 如需新增 RegExp 依赖，应先与 C 协调，最好本轮不再引入新的 RegExp crate。

---

# 六、提交和合并顺序

## 第一批：独立基础提交

### A

```text
A1 resource discovery
A2 preload embedding and manifest
A3 smoke test and phase markers
```

### B

```text
B1 validatorjs minimal regression tests
B2 RegExp translation fix
B3 exception propagation fix（如确有需要）
```

### C

```text
C1 Intl code isolation
C2 real NumberFormat formatting
C3 JetStream diagnostic options
```

每个 commit 必须单独可编译，不允许一个 commit 同时包含：

* 大规模格式化；
* 文件移动；
* 功能修复；
* 报告更新。

## 合并顺序

1. 合并 A：获得可信的新 runner。
2. 重新生成所有 runner。
3. B、C 在新 runner 上复测各自项目。
4. 合并 B。
5. 合并 C。
6. 统一执行：

   * cargo test；
   * focused Test262；
   * Test262 全量；
   * 19 个 JetStream runner；
   * AgentJS/Boa 对比。
7. 最后单独提交报告更新。

A 先合并不是因为 A 的代码最重要，而是因为后续所有结果都依赖正确生成的 runner。

---

# 七、第一轮结束后的统一判定表

| Benchmark        | 第一责任人 | 第一轮期望                    |
| ---------------- | ----- | ------------------------ |
| jsdom-d3-startup | A     | 不再 RESOURCE_MISSING      |
| mobx             | A     | 不再 RESOURCE_MISSING      |
| web-ssr          | A     | 不再 RESOURCE_MISSING      |
| validatorjs      | B     | 修复 RegExp 编译与调用错误        |
| regexp           | B     | checksum 正确              |
| intl             | C     | NumberFormat 输出达到真实格式化要求 |
| threejs          | A+C   | 定性为正常慢、初始化慢或不终止          |
| WSL              | A+C   | 获得内部执行位置和 limit 信息       |

第一轮结束时，最重要的不是立即达到某个 PASS 数字，而是将 8 个异常项目全部变成以下三类之一：

```text
确定通过
确定的语义错误
有内部证据的性能问题
```

不能继续保留：

```text
RESOURCE_MISSING
无输出 TIMEOUT
错误级联导致的模糊 CALL_ERROR
```

---

# 八、第二轮：再做性能优化

只有第一轮完成、19 个 runner 都能真实执行后，才开始性能优化。

当前 release 已经开启：

```text
opt-level = 3
thin LTO
codegen-units = 1
```

所以性能问题不是简单的“忘记 release 编译”。

第二轮再根据剖析结果选择三条线：

| 可能热点                 | 后续负责人 | 相关代码                             |
| -------------------- | ----- | -------------------------------- |
| 局部变量名称查找             | A     | compiler、opcode、environment slot |
| 属性访问和对象模型            | B     | property map、object、inline cache |
| GC、分配和 JsValue clone | C     | heap、gc、value、context            |

当前 Environment 使用 `HashMap<String, Binding>`，普通名称访问仍可能需要字符串哈希和环境链查找。

对象属性表采用 `Vec<PropertyEntry> + HashMap<String, usize>`，属性删除、枚举及 key clone 也可能成为对象密集 workload 的主要成本。

但在没有计数器和分阶段耗时前，不建议直接重写对象模型或 GC。

---

# 九、最终建议的优先级

```text
P0  A：修复三个资源缺失
P0  B：修复 validatorjs 的 /[@_\- ]/g
P0  C：修复 Intl.NumberFormat identity formatting

P1  B：定位 regexp checksum
P1  A+C：定性 threejs 和 WSL
P1  全员：重新跑 19 个 runner 与 focused Test262

P2  全员：基于 profiler 做解释器性能优化
```

最理想的第一轮结果是：

* 3 个资源错误全部消失；
* validatorjs 和 regexp 恢复正确性；
* Intl 使用真实 NumberFormat；
* threejs、WSL 获得明确内部诊断；
* 原有 11 个 PASS 不发生回归；
* Test262 RegExp 和 Intl focused suite 不下降。
