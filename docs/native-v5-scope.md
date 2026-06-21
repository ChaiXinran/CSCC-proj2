# Native V5 异常处理与 for 循环里程碑

本文档冻结 AgentJS Native V5 的功能范围、协作分工和验收标准。V4 建立了完整的对象模型、属性描述符、原型链和三大标准构造器（Object / Array / Function）；V5 在此基础上引入 `try/catch/finally`、标准 Error 类层次、经典 `for(;;)` 循环以及 `for-in` 语句，使 Native 引擎能够正确抛出、传播和捕获 JavaScript 异常，并支持对象属性的标准枚举方式。

共享类型和字节码契约见 [Native V5 共享接口规格](native-v5-interface.md)。

---

## 1. 完成标准

V5 完成时必须满足：

- V1、V2、V3、V4 固定 Test262 清单零回退、零跳过；
- `try/catch` 能捕获 `throw` 抛出的任意 JS 值；
- `try/catch/finally` 三段完整形式中，finally 在正常完成、throw 和 return 三种路径下均执行；
- `finally` 内部的正常完成不得吞掉外层 throw 或 return；
- 标准 Error 构造器（`Error`、`TypeError`、`RangeError`、`SyntaxError`、`ReferenceError`）创建带正确 `name`/`message` 属性和原型链的对象；
- 运行时内部错误（类型错误、范围错误等）转换为对应 Error 对象后可被 `catch` 捕获，不得直接终止执行；
- `catch(e)` 将捕获值绑定到 `e`；
- `catch` 可选绑定（`catch {}` 无参数，ES2019）正常解析；
- 嵌套 `try/catch/finally` 从内层向外层正确传播；
- `for (init; test; update) body` 作为经典三段式 for 循环正确执行；
- `for (var x in obj)` 枚举对象自有可枚举字符串键及原型链可枚举键，不重复，按插入顺序；
- `break`/`continue` 在 for 和 for-in 中行为正确；
- 新增 `--native-v5` 覆盖 V5 相关 Test262 目录；
- 不支持的语法（for-of、解构、let/const、Symbol.iterator 等）不得由 Boa 兜底。

V5 分三批合并：

1. **V5.1** — `for(;;)` + `for-in`；
2. **V5.2** — `try/catch`、异常传播与 VM 捕获机制；
3. **V5.3** — `finally`、标准 Error 构造器与 Error 原型链。

---

## 2. 语言范围

### 2.1 Lexer

新增关键字（均为保留字，不得作标识符）：

```
try   catch   finally   for
```

`throw` 已在 V2 中支持，不重新定义。

### 2.2 Parser 与 AST

新增或扩展：

**try 语句：**
- `try { } catch(e) { }` — 带绑定的 catch；
- `try { } catch { }` — 可选绑定（ES2019，binding 为 None）；
- `try { } finally { }` — 仅 finally；
- `try { } catch(e) { } finally { }` — 完整三段形式；
- `try/catch/finally` 嵌套；
- `throw expr;` 语义不变，但结果须可被封闭 `catch` 块捕获。

**for 语句：**
- `for (init; test; update) body` — init 可为 `var` 声明或表达式，也可省略；
- `for (;;) body` — 无限循环；
- `for (var x in expr) body` — for-in 循环；
- `for (x in expr) body` — for-in，左侧为赋值目标标识符。

V5 暂不支持：

- `for-of`、迭代器协议；
- `let`/`const`、TDZ、块作用域；
- `for (let x in obj)` / `for (const x in obj)`；
- for-in 左侧解构绑定；
- 标签语句（labeled break/continue）；
- class、arrow function、generator、async；
- `with` 语句；
- Error.prototype.stack。

### 2.3 Runtime 与执行语义

**try/catch/finally（V5.2 + V5.3）：**

- VM 遇到 Throw completion 时，扫描当前 Chunk 的异常处理器表，寻找覆盖当前 IP 的处理器；
- 若有 catch 处理器，跳转到 catch 块入口，`LoadException` 指令将捕获值压栈；
- `catch(e)` 在当前环境声明绑定 `e`，值为捕获到的异常；
- 若无匹配 catch，Throw 沿调用栈向上传播，跨调用帧时继续查表；
- finally 块在正常完成、Throw、Return 三种路径下均执行；
- finally 执行后恢复进入前的 completion（Throw 重新传播，Return 保留返回值）；
- finally 块内若发生新的 Throw，替换原有 completion；
- 到达顶层仍未处理时，Throw(value) 映射为 NativeError::Execute；
- 内部 VmError::type_error / range_error / reference_error 必须先转换为对应 JS Error 对象后再抛出，使 catch 块能捕获；
- 嵌套 try/catch 时外层处理器在内层未处理时激活。

**for(;;) 循环（V5.1）：**

- `for (init; test; update) body` 语义等同于 while 展开；
- init 中的 `var` 在当前函数/全局环境声明；
- `break` 退出循环；
- `continue` 跳到 update 后再执行 test；
- `for (;;)` test 缺省等同于常量 true（须有 break 退出）。

**for-in 循环（V5.1）：**

- 对非 null/undefined 的值枚举可枚举字符串键；
- 枚举顺序：先对象自有属性（插入顺序），然后沿原型链向上（各层按插入顺序，已出现的键跳过）；
- 对 null/undefined 直接跳过循环体，不报错；
- 对原始值的 for-in 跳过循环体（不包装，不报 TypeError）；
- `break` 退出循环并清理内部迭代器；
- `continue` 进入下一轮键枚举。

**标准 Error 对象（V5.3）：**

- `new TypeError("msg")` 创建对象，自有属性 `message: "msg"`；
- 原型链：`instance → TypeError.prototype → Error.prototype → Object.prototype`；
- `Error.prototype.name === "Error"`，`TypeError.prototype.name === "TypeError"` 等；
- `instanceof TypeError` / `instanceof Error` 正确（复用 V4 的 instanceof 操作）；
- Error 构造器不带参数时 `message` 为 `""`；
- Error(msg) 和 new Error(msg) 行为相同；
- V5 不实现 `stack` 属性、`AggregateError`、`EvalError`、`URIError`。

### 2.4 Bytecode

V5 新增指令：

```
ForIn                   — 弹出对象，压入内部 ForInIterator
ForInNext(jump_offset)  — 若迭代完毕则弹出迭代器并跳转；否则压入下一个键字符串
ForInDispose            — 弹出并销毁 ForInIterator（用于 break 提前退出）
LoadException           — 将当前捕获的异常值压栈（catch 块入口使用）
```

Chunk 新增字段：

```rust
pub exception_handlers: Vec<ExceptionHandler>
```

其中 `ExceptionHandler` 结构见 [V5 接口规格](native-v5-interface.md)。

### 2.5 Completion

V3 已引入 Completion 枚举；V5 要求：

- `Throw` completion 携带 `JsValue`（不再直接携带裸 VmError 终止执行）；
- 内部 VmError 在到达 catch 块前必须转换为对应 JS Error 对象（JsValue::Object）；
- 调用帧退出时，若有 finally 块，必须先执行 finally 再恢复 completion；
- finally 内的正常完成不得覆盖外层 Throw/Return；
- finally 内的新 Throw 替换原有 completion。

---

## 3. 四组分工

### A 组：前端（`lexer/`、`ast/`、`parser/`）

**负责：**
- 增加 Token：`try`、`catch`、`finally`、`for`；
- 解析 `try/catch`、`try/finally`、`try/catch/finally`，支持嵌套；
- 解析 catch 可选绑定（`catch {}` 无参数）；
- 解析 `for (;;)` 三段式，init 支持 `var` 声明和表达式，三段均可省略；
- 解析 `for (var x in expr)` 和 `for (x in expr)`；
- 添加负向解析测试：try 无 catch 且无 finally、for-in left 为解构、for-in left 为 let/const；
- 不修改 Compiler、VM、Runtime 或 Builtins。

**验收指标：**
- 所有 for/try/catch/finally/for-in 的 AST 形态与 V5 接口规格完全吻合；
- A 组单元测试全绿，不依赖 Compiler 或 VM。

### B 组：编译器（`bytecode/`）

**负责：**
- 扩展 `Chunk`，增加 `exception_handlers: Vec<ExceptionHandler>`；
- 编译 try body / catch body（首指令为 `LoadException`）/ finally body；
- 将 try/catch/finally IP 范围写入 `ExceptionHandler`；
- 编译 `for(;;)` 为 `[init] Jump(test) loop: [body] [update] test: [test] JumpIfTrue(loop)` 形式；
- 编译 `for-in` 为 `[obj] ForIn loop: ForInNext(exit) [bind] [body] Jump(loop) exit: ForInDispose?` 形式；
- 确保 `break`/`continue` 在 for / for-in 中跳转目标正确；
- break 在 for-in 中须在跳出前插入 `ForInDispose`；
- try 内部的 break/continue 不得跨越 finally 边界（defer 到 V5.3，若处理复杂可标注 Unsupported）；
- 使用手工构造 AST 测试，不依赖 Parser 或 VM。

**验收指标：**
- 生成的 ExceptionHandler 表与预期 IP 吻合（单元测试）；
- ForIn/ForInNext/ForInDispose 指令序列正确；
- 栈效果分析更新，覆盖新指令。

### C 组：VM / Runtime / Builtins（`vm/`、`runtime/`、`builtins/`）

**负责：**
- VM 异常传播：Throw completion 时查 `exception_handlers` 表，跳转 catch 或沿调用栈传播；
- 实现 `LoadException` 指令（读取 `NativeContext::current_exception`）；
- 实现 `ForIn` / `ForInNext` / `ForInDispose` 指令和 `ForInIteratorId` 分配；
- 在 `NativeContext` 增加 for-in 迭代器存储（`for_in_iterators: Vec<ForInIteratorEntry>`）；
- 在 `JsValue` 增加 `ForInIterator(ForInIteratorId)` 变体；
- 实现 finally 执行逻辑与 completion 恢复；
- 实现 VmError → JS Error 对象转换（`context.create_type_error_object` 等）；
- 安装 `Error`、`TypeError`、`RangeError`、`SyntaxError`、`ReferenceError` 构造器；
- 建立 Error 原型链（Error.prototype → Object.prototype，各子类 prototype → Error.prototype）；
- 使用手工 Chunk 直接测试 catch/finally 语义、for-in 枚举顺序、Error 对象属性。

**验收指标：**
- 手工 Chunk 测试全绿（单元测试不依赖 Parser/Compiler）；
- Error instanceof Error → true，TypeError instanceof Error → true；
- VmError::type_error 在 catch 块内可捕获（集成测试）。

### D 组：集成测试（`test262.rs`、`tests/`）

**负责：**
- `--native-v5` 标志：扫描 V5 相关 Test262 目录；
- 新增 `NATIVE_V5_SCAN_SUITES` 列表（见第 5 节）；
- 新增 `NATIVE_V5_TESTS` 固定清单（全部通过的最小集合）；
- 保留 V1–V4 固定门，零回退；
- 每批合并后更新 skipped / regressed 计数；
- 确保 skipped 不计入 passed。

**验收指标：**
- `--native-v1` 到 `--native-v4` 各自零回退；
- `--native-v5` 固定清单全部通过，无新 skipped。

---

## 4. 端到端验收示例

以下所有示例须在 `cargo run -- eval` 下通过（`--backend native`）：

```javascript
// 基本 catch
var x;
try { throw 42; } catch(e) { x = e; }
x;                                               // 42
```

```javascript
// Error 对象捕获
var caught;
try { throw new Error("oops"); } catch(e) { caught = e.message; }
caught;                                          // "oops"
```

```javascript
// TypeError instanceof 链
var ok = false;
try { null.x; } catch(e) { ok = e instanceof TypeError; }
ok;                                              // true
```

```javascript
// finally 在正常完成时执行
var log = [];
try { log.push("try"); } finally { log.push("finally"); }
log.join(",");                                   // "try,finally"
```

```javascript
// finally 在 throw 时执行，throw 继续传播
var log = [];
try {
  try { log.push("try"); throw 1; }
  finally { log.push("finally"); }
} catch(e) { log.push("catch:" + e); }
log.join(",");                                   // "try,finally,catch:1"
```

```javascript
// try/catch/finally 三段
var log = [];
try { log.push("t"); throw 0; }
catch(e) { log.push("c"); }
finally { log.push("f"); }
log.join(",");                                   // "t,c,f"
```

```javascript
// for(;;) 基本用法
var s = 0;
for (var i = 0; i < 5; i++) { s += i; }
s;                                               // 10
```

```javascript
// for-in 自有键
var obj = { a: 1, b: 2, c: 3 };
var keys = [];
for (var k in obj) { keys.push(k); }
keys.join(",");                                  // "a,b,c"
```

```javascript
// for-in 原型链不重复
var parent = { x: 1 };
var child = Object.create(parent);
child.y = 2;
var keys = [];
for (var k in child) { keys.push(k); }
keys.join(",");                                  // "y,x"
```

```javascript
// for-in break
var keys = [];
var obj = { a: 1, b: 2, c: 3 };
for (var k in obj) {
  if (k === "b") break;
  keys.push(k);
}
keys.join(",");                                  // "a"
```

```javascript
// 嵌套 try，内层未处理
var caught = false;
try {
  try { throw new TypeError("inner"); }
  catch(e) { if (e.message !== "inner") throw e; }
} catch(e) {
  caught = true;
}
caught;                                          // false  (内层处理了)
```

```javascript
// Error 原型链
new TypeError("t") instanceof Error;             // true
new RangeError("r") instanceof Error;            // true
TypeError.prototype.name;                        // "TypeError"
```

---

## 5. Test262 策略

**V5 优先候选目录（`NATIVE_V5_SCAN_SUITES`）：**

```
test/language/statements/try
test/language/statements/for
test/language/statements/for-in
test/built-ins/Error
test/built-ins/TypeError
test/built-ins/RangeError
test/built-ins/SyntaxError
test/built-ins/ReferenceError
```

**暂不纳入 V5 固定清单（留待后续版本）：**

- 依赖 `for-of` / 迭代器协议的 for-in 测试；
- 依赖 `Symbol.iterator` 的任何测试；
- 依赖 `let`/`const` TDZ 的 try/catch 测试；
- 依赖 `Error.captureStackTrace` 或 `.stack` 属性的测试；
- 依赖 `AggregateError`、`EvalError`、`URIError` 的测试；
- 依赖标签语句（labeled break/continue）的 for/for-in 测试；
- 依赖解构绑定的 catch/for-in 测试。

**计分规则：**
- Skipped 不计 passed；
- 每次 PR 必须报告 newly-passed / newly-failed / skipped / regressed 数量。

---

## 6. 合并顺序

**前置条件：**
V1–V4 固定清单零回退，`cargo check` / `cargo test` / `cargo clippy -D warnings` 全绿。

**V5 合并序列：**

1. **契约 PR（A + B + C 联合）**：新增共享 AST 节点（TryCatch / For / ForIn）、ExceptionHandler、ForInIteratorId、JsValue::ForInIterator、新 Opcode，以及 NativeContext 接口签名；
2. **V5.1**：for(;;) + for-in（A 解析 + B 编译 + C 执行 + D 测试）；
3. **V5.2**：try/catch + LoadException + 异常传播（A 解析 + B 编译 + C VM + D 测试）；
4. **V5.3**：finally + Error 构造器 + 原型链（B finally 编译 + C builtins + D 测试）。

每批合并前执行：

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run --release -- test262 --native-v1 --jobs 1
cargo run --release -- test262 --native-v2 --jobs 1
cargo run --release -- test262 --native-v3 --jobs 1
cargo run --release -- test262 --native-v4 --jobs 1
```

V5.3 合并后额外执行：

```sh
cargo run --release -- test262 --native-v5 --jobs 1 --verbose
```

V5 完成时，V1–V5 固定清单全部零回退、零跳过。
