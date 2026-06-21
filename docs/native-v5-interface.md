# Native V5 Shared Interface

This document freezes the cross-team contracts for V5. Implementations may
remain isolated behind hand-built ASTs, chunks, and runtime tests until V4 is
stable, but shared shapes must not diverge.

## 1. AST Contract

```rust
pub struct CatchClause {
    pub parameter: Option<String>,
    pub body: Vec<Statement>,
}

pub struct SwitchCase {
    pub test: Option<Expression>, // None means default
    pub consequent: Vec<Statement>,
}

pub enum Statement {
    // existing variants...
    Try {
        block: Vec<Statement>,
        handler: Option<CatchClause>,
        finalizer: Option<Vec<Statement>>,
    },
    Switch {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    },
}
```

Parser invariants:

- `try` requires a catch clause, a finally clause, or both;
- a switch contains at most one `default`;
- catch parameters are optional but, in V5 Core, must be identifiers;
- destructuring catch parameters are deferred;
- `break` is valid inside loops or switches; `continue` remains loop-only.

Existing `VariableKind::{Var, Let, Const}` remains unchanged.

## 2. Completion Contract

V5 uses one shared completion representation:

```rust
pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Throw(JsValue),
    Break(Option<String>),
    Continue(Option<String>),
}
```

Labels are reserved for later support; V5 Core emits `None`. A JavaScript
`throw` must remain `Completion::Throw` while executing language constructs.
Only the outer script/backend boundary converts an uncaught throw into a public
execution error.

`finally` rules:

1. Save the incoming completion.
2. Execute the finalizer after restoring its expected stack depth.
3. If the finalizer is normal, resume the saved completion.
4. If the finalizer is abrupt, discard and replace the saved completion.

## 3. Bytecode Contract

V5.0 introduces structured handler metadata rather than a dedicated opcode for
each source construct:

```rust
pub enum HandlerKind {
    Catch,
    Finally,
}

pub struct ExceptionHandler {
    pub start: usize,
    pub end: usize,
    pub target: usize,
    pub kind: HandlerKind,
    pub stack_depth: u32,
    pub environment_depth: u32,
}

pub struct Chunk {
    // existing fields...
    pub handlers: Vec<ExceptionHandler>,
}
```

Proposed instructions:

```rust
Instruction::CreateLexicalEnvironment
Instruction::PopEnvironment
Instruction::CreateMutableBinding(u16)
Instruction::CreateImmutableBinding(u16)
Instruction::InitializeBinding(u16)
Instruction::LoadException
Instruction::EndFinally
```

`switch` should compile with existing strict equality and jump instructions.
It must not receive a builtin or runtime-specific opcode.

`Chunk::validate()` must verify handler ranges and targets, declared stack and
environment depths, constant indexes, and that environment push/pop paths are
balanced.

## 4. Runtime Binding Contract

`Binding.initialized` becomes observable runtime state:

```rust
impl Environment {
    pub fn create_mutable_binding(&mut self, name: String, initialized: bool)
        -> Result<(), VmError>;
    pub fn create_immutable_binding(&mut self, name: String)
        -> Result<(), VmError>;
    pub fn initialize_binding(&mut self, name: &str, value: JsValue)
        -> Result<(), VmError>;
    pub fn get_binding_value(&self, name: &str) -> Result<JsValue, VmError>;
    pub fn set_mutable_binding(&mut self, name: &str, value: JsValue)
        -> Result<(), VmError>;
}
```

Rules:

- reading an uninitialized binding produces `ReferenceError`;
- assigning before initialization produces `ReferenceError`;
- assigning an initialized immutable binding produces `TypeError`;
- duplicate bindings in one lexical environment are rejected;
- leaving a block restores the previous environment on every completion path.

## 5. VM Handler Contract

Each active handler records:

```rust
pub struct HandlerFrame {
    pub target: usize,
    pub kind: HandlerKind,
    pub stack_depth: usize,
    pub environment_depth: usize,
}
```

Before entering catch/finally, the VM truncates the operand stack and restores
lexical environments to the recorded depths. Catch creates a fresh lexical
environment and initializes its optional parameter from the thrown value.

V5 code must not infer exception behavior from error-message strings.

## 6. Compatibility Rules

- Do not change V4 `JsValue`, object identity, descriptors, or builtin IDs for
  frontend/compiler-only V5 work.
- Shared AST and bytecode contract changes merge before group implementations.
- Until V4 repair finishes, C-group experiments stay in isolated files/tests
  or wait for the new baseline.
- Boa may be used for differential diagnosis, never as Native fallback.
