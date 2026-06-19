//! AST-to-bytecode compiler.

use std::fmt;

use crate::ast::Program;

use super::{Chunk, Instruction};

/// Compilation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompileError {}

/// Compiles an AST into stack-based AgentJS bytecode.
#[derive(Debug, Default)]
pub struct Compiler;

impl Compiler {
    /// Compiles an empty script. Statement support is added one node at a time.
    pub fn compile(&mut self, program: &Program) -> Result<Chunk, CompileError> {
        if !program.body.is_empty() {
            return Err(CompileError {
                message: "AST node is not implemented by the bytecode compiler yet".into(),
            });
        }

        let mut chunk = Chunk::default();
        chunk.emit(Instruction::ReturnUndefined);
        Ok(chunk)
    }
}

#[cfg(test)]
mod tests {
    use crate::{ast::Program, bytecode::Instruction};

    use super::Compiler;

    #[test]
    fn compiles_empty_program() {
        let chunk = Compiler
            .compile(&Program::default())
            .expect("empty program should compile");
        assert_eq!(chunk.instructions, [Instruction::ReturnUndefined]);
    }
}
