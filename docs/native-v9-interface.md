# Native V9 Shared Interface

Native V9 freezes the shared contracts for async/generator/for-of lowering,
Promise/job queue/iterator runtime, and collection/iterator builtins.

This document supplements `docs/interface-spec.md` and the V1-V8 interface
documents. If a V9 contract conflicts with older placeholder text, this V9
contract is authoritative for the new behavior.

## 1. Frontend Contract

V9-A may add AST forms for syntax that currently fails before runtime.

Expected shapes may use existing project naming, but must cover:

```rust
pub enum Statement {
    ForOf {
        left: ForBinding,
        right: Expression,
        body: Box<Statement>,
        is_await: bool,
    },
    // existing variants...
}

pub enum Expression {
    Yield {
        argument: Option<Box<Expression>>,
        delegate: bool,
    },
    Await(Box<Expression>),
    // existing variants...
}

pub struct FunctionLiteral {
    pub is_async: bool,
    pub is_generator: bool,
    // existing fields...
}
```

Rules:

- unsupported advanced forms must fail clearly, not panic;
- parser/lowering must preserve source-kind strictness from V8;
- syntax support should expose B runtime failures instead of masking them as
  parser failures.

## 2. Job Queue and Promise Contract

V9-B owns the minimal native job queue and Promise substrate.

Target runtime API shape:

```rust
pub enum Job {
    PromiseReaction(PromiseJob),
    HostCallback(NativeJob),
}

impl NativeContext {
    pub fn enqueue_job(&mut self, job: Job) -> Result<(), VmError>;
    pub fn drain_jobs(&mut self) -> Result<(), VmError>;
}
```

Minimal Promise state:

```rust
pub enum PromiseState {
    Pending,
    Fulfilled(JsValue),
    Rejected(JsValue),
}
```

Rules:

- job draining must be deterministic for Test262;
- async tests must not be marked complete before queued jobs drain;
- unimplemented Promise algorithms should throw explicit errors, not silently
  report success.

## 3. Iterator Runtime Contract

V9-B exposes shared iterator helpers for A and C:

```rust
impl NativeContext {
    pub fn get_iterator(&mut self, value: JsValue) -> Result<IteratorRecord, VmError>;
    pub fn iterator_next(&mut self, iterator: &IteratorRecord) -> Result<JsValue, VmError>;
    pub fn iterator_close(&mut self, iterator: &IteratorRecord) -> Result<(), VmError>;
}
```

Rules:

- helpers should use `Symbol.iterator` where available;
- array/string fallback may be implemented first if clearly documented;
- iterator close must run on abrupt completion once the helper exists.

## 4. Collection Builtin Contract

V9-C owns collection and iterator builtins but must use runtime helpers rather
than bypassing the object model.

Initial globals:

```text
Map
Set
WeakMap
WeakSet
Iterator
```

Rules:

- constructors/prototypes must be installed through normal builtin APIs;
- implemented methods must have honest descriptor shape;
- unsupported methods should be absent or throw explicit errors;
- collection storage can start minimal, but observable insertion order for
  `Map`/`Set` must be preserved for implemented iteration methods.

## 5. Merge Compatibility

Recommended order:

```text
interface docs
 -> AST flags and iterator/job runtime data shapes
 -> runtime helper signatures
 -> A/B/C implementation
 -> focused Test262 reports
 -> V9 scan integration
```

Shared files require coordination:

- A changes to `src/bytecode/compiler.rs` that need iterator runtime helpers
  require B review.
- C collection builtins must coordinate with B before adding storage helpers in
  `src/runtime/`.
- B job queue changes must preserve V8 Test262 async/module behavior.
