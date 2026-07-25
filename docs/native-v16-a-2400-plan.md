# Native V16 A 组 2400+ 修复计划

## 目标与口径

- 基线：Test262 全量 `53379 / 44626 passed / 8751 failed / 2 skipped`，83.60%。
- A 组责任边界：`lexer -> parser -> AST -> bytecode lowering -> VM completion -> environment`。
- A 组最低目标：相对当前基线净新增至少 **2400** 个 Test262 通过；计划目标为 **2600**，预留约 200 个回归缓冲。
- 不修改 Test262 预期、不扩大量 skip；所有收益必须来自 native 行为修复。
- 每阶段保存 before/after JSON、失败签名、净新增通过和回归数量。

## 收益拆分

| 阶段 | 语义簇 | 主要目录 | 目标净新增 |
|---|---|---|---:|
| A1 | class 声明/表达式、computed key、method/accessor、super、private/static 初始化 | `language/statements/class`、`language/expressions/class`、`language/expressions/super` | +750 |
| A2 | 统一 destructuring lowering：声明、参数、赋值、for-in/of、catch、rest/default、IteratorClose 接口 | `language/*/dstr`、`language/statements/for-of` | +500 |
| A3 | try/catch/finally completion、break/continue/return/throw 覆盖、for/switch 控制流 | `language/statements/try`、`for`、`switch`、`with` | +350 |
| A4 | eval、global/function declaration instantiation、TDZ、arguments、scope/early-error | `language/eval-code`、`language/function-code`、`language/global-code`、`identifier-resolution` | +450 |
| A5 | Annex B sloppy block functions、if/switch/label/function code 的 binding/update/descriptor | `annexB/language` | +350 |
| A6 | function/arrow/generator/async-generator 的语法、参数静态语义和 completion/lowering | `language/*/function`、`arrow-function`、`yield`、`generators`、`async-generator` | +250 |
| A7 | object literal、compound assignment、computed property、private name 及前述簇回归 | `language/expressions/object`、`compound-assignment`、`private` | +200 |
| **合计** |  |  | **+2850** |

## 执行顺序与依赖

1. 先固定 A 组目录基线和失败签名；不得在没有 JSON 基线时宣称收益。
2. A1/A2 先于 A4/A5：class 和 destructuring 共享 parser/AST/compiler 入口，是后续 eval/Annex B 的前置依赖。
3. A3 先修 VM completion 的结构化传播，再修 parser early-error；否则 try/finally 的错误会污染所有控制流目录。
4. A4/A5 共用 environment declaration-instantiation 路径，禁止分别写第二套绑定逻辑。
5. A6/A7 只复用既有 lowering，不增加按测试文件名特判。
6. 每阶段结束都运行该阶段套件、已完成阶段回归套件和 `cargo check --all-targets`；阶段净收益小于 0 时停止扩展并定位回归。

## 代码所有权与文件锁

- Parser/AST：`src/parser/`、`src/ast/`、`src/lexer/`。
- Lowering：`src/bytecode/compiler.rs`、`src/bytecode/opcode.rs`。
- Completion/control flow：`src/vm/interpreter.rs`。
- Bindings/TDZ/private：`src/runtime/environment.rs`、`src/runtime/private.rs`。
- 共享文件 `src/bytecode/compiler.rs`、`src/vm/interpreter.rs` 修改必须在本报告记录影响范围；不实现 B 组 abstract ops 的第二份副本。

## 阶段验收门

- A1：class 两目录新增通过 >= 750，private/super 不得回归。
- A2：destructuring 相关目录新增通过 >= 500，`try/dstr` 保持 93/93。
- A3：try/finally 控制流失败至少减少 250，RuntimeLimit 不增加。
- A4：eval/function/global 失败至少减少 450，静态错误必须在 parse/compile 阶段产生。
- A5：Annex B 失败至少减少 350，sloppy-only 行为不得泄漏到 strict mode。
- A6/A7：function/object/compound/private 失败至少减少 450，且已完成阶段无净回归。
- 最终 A 组门：聚焦目录净新增 >= 2400；完整 A 组失败清单可重现；skip 仍为 2。

## 命令模板

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite <suite> --jobs 4 --progress --json <before-or-after>.json
```

完整证据写入 `reports/v16-partA-report.md`，阶段 JSON 使用 `reports/native-v16-a-<phase>-<suite>-{baseline,after}.json` 命名。
