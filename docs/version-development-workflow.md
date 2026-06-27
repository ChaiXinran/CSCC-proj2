# Native 版本开发工作流

本文档总结 Native V1–V3 已验证的协作方式，并作为后续版本的统一开发、合并和验收流程。新版本原则上只需复制本文的检查清单，再补充该版本的功能范围与 Test262 清单。

## 1. 先冻结范围和接口

开发前必须建立：

- `docs/native-vN-scope.md`：功能、非目标、分组任务、端到端样例和验收标准。
- `docs/native-vN-interface.md`：AST、Opcode、栈布局、Runtime API、错误类型等共享契约。

将功能拆成可独立验收的小批次，如 `V4.1`、`V4.2`。每项必须标记为“未开始、接口完成、实现完成、端到端通过、Test262 通过”之一。仅有数据结构或单元测试时不能宣称版本完成。

## 2. 四组并行开发

| 分组 | 负责目录 | 独立测试方式 |
| --- | --- | --- |
| A 前端 | `lexer/`、`ast/`、`parser/` | source/token → AST |
| B 编译器 | `bytecode/` | 手工 AST → Chunk |
| C 执行内核 | `vm/`、`runtime/`、`builtins/` | 手工 Chunk、直接 Runtime API |
| D 集成 | `backend/`、CLI、Test262、CI、报告 | source → Native 执行结果 |

未完成的上下游使用手工 AST、手工 Chunk 或 Fake Stage 隔离。Boa 只能作为参考和差分基线，不能替代 Native 实现或计入 Native 通过率。

## 3. 推荐实现顺序

1. 合并共享 AST、Opcode、Runtime 类型和 Trait。
2. A/B/C 各自在所属目录完成实现与单元测试。
3. 验证相邻连接：A→B、B→C。
4. 接通完整链路：source→AST→Chunk→VM→Runtime。
5. 添加 Native 端到端测试。
6. 筛选并冻结 Test262 文件。
7. 更新 CLI、README、报告和 CI。

每一步都应产生可运行测试，避免最后一次性联调。

## 4. 合并与冲突处理

推荐合并顺序：

```text
AST/Token → Opcode/Chunk → Runtime/Value → 各组实现 → Test262/CLI
```

共享文件冲突以接口文档为准，不直接选择整份 ours/theirs。合并后检查 Git 冲突标记、重复枚举、重复 `match` 分支、重复参数和被拼接的半截函数：

```powershell
rg "^(<<<<<<<|=======|>>>>>>>)" src tests
cargo check --all-targets
```

## 5. 分层测试门禁

每个版本必须依次通过：

```text
模块单元测试
→ 手工 AST/Chunk/Runtime 契约测试
→ 相邻模块集成测试
→ Native 源码端到端测试
→ 固定 Test262 清单
→ 旧版本回归
```

合并前统一运行：

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test native_test262
```

## 6. Test262 工作流

1. 扫描与本版本功能相关的官方目录。
2. 记录目录级 `passed/failed/skipped` 基线。
3. 逐文件复验候选，检查是否依赖未实现语法、Builtins 或 harness。
4. 排除因“其他不支持语法”而碰巧产生预期错误的假通过。
5. 将零失败、零跳过的真实覆盖项加入 `NATIVE_VN_TESTS`。
6. 保留 V1–V(N-1) 固定门并更新报告。

固定清单是新增回归门，不代表整个目录的通过率。

## 7. 版本完成判定

版本只有同时满足以下条件才能标记完成：

- Scope 中所有必需功能均有实际实现，不是空函数、占位注释或 `Unsupported`。
- Parser、Compiler、VM、Runtime、Builtins 已通过完整调用链执行。
- 文档中的端到端样例全部通过。
- 对应 Test262 固定门零失败、零跳过。
- 旧版本固定门零回归。
- 全量测试、格式和 Clippy 通过。
- 报告如实记录目录基线、未支持项和下一步。

特别检查 `builtins/`：底层对象模型完成不等于 `Object`、`Array`、`Function` 标准对象已经完成。

## 8. 新版本复制清单

```text
[ ] 创建 scope 和 interface 文档
[ ] 冻结共享类型与栈/调用契约
[ ] 分配 A/B/C/D 文件所有权
[ ] 各组完成独立测试
[ ] 完成 A→B、B→C 连接测试
[ ] 完成 Native 端到端测试
[ ] 检查 Builtins 与 CLI 真实可用
[ ] 建立 NATIVE_VN_TESTS
[ ] 运行旧版本回归
[ ] 更新 README、报告和 CI
[ ] 全量 fmt/check/test/clippy 通过
```

## 9. Per-worker reports for AI-assisted collaboration

From V8 onward, every Native version must create one report per active worker
track before feature implementation starts:

```text
reports/vN-partA-report.md
reports/vN-partB-report.md
reports/vN-partC-report.md
```

If a future version changes the number or name of tracks, use the same pattern
with the actual track labels and document the mapping in
`docs/native-vN-team-plan.md`.

Each report must contain:

- owner and scope;
- locked baseline source;
- baseline totals and relevant failure classes;
- change log, newest first;
- implemented functionality;
- tests run;
- result deltas against the locked baseline;
- newly exposed failures and regressions;
- cross-group coordination notes.

Mandatory AI-agent rule: when an AI agent or human contributor changes a track,
the corresponding `reports/vN-part*-report.md` file must be updated in the same
change. The user does not need to explicitly ask for this. If no test was run,
the report must say so and explain why.

Do not overwrite locked baseline analysis files. For later analysis, create a
new dated or versioned report such as:

```text
reports/test262-analysis-YYYY-MM-DD.md
reports/native-vN-test262-analysis.md
```

## 10. Lightweight failed-case scan for each version

From V8 onward, every Native version must provide a lightweight scan command
before implementation starts:

```sh
cargo run --release --no-default-features -- test262 --native-vN-scan --jobs 4 --json reports/native-vN-scan-summary.json
```

The scan must run a locked manifest of 5,000 Test262 cases that did not pass in
the previous full or version baseline:

```text
reports/native-vN-scan-failures.txt
```

The manifest is a planning and regression artifact, not a formal conformance
number. It should be sampled from the locked baseline analysis/output and should
cover the version's parallel tracks. For example, V8 samples frontend,
module-runner, and builtin-skeleton hotspots.

Required implementation steps for each new version:

1. Generate `reports/native-vN-scan-failures.txt` with exactly 5,000 normalized
   `test/.../*.js` paths from previously non-passing cases.
2. Add allow-list entries to `.gitignore` for:
   - `reports/vN-part*-report.md`;
   - `reports/native-vN-scan-failures.txt`;
   - `reports/native-vN-scan-summary.json`.
3. Add constants in `src/test262.rs`:
   - `NATIVE_VN_SCAN_TESTS`;
   - `NATIVE_VN_SCAN_TEST_COUNT`;
   - `RunnerOptions::select_native_vN_scan()`.
4. Add `--native-vN-scan` handling in `src/main.rs`.
5. Add a selector test in `tests/native_test262.rs`.
6. Run the command once and record the initial
   `reports/native-vN-scan-summary.json`.
7. Document the command in:
   - `AGENTS.md`;
   - `readme.md`;
   - `docs/native-vN-scope.md`;
   - `docs/native-vN-team-plan.md`;
   - the active roadmap under `thoughts/`.
8. Record the baseline command and result in every `reports/vN-part*-report.md`.

Mandatory AI-agent rule: while working on version N, after focused tests the
agent should run `--native-vN-scan` when the change is likely to affect that
version's scan, unless the user explicitly asks for a narrower check. The
result delta must be recorded in the relevant worker report.

## 11. Updated new-version checklist

Use this checklist in addition to the legacy checklist above:

```text
[ ] Create docs/native-vN-scope.md
[ ] Create docs/native-vN-interface.md
[ ] Create docs/native-vN-team-plan.md
[ ] Create reports/vN-partA-report.md
[ ] Create reports/vN-partB-report.md
[ ] Create reports/vN-partC-report.md
[ ] Lock the baseline analysis; do not overwrite it later
[ ] Generate reports/native-vN-scan-failures.txt with 5,000 prior non-passing cases
[ ] Add --native-vN-scan in src/test262.rs and src/main.rs
[ ] Add selector coverage in tests/native_test262.rs
[ ] Run --native-vN-scan once and save reports/native-vN-scan-summary.json
[ ] Document the scan command in AGENTS.md, readme.md, scope, team plan, and roadmap
[ ] Require AI agents to update the corresponding part report on every track change
```
## 12. Fixup 阶段工作流

Native V1–V11 的主线开发流程适用于“新增一批明确语言/运行时能力”的版本阶段。但当项目进入 Fixup 阶段后，主要目标不再是从零实现某个新版本能力，而是基于最新全量 Test262 结果，围绕高失败簇进行准确率冲刺。因此 Fixup 阶段采用独立工作流。

Fixup 阶段的典型目标是：

```text
1. 锁定最新全量 Test262 结果作为 baseline。
2. 从失败用例中识别最高收益簇。
3. 按功能闭环分组，而不是按 parser/compiler/runtime/builtins 分层分组。
4. 每组可以跨层修改，但必须限制在自己的功能簇内。
5. 共享接口必须先冻结，再实现。
6. 每次修改必须更新对应 track report。
7. 用 fixup scan 和全量 Test262 同时衡量收益与回归。
```

### 12.1 Fixup 阶段适用条件

当满足以下条件之一时，应进入 Fixup 工作流，而不是继续使用普通 Native 版本工作流：

```text
[ ] 已经达到阶段性通过率目标，例如 60%。
[ ] 当前主要目标是继续提升 Test262 全量准确率，例如冲 70%。
[ ] 失败主要集中在多个跨层功能簇中，例如 Temporal、RegExp、Array、class、module、Promise。
[ ] 原 A/B/C/D 层次分工会导致大量跨组改动和冲突。
[ ] 修改目标更多是“收割失败簇”，而不是新增一个完整版本特性。
```

普通 Native 版本工作流仍然用于：

```text
1. 新语法大版本；
2. 新 VM 执行模型；
3. 新 runtime 基础设施；
4. 新 Test262 固定门版本；
5. 需要明确 A 前端 / B 编译器 / C 执行内核 / D 集成拆分的阶段。
```

Fixup 阶段用于：

```text
1. conformance 冲刺；
2. builtins 收割；
3. descriptor / object shape 修正；
4. Temporal skeleton；
5. RegExp 收尾；
6. class/destructuring/Annex B 语义收尾；
7. module / dynamic import / Promise / Iterator 残余修复。
```

### 12.2 Fixup 阶段必须先锁定 baseline

Fixup 开始前必须保存最新全量 Test262 结果，作为不可覆盖的 baseline。

必须记录：

```text
total
passed
failed
skipped
conformance
elapsed
full command
test262 revision
AgentJS/native revision
date
platform
```

建议文件：

```text
reports/fixupN-baseline-output.txt
reports/fixupN-test262-analysis.md
reports/fixupN-baseline-summary.json
```

禁止覆盖旧 baseline。后续分析必须使用新的 dated/versioned 文件，例如：

```text
reports/fixup8-test262-analysis.md
reports/fixup8-after-p1-analysis.md
reports/test262-analysis-YYYY-MM-DD.md
```

### 12.3 Fixup 阶段分组方式

Fixup 阶段不再强制使用：

```text
A = lexer / parser / ast
B = bytecode / compiler
C = vm / runtime / builtins
D = integration
```

而是按功能闭环分组。

推荐规则：

```text
1. 每组负责一个高收益测试簇。
2. 每组可以跨 parser/compiler/runtime/builtins。
3. 每组只能跨层修改自己功能簇需要的文件。
4. 共享 helper 必须有唯一 owner。
5. 共享接口变化必须先写入 fixup interface 文档。
6. 不允许各组分别写一套 descriptor、iterator、Promise queue、call path、binding path。
```

三人组推荐模板：

```text
P1 = Builtin Core + Temporal + Descriptor
P2 = RegExp + String Dispatch
P3 = Language + Module + Async Protocol
```

对应职责：

| Track | 主攻方向                                                                 | 典型测试簇                                                                                                          |
| ----- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| P1    | Temporal skeleton、Array/Object/Function descriptor、builtin installer | `test/built-ins/Temporal`、`test/built-ins/Array`、`test/built-ins/Object`、`test/built-ins/Function`             |
| P2    | RegExp property escapes、RegExp prototype、String-RegExp dispatch      | `test/built-ins/RegExp`、`test/built-ins/String/prototype/match`、`replace`、`search`、`split`                     |
| P3    | class/dstr、Annex B binding、dynamic import、module、Promise、Iterator    | `test/language/*/class`、`test/annexB/language`、`test/language/module-code`、`test/built-ins/Promise`、`Iterator` |

如果未来人数变化，可以继续使用同一原则：

```text
2 人：合并为 Builtins track + Language/Async track
3 人：P1/P2/P3
4 人：拆出 Temporal 独立组
5 人：拆出 Module/Promise/Iterator 独立组
```

### 12.4 Fixup 阶段必须创建的文档

每个 Fixup 阶段开始前必须创建：

```text
docs/fixupN-teamplan.md
docs/fixup-interface.md
reports/fixupN-p1-report.md
reports/fixupN-p2-report.md
reports/fixupN-p3-report.md
reports/fixupN-scan-failures.txt
reports/fixupN-scan-summary.json
```

其中：

```text
docs/fixupN-teamplan.md
```

必须说明：

```text
1. baseline；
2. 距离目标准确率还差多少 pass；
3. 每个人的功能闭环；
4. 每个 track 的主攻测试簇；
5. 每个 track 可以修改的文件；
6. 每个 track 禁止修改的文件；
7. 预估收益；
8. focused test 命令；
9. merge order；
10. acceptance gate。
```

```text
docs/fixup-interface.md
```

必须说明：

```text
1. 共享文件所有权；
2. descriptor / builtin installer 接口；
3. iterator helper 接口；
4. Promise job queue 接口；
5. call / construct 接口；
6. environment / Annex B binding 接口；
7. RegExp dispatch 接口；
8. Temporal skeleton 接口；
9. module / dynamic import 接口；
10. error convention；
11. merge blocker checklist。
```

### 12.5 Fixup 阶段共享接口规则

Fixup 阶段最容易出现的问题不是单个功能写错，而是多组重复实现同一类共享机制。

以下机制必须只有一套实现：

```text
descriptor / property path
builtin installer
iterator helper
Promise job queue
call / construct helper
environment binding helper
RegExp dispatch helper
module evaluation state
Test262 scan selector
```

禁止行为：

```text
[ ] 某个 builtin 自己手写一套 name/length/prototype descriptor。
[ ] Array.from 自己写一套 iterator next/close。
[ ] TypedArray.from 自己写一套 iterator next/close。
[ ] Promise combinator 自己写一套 job queue。
[ ] String.prototype.replace 自己直接调用函数指针，绕过统一 call path。
[ ] Annex B eval/function/global-code 分别写三套 binding 逻辑。
[ ] RegExp 和 String dispatch 各自维护不一致的 match/split 逻辑。
[ ] Temporal 为了过测试返回不可解释的伪值，导致语义假通过。
```

共享接口变更流程：

```text
1. 先在 docs/fixup-interface.md 增加接口说明。
2. 标明 owner、使用方、影响文件。
3. 提交小型 contract PR。
4. 其他 track 基于该接口调用。
5. 不允许在大型 feature PR 中顺手改共享接口。
```

### 12.6 Fixup scan 工作流

Fixup 阶段必须创建固定 5,000 条失败用例 scan。

文件：

```text
reports/fixupN-scan-failures.txt
reports/fixupN-scan-summary.json
```

命令建议：

```powershell
cargo run --release --no-default-features -- test262 --fixupN-scan --jobs 4 --json reports/fixupN-scan-summary.json
```

也可以使用项目旧风格命名：

```powershell
cargo run --release --no-default-features -- test262 --native-fixupN-scan --jobs 4 --json reports/fixupN-scan-summary.json
```

但一个阶段内必须保持命名一致。

`fixupN-scan-failures.txt` 必须来自上一轮 full Test262 baseline 中未通过的用例，且覆盖所有活跃 track。

三人组推荐采样结构：

| Bucket                                       | Cases |
| -------------------------------------------- | ----: |
| Temporal / Intl Temporal skeleton            |  1000 |
| Array / Object / Function descriptor         |   700 |
| RegExp / String-RegExp dispatch              |   700 |
| class / destructuring                        |   700 |
| Annex B binding / eval / function-code       |   600 |
| dynamic import / module / top-level await    |   600 |
| Promise / Iterator / for-of / for-await-of   |   500 |
| TypedArray / DataView / Date / Set residuals |   200 |
| Total                                        |  5000 |

Fixup scan 是计划和回归工具，不是正式准确率。正式准确率仍然以完整 53k Test262 全量扫描为准。

### 12.7 Fixup 阶段报告规则

每个 track 必须维护一个 report。

三人组模板：

```text
reports/fixupN-p1-report.md
reports/fixupN-p2-report.md
reports/fixupN-p3-report.md
```

每个 report 必须包含：

```text
owner and scope
locked baseline source
baseline totals
relevant failure classes
change log, newest first
implemented functionality
tests run
focused test deltas
fixup scan deltas
full scan deltas if available
newly exposed failures
regressions
cross-track dependencies
next action
```

强制规则：

```text
1. 每次修改 track 代码，必须同时更新对应 report。
2. 如果没有跑测试，必须写 Tests not run 和 Risk。
3. 如果改动共享接口，必须更新 docs/fixup-interface.md。
4. 如果改动 scan selector，必须更新 reports/fixupN-scan-summary.json。
5. 不允许只提交代码不提交报告。
```

### 12.8 Fixup 阶段测试门禁

每个 PR 至少运行：

```powershell
cargo fmt --all -- --check
cargo check --release --no-default-features --all-targets
cargo test --release --no-default-features --all-targets
```

如果时间允许：

```powershell
cargo clippy --release --no-default-features --all-targets -- -D warnings
```

每个 track 必须运行自己的 focused tests。

P1 示例：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Temporal --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Array --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Object --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Function --jobs 4 --progress
```

P2 示例：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/RegExp/property-escapes --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String/prototype/match --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/String/prototype/split --jobs 4 --progress
```

P3 示例：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/statements/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/class --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/annexB/language/eval-code --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/expressions/dynamic-import --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Promise --jobs 4 --progress
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/built-ins/Iterator --jobs 4 --progress
```

合并到 integration branch 后必须运行：

```powershell
cargo run --release --no-default-features -- test262 --fixupN-scan --jobs 4 --json reports/fixupN-scan-summary.json
```

重大合并后或每日收尾运行完整扫描：

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress
```

### 12.9 Fixup 阶段合并顺序

推荐合并顺序：

```text
1. docs/fixupN-teamplan.md
2. docs/fixup-interface.md
3. reports/fixupN-p*-report.md 初始版本
4. reports/fixupN-scan-failures.txt
5. fixup scan selector
6. shared interface contract PR
7. 各 track focused feature PR
8. fixup scan integration
9. full Test262 integration
10. final analysis report
```

三人组示例：

```text
docs/fixupN-contracts
  -> P1 builtin installer / descriptor contract
  -> P3 iterator / Promise / call / environment contract
  -> P2 RegExp/String focused fixes
  -> P1 Temporal + Array/Object/Function sweep
  -> P3 class/dstr + Annex B + module async sweep
  -> integration branch
  -> fixup scan
  -> full Test262
```

如果出现共享文件冲突，以 `docs/fixup-interface.md` 为准，不能直接选择整份 ours/theirs。

合并后必须检查：

```powershell
rg "^(<<<<<<<|=======|>>>>>>>)" src tests docs reports
cargo check --release --no-default-features --all-targets
```

### 12.10 Fixup 阶段完成判定

Fixup 阶段完成必须满足：

```text
[ ] 达到本阶段目标 passed 数。
[ ] skipped 不计入 passed。
[ ] 无 runner crash。
[ ] 无 panic。
[ ] 无 memory allocation failure。
[ ] 无严重 RuntimeLimit 扩散。
[ ] fixup scan 相比 baseline 有净收益。
[ ] full Test262 相比 baseline 有净收益。
[ ] 所有 track reports 已更新。
[ ] docs/fixup-interface.md 已记录所有共享接口变化。
[ ] README / AGENTS / thoughts roadmap 已记录新的 scan 命令。
```

如果阶段目标是 70%，则完成线为：

```text
passed >= ceil(total * 0.70)
```

建议设置安全线：

```text
passed >= ceil(total * 0.70) + 200
```

### 12.11 Fixup 阶段非目标

Fixup 阶段不应投入大量时间做：

```text
complete Temporal semantics
complete Intl402
complete Atomics wait/wake host behavior
full ShadowRealm
large VM rewrite
replacing RegExp engine wholesale
using Boa as fallback
counting skipped/crashed/timeout as pass
```

Fixup 阶段允许：

```text
Temporal skeleton
prototype shape
descriptor exactness
simple from/toString
catchable TypeError / RangeError
low-risk parser unlocker
low-risk builtin method sweep
```

### 12.12 Fixup 阶段复制清单

```text
[ ] 锁定最新 full Test262 baseline
[ ] 创建 reports/fixupN-test262-analysis.md
[ ] 创建 docs/fixupN-teamplan.md
[ ] 创建 docs/fixup-interface.md
[ ] 创建 reports/fixupN-p1-report.md
[ ] 创建 reports/fixupN-p2-report.md
[ ] 创建 reports/fixupN-p3-report.md
[ ] 从 baseline failed cases 生成 reports/fixupN-scan-failures.txt
[ ] 添加 --fixupN-scan 或 --native-fixupN-scan
[ ] 添加 selector 测试
[ ] 运行 fixup scan 并保存 reports/fixupN-scan-summary.json
[ ] 在 AGENTS.md / readme.md / roadmap 中记录 scan 命令
[ ] 冻结共享接口 owner
[ ] 各 track 运行 focused tests
[ ] 各 track 更新对应 report
[ ] 合并到 integration branch
[ ] 运行 fixup scan
[ ] 运行 full Test262
[ ] 生成最终 fixupN analysis report
[ ] 更新 docs/status.md
```
