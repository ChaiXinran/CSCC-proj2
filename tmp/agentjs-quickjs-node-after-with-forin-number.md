# QuickJS Correctness Tests

- Label: `agentjs-quickjs-node-after-with-forin-number`
- Engine: `C:\Users\36123\Desktop\OS 功能赛\CSCC-proj2\target\release\agentjs.exe jetstream`
- Result: **1/5 passed**

| Case | Status | Time ms |
|:---|:---|---:|
| quickjs\tests\test_closure.js | pass | 228.5 |
| quickjs\tests\test_language.js | runtime-error | 13.9 |
| quickjs\tests\test_builtin.js | syntax-error | 11.5 |
| quickjs\tests\test_loop.js | runtime-error | 17.4 |
| quickjs\tests\test_bigint.js | runtime-error | 13.8 |

## Failure Details

### quickjs\tests\test_language.js

- Status: `runtime-error`
- Return code: `1`

```text
agentjs: TypeError: execution error: undefined is not callable
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
agentjs: Error: execution error: uncaught Error
```

### quickjs\tests\test_bigint.js

- Status: `runtime-error`
- Return code: `1`

```text
agentjs: Unsupported: compile error: bytecode compiler does not support BigInt literal `515377520732011331036461129765621272702107522001n` is outside the native i128 range yet
```
