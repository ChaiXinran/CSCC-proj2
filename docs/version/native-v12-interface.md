# Native V12 Shared Interface

Native V12 freezes shared contracts for BigInt representation, property-key
coercion, backend selection, and the staged VM scheduler. This document
supplements `docs/interface-spec.md` and the V1-V11 interface documents. If a
new V12 contract conflicts with older placeholder text, this V12 contract is
authoritative for the V12 repair continuation.

## 1. Backend Surface Contract

V12-A owns removal of the embedded Boa runtime backend.

Target behavior:

```rust
pub enum BackendKind {
    Native,
}
```

Rules:

- `BackendKind::default()` must remain native.
- AgentJS must not silently fall back to Boa for native failures.
- CLI help must not advertise `--backend boa` after the embedded backend is
  removed.
- The Boa submodule may remain as an external oracle/reference engine.
- Benchmark scripts may keep `--ref-engine` support for external engines,
  including `boa_cli`, Node, QuickJS, or other standalone binaries.
- Tests that assert embedded Boa selectability must be removed or rewritten to
  assert native-only behavior.

## 2. BigInt Value Contract

V12-B owns the arbitrary-precision BigInt substrate.

Expected public runtime shape:

```rust
// src/runtime/bigint.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BigIntValue {
    // representation is private
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BigIntParseError {
    InvalidSyntax,
    UnsupportedRadix,
}

pub fn parse_bigint_literal(raw: &str) -> Result<BigIntValue, BigIntParseError>;
pub fn parse_bigint_string(input: &str) -> Option<BigIntValue>;
pub fn from_i64(value: i64) -> BigIntValue;
pub fn from_u64(value: u64) -> BigIntValue;
pub fn to_i128_if_exact(value: &BigIntValue) -> Option<i128>;
pub fn to_f64_lossy(value: &BigIntValue) -> f64;
pub fn is_zero(value: &BigIntValue) -> bool;
pub fn sign(value: &BigIntValue) -> std::cmp::Ordering;
pub fn to_radix_string(value: &BigIntValue, radix: u32) -> String;
```

`JsValue` must store the shared value type:

```rust
pub enum JsValue {
    BigInt(BigIntValue),
}
```

`Constant::BigInt` must also store `BigIntValue` or another representation that
is lossless and lowered to `BigIntValue` before VM execution. It must not store
raw `i128` after the V12-B migration.

Rules:

- BigInt parsing must not reject valid literals only because they exceed
  `i128`.
- Numeric separators are accepted only where the lexer/parser already accepts
  them; parser syntax validation remains A-owned.
- `parse_bigint_literal("0x...n")`, binary, octal, decimal, and separator forms
  must share one implementation.
- `parse_bigint_string` must follow `StringToBigInt`: trimmed empty string
  becomes zero, signed non-decimal prefixed strings are rejected, and invalid
  digits return `None`.
- `BigInt()` may convert integral finite `Number` values, strings, booleans,
  BigInts, and object primitives according to existing coercion order.
- `new BigInt()` remains a `TypeError`.

## 3. BigInt Operation Contract

V12-B owns shared operation helpers. VM and builtins must call these helpers
instead of open-coding BigInt arithmetic.

Expected helper shape:

```rust
pub fn add(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn sub(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn mul(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn div(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError>;
pub fn rem(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError>;
pub fn pow(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError>;

pub fn bitand(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn bitor(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn bitxor(left: &BigIntValue, right: &BigIntValue) -> BigIntValue;
pub fn bitnot(value: &BigIntValue) -> BigIntValue;

pub fn shl(value: &BigIntValue, shift: &BigIntValue) -> Result<BigIntValue, VmError>;
pub fn shr(value: &BigIntValue, shift: &BigIntValue) -> Result<BigIntValue, VmError>;

pub fn cmp(left: &BigIntValue, right: &BigIntValue) -> std::cmp::Ordering;
pub fn compare_bigint_number(value: &BigIntValue, number: f64) -> Option<std::cmp::Ordering>;
pub fn number_equals_bigint(number: f64, value: &BigIntValue) -> bool;

pub fn as_int_n(bits: u64, value: &BigIntValue) -> BigIntValue;
pub fn as_uint_n(bits: u64, value: &BigIntValue) -> BigIntValue;
```

Rules:

- Mixed `Number`/`BigInt` arithmetic and bitwise operators must throw
  `TypeError`.
- Relational comparisons between finite integral numbers and BigInts must be
  exact, not based only on lossy `f64` conversion.
- `BigInt` division by zero and remainder by zero must throw `RangeError`.
- Negative BigInt exponent must throw `RangeError`.
- Unsigned right shift with BigInt must throw `TypeError`.
- Shift counts may have implementation resource limits, but limits must be
  explicit `RangeError`/`RuntimeLimit`, not overflow or panic.

## 4. PropertyKey Contract

V12-C owns the VM/runtime path for computed property keys.

The shared key type already exists in the runtime and must remain the boundary
for operations that can observe symbols:

```rust
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}
```

Rules:

- Computed property access, computed property definition, object spread/rest
  exclusions, `Reflect`, `Object` descriptor helpers, and Proxy traps must use
  `PropertyKey` where symbol keys are valid.
- `ToPropertyKey` returns a symbol unchanged. It must not route symbol keys
  through `ToString`.
- String-only helper paths may remain for static property names and legacy
  internals, but they must not be called for computed symbol keys.
- Error messages may differ slightly, but symbols must not be rejected for
  valid property access.
- Property enumeration order remains B/C coordinated: integer-index strings,
  ordinary strings, then symbols where implemented.

## 5. Computed Update Contract

V12-C owns `obj[key]++`, `obj[key]--`, and related computed update lowering.

Rules:

- The object expression is evaluated exactly once.
- The key expression is evaluated exactly once.
- The old property value is read exactly once.
- Getter side effects occur once before the write.
- Setter side effects occur once during the write.
- Prefix updates evaluate to the new numeric value.
- Postfix updates evaluate to the old `ToNumeric` value.
- The implementation may introduce new bytecode such as a precomputed element
  store, but any opcode added here must update `Instruction::stack_effect`,
  `Chunk::validate`, and focused bytecode tests in the same change.

## 6. Function Call/Apply Contract

V12-B owns `Function.prototype.call` and `Function.prototype.apply` correctness
in the builtin/VM forwarding path.

Rules:

- `call` and `apply` must reject non-callable targets with `TypeError`.
- `call` uses its first argument as `thisArg` and forwards remaining arguments
  in order.
- `apply` treats `null` and `undefined` argument lists as empty.
- `apply` consumes array-like argument lists in index order and propagates
  abrupt completions.
- Bound functions prepend bound arguments and use the bound `this` for ordinary
  calls.
- Constructor behavior for bound functions must keep ignoring bound `this`.
- C must coordinate before changing call/apply descriptors or name/length
  metadata.

## 7. VM Scheduler Contract

V12-C owns the staged explicit-frame scheduler. This is a design-first change:
large interpreter rewrites must follow the contract document and merge after
BigInt and property-key fixes.

Target conceptual shape:

```rust
pub struct VmFrame {
    pub function: Option<FunctionId>,
    pub chunk: Chunk,
    pub ip: usize,
    pub environment: EnvironmentId,
    pub this_value: JsValue,
    pub new_target: JsValue,
    pub stack_base: usize,
    pub resume: FrameResume,
}

pub enum FrameResume {
    Entry,
    ReturnToCaller,
    Eval,
    GeneratorResume,
}
```

Rules:

- Synchronous JS calls remain stack-ordered. Do not introduce fairness or
  round-robin scheduling for ordinary function calls.
- The scheduler may be called a trampoline or explicit-frame loop; it must
  preserve observable evaluation order.
- `Call` pushes a callee frame or invokes a builtin through the existing
  catchable throw path.
- `Return` writes the return value to the caller operand stack and resumes the
  caller.
- `Throw` unwinds explicit frames until a bytecode handler is found.
- Generator/async migration may be staged after ordinary user functions.
- Existing call-depth, recursion, VM stack, wall-clock, GC, and backtrace limits
  must remain enforced.

## 8. Merge Compatibility

Recommended order:

```text
V12 interface docs
  -> A de-Boa backend surface
  -> B BigIntValue substrate
  -> B BigInt VM and builtin migration
  -> C PropertyKey symbol path
  -> A/B/C focused semantic fixes
  -> C VM scheduler prototype
  -> cleanup and integration reports
```

Shared files require coordination:

- A changes to `Cargo.toml`, `src/backend/`, and CLI parsing must merge before
  tests are rewritten to remove embedded Boa behavior.
- A changes to `src/bytecode/compiler.rs` for BigInt literals must use B's
  shared parser once it lands.
- B changes to `src/runtime/value.rs` and BigInt helpers must merge before C
  rewrites VM BigInt operator paths.
- C changes to `src/vm/interpreter.rs` must wait for B's BigInt helper
  migration unless the change is isolated to property-key or scheduler design.
- C changes to `PropertyKey` call sites must coordinate with B if BigInt object
  wrappers or primitive coercion paths are touched.
