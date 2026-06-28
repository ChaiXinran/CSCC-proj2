# Native V11 Shared Interface

Native V11 freezes shared contracts for RegExp parser/static-error work,
runtime object-model precision, and RegExp/Annex B/descriptor builtin sweeps.

This document supplements `docs/interface-spec.md` and the V1-V10 interface
documents. If a V11 contract conflicts with older placeholder text, this V11
contract is authoritative for the new behavior.

## 1. RegExp Frontend Contract

V11-A owns RegExp syntax/static-error improvements.

Expected shapes may use existing project naming, but must cover:

```rust
pub struct RegExpLiteral {
    pub pattern: String,
    pub flags: String,
    pub groups: RegExpGroupMeta,
}

pub struct RegExpGroupMeta {
    pub named_groups: Vec<String>,
    pub backreferences: Vec<String>,
}
```

Rules:

- invalid RegExp syntax must become `SyntaxError`, not `Runtime` or panic;
- A must not implement RegExp runtime matching algorithms in lexer/parser code;
- duplicate/named-group rules should be documented when intentionally partial.

## 2. Object Model Precision Contract

V11-B owns shared object-model precision used by builtins.

Target runtime API shape may evolve from existing `NativeContext` helpers, but
must preserve these observable rules:

```rust
impl NativeContext {
    pub fn define_own_property(
        &mut self,
        object: ObjectId,
        key: String,
        descriptor: PropertyDescriptor,
    ) -> Result<bool, VmError>;

    pub fn get_property(&mut self, receiver: JsValue, key: &str) -> Result<JsValue, VmError>;
    pub fn set_property(
        &mut self,
        receiver: JsValue,
        key: impl Into<String>,
        value: JsValue,
    ) -> Result<JsValue, VmError>;
}
```

Rules:

- descriptor flags must be exact for implemented builtins;
- getters/setters must receive the correct receiver;
- property enumeration order must be deterministic and spec-aligned for
  integer-index keys, string keys, and symbol keys where implemented;
- shared object-model fixes belong in `src/runtime/`, not one-off builtin
  patches.

## 3. RegExp / Annex B / Descriptor Builtin Contract

V11-C owns JS-visible builtin semantics and descriptor sweeps.

Rules:

- RegExp and String methods must use normal builtin dispatch paths;
- Annex B behavior should be isolated and documented when intentionally partial;
- descriptor sweep changes must record before/after test deltas in the relevant
  report;
- C must coordinate with B before changing descriptor or property-order helpers.

## 4. Merge Compatibility

Recommended order:

```text
interface docs
 -> RegExp syntax/static-error metadata
 -> object-model precision helpers
 -> RegExp/Annex B/builtin descriptor sweeps
 -> focused Test262 reports
 -> V11 scan integration
```

Shared files require coordination:

- A changes to `src/lexer/`, `src/parser/`, or `src/ast/` must preserve V10-A
  in-progress numeric/source-text work.
- B changes to `src/runtime/` or `src/vm/` must coordinate with C before
  descriptor sweep work depends on new behavior.
- C changes to `src/builtins/` must not duplicate object-model logic that
  belongs in B-owned runtime helpers.
