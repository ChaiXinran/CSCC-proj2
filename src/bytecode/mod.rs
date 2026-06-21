//! Stack-based bytecode representation and compiler.
//!
//! This module is the implementation boundary owned by the compiler team.
//! Its only language-level input is [`crate::ast::Program`], and its only
//! successful output is [`Chunk`]. Code in this module must not depend on the
//! lexer, parser, VM, runtime, backend, or Boa.
//!
//! Use [`Compiler::compile_program`] as the single direct entry point. The
//! cross-team [`crate::contracts::ProgramCompiler`] adapter delegates to it.

mod chunk;
mod compiler;
mod opcode;

pub use chunk::{
    Chunk, ChunkError, Constant, EnvironmentCapturePolicy, ExceptionHandler, FunctionTemplate,
    HandlerKind, StackAnalysis,
};
pub use compiler::{CompileError, Compiler};
pub use opcode::{Instruction, StackEffect};
