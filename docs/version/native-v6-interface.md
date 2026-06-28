# Native V6 Shared Interface

This document freezes the contracts connecting the V6 runtime and builtin
work. Implementations may be developed in separate modules, but these shapes
must not diverge between branches.

## 1. Primitive Wrapper Contract

Primitive wrapper state is an internal slot, not a JavaScript-visible
property:

```rust
pub enum PrimitiveValue {
    Boolean(bool),
    Number(f64),
    String(String),
}

pub enum ObjectKind {
    Ordinary,
    Array {
        elements: Vec<Option<PropertyDescriptor>>,
        length_writable: bool,
    },
    PrimitiveWrapper(PrimitiveValue),
}
```

`PrimitiveValue` deliberately excludes objects, functions, errors, null, and
undefined. Wrapper objects retain normal properties and a prototype link.

Required runtime operations:

```rust
impl NativeContext {
    pub fn create_primitive_wrapper(
        &mut self,
        value: PrimitiveValue,
        prototype: ObjectId,
    ) -> Result<JsValue, VmError>;

    pub fn primitive_value(&self, object: ObjectId) -> Option<&PrimitiveValue>;
}
```

## 2. Coercion Contract

Object coercion may invoke JavaScript functions and therefore requires the VM:

```rust
pub enum PreferredType {
    Default,
    Number,
    String,
}

impl Vm {
    pub(crate) fn to_primitive(
        &mut self,
        value: JsValue,
        hint: PreferredType,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError>;

    pub(crate) fn to_number(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<f64, VmError>;

    pub(crate) fn to_string(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<String, VmError>;

    pub(crate) fn to_object(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<ObjectId, VmError>;
}
```

Rules:

- primitive conversions must not allocate unless `ToObject` is requested;
- null and undefined fail `ToObject` with `TypeError`;
- ordinary conversion tries methods in hint order;
- a non-primitive result from both conversion methods is a `TypeError`;
- JavaScript throws from conversion methods remain JavaScript throws and are
  catchable by V5 handlers;
- builtins must not use display formatting as semantic conversion.

Pure primitive helpers may remain on `JsValue`, but object-aware paths must use
the VM contract.

## 3. Intrinsics Contract

`Intrinsics` gains stable identities for:

```rust
pub struct Intrinsics {
    // existing Object, Function, and Array entries...
    pub string_prototype: ObjectId,
    pub number_prototype: ObjectId,
    pub boolean_prototype: ObjectId,
    pub error_prototype: ObjectId,
}
```

All constructor and prototype links are installed once per
`NativeContext`. Tests must not depend on builtin registration order or numeric
`BuiltinId` values.

## 4. Builtin Module Contract

Each standard object owns one module:

```text
src/builtins/string.rs
src/builtins/number.rs
src/builtins/boolean.rs
src/builtins/math.rs
src/builtins/error.rs
src/builtins/json.rs
```

Each module exposes only an installer to the parent module:

```rust
pub(crate) fn install(context: &mut NativeContext) -> Result<(), VmError>;
```

Builtin functions continue using `NativeCall`/`NativeConstruct`. Method
metadata is declared beside its implementation:

```rust
struct MethodSpec {
    name: &'static str,
    length: u8,
    call: NativeCall,
}
```

Installers must define descriptor flags explicitly. They must not add
builtin-specific bytecode instructions or parser special cases.

## 5. String Representation

Rust `String` remains the storage type, but String builtin indexing and length
operate on ECMAScript UTF-16 code units. Shared helpers must make that choice
visible:

```rust
fn utf16_length(value: &str) -> usize;
fn utf16_code_unit_at(value: &str, index: usize) -> Option<u16>;
fn utf16_slice(value: &str, start: usize, end: usize) -> String;
```

Methods must not silently use Rust byte offsets.

## 6. JSON Contract

JSON parsing is independent of the JavaScript parser and produces `JsValue`
using `NativeContext`. Stringification tracks visited object identities for
cycle detection. Object key order follows the existing runtime property-order
contract.

Core signatures:

```rust
pub(crate) fn parse_json(
    source: &str,
    context: &mut NativeContext,
) -> Result<JsValue, VmError>;

pub(crate) fn stringify_json(
    value: JsValue,
    context: &NativeContext,
) -> Result<Option<String>, VmError>;
```

The public `JSON.parse` and `JSON.stringify` wrappers apply coercion and later
reviver/replacer behavior around these core functions.

## 7. Compatibility Rules

- Do not change AST or add opcodes for standard builtin names.
- Preserve V4 object identity, descriptors, sparse arrays, and callback
  behavior.
- Preserve V5 completion and catchable throw semantics.
- Boa is a differential oracle only and is never called by Native builtins.
- Shared contract changes merge before individual builtin modules.
