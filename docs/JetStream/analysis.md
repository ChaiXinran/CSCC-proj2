# 总体判断

基于 `7bcd72a` 的代码和这次 19 个 generated runner 的结果，AgentJS 现在已经不是“功能残缺、跑不起来”的早期状态，而是进入了下面这个阶段：

> **主流 ECMAScript 执行链路基本可用，但生态兼容层仍有明显缺口，解释器底层性能架构尚未成熟。**

当前问题可以分成三类：

| 类型          | 对应项目                          | 性质                         |
| ----------- | ----------------------------- | -------------------------- |
| runner 生成问题 | jsdom-d3-startup、mobx、web-ssr | 不是引擎失败，属于测试基础设施缺陷          |
| 真实语义问题      | Intl、validatorjs、regexp       | 引擎实现与 ECMAScript/真实库需求存在差异 |
| 性能或未定性问题    | threejs、WSL                   | 暂时无法区分“极慢”和“逻辑不终止”         |
| 系统性性能差距     | 11 个共同通过项目                    | 解释器、对象模型、属性访问、内存管理共同造成     |

因此，**11/19 并不能简单理解为 AgentJS 只有 57.9% 的 JetStream 能力**。其中至少 3 项根本没有真正进入 workload；但另一方面，即使扣除测试生成问题，6.3–33.7 倍的性能差距仍然是真实而系统性的。

---

# 一、三个 `RESOURCE_MISSING` 是明确的 runner 生成缺陷

`prepare-jetstream2.mjs` 当前只收集：

```js
const benchmarkFiles =
    plan.files ?? benchmark._files ?? benchmark.files ?? [];
```

随后只将这些路径写入 `__jetstreamResources`。运行期一旦读取未打包文件，就直接抛出：

```js
throw new Error("JetStream resource not embedded: " + normalized);
```

也就是说，生成器假设所有依赖都存在于 `plan.files`，但 JetStream 的 startup benchmark 还通过 preload/BUNDLE 机制动态读取资源。

以生成后的 `jetstream2-mobx.js` 为例，资源表里只有：

* `./utils/StartupBenchmark.js`
* `./mobx/benchmark.js`

但 `StartupBenchmark.init()` 会读取 `JetStream.preload.BUNDLE`，最终需要 `./mobx/dist/bundle.es6.min.js`。该 bundle 并未进入资源表，因此一定会报错。

## 这意味着什么

这三个项目：

* `jetstream2-jsdom-d3-startup`
* `jetstream2-mobx`
* `jetstream2-web-ssr`

目前**不能用于判断 AgentJS 是否支持对应 workload**。

修好生成器后，它们可能通过，也可能继续暴露 parser、runtime 或性能问题。不能直接推断修复后 AgentJS 会从 11/19 上升到 14/19，但至少会从“没有测试到”变成“真正测试到了”。

## 建议修法

不要只在脚本里手工补三个 bundle，而应统一修资源发现机制：

1. 同时读取 `plan.files`、preload 映射和 benchmark 的资源描述。
2. 在 discovery 阶段对 `readFile()` 做路径记录，而不是固定返回空字符串。
3. 递归收集运行期请求的文本资源。
4. 生成完成后检查所有 preload 路径是否存在于 `__jetstreamResources`。
5. 为 startup benchmark 增加生成物 smoke test。

当前 discovery 环境中的：

```js
readFile: () => ""
```

还会掩盖依赖发现问题，所以应改成“记录请求路径并返回占位内容”。

---

# 二、Intl 不是小误差，而是实现仍处于 skeleton 阶段

代码已经非常直接地说明了这个问题。`src/builtins/binary_data.rs` 的模块说明将当前实现称为：

> `Intl skeleton`

并说明主要提供 constructor、prototype、descriptor 和确定性的 options object。

`NumberFormatRecord` 虽然保存了：

* style
* currency
* unit
* fraction digits
* grouping
* notation
* sign display

但实际 `Intl.NumberFormat.prototype.format` 的实现只是：

1. 将输入转换为字符串；
2. 把 `Infinity` 替换成 `∞`；
3. 其他值原样返回。

它没有真正执行：

* 千位分组；
* 小数位舍入；
* 百分比转换；
* currency symbol；
* locale digit shaping；
* scientific/compact notation；
* signDisplay；
* unit formatting。

所以：

```text
totalLength = 31112
expected >= 80000
```

不是普通的边界条件错误，而是 **NumberFormat 输出基本相当于原始数字字符串，格式化内容显著少于标准实现**。

同时，当前 provider 明确叫作 `MinimalIntlProvider`，它对“是否支持 locale”的判断基本只是 locale tag 能否被 canonicalize，并不等于真的拥有对应 locale 的格式化数据。

## 可能继续出现的问题

即使修正这一个长度断言，后面仍可能出现：

* currency、percent、unit 输出错误；
* 不同 locale 输出完全一致；
* roundingMode、roundingIncrement 未实际生效；
* compact notation 不生效；
* Collator 只做 Rust 字符串字典序比较；
* RelativeTimeFormat、ListFormat 只有对象外壳，没有完整语义；
* resolvedOptions 与真实格式化行为不一致。

## 建议

这里有两种合理路线：

### 路线 A：诚实标记为不支持

短期内将不具备真实格式化能力的 Intl 服务标记为 unsupported，而不是让 constructor 存在但输出错误。这样测试结果更可信。

### 路线 B：接入真正的数据驱动格式化

可以基于 ICU4X 增加 NumberFormatter，包括：

* decimal/currency/percent；
* grouping；
* fraction/significant digits；
* locale symbols；
* notation；
* sign display。

鉴于你们已经使用 `icu_calendar` 和 `icu_properties`，继续向 ICU4X NumberFormatter 扩展在架构上比较自然。

---

# 三、RegExp 已经显著进步，但“翻译到 fancy-regex”仍是风险边界

这次提交本身对 RegExp 做了大量修复，包括：

* scoped modifiers；
* Unicode property escapes；
* Unicode Sets；
* properties of strings；
* Annex B legacy escapes；
* quantifiable lookahead。

提交报告显示 `test/built-ins/RegExp` 从 1504/1879 上升到 1757/1879，`unicodeSets` 已达到 114/114。

这说明 AgentJS 不是“RegExp 整体未实现”，而是已经拥有较完整的 RegExp 前端和兼容转换层。

但当前架构仍然是：

```text
ECMAScript RegExp
        ↓
AgentJS 自定义解析/重写
        ↓
fancy-regex RegexBuilder
```

这会形成一个长期风险：ECMAScript RegExp 和 `fancy-regex` 的语义并不完全相同，AgentJS 必须正确翻译所有差异。

## validatorjs 的问题

合法表达式：

```js
/[@_\- ]/g
```

出现编译失败，随后产生：

```text
undefined is not callable
```

很可能是两层问题串联：

1. RegExp 创建失败或返回错误值；
2. 后续调用路径没有正确停止，继续把缺失的方法或结果当作 callable。

这不是普通的 validatorjs 断言不一致，而是可能同时包含：

* 字符类中的 identity escape 翻译错误；
* `\-` 在字符类中的处理错误；
* RegExp 构造失败后的异常传播错误；
* 调用指令在异常状态下继续执行。

应首先制作最小复现，而不是直接调完整 validatorjs：

```js
let r = /[@_\- ]/g;
print(r.test("-"));
print(r.test("@"));
print(r.test("_"));
print(r.test(" "));
```

然后分别检查：

* Lexer 得到的原始 pattern；
* AgentJS 转换后的 pattern；
* 传给 `RegexBuilder` 的最终字符串；
* fancy-regex 的具体编译错误。

## `regexp` checksum mismatch

checksum 错误说明 workload 已经执行了相当一部分，但某个 RegExp 结果与预期不同。可能涉及：

* `lastIndex`；
* global/sticky 状态；
* capture group；
* replace replacement token；
* UTF-16 code unit 与 Rust Unicode scalar 的差异；
* 空匹配推进；
* Unicode case folding；
* backreference；
* 字符类转义。

现有结果不足以定位到其中哪一个，因此不宜直接将其归因于 `/\-`。最好给 runner 增加分段 checksum，二分找到第一个产生差异的子用例。

---

# 四、性能差距不是单个慢函数，而是当前解释器架构的综合结果

11 个共同通过 workload 全部慢于 Boa，而且差距从 6.3 倍到 33.7 倍连续分布，这更像系统性开销，而不是某一个 benchmark 特有的 bug。

当前 release 配置已经开启：

* `opt-level = 3`
* thin LTO
* `codegen-units = 1`
* symbols stripping

因此不能简单归因于“没有使用 release 优化”。

## 1. 名称访问仍然偏重

VM 是 stack-based bytecode interpreter。变量访问指令包括：

```rust
LoadName(u16)
StoreName(u16)
```

其含义是沿 environment chain 查找名称。

每个 Environment 内部使用：

```rust
HashMap<String, Binding>
```

读取 binding 时还会 clone `JsValue`。

因此热点循环里的局部变量访问可能包含：

* constant table 找字符串；
* environment chain 遍历；
* String HashMap 查找；
* JsValue clone。

相比局部 slot/register index，这一成本会非常明显。

### 优化方向

编译阶段区分：

* 局部变量；
* closure captured variable；
* global name；
* with/eval 动态名称。

为普通局部变量增加类似：

```text
LoadLocal(slot)
StoreLocal(slot)
LoadClosure(depth, slot)
```

只有真正动态的名称才走 HashMap/environment chain。

---

## 2. 对象属性访问缺少通用 shape/inline cache

当前对象保存：

```rust
properties: PropertyMap
```

`PropertyMap` 内部是：

```rust
Vec<PropertyEntry>
HashMap<String, usize>
```

每次定义属性需要保存并复制字符串 key。删除一个属性时，会从 Vec 中移除元素，并遍历修改后续所有 index。枚举时还会复制 key 并重新排序数字索引。

对象模型中：

* symbol property 使用 Vec，查找为线性；
* array descriptor override 使用 Vec 并线性搜索；
* prototype access 仍需通用路径。

这会直接影响：

* `hash-map`
* `richards`
* `ai-astar`
* `splay`
* `threejs`
* MobX 等对象密集 workload。

### 优化方向

优先顺序应是：

1. property key intern；
2. 为固定名称属性访问增加 monomorphic inline cache；
3. 引入 shape/hidden class；
4. 对数组索引和普通属性完全分流；
5. 再考虑 polymorphic inline cache。

不一定需要一开始就做完整 JIT，仅在解释器里增加：

```text
GetPropertyCached(name, cached_shape, cached_offset)
```

就可能得到明显收益。

---

## 3. `JsValue` 和字符串复制成本较高

当前 `JsValue` 是普通 Rust enum，里面直接存储：

```rust
String(String)
BigInt(...)
Error(...)
```

并大量依赖 `Clone`。

这比使用：

* interned string；
* shared string；
* tagged pointer；
* NaN boxing；
* arena-owned immutable string；

更容易产生复制、分配和较大的 operand stack 元素。

这很可能影响 `stanford-crypto-sha256` 的 33.7 倍差距，因为 SHA256 workload 除了位运算，也会密集使用：

* 数组/TypedArray；
* 索引访问；
* 数字值搬运；
* 函数调用；
* 字符串和输入处理。

但具体占比仍需要 profiler 确认。

---

## 4. GC 是非移动式全堆 mark-and-sweep

当前 collector 明确是：

> Non-moving mark-and-sweep collector

标记集合使用多个 `HashSet`，每次收集需要从 root set 出发遍历对象、环境和函数，再 sweep heap。

对于短生命周期对象非常多的 workload：

* threejs；
* MobX；
* Web SSR；
* AST/DOM 类 workload；

这种 GC 容易造成周期性长暂停和较高遍历成本。

当前 JetStream 模式将 GC allocation threshold 提高到 100,000，虽然减少了收集次数，也可能让单次收集范围和内存峰值更大。

短期不必直接重写 GC，可以先记录：

* allocation count；
* GC count；
* 每次 GC 前后对象数；
* GC 总耗时；
* peak estimated bytes。

确认 threejs 是否主要消耗在 GC 后，再决定是否引入 nursery/generational GC。

---

# 五、`threejs` 目前更像性能超时，不像已证明的死循环

Boa 的耗时已经达到：

```text
146.489 秒
```

距离统一 150 秒门限只有约 3.5 秒。

AgentJS 在其他 workload 中通常比 Boa 慢 6–34 倍，因此 AgentJS 无法在 150 秒内完成 threejs 完全符合已有性能趋势。

所以当前结果只能写成：

> `PERF_TIMEOUT / INCONCLUSIVE`

而不是：

> 引擎存在 threejs 死循环。

## 建议重新测试

将 iteration 分别降低到：

```text
1、2、5、10
```

记录：

* parse 时间；
* bytecode compile 时间；
* initialization 时间；
* 单次 runIteration 时间；
* validate 时间；
* GC 时间。

如果 1 次 iteration 可以结束，且耗时随 iteration 近似线性增长，那么就是单纯性能问题。

如果 1 次都不能结束，再增加 opcode/loop 计数定位。

---

# 六、WSL 的风险比 threejs 更高

WSL 的特点是：

* AgentJS 持续高 CPU；
* 150 秒无输出；
* Boa 很快 TypeError；
* 没有任何一个引擎提供正确结果。

因此 Boa 不能作为正确性 oracle，当前也无法知道 AgentJS 是：

* 进入了无限循环；
* 错误的 loop termination；
* 某个操作退化成极慢路径；
* 或者只是在执行一个远超预期的 workload。

更值得注意的是，JetStream 模式使用：

```rust
loop_limit: u64::MAX
wall_clock_limit: None
```

也就是说，内部没有合作式超时；只有外部测试脚本在 150 秒后杀进程。

## 建议增加诊断模式

例如：

```text
agentjs jetstream --instruction-limit 500000000
agentjs jetstream --trace-hot-opcodes
agentjs jetstream --dump-on-limit
```

超过限制时输出：

* 当前 function；
* bytecode IP；
* 当前 opcode；
* 最近若干跳转；
* loop back-edge 次数；
* call depth；
* operand stack depth。

这可以区分：

* 正常但慢；
* 某一条指令退化；
* 真正不终止。

---

# 七、256 MiB 线程栈解决了 crypto，但带来“轻量级”风险

JetStream 命令会为每次执行创建专用线程：

```rust
.stack_size(256 * 1024 * 1024)
```

并设置：

```text
recursion_limit = 8192
```

这确实解决了之前 crypto 深递归导致的阻塞，也证明当前 class/super 已不是主要问题。

但它也带来一个需要在报告中主动说明的问题：

> 这是 benchmark 专用兼容措施，不应成为 AgentJS 面向多智能体并发执行时的默认配置。

如果每个并发脚本都需要 256 MiB 栈地址空间，那么：

* 高频短任务不够轻量；
* 多实例并发能力受限；
* 容器和 Windows 环境资源压力较大；
* 难以支撑“一个 agent 一个 isolate/thread”的架构。

长期应尽量让 JS 调用栈主要由 VM 的 `Vec<CallFrame>` 承担，而不是递归依赖 Rust/C 原生调用栈。

---

# 八、当前测试方法还不足以形成正式性能结论

你们目前的报告对限制说明得很诚实，但正式提交前还需要补充稳定性测试。

## 建议的正式测试协议

每项至少运行 5 次，最好 10 次，报告：

* median；
* min/max；
* p90；
* MAD 或标准差；
* peak RSS；
* allocation count；
* GC count；
* parse/compile/run 分段时间。

测试顺序不要总是：

```text
AgentJS → Boa
```

应交替：

```text
A-B-B-A
```

或者随机化，以减少 CPU 温度、缓存和后台任务影响。

同时记录：

* CPU 型号；
* Windows 版本；
* Rust 版本；
* AgentJS commit；
* Boa submodule commit；
* JetStream submodule commit；
* runner iteration count。

当前 JetStream 是固定在仓库 submodule commit 上的，这一点是好的，可复现性比直接追踪最新主分支更强。

---

# 九、建议的修复优先级

## P0：先修测试基础设施

负责文件：

```text
scripts/prepare-jetstream2.mjs
scripts/run-jetstream2*.ps1
benchmarks/generated/
```

目标：

* 正确嵌入 preload bundle；
* runner 生成后自动 smoke test；
* 输出完整资源 manifest；
* 区分 `GENERATION_FAILURE`、`RESOURCE_MISSING` 和引擎失败。

这是成本最低、最容易立刻改善报告可信度的工作。

## P1：修两个明确的 RegExp 正确性问题

顺序：

1. 最小复现 `/[@_\- ]/g`；
2. 修异常传播，避免继续出现 `undefined is not callable`；
3. 对 regexp workload 做分段 checksum；
4. 为发现的语义差异增加单元测试和 Test262 回归测试。

重点文件：

```text
src/lexer/
src/builtins/regexp.rs
src/unicode_set.rs
src/vm/interpreter.rs
```

## P1：明确 Intl 定位

短期二选一：

* 标记真实未支持的 formatter；
* 或实现一个最小但真实的 ICU4X NumberFormat。

不要继续扩大“对象外壳”，却让真实输出保持 identity formatting。

重点文件：

```text
src/intl/
src/builtins/binary_data.rs
```

此外，`binary_data.rs` 同时放置 BinaryData、Intl 和 Test262 host，文件已经超过五千行，建议拆分为：

```text
src/builtins/binary_data/
src/builtins/intl/
src/builtins/test262_host.rs
```

这对后续三人并行开发和减少 merge conflict 很重要。

## P1：建立性能观测能力

在开始大规模优化前增加：

* opcode execution counter；
* name lookup counter；
* property lookup/prototype depth；
* allocation/GC time；
* function-call count；
* parse/compile/run time；
* hottest bytecode functions。

否则很容易花大量时间优化一个并非主要瓶颈的模块。

## P2：解释器快速路径

按投入产出比排序：

1. 局部变量 slot；
2. interned property key；
3. named property inline cache；
4. 数组索引快速路径；
5. 减少 `JsValue`/String clone；
6. shape/hidden class；
7. generational GC；
8. 最后才考虑 JIT。

---

# 十、从赛题评分角度看

赛题要求的是：

* Test262 通过率超过 60%；
* 文档 20%；
* 功能完整度 30%；
* 性能 benchmark 30%；
* 创新性 20%。

因此要注意：

1. **11/19 JetStream runner 通过率不能代替 Test262 通过率。**
2. 你们目前的主要得分风险已经不再只是功能，而是性能 benchmark。
3. 资源缺失会让评委认为 benchmark 基础设施不完整，应尽快清除。
4. 256 MiB 栈虽然解决了兼容性，但需要在文档里明确属于 benchmark 专用配置。
5. 当前最值得展示的创新点不是“比 Boa 快”，而应是：

   * 自研 Rust parser/bytecode/runtime；
   * 非套壳；
   * 跨平台；
   * 对 Test262 新特性的较高覆盖；
   * 为短时 agent 执行设计的安全预算、资源限制和可观测性。
6. 若主打“轻量级”，正式报告必须补：

   * binary size；
   * cold-start latency；
   * 空脚本启动时间；
   * peak RSS；
   * 短脚本吞吐；
   * 多实例并发资源占用。

# 最终结论

当前 AgentJS 的状态可以概括为：

> **功能层已经形成可运行、可扩展的完整自研 JS 引擎骨架；剩余问题正从“缺少语法功能”转变为“生态语义尾部、测试适配和解释器性能”。**

最应该避免的两个误判是：

* 不要把 3 个资源缺失算成引擎功能失败；
* 不要把 threejs 超时直接算成死循环。

但也不能低估两个真实问题：

* Intl 目前仍主要是外壳，`NumberFormat` mismatch 是必然结果；
* 6.3–33.7 倍差距来自底层执行模型，无法通过调一个参数解决。

近期最合理的目标是：**先修 runner 资源发现，使 19 个项目都能真正进入 workload；同时解决两个 RegExp 正确性问题并建立分段 profiler。** 在此之后，再根据 opcode、属性访问和 GC 数据选择性能优化点。
