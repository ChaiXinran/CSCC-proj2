已按这个设计重写测试文件：[tests/native_full_test262_by_dir.rs](D:/00_OS/CSCC/tests/native_full_test262_by_dir.rs:1)。

现在它有 3 个模式，都是 `#[ignore]`，不会影响普通 `cargo test`。

### 1. 全量一级目录仪表盘

跑 `test262/test` 下的一级目录：

```text
test/annexB
test/built-ins
test/harness
test/intl402
test/language
test/staging
```

命令：

```powershell
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture
```

默认输出：

```text
reports/native-full-test262-summary.json
```

这个模式现在是“每个目录一个子进程”。如果某个目录触发崩溃，不会把整个父测试一起带走；报告里会记录该目录 `status: "crashed"`，然后继续下一个目录。

### 2. 热点目录细分

默认细分：

```text
test/built-ins/*
```

命令：

```powershell
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

默认输出：

```text
reports/native-hotspot-test262-summary.json
```

如果你想细分别的目录，比如 `test/language`：

```powershell
$env:AGENTJS_TEST262_SUITE = "test/language"

cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
```

### 3. 失败样本模式

用于小范围定位问题。默认跑：

```text
test/built-ins/Symbol
```

命令：

```powershell
cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_failure_samples -- --ignored --nocapture
```

默认输出：

```text
reports/native-test262-failure-samples.json
```

指定目录和样本上限：

```powershell
$env:AGENTJS_TEST262_SUITE = "test/built-ins/Symbol"
$env:AGENTJS_TEST262_SAMPLE_LIMIT = "100"

cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_failure_samples -- --ignored --nocapture
```

### 通用环境变量

```powershell
$env:AGENTJS_TEST262_JOBS = "4"
$env:AGENTJS_TEST262_REPORT = "reports/my-report.json"
```

清理环境变量：

```powershell
Remove-Item Env:AGENTJS_TEST262_JOBS
Remove-Item Env:AGENTJS_TEST262_REPORT
Remove-Item Env:AGENTJS_TEST262_SUITE
Remove-Item Env:AGENTJS_TEST262_SAMPLE_LIMIT
```