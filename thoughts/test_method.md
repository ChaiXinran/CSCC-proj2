# 本地构建与测试方法

在仓库根目录 `D:\00_OS\CSCC` 打开 PowerShell。以下命令默认从该目录执行。

## 基本运行

```powershell
cargo build --release
cargo run --release -- eval "6 * 7"
cargo run --release -- run examples/hello.js
cargo run --release -- repl
```

需要明确使用自研后端时，添加 `--backend native`：

```powershell
cargo run --release -- eval --backend native "1 + 2"
```

## 合并后的标准验证流程

每次合并多人开发分支后，严格按以下顺序运行：

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

依次检查格式、所有构建目标、完整 Rust 测试以及 Clippy 警告。任何一步失败都应先修复，再继续执行后面的测试。

## 前序 Test262 回归门

V4 改动不能破坏已经通过的 V1、V2 和 V3 测试。固定回归命令如下：

```powershell
cargo run -- test262 --native-v1 --jobs 1 --verbose
cargo run -- test262 --native-v2 --jobs 1 --verbose
cargo run -- test262 --native-v3 --jobs 1 --verbose
```

使用单任务和详细输出，便于定位具体回归文件。跳过项不能计为通过。

## V4 Test262 全量扫描

合并验证的最终步骤是扫描 V4 对应的真实 Test262 目录：

```powershell
cargo run -- test262 --native-v4 --jobs 4 --progress
```

只查看汇总、不逐文件显示进度：

```powershell
cargo run -- test262 --native-v4 --jobs 4
```

保存机器可读的结果：

```powershell
cargo run -- test262 --native-v4 --jobs 4 --json reports/native-v4-summary.json
```

提交功能变更时，应记录总数、通过数、失败数、跳过数和相对上一次的变化。若 PowerShell 找不到 `cargo`，可使用：

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --all-targets
```
