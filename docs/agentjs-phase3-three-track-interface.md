# AgentJS 第三阶段三人并行修复方案与接口共享文档

> 仓库：`ChaiXinran/CSCC-proj2`  
> 扫描基线：`main@229881b2af45cbb02d4ff032c4dcab3a3cc3079a`  
> 日期：2026-08-04  
> 目标：在 Shared String、Compact PropertyMap、Local Slot 已落地后，继续解决闭包名字查找、属性访问热路径以及 JetStream 生命周期与大内存问题。

---

## 1. 结论与本轮优先级

上一轮已经完成或基本完成：

1. `JsValue::String` 已迁移到 `JsString(Arc<str>)`；
2. `PropertyMap` 的 entry 与 index 共享属性名 backing storage；
3. `PropertyMap::delete()` 已改为 tombstone，不再 `Vec::remove()` 并重写后续索引；
4. 当前函数 activation 的参数、rest、函数级 `var`、函数体直接函数声明已进入 Local Slot；
5. 默认参数、pattern 参数和 rest pattern 在可用时使用 Local opcode；
6. Runner 已统一为 staged 外部资源执行，不再嵌入 workload 或拼接完整 workload。

下一轮三条主线：

| 组别 | 主线 | 主要目标 |
|---|---|---|
| G 组 | Upvalue Slot | 闭包访问父级/祖先函数变量时不再逐层按字符串查找 |
| H 组 | Shape + Monomorphic Property IC | 加速普通对象的命名属性读写，建立后续 Hidden Class/IC 基础 |
| I 组 | Run Lifecycle + Frontend Memory | 去除固定 256 MiB 栈；让 wall-clock 覆盖整个 runner；降低 WSL/jsdom 前端峰值内存并定位 threejs 运行期增长 |

本轮不同时实现：

- JIT；
- 分代 GC；
- 多态/巨型属性 IC；
- 完整模块 binding slot；
- block lexical slot；
- 全局字符串驻留池；
- 将全部 AST 字符串一次性改为 Atom；
- Proxy、Accessor、Symbol、computed property 的 IC；
- 将所有 JetStream 功能失败混入性能重构。

---

## 2. 开工前 P0：修复最新 main 的合并错误

当前 `src/runtime/string_value.rs` 中：

```rust
impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
        Self::new(value)
    }
}
```

这是一个确定的编译错误。必须先单独修为：

```rust
impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}
```

或者：

```rust
impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
```

不得将该修复混入 G/H/I 任一功能分支。

建议提交：

```text
fix/js-string-merge-regression
```

开工门槛：

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

三组必须从上述门禁通过后的同一个 SHA 建立分支。

---

## 3. 最新问题状态

| 原问题 | 最新状态 | 本轮处理 |
|---|---|---|
| `JsValue::String(String)` 深复制 | 结构已修复，但最新合并有编译错误 | P0 修复，不再作为主线 |
| PropertyMap 重复保存字符串 backing | 已修复 | 保持接口稳定 |
| `PropertyMap::delete()` O(n) 更新索引 | 已修复 | H 组利用 stable slot/generation 建 IC |
| 当前 activation Local Slot | 已完成第一阶段 | 保持兼容 |
| Upvalue Slot | 未实现 | G 组 |
| Shape / Hidden Class / Inline Cache | 未实现 | H 组 |
| JetStream 固定 256 MiB 栈 | 未修复 | I 组 |
| wall-clock 不覆盖完整 runner/解析 | 未修复 | I 组 |
| WSL/jsdom/threejs 大内存 | 未修复 | I 组主导，H 组提供运行期属性加速 |
| regexp staged 作用域差异 | 未修复 | 本轮记录，不恢复整包拼接；不作为三组主目标 |
| validatorjs 断言差异 | 未修复 | 独立功能 issue，不混入本轮 |

现有受保护矩阵为 12/19 PASS，内存超限项包括：

```text
ai-astar
jsdom-d3-startup
threejs
WSL
splay
```

其中 WSL 和 jsdom-d3-startup 很快达到限制，更像前端解析/编译峰值；threejs 在长时间执行后达到限制，更像运行期对象、属性或 GC 增长。该判断属于基于阶段时间的工程推断，必须由 I 组的阶段内存数据验证。

---

## 4. 分支与合并规则

### 4.1 分支

从 P0 修复后的同一 SHA 建立：

```text
perf/upvalue-slots
perf/shape-property-ic
perf/run-lifecycle-frontend-memory
integration/phase3
```

### 4.2 合并顺序

建议：

```text
P0 compile fix
→ I：Run Lifecycle / Frontend Memory
→ G：Upvalue Slot
→ H：Shape / Property IC
→ integration/phase3
```

理由：

1. I 组主要修改 `main/engine/backend/lexer/parser`，先合并可建立统一预算和诊断；
2. G 组主要修改 compiler/environment/opcode 的名字访问路径；
3. H 组修改 object/property/context/interpreter，运行时影响最大，最后合并便于集中处理冲突；
4. 最终集成人负责把 Compiler deadline checkpoint 接入 G 合并后的 compiler。

### 4.3 提交粒度

每组至少拆分：

```text
1. 接口与数据结构
2. 慢路径保持与快路径实现
3. 正确性测试
4. diagnostics
5. benchmark/report
```

禁止一个提交同时改接口、重写执行逻辑、更新 generated runner 和测试报告。

---

# Part G：Upvalue Slot

## 5. 目标

当前 Local Slot 只优化当前函数 activation。闭包读取或写入父函数变量仍使用：

```text
LoadName / StoreName
→ 当前环境检查
→ outer 链遍历
→ 每层 slot_index / bindings HashMap 字符串查找
```

G 组目标：

```text
静态闭包变量
→ Upvalue descriptor
→ 从函数捕获环境按固定 hops 定位目标 activation
→ 直接 LocalSlot 读写
```

第一阶段支持：

- 读取直接父函数 slot；
- 写入直接父函数 slot；
- 读取/写入祖先函数 slot；
- 箭头函数；
- 普通函数表达式和函数声明；
- async/generator 恢复后的 upvalue；
- 在 block/loop 中创建的闭包，通过固定 environment hops 定位 activation。

第一阶段不支持：

- `with` 可见范围中的 upvalue；
- 含 direct eval 的函数及其动态受影响引用；
- module import/export cell；
- global binding；
- block lexical 直接槽位；
- catch binding 直接槽位。

这些情况继续 `LoadName/StoreName`。

---

## 6. G 组共享类型

### 6.1 UpvalueSlot

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpvalueSlot(pub u16);
```

`UpvalueSlot` 是当前函数 `UpvalueLayout` 中的索引，不是 Environment slot。

### 6.2 UpvalueDescriptor

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpvalueDescriptor {
    /// 从 JsFunction.environment（函数创建时捕获的声明环境）开始，
    /// 沿 outer 需要移动的次数。
    pub environment_hops: u16,

    /// 目标函数 activation 中的 LocalSlot。
    pub local_slot: LocalSlot,
}
```

约定：

- `environment_hops == 0` 表示捕获环境本身就是目标 activation；
- 子函数 activation 自身不计入 hops；
- 运行时从 `JsFunction.environment` 开始，而不是从当前 block environment 开始；
- descriptor 不保存名字，名字只用于编译、诊断和 fallback。

### 6.3 UpvalueLayout

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpvalueLayout {
    pub bindings: Vec<UpvalueBindingLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpvalueBindingLayout {
    pub name: String,
    pub descriptor: UpvalueDescriptor,
    pub mutable: bool,
}
```

`FunctionTemplate` 与 `JsFunction` 增加：

```rust
pub upvalue_layout: Arc<UpvalueLayout>,
```

所有函数实例共享 layout，不复制 descriptor 列表。

---

## 7. G 组 bytecode

新增：

```rust
Instruction::LoadUpvalue(UpvalueSlot)
Instruction::StoreUpvalue(UpvalueSlot)
```

栈效果：

```text
LoadUpvalue:   []      → [value]
StoreUpvalue:  [value] → [value]
```

不新增 `InitializeUpvalue`。Upvalue 所属 binding 必须由拥有它的 activation 初始化。

Chunk 验证：

- `UpvalueSlot` 必须小于模板的 `upvalue_layout.bindings.len()`；
- `environment_hops` 不要求在编译时读取真实 heap，但必须不溢出 u16；
- StoreUpvalue 对 immutable descriptor 必须在运行时抛 TypeError；
- nested chunk 独立验证自己的 layout。

---

## 8. G 组编译解析规则

标识符解析顺序：

```text
1. 当前函数 LocalSlot
2. 当前函数可静态解析的 UpvalueSlot
3. 当前 lexical/dynamic name path
4. global path
```

建议编译结构：

```rust
enum ResolvedBinding {
    Local(LocalSlot),
    Upvalue(UpvalueSlot),
    DynamicName,
    Global,
}
```

`CompileContext` 新增只读捕获作用域链：

```rust
struct CaptureScope {
    local_slots: HashMap<String, LocalSlot>,
    /// 函数定义位置到该 activation 的环境距离。
    environment_hops: u16,
    dynamic_scope: DynamicScopePolicy,
}
```

规则：

- 就近作用域优先；
- 父函数没有对应 LocalSlot 时继续向祖先查找；
- 任一相关作用域由 `with/direct eval` 影响时回退名字路径；
- 子函数自身含 direct eval/with 时，子函数全部 Upvalue opcode 回退名字路径；
- 不得因为父函数有 LocalSlot 就错误捕获同名 block lexical；
- 函数创建位置的 `environment_depth` 必须计入 hops；
- loop per-iteration environment 仍保持每次迭代独立闭包语义。

---

## 9. G 组运行时接口

在 `NativeContext` 增加：

```rust
pub fn resolve_environment_hops(
    &self,
    start: EnvironmentId,
    hops: u16,
) -> Result<EnvironmentId, VmError>;

pub fn get_upvalue(
    &self,
    function: FunctionId,
    slot: UpvalueSlot,
) -> Result<JsValue, VmError>;

pub fn set_upvalue(
    &mut self,
    function: FunctionId,
    slot: UpvalueSlot,
    value: JsValue,
) -> Result<(), VmError>;
```

运行流程：

```text
当前 CallFrame.function
→ JsFunction.upvalue_layout[slot]
→ JsFunction.environment
→ resolve_environment_hops
→ Environment.get_local / set_local
```

GC 约束：

- 不新增独立 environment root；
- `JsFunction.environment` 已是捕获链根；
- UpvalueLayout 只含整数、名字和标志，不参与 GC Trace；
- generator/async suspended record 中的 FunctionId 必须继续保持函数可达。

---

## 10. G 组 diagnostics 与验收

新增：

```rust
pub struct NameResolutionMetrics {
    // existing
    pub load_local_count: u64,
    pub store_local_count: u64,
    pub load_name_count: u64,
    pub store_name_count: u64,
    pub environment_hops: u64,

    // new
    pub load_upvalue_count: u64,
    pub store_upvalue_count: u64,
    pub upvalue_environment_hops: u64,
}
```

定向测试：

```text
父变量读取
父变量写入
祖父变量读取/写入
同名 shadowing
block 中创建闭包
for-let 每迭代闭包
箭头 lexical this + upvalue
async await 后 upvalue
生成器 yield 后 upvalue
direct eval 回退
with 回退
TDZ/const 写入
GC 后闭包继续访问
```

Test262 重点：

```text
language/expressions/arrow-function
language/expressions/function
language/statements/function
language/expressions/generators
language/expressions/async-function
language/statements/let
language/statements/const
language/statements/for
language/statements/for-of
built-ins/eval
language/statements/with
```

性能 workload：

```text
richards
splay
crypto
raytrace
navier-stokes
mobx
```

DoD：

- [ ] 当前 Local Slot 测试全部保持通过；
- [ ] 闭包静态引用生成 Upvalue opcode；
- [ ] dynamic scope 100% fallback；
- [ ] Test262 无净回归；
- [ ] eligible closure accesses 中 Upvalue 命中率 >= 70%；
- [ ] 五个核心 workload median 几何平均不回退；
- [ ] 至少一个闭包密集 workload median 改善 >= 10%；
- [ ] GC threshold 10k/100k/1m 下语义一致。

---

# Part H：Shape + Monomorphic Property IC

## 11. 目标

Compact PropertyMap 已提供：

- 共享 PropertyName；
- stable `PropertySlotId`；
- `descriptor_at(slot)`；
- tombstone 和 compaction。

但当前命名属性访问仍然每次走：

```text
ToObject
→ 特殊对象判断
→ PropertyMap HashMap 查询
→ descriptor clone/accessor/prototype 处理
```

H 组目标：对最常见的“普通对象、自有、字符串命名、data property”建立 Shape 与单态缓存。

第一阶段只缓存：

```text
Ordinary object
string named property
own data property
普通 GetProperty
已有 writable data property 的 SetProperty
```

以下必须慢路径：

```text
Proxy
Accessor
Symbol
computed GetElement/SetElement
Array dense index
TypedArray
String wrapper index/length
RegExp/Promise/Generator exotic behavior
prototype property
super property
private field
missing property/negative cache
```

第一阶段不缓存 prototype chain 和 accessor，先保证失效规则正确。

---

## 12. H 组 Shape 接口

### 12.1 ShapeId

```rust
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);
```

### 12.2 ShapeMode

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeMode {
    Fast,
    Dictionary,
}
```

删除、复杂 descriptor 重配置或无法共享的结构可以进入 `Dictionary`，该模式禁用 IC，但不改变 PropertyMap 语义。

### 12.3 ShapeRecord

```rust
pub struct ShapeRecord {
    pub parent: Option<ShapeId>,
    pub property: Option<PropertyName>,
    pub slot: Option<PropertySlotId>,
    pub attributes: Option<PropertyAttributes>,
    pub mode: ShapeMode,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyAttributes {
    pub kind: PropertyKindTag,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}
```

Shape 不保存属性值，也不保存 GC ObjectId。

### 12.4 ShapeTable

建议新增：

```text
src/runtime/shape.rs
```

```rust
pub struct ShapeTable {
    shapes: Vec<ShapeRecord>,
    transitions: HashMap<ShapeTransitionKey, ShapeId>,
}
```

相同父 Shape、属性名、slot 和 attributes 的 ordinary object 共享 transition shape。

`JsObject` 增加：

```rust
pub shape: ShapeId,
```

ShapeTable 属于一个 NativeContext/isolate，不跨 Runtime 共享。

---

## 13. PropertyMap generation 合同

因为 compaction 会重排 `PropertySlotId`，IC 不能只验证 ShapeId。

`PropertyMap` 墈加：

```rust
pub fn generation(&self) -> u64;
```

结构变化时 generation 增加：

- 新增属性；
- 删除属性；
- delete 后 redefine；
- data/accessor kind 改变；
- writable/enumerable/configurable 改变；
- compact；
- 任何可能改变 slot 或 descriptor 解释的操作。

只修改已有 writable data property 的 value，不增加 generation。

新增内部 mutation API，保留现有公开兼容包装：

```rust
pub struct PropertyMutation {
    pub slot: Option<PropertySlotId>,
    pub structural: bool,
    pub compacted: bool,
}

pub(crate) fn define_with_outcome(...) -> PropertyMutation;
pub(crate) fn delete_with_outcome(...) -> Option<(PropertyDescriptor, PropertyMutation)>;
```

现有：

```rust
PropertyMap::define(...)
PropertyMap::delete(...)
```

可以调用新 API 并丢弃 outcome，避免一次修改全部上层代码。

---

## 14. H 组 Inline Cache 接口

不新增 property opcode，继续复用现有：

```rust
Instruction::GetProperty(name_index)
Instruction::SetProperty(name_index)
```

缓存 site 使用当前 SharedChunk 的稳定 Arc 地址和 instruction offset：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BytecodeSite {
    pub chunk_address: usize,
    pub instruction_offset: u32,
}
```

缓存只存在于当前 VM/Runtime 生命周期，不序列化、不写入 SharedChunk、不跨进程，因此 Arc 地址只作为 opaque key，不解引用。

```rust
pub struct GetPropertyCacheEntry {
    pub receiver_shape: ShapeId,
    pub property_generation: u64,
    pub slot: PropertySlotId,
}

pub struct SetPropertyCacheEntry {
    pub receiver_shape: ShapeId,
    pub property_generation: u64,
    pub slot: PropertySlotId,
}
```

建议新增：

```text
src/vm/property_cache.rs
```

```rust
pub struct PropertyInlineCaches {
    get: HashMap<BytecodeSite, GetPropertyCacheEntry>,
    set: HashMap<BytecodeSite, SetPropertyCacheEntry>,
}
```

---

## 15. H 组缓存命中与失效

### GetProperty 命中

```text
receiver 是 ObjectId
→ ObjectKind::Ordinary
→ shape == cached.receiver_shape
→ properties.generation == cached.property_generation
→ descriptor_at(cached.slot) 仍是 data property
→ clone value
```

任一条件不满足：执行现有慢路径，并在 eligible 时更新 cache。

### SetProperty 命中

```text
receiver 是 ordinary object
→ shape/generation 匹配
→ descriptor_at_mut(slot) 是 writable data property
→ 只替换 value
→ shape/generation 保持不变
```

以下操作必须失效：

- 添加新属性；
- 删除属性；
- compaction；
- accessor/data 转换；
- descriptor attributes 改变；
- prototype 改变；
- Object.preventExtensions/seal/freeze 影响属性语义；
- 对象转 dictionary mode。

第一阶段不做 negative cache，缺失属性永远慢路径。

---

## 16. H 组 diagnostics 与验收

新增：

```rust
pub struct PropertyCacheMetrics {
    pub get_hits: u64,
    pub get_misses: u64,
    pub set_hits: u64,
    pub set_misses: u64,
    pub shape_transitions: u64,
    pub dictionary_objects: u64,
    pub invalidations: u64,
}
```

测试：

```text
同构 ordinary objects 共享 shape
新增相同属性序列产生相同 shape
不同属性顺序产生不同 shape
更新 data value 不改变 shape/generation
新增/删除/reconfigure 改变 generation
compaction 失效旧 slot cache
delete + redefine 插入顺序不变
accessor 不缓存
Proxy 不缓存
prototype property 不缓存
freeze/seal/preventExtensions 正确
GC 后 cache 不持有 ObjectId 根
```

Test262 重点：

```text
built-ins/Object
built-ins/Reflect
built-ins/Proxy
language/types/object
language/expressions/object
language/expressions/property-accessors
language/expressions/delete
language/statements/for-in
```

性能 workload：

```text
hash-map
mobx
web-ssr
threejs
jsdom-d3-startup
raytrace
```

DoD：

- [ ] PropertyMap 现有顺序/tombstone 测试全部通过；
- [ ] IC 不改变 observable property semantics；
- [ ] cache 不保存 ObjectId，不形成 GC root；
- [ ] eligible GetProperty 命中率 >= 70%；
- [ ] eligible SetProperty 命中率 >= 60%；
- [ ] 共同 PASS workload 无 >5% median 回退；
- [ ] hash-map/mobx/threejs 至少一项 wall-time 改善 >=10%；
- [ ] 完整 Test262 无净回归。

---

# Part I：Run Lifecycle + Frontend Memory

## 17. 目标

当前 JetStream CLI 存在三项相关问题：

1. 每次启动固定申请 `256 * 1024 * 1024` 字节线程栈；
2. `wall_clock_limit` 在每次 `runtime.eval()` 前重置，staged runner 的 prelude、每个资源和 launch 各自获得完整预算；
3. deadline 存在于 NativeContext，Lexer/Parser/Compiler 不做周期检查，无法中断长解析/编译；
4. Token 已有 Span，但 Identifier/String/Template/PrivateName 等仍各自持有 String，前端峰值可能同时保留 source、token 文本和 AST 文本。

I 组目标：

- 使用一个贯穿 runner read、prelude、全部 resources、launch 和 jobs 的绝对 deadline；
- 将线程栈改为可配置且默认不超过 32 MiB；
- 增加前端 cooperative checkpoint；
- 将无转义 token 文本改为 source slice 表示，降低 WSL/jsdom 前端峰值；
- 建立阶段内存诊断，区分前端峰值、heap 增长和 GC 行为。

---

## 18. I 组执行预算接口

### 18.1 AbsoluteDeadline

建议新增：

```text
src/runtime/deadline.rs
```

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct AbsoluteDeadline {
    at: Option<Instant>,
}

impl AbsoluteDeadline {
    pub fn from_duration(start: Instant, duration: Option<Duration>) -> Self;
    pub fn check(self) -> Result<(), RuntimeLimitError>;
    pub fn remaining(self) -> Option<Duration>;
    pub fn is_expired(self) -> bool;
}
```

### 18.2 RunControl

```rust
#[derive(Debug, Clone, Copy)]
pub struct RunControl {
    pub deadline: AbsoluteDeadline,
}
```

`Runtime` 增加：

```rust
pub fn set_run_control(&mut self, control: Option<RunControl>);
```

规则：

- 未设置 RunControl 时，继续使用 `RuntimeConfig.wall_clock_limit` 作为单 eval 预算；
- 设置后，所有 eval 使用同一个 absolute deadline；
- `NativeRuntime::evaluate()` 不得重新延长 deadline；
- staged runner 的每个资源不能重置预算；
- job drain、Promise callback、dynamic Function、eval 使用同一 run deadline。

### 18.3 FrontendControl

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct FrontendControl {
    pub deadline: AbsoluteDeadline,
}

impl FrontendControl {
    #[inline]
    pub fn checkpoint(self) -> Result<(), NativeError>;
}
```

接入频率建议：

```text
Lexer：每扫描 4096 bytes
Parser：每消费 1024 tokens 或每 256 statements
Compiler：每 emit 1024 instructions 或每 256 AST nodes
Module graph：每读取/解析一个 module
Host staged runner：每个 resource 前后
```

不得每个字符/每条指令调用 `Instant::now()`，避免显著性能回退。

---

## 19. I 组线程栈接口

JetStream CLI 增加：

```text
--thread-stack-mib N
```

建议默认：

```text
32 MiB
```

允许范围：

```text
4..=256 MiB
```

定义：

```rust
const DEFAULT_JETSTREAM_THREAD_STACK: usize = 32 * 1024 * 1024;
```

规则：

- 线程栈属于 CLI Host 配置，不加入 `RuntimeConfig`；
- 不能再硬编码 256 MiB；
- `crypto`、深递归 Test262、call-depth limit 在 8/16/32 MiB 下分别验证；
- 默认选择满足正确性的最小档；
- 栈溢出必须表现为受控错误或线程失败，不能 silently retry 256 MiB。

---

## 20. I 组 Frontend token 文本接口

当前 Token 已有 `Span`。目标不是一次重写 AST，而是先减少 tokenize/parse 峰值。

新增：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenText {
    /// 原文无需解码，使用 Token.span 从 Arc<str> source 读取。
    SourceSlice,
    /// 包含 escape 或需要 cooked value，保存共享文本。
    Cooked(JsString),
}
```

建议逐步修改：

```rust
TokenKind::Identifier(TokenText)
TokenKind::BigInt(TokenText)
TokenKind::PrivateName(TokenText)
TokenKind::String(TokenText)
TokenKind::TemplateLiteral(TokenText)
TokenKind::TemplateHead(TokenText)
TokenKind::TemplateMiddle(TokenText)
TokenKind::TemplateTail(TokenText)
```

Parser 统一通过：

```rust
impl Token {
    pub fn text<'a>(&'a self, source: &'a str) -> &'a str;
    pub fn text_owned(&self, source: &str) -> String;
    pub fn text_shared(&self, source: &str) -> JsString;
}
```

规则：

- 无 escape identifier/private name/bigint 使用 SourceSlice；
- 有 Unicode escape identifier 使用 Cooked；
- String/template 需要 cooked 语义时使用 Cooked；
- `template_raw` 优先也使用 Span/TokenText，不保留第二份 raw String；
- AST 仍只保存真正需要长期存在的语义文本；
- Parser 完成后 token Vec 必须可立即释放；
- 不在本轮将全部 AST 字段替换为 JsString。

这条优化主要针对 WSL/jsdom 等大 bundle 的 parse 峰值。

---

## 21. I 组内存与阶段诊断

新增统一 diagnostics：

```rust
pub struct PhaseDiagnostics {
    pub phase: &'static str,
    pub elapsed_ms: u64,
    pub source_bytes: usize,
    pub token_count: Option<usize>,
    pub instruction_count: Option<usize>,
    pub constant_count: Option<usize>,
    pub function_count: Option<usize>,
    pub heap: HeapStats,
    pub gc: GcMetrics,
}
```

阶段：

```text
runner_read_start/end
prelude_parse/compile/execute
resource_read_start/end
resource_parse/compile/execute
launch_parse/compile/execute
job_drain_start/end
run_end
```

外部 diagnostics 脚本继续按 100ms 或更高频率采样 process working set，并与阶段 marker 对齐。

内部 heap stats 与外部 RSS 必须同时记录，因为：

- RSS 高而 heap estimated bytes 低：更可能是 source/token/AST/allocator/stack；
- heap 与 RSS 同时增长：更可能是 runtime object/property/string；
- GC 后 heap 降而 RSS 不降：可能是 allocator retention，不代表 live heap 未释放。

---

## 22. I 组 GC 与大内存处理边界

I 组先测试，不直接武断修改 GC 默认值：

```text
10_000
100_000
1_000_000
usize::MAX
```

对 WSL、jsdom、threejs、ai-astar、splay 记录：

```text
峰值 RSS
heap estimated bytes
collection count
bytes before/after
pause
功能状态
```

决策：

- WSL/jsdom 在 parse/compile 阶段达到峰值：优先 TokenText/前端释放；
- threejs/ai-astar/splay 在 execute 阶段 live heap 持续增长：评估较低 threshold 或 adaptive policy；
- 若低 threshold 出现语义差异，先修 GC roots，不用高 threshold 掩盖；
- 若 GC 后 live heap 下降但 RSS 不降，记录 allocator retention，不声称 GC 泄漏。

本轮允许在数据支持后增加：

```rust
pub enum GcPolicy {
    Fixed { allocation_threshold: usize },
    Adaptive {
        min_threshold: usize,
        max_threshold: usize,
        growth_percent: u16,
    },
}
```

但只有阈值矩阵证明正确且改善内存时才能改变 JetStream 默认策略。

---

## 23. I 组验收

正确性：

- [ ] 1 秒 deadline 能中止 lexer/parser/compiler/execute 中的人造长任务；
- [ ] staged 多 eval 的总时长不能获得 N 倍预算；
- [ ] timeout 误差目标 <= 2 秒；
- [ ] 默认 thread stack <= 32 MiB；
- [ ] crypto 和递归定向测试通过；
- [ ] 无 escape 与 escaped identifier/string/template 语义一致；
- [ ] Test262 无净回归。

内存：

- [ ] WSL/jsdom token 文本分配显著下降；
- [ ] WSL 或 jsdom 至少一个峰值 RSS 改善 >= 20%，或从 1.5 GiB 超限降到限制内；
- [ ] threejs 明确定位峰值阶段；
- [ ] 19 项 runner 无残留进程；
- [ ] 所有失败分类为 RuntimeLimit/MemoryLimit/功能错误，不出现无限挂起。

---

# 共享文件所有权

## 24. 文件分配

| 文件 | G Upvalue | H Shape/IC | I Lifecycle/Frontend | 集成人 |
|---|---:|---:|---:|---:|
| `src/bytecode/chunk.rs` | Upvalue 类型/layout | 禁止 | 禁止 | 公共导出冲突 |
| `src/bytecode/opcode.rs` | Upvalue opcode | 禁止新增 property opcode | 禁止 | 最终格式化 |
| `src/bytecode/compiler.rs` | 主负责人 | 禁止 | 只提出 deadline hook | 合并后接 deadline checkpoint |
| `src/runtime/environment.rs` | 主负责人 | 禁止 | 禁止 | 审核 |
| `src/runtime/function.rs` | upvalue layout | 禁止 | 禁止 | 审核 |
| `src/runtime/property_map.rs` | 禁止 | 主负责人 | 禁止 | 审核 |
| `src/runtime/shape.rs` | 禁止 | 主负责人 | 禁止 | 公共导出 |
| `src/runtime/object.rs` | 禁止 | 主负责人 | 字符串适配禁止扩散 | 审核 |
| `src/runtime/context.rs` | binding/upvalue 区域 | property/shape 区域 | budget/metrics 区域 | 解决冲突 |
| `src/vm/interpreter.rs` | Upvalue match arms | Get/Set property fast path | 禁止大改 | 解决冲突 |
| `src/vm/property_cache.rs` | 禁止 | 主负责人 | 禁止 | 导出 |
| `src/engine.rs` | 禁止 | 禁止 | 主负责人 | 审核 |
| `src/backend/mod.rs` | 禁止 | 禁止 | 主负责人 | 审核 |
| `src/main.rs` | 禁止 | 禁止 | 主负责人 | 最终 CLI 合并 |
| `src/lexer/*` | 禁止 | 禁止 | 主负责人 | 审核 |
| `src/parser/*` | 禁止 | 禁止 | 主负责人 | 审核 |
| `src/runtime/string_value.rs` | 不修改 | 不修改 | 仅使用接口 | P0/集成人 |
| `src/runtime/mod.rs` | 不直接修改 | 不直接修改 | 不直接修改 | 唯一合并者 |
| `src/contracts.rs` | 提议 | 提议 | 提议 | 唯一合并者 |

---

## 25. 共享大文件规则

### `src/bytecode/compiler.rs`

- G 组拥有 binding resolution、nested function capture、Upvalue lowering；
- I 组不得直接重写 compiler，只提交 `FrontendControl` 接口和 checkpoint 位置清单；
- 集成人在 G 合并后增加少量 checkpoint；
- 禁止全文件格式化产生无关 diff。

### `src/runtime/context.rs`

按区域：

```text
G：Environment/Binding/Function upvalue
H：Object property/shape/cache slow path
I：ExecutionBudget/deadline/heap diagnostics
```

三组不得移动其他区域函数或重命名无关 API。

### `src/vm/interpreter.rs`

- G 组只加入 Upvalue instruction arms；
- H 组只改 GetProperty/SetProperty 热路径及 cache site；
- I 组不修改主解释器 match，只通过 Context 的 absolute deadline；
- 集成人最终解决 import 和 match 区域冲突。

---

# 统一测试与验收

## 26. 基线

P0 后生成：

```text
reports/phase3-baseline-<sha>/
├── cargo-gates.txt
├── test262-summary.json
├── jetstream-19-summary.json
├── gc-threshold-summary.json
├── phase-memory-summary.json
└── logs/
```

当前旧矩阵只能作为历史参考，不能代替 P0 后最新 Shared String + PropertyMap + Local Slot 的统一基线。

---

## 27. 每次合并门禁

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

每组至少跑自己的定向 Test262；最终集成人跑完整 Test262。

---

## 28. JetStream 统一矩阵

19 个 canonical runner：

```text
ai-astar
crypto
gaussian-blur
hash-map
cdjs
intl
jsdom-d3-startup
mobx
threejs
validatorjs
web-ssr
WSL
navier-stokes
raytrace
regexp
richards
splay
stanford-crypto-sha256
test-cdjs
```

每项记录：

```text
status
wall median / p90（性能项 5 次）
peak RSS
phase at peak
heap estimated bytes
GC count / total pause / max pause
Local/Upvalue/Name counts
property IC hits/misses
thread stack size
absolute deadline
```

---

## 29. 回归门槛

正确性：

```text
完整 Test262 不得出现未记录净回归
cargo gates 全通过
GC threshold 语义一致
```

性能：

```text
共同 PASS workload median 不得回退 >5%
任何 >5% 回退必须定位并回滚或记录明确例外
不得用增大内存限制换取速度
不得用恢复 workload 拼接绕过 staged 语义
```

内存：

```text
默认 thread stack <=32 MiB
1.5 GiB 受保护矩阵 PASS 数不得低于最新 P0 基线
至少一个当前 MEMORY_LIMIT workload 被拉回限制内，或峰值下降 >=20%
```

---

# 风险与回滚

## 30. G 组回滚

出现以下情况，回滚到“只优化直接父函数 slot”：

```text
祖先 hops 错误
for-let 闭包共享错误
direct eval/with 语义回归
async/generator resume 后引用错误
```

不得删除 `LoadName/StoreName` fallback。

## 31. H 组回滚

出现以下情况，先关闭 IC，保留 Shape metrics：

```text
属性顺序变化
accessor/Proxy 被错误缓存
compaction 后读错 slot
freeze/seal 语义变化
GC 因 cache 保留对象
```

缓存必须可以通过配置/测试完全禁用，慢路径永远是语义真值。

## 32. I 组回滚

出现以下情况，保留 absolute deadline 与可配置栈，回滚 TokenText 扩展：

```text
escaped identifier/string/template 语义回归
Parser API 改动过大
Test262 大面积退化
前端内存无可测改善
```

不得因为 deadline 难接入而继续为每个 staged eval 重置完整预算。

---

# 最终 Definition of Done

## 33. G：Upvalue Slot

- [ ] LoadUpvalue/StoreUpvalue 全链路完成；
- [ ] 父级和祖先变量读写正确；
- [ ] dynamic scope 回退；
- [ ] async/generator/loop closure 正确；
- [ ] diagnostics 与性能报告完成；
- [ ] Test262 无净回归。

## 34. H：Shape / Property IC

- [ ] ShapeTable 与 ordinary object shape 完成；
- [ ] PropertyMap generation 完成；
- [ ] own data Get/Set 单态 cache 完成；
- [ ] 所有 exotic/accessor/prototype 情况慢路径；
- [ ] 失效和 compaction 测试完成；
- [ ] cache hit 与 workload 报告完成。

## 35. I：Lifecycle / Frontend Memory

- [ ] 默认线程栈不超过 32 MiB；
- [ ] 一个 absolute deadline 覆盖完整 runner；
- [ ] Lexer/Parser/Compiler cooperative checkpoint；
- [ ] TokenText/Span 降低前端重复文本；
- [ ] phase memory diagnostics 完成；
- [ ] 至少一个 MEMORY_LIMIT workload 明显改善。

## 36. 集成

- [ ] P0 merge regression 修复；
- [ ] 三分支顺序合并；
- [ ] cargo gates 全通过；
- [ ] 完整 Test262；
- [ ] 19 项 JetStream；
- [ ] 1.5 GiB 矩阵；
- [ ] GC threshold 矩阵；
- [ ] stack 8/16/32 MiB 矩阵；
- [ ] deadline 覆盖测试；
- [ ] 输出 `reports/phase3-integration-report.md`；
- [ ] 明确下一阶段是否进入 prototype IC、多态 IC、block slot 或 persistent Host script environment。

---

# 37. 推荐七天节奏

### Day 0

```text
修 P0 编译错误
跑统一基线
冻结共享类型与文件所有权
```

### Day 1–2

```text
G：UpvalueLayout + 编译解析
H：ShapeTable + PropertyMap generation
I：absolute deadline + 可配置 thread stack
```

### Day 3–4

```text
G：VM Upvalue + async/generator/GC 测试
H：Get/Set property IC + invalidation
I：TokenText/Span + Lexer/Parser checkpoint
```

### Day 5

```text
各组定向 Test262
各组 5 轮 benchmark
独立报告
```

### Day 6

```text
I → G → H 顺序合并
每次合并跑 cargo gates
```

### Day 7

```text
完整 Test262
19 项 JetStream
RSS/GC/stack/deadline 矩阵
集成报告与下一轮决策
```
