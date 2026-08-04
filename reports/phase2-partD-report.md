# Phase 2 D 组：Local Slot 报告

日期：2026-08-04  
基线：`e4382f93fc400aefd07247e301dc40132634e5fd`

## 实现范围

- 新增 `LocalSlot`、`LocalBindingLayout`、`LocalLayout` 和 `DynamicScopePolicy`。
- `FunctionTemplate` 与 `JsFunction` 共享同一个 `Arc<LocalLayout>`。
- activation environment 使用连续 `Binding` slots，同时保留 name-to-slot 索引供闭包、eval、调试和旧名字指令访问；slot 与动态 binding 不保存两份值。
- 新增 `LoadLocal`、`StoreLocal`、`InitializeLocal`，补齐 stack effect、越界验证、TDZ、不可变写入及赋值结果保留语义。
- 参数、rest 参数、函数级 `var` 和函数声明绑定进入当前函数槽位；block lexical、catch、module、全局及外层闭包继续使用原名字路径。
- 含 direct eval 或 `with` 的函数整体回退到名字指令并清空 layout。
- GC trace 和环境内存估算包含 local slots。
- `--diagnostics` 输出 local/name load/store 计数与 environment hops。

## 正确性验证

新增 `tests/local_slots.rs`，覆盖参数/var lowering、闭包父槽慢路径、direct eval/with 整体回退、默认参数、rest、函数声明 hoisting、闭包读写、70% 快路径门槛和非法槽位验证。

通过：

- `cargo fmt --all -- --check`
- `cargo check --locked --all-targets`
- `cargo test --locked --all-targets`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --release --no-default-features --test native_test262`（15/15）
- 闭包、scope、iteration、runtime、native GC 定向套件

完整 Test262 与 19 项 JetStream/GC threshold 集成矩阵由集成人在 F → D → E 合并后统一执行；D 组未改写锁定的 Test262 baseline 或失败 manifest。

## JetStream 5 轮对照

相同机器、release、当前 staged runner、1 iteration、120 秒 wall-clock 上限。基线二进制从 `e4382f93` 的独立源码导出构建；所有运行均功能通过。

| workload | baseline median / p90 (ms) | D median / p90 (ms) | median 变化 |
|---|---:|---:|---:|
| richards | 7966 / 8013 | 7890 / 7940 | -0.95% |
| splay | 2329 / 2355 | 2156 / 2167 | -7.43% |
| crypto | 4236 / 4261 | 2470 / 2493 | -41.69% |
| raytrace | 14072 / 14155 | 14177 / 14227 | +0.75% |
| navier-stokes | 2747 / 2767 | 1509 / 1521 | -45.07% |

五项 median 几何平均约改善 21.6%；没有共同 PASS workload 回退超过 5%。简单函数循环的自动化诊断测试要求 Local load/store 占 Local+Name 访问至少 70%，当前通过。Richards 含大量全局和闭包访问，因此其整体 workload 比例不代表普通 activation 的命中率。

## 协调说明

- 未修改 `PropertyMap`、`JsValue::String`、GC collector、模块系统或 `contracts.rs`。
- E/F 合并若改变 `Binding` 内的字符串/值类型，只需适配 `Environment.slots` 的元素类型；槽位索引和 opcode 语义不应改变。
- 共享接口未覆盖的事项见 `docs/phase2-d-interface-deviations.md`。

## 遗漏项补全（2026-08-04）

- 直接位于函数体的 `FunctionDeclaration` 现在进入 `LocalLayout`；嵌套 block 声明仍保持原作用域路径。
- 默认参数、对象模式参数和 rest pattern 的 preamble 在存在槽位时改用 `LoadLocal`/`StoreLocal`，不再重复按名字遍历环境链。
- 修复两项随补全暴露的语义回归：显式 `var` 与 Annex B 函数同名时不重复初始化槽位；非简单参数列表中的默认表达式仍能看到正确的隐式 `arguments` 对象。
- `tests/local_slots.rs` 扩展到 10/10，通过直接函数声明、三类参数 preamble、Annex B 和 `arguments` 组合测试。
- JetStream staged 诊断器现在累加一个进程内全部 `name_resolution:` 样本，输出 Local/Name load/store、environment hops 和 Local 快路径比例。

定向 Test262 与补全前锁定结果完全一致，没有新增失败：

| 目录 | 结果 |
|---|---:|
| `language/function-code` | 354/376 |
| `language/expressions/function` | 249/264 |
| `language/statements/function` | 439/452 |
| `built-ins/eval` | 8/10 |
| `language/statements/with` | 127/181 |

方案中写作 `language/expressions/direct-eval` 的目录在当前 Test262 树中不存在，实际 direct-eval 定向目录为 `built-ins/eval`。最终项目门禁全部通过：fmt、locked all-target check/test、clippy `-D warnings`、release build，以及 native Test262 集成测试 15/15。
