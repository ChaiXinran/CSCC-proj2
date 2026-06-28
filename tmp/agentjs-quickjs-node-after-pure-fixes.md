# QuickJS Correctness Tests

- Label: `agentjs-quickjs-node-after-pure-fixes`
- Engine: `C:\Users\36123\Desktop\OS 功能赛\CSCC-proj2\target\release\agentjs.exe jetstream`
- Result: **0/5 passed**

| Case | Status | Time ms |
|:---|:---|---:|
| quickjs\tests\test_closure.js | runtime-limit | 417.2 |
| quickjs\tests\test_language.js | syntax-error | 13.2 |
| quickjs\tests\test_builtin.js | syntax-error | 13.6 |
| quickjs\tests\test_loop.js | runtime-error | 12.5 |
| quickjs\tests\test_bigint.js | runtime-error | 11.7 |

## Failure Details

### quickjs\tests\test_closure.js

- Status: `runtime-limit`
- Return code: `1`

```text
agentjs: RuntimeLimit: execution error: call stack limit exceeded
```

### quickjs\tests\test_language.js

- Status: `syntax-error`
- Return code: `1`

```text
agentjs: SyntaxError: parse error: expected `)` but found identifier `a` at bytes 14738..14739
```

### quickjs\tests\test_builtin.js

- Status: `syntax-error`
- Return code: `1`

```text
agentjs: SyntaxError: parse error: Unicode sets character class syntax character must be escaped at bytes 24797..24803
```

### quickjs\tests\test_loop.js

- Status: `runtime-error`
- Return code: `1`

```text
agentjs: Unsupported: compile error: bytecode compiler does not support for-in target must be a variable or a simple identifier yet
```

### quickjs\tests\test_bigint.js

- Status: `runtime-error`
- Return code: `1`

```text
agentjs: Unsupported: compile error: bytecode compiler does not support BigInt literal `515377520732011331036461129765621272702107522001n` is outside the native i128 range yet
```
