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
