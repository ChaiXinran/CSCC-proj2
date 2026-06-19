在仓库根目录 `D:\00_OS\CSCC` 打开 PowerShell。

构建：

```powershell
cargo build --release
```

运行一段 JavaScript：

```powershell
cargo run --release -- eval "6 * 7"
```

运行 JS 文件：

```powershell
cargo run --release -- run examples/hello.js
```

启动交互式 REPL：

```powershell
cargo run --release -- repl
```

运行项目测试：

```powershell
cargo test
```

运行少量 Test262：

```powershell
cargo run --release -- test262 `
  --root test262 `
  --suite test/language/expressions/addition `
  --limit 20 `
  --jobs 4 `
  --verbose
```

也可以直接使用编译后的程序：

```powershell
.\target\release\agentjs.exe eval "Array.from({length: 5}, (_, i) => i * i)"
```

目前这些命令默认使用 Boa 后端。Native 自研后端只有代码骨架，尚不能执行 JavaScript。若 PowerShell 找不到 `cargo`，重新打开终端，或使用：

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" run --release -- eval "1 + 2"
```