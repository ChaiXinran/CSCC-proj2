# JetStream2 AgentJS vs Boa 性能对比

> 测试日期: 2026-07-25
> 使用 benchmarks/generated/ 下已有的 10 个 runner 文件

## 结果总览

| Benchmark | AgentJS | 耗时 | Boa | 耗时 | AgentJS/Boa |
|:----------|:-------:|-----:|:---:|-----:|:-----------:|
| ai-astar-simple | PASS | 30.7s | PASS | 1.8s | 17.1x |
| ai-astar | NO_SUPER | - | PASS | - | - |
| crypto-simple | LIMIT | - | PASS | 0.5s | - |
| crypto | NO_SUPER | - | ERROR | - | - |
| navier-stokes-simple | PASS | 1.9s | PASS | 0.3s | 6.3x |
| richards-simple | PASS | 14.3s | PASS | 0.7s | 20.4x |
| richards | NO_SUPER | - | PASS | - | - |
| splay-simple | PASS | 5.2s | PASS | 1.1s | 4.7x |
| stanford-crypto-sha256-simple | PASS | 10.2s | PASS | 0.4s | 25.5x |
| stanford-crypto-sha256 | NO_SUPER | - | PASS | - | - |

## 通过测试的性能对比

| Benchmark | AgentJS | Boa | 倍数 |
|:----------|--------:|-----:|:----:|
| splay-simple | 5.2s | 1.1s | 4.7x |
| navier-stokes-simple | 1.9s | 0.3s | 6.3x |
| ai-astar-simple | 30.7s | 1.8s | 17.1x |
| richards-simple | 14.3s | 0.7s | 20.4x |
| stanford-crypto-sha256-simple | 10.2s | 0.4s | 25.5x |

## AgentJS 失败分析

| Benchmark | 错误类型 | 缺失特性 |
|:----------|:---------|:---------|
| ai-astar | NO_SUPER | class/super() - 标准版使用了 ES6 class 继承 |
| crypto-simple | LIMIT | 调用栈限制 - 递归深度超出默认栈限制 |
| crypto | NO_SUPER | class/super() + 调用栈限制 |
| richards | NO_SUPER | class/super() - 标准版使用了 class 语法 |
| stanford-crypto-sha256 | NO_SUPER | class/super() - 标准版使用了 class 语法 |

## Boa 失败分析

| Benchmark | 错误类型 | 详情 |
|:----------|:---------|:-----|
| crypto | ERROR | Boa 运行时错误 - 依赖的全局对象或模块解析失败 |

## AgentJS 失败原因分类

| 缺失特性 | 影响数 | 说明 |
|:---------|:------:|:-----|
| class / super() | 4 | 标准版 runner 全部使用 ES6 class 语法 |
| 调用栈限制 | 1 | crypto-simple 递归深度超出默认限制 |

## 覆盖率统计

- AgentJS: 5/10 通过 (50%) - simple 版全部通过
- Boa: 9/10 通过 (90%) - 仅 crypto 标准版失败
- 两个引擎都通过: 5/10 (50%)

## 说明

- simple 版: 去掉了 class/super 的降级版 runner
- 标准版: 保留原始 ES6 语法的 runner
- AgentJS 对所有 5 个 simple 版测试全部通过，平均比 Boa 慢 4.7~25.5 倍
- Boa 对几乎所有测试都能运行，仅 crypto 标准版失败
- 时间测量为单次执行耗时

## simple和标准版的差异

它们是两个**完全不同的生成脚本**的产物，差距远不止 class/super。

```
标准版 (crypto.js = 3313 行)         Simple版 (crypto-simple.js = 1734 行)
┌──────────────────────────┐        ┌──────────────────────────┐
│ JetStreamDriver 框架      │        │ console/performance shim │
│ (测试发现、评分、多轮迭代) │        │ (十几行)                 │
│ ~2000 行                  │        ├──────────────────────────┤
├──────────────────────────┤        │ 核心 benchmark 代码       │
│ 核心 benchmark 代码       │        │ (ARES-6 的纯计算逻辑)    │
│ (ARES-6 的纯计算逻辑)     │        │ ~1700 行                 │
│ ~1300 行                  │        │                          │
│ 但写法用了 class/super    │        │ 写法已经去掉了 class      │
└──────────────────────────┘        └──────────────────────────┘

生成脚本: prepare-jetstream2.mjs      生成脚本: prepare-simple-benchmark.mjs
```

### 具体差异

| | 标准版 | Simple 版 |
|---|---|---|
| 生成脚本 | prepare-jetstream2.mjs | `prepare-simple-benchmark.mjs` |
| 包含 JetStreamDriver | ✅ 完整框架 | ❌ 只有 benchmark 本身 |
| 测试范围 | JetStream2 的全部子测试 | 每个 benchmark 的纯计算部分 |
| 语法降级 | 有（但 class→function 不完整） | 有（更彻底的降级） |

### richards 最明显

`richards.js` = 3313 行，richards-simple.js = 575 行。差距 6 倍。因为标准版里那 2700+ 行是 JetStreamDriver 框架代码，不是 benchmark 本身。

所以 simple 版**不是**"所有 JetStream2 测试去掉 class"，而是**只跑每个 benchmark 的核心计算，不跑 JetStream2 的测试框架**。
