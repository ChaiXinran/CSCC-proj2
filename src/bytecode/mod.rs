//! Stack-based bytecode representation and compiler.

mod chunk;
mod compiler;
mod opcode;

pub use chunk::{Chunk, Constant};
pub use compiler::{CompileError, Compiler};
pub use opcode::Instruction;
