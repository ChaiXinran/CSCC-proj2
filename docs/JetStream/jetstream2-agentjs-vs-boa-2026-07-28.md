# JetStream2 AgentJS vs Boa 性能与兼容性对比

> 测试日期：2026-07-28  
> 测试对象：`benchmarks/generated/` 中与 2026-07-25 报告相同的 10 个 runner  
> AgentJS：`target/release/agentjs.exe`  
> Boa：`boa/target/release/boa.exe`  
> 测量方式：单次进程墙钟时间；simple runner 使用 1 次 workload 迭代

## 结果总览

| Benchmark | AgentJS | 耗时 | Boa | 耗时 | AgentJS/Boa |
|:----------|:-------:|-----:|:---:|-----:|:-----------:|
| ai-astar-simple | PASS | 123.274s | PASS | 1.290s | 95.6x |
| ai-astar | INVALID_RUNNER | 0.044s | INVALID_RUNNER | 0.068s | - |
| crypto-simple | STACK_LIMIT | 0.316s | PASS | 0.488s | - |
| crypto | INVALID_RUNNER | 0.032s | INVALID_RUNNER | 0.022s | - |
| navier-stokes-simple | PASS | 1.355s | PASS | 0.241s | 5.6x |
| richards-simple | PASS | 10.258s | PASS | 0.541s | 19.0x |
| richards | INVALID_RUNNER | 0.029s | INVALID_RUNNER | 0.020s | - |
| splay-simple | PASS | 7.563s | PASS | 0.848s | 8.9x |
| stanford-crypto-sha256-simple | PASS | 7.205s | PASS | 0.321s | 22.4x |
| stanford-crypto-sha256 | INVALID_RUNNER | 0.043s | INVALID_RUNNER | 0.022s | - |

## 通过测试的性能对比

| Benchmark | AgentJS | Boa | 倍数 |
|:----------|--------:|-----:|:----:|
| navier-stokes-simple | 1.355s | 0.241s | 5.6x |
| splay-simple | 7.563s | 0.848s | 8.9x |
| richards-simple | 10.258s | 0.541s | 19.0x |
| stanford-crypto-sha256-simple | 7.205s | 0.321s | 22.4x |
| ai-astar-simple | 123.274s | 1.290s | 95.6x |

`ai-astar-simple` 在 AgentJS 内部记录的 workload 时间为 56.897s，但进程
墙钟时间为 123.274s。本表沿用旧报告的单次进程墙钟口径，因此使用后者。

## AgentJS 失败分析

| Benchmark | 错误类型 | 详情 |
|:----------|:---------|:-----|
| crypto-simple | STACK_LIMIT | `RuntimeLimit: call stack limit exceeded` |
| ai-astar | INVALID_RUNNER | base class 构造器中出现 `super(args)`，parse-phase SyntaxError |
| crypto | INVALID_RUNNER | 同上 |
| richards | INVALID_RUNNER | 同上 |
| stanford-crypto-sha256 | INVALID_RUNNER | 同上 |

## Boa 失败分析

| Benchmark | 错误类型 | 详情 |
|:----------|:---------|:-----|
| ai-astar | INVALID_RUNNER | `SyntaxError: invalid super usage` |
| crypto | INVALID_RUNNER | 同上 |
| richards | INVALID_RUNNER | 同上 |
| stanford-crypto-sha256 | INVALID_RUNNER | 同上 |

Boa simple runner 通过一个仅提供空 `print()` 的宿主前置脚本执行。该前置脚本
只承接 runner 最后的结果输出，不参与 workload 计算。

## 标准版 runner 有效性

当前仓库中的 4 个标准版文件不是有效的 JavaScript class 降级结果。它们包含：

```js
class DefaultBenchmark {
    constructor({worstCaseCount, ...args}) {
        super(args);
    }
}
```

`DefaultBenchmark` 没有 `extends`，却直接调用 `super()`。AgentJS 和 Boa 均在
解析阶段拒绝，因此这些结果不能用于评价任一引擎的原生 class/super 支持。

这也说明旧报告中的 `NO_SUPER` 标签已经不够准确：当前失败首先是生成物自身
无效，而不是“引擎完全不支持 class”。

## 使用当前生成器的全 Driver 交叉验证

为区分陈旧生成物与引擎能力，本轮还使用当前
`scripts/prepare-jetstream2.mjs` 重新生成 2-iteration runner，并通过
`agentjs jetstream` 执行：

| Benchmark | 结果 | 说明 |
|:----------|:----:|:-----|
| ai-astar | PASS | 完成两次迭代，无 class/super 错误 |
| richards | PASS | 完成两次迭代，无 class/super 错误 |
| stanford-crypto-sha256 | PASS | 完成两次迭代，无 class/super 错误 |
| splay | PASS | 完成两次迭代，无 class/super 错误 |
| navier-stokes | PASS | 完成两次迭代，无 class/super 错误 |
| crypto | STACK_LIMIT | `call stack limit exceeded` |
| regexp | REGEXP_UNSUPPORTED | look-around 尚未实现 |

两次迭代不足以填满 JetStream 的 worst-case 样本窗口，因此这些快速 runner
会显示 `Worst Case: NaN` 和 `Score: NaN`。这不表示 workload 未执行；
`Startup`、`Average`、退出码和错误信息仍可用于兼容性门禁。

## 覆盖率统计

按现有 10 文件矩阵：

- AgentJS：5/10 通过（50%）。
- Boa：6/10 通过（60%）。
- 两个引擎都通过：5/10（50%）。
- 4/10 标准版文件为共同不可解析的无效生成物。
- 排除无效生成物后，AgentJS 为 5/6（83.3%），Boa 为 6/6（100%）。

按当前生成器的 7 个全 Driver 快速 runner：

- AgentJS：5/7 通过（71.4%）。
- Class/super 相关失败：0。
- 剩余阻塞：调用栈 1 项、RegExp look-around 1 项。

## 与 2026-07-25 报告对比

1. 原报告所称的“标准版 class/super 全部失败”不再适用于当前引擎：
   当前重新生成的全 Driver runner 已能运行 `ai-astar`、`richards`、
   `stanford-crypto-sha256`、`splay` 和 `navier-stokes`。
2. `crypto-simple` 的深调用栈失败仍然存在，属于 Invocation/CallFrame
   路径，不属于 Class/Super。
3. `super.updateUIAfterRun()` 已保留为原生 super 方法调用并通过上述
   全 Driver workload。
4. `benchmarks/generated` 中 4 个旧标准版文件需要重新生成；在此之前不应
   将它们的 SyntaxError 计作 AgentJS class conformance 回退。

## 测试命令

AgentJS：

```powershell
target/release/agentjs.exe jetstream benchmarks/generated/<runner>.js
```

Boa simple runner：

```powershell
boa/target/release/boa.exe target/boa-jetstream-prelude.js benchmarks/generated/<runner>.js
```

当前全 Driver 快速 runner：

```powershell
node scripts/prepare-jetstream2.mjs benchmarks/JetStream2 <test> 2 target/<test>.js
target/release/agentjs.exe jetstream target/<test>.js
```

## 结论

- AgentJS 当前不应再描述为“完全不支持 class/super”。
- 当前生成器产出的全 Driver runner 未发现 Class/Super 相关 benchmark
  报错。
- 现有标准版生成文件已经陈旧且语法无效，应重新生成后再作为交付产物。
- 当前 JetStream 兼容性优先阻塞是 `crypto` 调用栈和 `regexp` look-around。
- 性能上 AgentJS 仍明显慢于 Boa，尤其是 `ai-astar`；本轮目标是兼容性验证，
  单次墙钟数据不应替代稳定的多轮性能测量。
