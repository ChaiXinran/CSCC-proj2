//! Function bytecode and constant pool.

use super::Instruction;

/// Immutable values stored in a bytecode constant pool.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

/// Bytecode for one script or function.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Constant>,
}

impl Chunk {
    #[must_use]
    pub fn add_constant(&mut self, constant: Constant) -> Option<u16> {
        let index = u16::try_from(self.constants.len()).ok()?;
        self.constants.push(constant);
        Some(index)
    }

    pub fn emit(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }
}
