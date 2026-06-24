# Native V10 Shared Interface

Native V10 freezes shared contracts for numeric syntax tail work,
ArrayBuffer/TypedArray/DataView runtime substrate, and Date/Intl/Temporal
builtin semantics.

This document supplements `docs/interface-spec.md` and the V1-V9 interface
documents. If a V10 contract conflicts with older placeholder text, this V10
contract is authoritative for the new behavior.

## 1. Frontend Contract

V10-A may add literal/source-text metadata needed by numeric and BigInt syntax
tail work.

Expected shapes may use existing project naming, but must cover:

```rust
pub enum Literal {
    Number(f64),
    BigInt(String),
    // existing variants...
}

pub struct NumericLiteralMeta {
    pub raw: String,
    pub has_separator: bool,
}
```

Rules:

- unsupported numeric forms must fail with `SyntaxError`, not panic;
- BigInt syntax support must not imply BigInt arithmetic support unless C has
  installed matching builtins/operators;
- frontend changes must not touch typed-array storage or Date/Intl/Temporal
  algorithms.

## 2. Buffer and TypedArray Runtime Contract

V10-B owns the shared byte-storage substrate. C builtins must use this runtime
storage instead of keeping parallel storage in `src/builtins/`.

Target runtime API shape:

```rust
pub struct ArrayBufferId(pub u32);
pub struct ArrayBufferRecord {
    pub bytes: Vec<u8>,
    pub detached: bool,
}

pub enum TypedArrayElementKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

pub struct TypedArrayView {
    pub buffer: ArrayBufferId,
    pub byte_offset: usize,
    pub length: usize,
    pub element_kind: TypedArrayElementKind,
}
```

Rules:

- all byte-length, offset, and element-index calculations must be bounds checked;
- detached buffer behavior may start minimal, but it must be observable through
  one shared runtime flag;
- DataView and TypedArray must share the same `ArrayBufferRecord` storage;
- BigInt element kinds may initially throw explicit unsupported errors if BigInt
  values are not fully implemented.

## 3. Date / Intl / Temporal Builtin Contract

V10-C owns JS-visible builtins and algorithms for Date, Intl, and Temporal.

Rules:

- implemented constructors/prototypes must be installed through normal builtin
  APIs and descriptors;
- deterministic fallback formatting is acceptable for V10 if documented in the
  relevant report;
- unsupported locale/time-zone behavior must throw explicit errors or expose a
  documented deterministic fallback;
- C must not bypass B buffer storage for typed-array-facing Date/Intl helpers.

## 4. Merge Compatibility

Recommended order:

```text
interface docs
 -> numeric AST/runtime value additions
 -> ArrayBuffer/TypedArray runtime storage
 -> Date/Intl/Temporal builtin algorithms
 -> focused Test262 reports
 -> V10 scan integration
```

Shared files require coordination:

- A changes to `src/lexer/`, `src/parser/`, `src/ast/`, or
  `src/bytecode/compiler.rs` must preserve V9-A in-progress work.
- B changes to `src/runtime/` must coordinate with C before JS-visible typed
  array constructors use storage helpers.
- C changes to `src/builtins/` must use B runtime storage for any typed-array or
  buffer object state.
