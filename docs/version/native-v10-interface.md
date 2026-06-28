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
pub struct TypedArrayViewId(pub u32);
pub struct DataViewId(pub u32);

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

pub struct DataViewRecord {
    pub buffer: ArrayBufferId,
    pub byte_offset: usize,
    pub byte_length: usize,
}

impl NativeContext {
    pub fn create_array_buffer(&mut self, byte_length: usize) -> Result<ArrayBufferId, VmError>;
    pub fn array_buffer_byte_length(&self, buffer: ArrayBufferId) -> Result<usize, VmError>;
    pub fn is_array_buffer_detached(&self, buffer: ArrayBufferId) -> Result<bool, VmError>;
    pub fn detach_array_buffer(&mut self, buffer: ArrayBufferId) -> Result<(), VmError>;

    pub fn create_typed_array_view(
        &mut self,
        buffer: ArrayBufferId,
        element_kind: TypedArrayElementKind,
        byte_offset: usize,
        length: usize,
    ) -> Result<TypedArrayViewId, VmError>;
    pub fn typed_array_byte_length(&self, view: TypedArrayViewId) -> Result<usize, VmError>;
    pub fn typed_array_load_element(
        &self,
        view: TypedArrayViewId,
        index: usize,
    ) -> Result<JsValue, VmError>;
    pub fn typed_array_store_element(
        &mut self,
        view: TypedArrayViewId,
        index: usize,
        value: JsValue,
    ) -> Result<(), VmError>;

    pub fn create_data_view(
        &mut self,
        buffer: ArrayBufferId,
        byte_offset: usize,
        byte_length: usize,
    ) -> Result<DataViewId, VmError>;
    pub fn data_view_get(
        &self,
        view: DataViewId,
        request_index: usize,
        element_kind: TypedArrayElementKind,
        little_endian: bool,
    ) -> Result<JsValue, VmError>;
    pub fn data_view_set(
        &mut self,
        view: DataViewId,
        request_index: usize,
        element_kind: TypedArrayElementKind,
        value: JsValue,
        little_endian: bool,
    ) -> Result<(), VmError>;
}
```

Rules:

- all byte-length, offset, and element-index calculations must be bounds checked;
- detached buffer behavior may start minimal, but it must be observable through
  one shared runtime flag;
- DataView and TypedArray must share the same `ArrayBufferRecord` storage;
- BigInt element kinds may initially throw explicit unsupported errors if BigInt
  values are not fully implemented.
- the first B implementation supports Number-backed element load/store and
  returns explicit `TypeError` for `BigInt64` / `BigUint64` paths.

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
