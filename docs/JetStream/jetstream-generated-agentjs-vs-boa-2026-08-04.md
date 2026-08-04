# JetStream generated runner：AgentJS vs Boa（2026-08-04）

> 测试日期：2026-08-04  
> 代码版本：`7bcd72a`  
> 测试对象：`benchmarks/generated/` 当前 25 个 JavaScript 文件中的 19 个 canonical runner  
> 排除项：6 个 `*-node.js`（与对应 canonical runner 内容相同，仅作 Node 参考副本）  
> AgentJS：由当前源码重新构建的 `target/release/agentjs.exe`  
> Boa：`boa/target/release/boa.exe`，使用可见输出 prelude 适配 `print()`  
> 测量方式：顺序、单进程、单次墙钟时间；每项超时 150 秒

## 结果总览

| Benchmark | AgentJS | 耗时 | Boa | 耗时 | AgentJS/Boa |
|:----------|:-------:|-----:|:---:|-----:|:-----------:|
| ai-astar | PASS | 37.527s | PASS | 1.288s | 29.1x |
| crypto | PASS | 4.551s | PASS | 0.412s | 11.0x |
| gaussian-blur | PASS | 22.484s | PASS | 1.262s | 17.8x |
| hash-map | PASS | 14.412s | PASS | 0.944s | 15.3x |
| jetstream2-cdjs | PASS | 21.456s | PASS | 3.413s | 6.3x |
| jetstream2-intl | RESULT_MISMATCH | 1.776s | UNSUPPORTED_INTL | 0.034s | - |
| jetstream2-jsdom-d3-startup | RESOURCE_MISSING | 0.154s | RESOURCE_MISSING | 0.029s | - |
| jetstream2-mobx | RESOURCE_MISSING | 0.162s | RESOURCE_MISSING | 0.031s | - |
| jetstream2-threejs | TIMEOUT | >150s | PASS | 146.489s | - |
| jetstream2-validatorjs | REGEXP/CALL_ERROR | 0.513s | ASSERTION_FAILURE | 0.353s | - |
| jetstream2-web-ssr | RESOURCE_MISSING | 0.169s | RESOURCE_MISSING | 0.028s | - |
| jetstream2-WSL | TIMEOUT | >150s | TYPE_ERROR | 0.082s | - |
| navier-stokes | PASS | 3.244s | PASS | 0.240s | 13.5x |
| raytrace | PASS | 7.413s | PASS | 0.676s | 11.0x |
| regexp | CHECKSUM_MISMATCH | 1.250s | PASS | 2.081s | - |
| richards | PASS | 41.201s | PASS | 2.601s | 15.8x |
| splay | PASS | 9.083s | PASS | 0.854s | 10.6x |
| stanford-crypto-sha256 | PASS | 9.389s | PASS | 0.279s | 33.7x |
| test-cdjs | PASS | 21.769s | PASS | 3.340s | 6.5x |

## 共同通过项目的性能对比

| Benchmark | AgentJS | Boa | 倍数 |
|:----------|--------:|----:|:----:|
| jetstream2-cdjs | 21.456s | 3.413s | 6.3x |
| test-cdjs | 21.769s | 3.340s | 6.5x |
| splay | 9.083s | 0.854s | 10.6x |
| raytrace | 7.413s | 0.676s | 11.0x |
| crypto | 4.551s | 0.412s | 11.0x |
| navier-stokes | 3.244s | 0.240s | 13.5x |
| hash-map | 14.412s | 0.944s | 15.3x |
| richards | 41.201s | 2.601s | 15.8x |
| gaussian-blur | 22.484s | 1.262s | 17.8x |
| ai-astar | 37.527s | 1.288s | 29.1x |
| stanford-crypto-sha256 | 9.389s | 0.279s | 33.7x |

以上均为单次进程墙钟时间，不是稳定的多轮微基准。大部分 runner 的迭代数不足以填满 JetStream worst-case 窗口，因此输出中的 `Worst Case`、`Average` 和 `Score` 多为 `NaN`；这不影响 PASS/FAIL 和墙钟时间判定。

## 覆盖率统计

- AgentJS：11/19 通过（57.9%），6 项失败，2 项超时。
- Boa：13/19 通过（68.4%），6 项失败，无超时。
- 两个引擎共同通过：11/19（57.9%）。
- Boa 单独通过：`jetstream2-threejs`、`regexp`。
- AgentJS 没有单独通过而 Boa 失败的项目。

## AgentJS 失败分析

| Benchmark | 分类 | 详情 |
|:----------|:-----|:-----|
| jetstream2-intl | RESULT_MISMATCH | `NumberFormat-intl` 的 `totalLength = 31112`，runner 要求至少 80000 |
| jetstream2-jsdom-d3-startup | RESOURCE_MISSING | `JetStream resource not embedded: ./jsdom-d3-startup/dist/bundle.min.js` |
| jetstream2-mobx | RESOURCE_MISSING | `JetStream resource not embedded: ./mobx/dist/bundle.es6.min.js` |
| jetstream2-threejs | TIMEOUT | 150 秒内无输出，进程被终止 |
| jetstream2-validatorjs | REGEXP/CALL_ERROR | 正则 `/[@_\\- ]/g` 编译失败，随后报 `undefined is not callable` |
| jetstream2-web-ssr | RESOURCE_MISSING | `JetStream resource not embedded: ./web-ssr/dist/bundle.min.js` |
| jetstream2-WSL | TIMEOUT | 150 秒内无输出，进程持续高 CPU 后被终止 |
| regexp | CHECKSUM_MISMATCH | workload 完成前报 `Wrong checksum` |

### 失败类别汇总

| 类别 | 数量 | 说明 |
|:-----|:----:|:-----|
| 资源未嵌入 | 3 | 生成 runner 时未包含启动 benchmark 运行期请求的 bundle |
| 超时 | 2 | `threejs`、`WSL` 超过统一 150 秒门限 |
| 结果或 checksum 不一致 | 2 | `intl`、`regexp` |
| RegExp/调用错误 | 1 | `validatorjs` |

## Boa 失败分析

| Benchmark | 分类 | 详情 |
|:----------|:-----|:-----|
| jetstream2-intl | UNSUPPORTED_INTL | `RelativeTimeFormat-intl` 报 `TypeError: not a constructor` |
| jetstream2-jsdom-d3-startup | RESOURCE_MISSING | `readFile` 路径失败 |
| jetstream2-mobx | RESOURCE_MISSING | `readFile` 路径失败 |
| jetstream2-validatorjs | ASSERTION_FAILURE | `Assertion failure: 2010-07-02,[object Object]` |
| jetstream2-web-ssr | RESOURCE_MISSING | `readFile` 路径失败 |
| jetstream2-WSL | TYPE_ERROR | `cannot convert 'null' or 'undefined' to object` |

## 与 2026-07-28 报告的关系

1. 旧修复报告记录的是当时的 31 个文件，其中 25 个用于 AgentJS、6 个用于 Node；当前目录只有 25 个文件，其中 canonical runner 19 个、`*-node.js` 副本 6 个。因此本报告不能直接用 11/19 与旧报告的 17/25 做回归百分比比较。
2. 旧对比报告仅抽取 10 个 runner；本报告覆盖当前全部 19 个 canonical runner。
3. `crypto` 已从旧报告中的调用栈阻塞变为 PASS；当前 release 的 JetStream 命令使用 256 MiB 专用线程栈和更高递归限制。
4. `ai-astar`、`richards`、`stanford-crypto-sha256` 等完整 Driver runner 均通过，继续证明当前主要阻塞不是 class/super。
5. 旧 Boa prelude 使用空 `print()`，会隐藏 harness 的失败文本。本报告改用预先绑定的 Boa 原生 `console.log`，并同时检查 `JetStream2 failed:`；因此 Boa 的资源、Intl 和断言失败不会被误计为 PASS。

## 测试命令与判定口径

AgentJS 构建：

```powershell
cargo build --release
```

AgentJS 单项：

```powershell
target/release/agentjs.exe jetstream benchmarks/generated/<runner>.js
```

Boa 单项：

```powershell
boa/target/release/boa.exe `
  reports/jetstream2-2026-08-04/boa-visible-prelude.js `
  benchmarks/generated/<runner>.js
```

判定规则：

- 150 秒内退出码为 0，且输出不包含 `JetStream2 failed:` 或 `Uncaught Error`：PASS。
- 输出包含 runner failure marker：FAIL，即使 Boa CLI 返回 0。
- 超过 150 秒：TIMEOUT，终止该进程后继续下一项。
- 测试顺序执行，不并行运行 workload。

## 原始数据

- AgentJS stdout/stderr：`reports/jetstream2-2026-08-04/`
- Boa 最终有效 stdout/stderr：`reports/jetstream2-2026-08-04/boa-final/`
- Boa 可见输出 prelude：`reports/jetstream2-2026-08-04/boa-visible-prelude.js`
- `reports/jetstream2-2026-08-04/boa/` 与 `boa-visible/` 是宿主输出适配诊断预跑，不作为本报告数据来源。

## 结论

- 当前 AgentJS 能通过 11 个完整 generated runner；经典计算 workload 的 class/super 与原先的 crypto 深递归阻塞均不再是首要问题。
- 最明确的生成物问题是 3 个启动 benchmark 缺少嵌入资源；应优先修复 `prepare-jetstream2.mjs` 的资源发现/打包。
- 引擎侧重点是 `regexp` checksum、`validatorjs` RegExp/调用链和 Intl 结果差异。
- `threejs` 与 `WSL` 需要更长诊断窗口或缩小迭代数后定位；本轮只能确定它们超过 150 秒，不能据此断言死循环。
- 在 11 个共同通过项目中，AgentJS 单次墙钟约为 Boa 的 6.3x–33.7x；性能瓶颈仍明显，但该单次测试不替代稳定的多轮统计。
