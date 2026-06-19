//! Function bytecode and constant pool.

use std::{collections::VecDeque, fmt};

use super::Instruction;

/// Immutable values stored in a bytecode constant pool.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

/// Error produced while constructing bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkError {
    ConstantPoolOverflow,
    FunctionTableOverflow,
    InvalidInstructionOffset {
        offset: usize,
    },
    ExpectedJumpInstruction {
        offset: usize,
    },
    InvalidConstantIndex {
        offset: usize,
        index: u16,
    },
    ExpectedStringConstant {
        offset: usize,
        index: u16,
    },
    InvalidJumpTarget {
        offset: usize,
        target: usize,
    },
    InvalidFunctionIndex {
        offset: usize,
        index: u16,
    },
    MissingTerminator,
    StackUnderflow {
        offset: usize,
        required: usize,
        available: usize,
    },
    InconsistentStackDepth {
        offset: usize,
        expected: usize,
        actual: usize,
    },
    InvalidTerminatorStackDepth {
        offset: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstantPoolOverflow => {
                f.write_str("bytecode constant pool exceeds the u16 index range")
            }
            Self::FunctionTableOverflow => {
                f.write_str("bytecode function table exceeds the u16 index range")
            }
            Self::InvalidInstructionOffset { offset } => {
                write!(f, "instruction offset {offset} is out of bounds")
            }
            Self::ExpectedJumpInstruction { offset } => {
                write!(f, "instruction at offset {offset} is not a patchable jump")
            }
            Self::InvalidConstantIndex { offset, index } => {
                write!(
                    f,
                    "instruction at offset {offset} references missing constant {index}"
                )
            }
            Self::ExpectedStringConstant { offset, index } => {
                write!(
                    f,
                    "instruction at offset {offset} requires string constant {index}"
                )
            }
            Self::InvalidJumpTarget { offset, target } => {
                write!(
                    f,
                    "instruction at offset {offset} has invalid jump target {target}"
                )
            }
            Self::InvalidFunctionIndex { offset, index } => {
                write!(
                    f,
                    "instruction at offset {offset} references missing function {index}"
                )
            }
            Self::MissingTerminator => {
                f.write_str("bytecode chunk must end with a return instruction")
            }
            Self::StackUnderflow {
                offset,
                required,
                available,
            } => write!(
                f,
                "instruction at offset {offset} requires {required} stack values, but only {available} are available"
            ),
            Self::InconsistentStackDepth {
                offset,
                expected,
                actual,
            } => write!(
                f,
                "control flow reaches offset {offset} with stack depths {expected} and {actual}"
            ),
            Self::InvalidTerminatorStackDepth {
                offset,
                expected,
                actual,
            } => write!(
                f,
                "terminator at offset {offset} requires stack depth {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ChunkError {}

// ---------------------------------------------------------------------------
// V3: Function template types
// ---------------------------------------------------------------------------

/// Whether a function captures its declaration environment (for closures).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentCapturePolicy {
    /// V3.1: function does not capture any outer environment.
    None,
    /// V3.2+: function captures the environment at declaration time.
    CaptureCurrent,
}

/// Compiled representation of one function definition, stored in the parent
/// chunk's function table and referenced by index.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTemplate {
    pub name: Option<String>,
    /// Formal parameter names, in declaration order.
    pub params: Vec<String>,
    /// Bytecode for the function body.
    pub chunk: Chunk,
    pub environment_policy: EnvironmentCapturePolicy,
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// Bytecode for one script or function.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Constant>,
    /// Compiled function bodies referenced by `CreateFunction` /
    /// `DeclareFunction` instructions.
    pub functions: Vec<FunctionTemplate>,
}

/// Stack requirements computed from all reachable bytecode paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StackAnalysis {
    pub max_depth: usize,
}

impl Chunk {
    /// Adds a constant and returns its lossless `u16` index.
    pub fn add_constant(&mut self, constant: Constant) -> Result<u16, ChunkError> {
        let index =
            u16::try_from(self.constants.len()).map_err(|_| ChunkError::ConstantPoolOverflow)?;
        self.constants.push(constant);
        Ok(index)
    }

    /// Adds a function template and returns its lossless `u16` index.
    pub fn add_function(&mut self, template: FunctionTemplate) -> Result<u16, ChunkError> {
        let index =
            u16::try_from(self.functions.len()).map_err(|_| ChunkError::FunctionTableOverflow)?;
        self.functions.push(template);
        Ok(index)
    }

    /// Appends one instruction and returns its offset.
    pub fn emit(&mut self, instruction: Instruction) -> usize {
        let offset = self.current_offset();
        self.instructions.push(instruction);
        offset
    }

    /// Returns the offset where the next instruction will be emitted.
    #[must_use]
    pub fn current_offset(&self) -> usize {
        self.instructions.len()
    }

    /// Replaces the target of an existing jump.
    pub fn patch_jump(&mut self, offset: usize, target: usize) -> Result<(), ChunkError> {
        let instruction = self
            .instructions
            .get_mut(offset)
            .ok_or(ChunkError::InvalidInstructionOffset { offset })?;

        match instruction {
            Instruction::JumpIfFalse(current)
            | Instruction::JumpIfTrue(current)
            | Instruction::Jump(current) => {
                *current = target;
                Ok(())
            }
            _ => Err(ChunkError::ExpectedJumpInstruction { offset }),
        }
    }

    /// Checks structural invariants required by the VM.
    pub fn validate(&self) -> Result<(), ChunkError> {
        for (offset, instruction) in self.instructions.iter().copied().enumerate() {
            // Check single constant indices (string or any value)
            let constant_index: Option<u16> = match instruction {
                Instruction::Constant(index)
                | Instruction::DeclareGlobal(index)
                | Instruction::LoadGlobal(index)
                | Instruction::StoreGlobal(index)
                | Instruction::GetProperty(index)
                | Instruction::TypeOfGlobal(index)
                | Instruction::DeclareLocal(index)
                | Instruction::LoadName(index)
                | Instruction::TypeOfName(index)
                | Instruction::StoreName(index)
                | Instruction::SetProperty(index)
                | Instruction::GetMethod(index) => Some(index),
                Instruction::DeclareFunction { name, .. } => Some(name),
                _ => None,
            };
            if let Some(index) = constant_index
                && usize::from(index) >= self.constants.len()
            {
                return Err(ChunkError::InvalidConstantIndex { offset, index });
            }

            // Check that name-bearing instructions have a string constant
            let name_index: Option<u16> = match instruction {
                Instruction::DeclareGlobal(index)
                | Instruction::LoadGlobal(index)
                | Instruction::StoreGlobal(index)
                | Instruction::GetProperty(index)
                | Instruction::TypeOfGlobal(index)
                | Instruction::DeclareLocal(index)
                | Instruction::LoadName(index)
                | Instruction::TypeOfName(index)
                | Instruction::StoreName(index)
                | Instruction::SetProperty(index)
                | Instruction::GetMethod(index) => Some(index),
                Instruction::DeclareFunction { name, .. } => Some(name),
                _ => None,
            };
            if let Some(index) = name_index
                && !matches!(
                    self.constants.get(usize::from(index)),
                    Some(Constant::String(_))
                )
            {
                return Err(ChunkError::ExpectedStringConstant { offset, index });
            }

            // Check function indices
            let function_index: Option<u16> = match instruction {
                Instruction::CreateFunction(f) => Some(f),
                Instruction::DeclareFunction { function, .. } => Some(function),
                _ => None,
            };
            if let Some(index) = function_index
                && usize::from(index) >= self.functions.len()
            {
                return Err(ChunkError::InvalidFunctionIndex { offset, index });
            }

            // Check jump targets
            if let Some(target) = instruction.jump_target()
                && target >= self.instructions.len()
            {
                return Err(ChunkError::InvalidJumpTarget { offset, target });
            }
        }

        if !self
            .instructions
            .last()
            .is_some_and(|instruction| instruction.is_terminator())
        {
            return Err(ChunkError::MissingTerminator);
        }

        self.analyze_stack()?;
        Ok(())
    }

    /// Verifies stack balance on every reachable control-flow path.
    pub fn analyze_stack(&self) -> Result<StackAnalysis, ChunkError> {
        if !self
            .instructions
            .last()
            .is_some_and(|instruction| instruction.is_terminator())
        {
            return Err(ChunkError::MissingTerminator);
        }

        let mut entry_depths = vec![None; self.instructions.len()];
        let mut queue = VecDeque::from([(0usize, 0usize)]);
        let mut max_depth = 0;

        while let Some((offset, depth)) = queue.pop_front() {
            match entry_depths[offset] {
                Some(expected) if expected != depth => {
                    return Err(ChunkError::InconsistentStackDepth {
                        offset,
                        expected,
                        actual: depth,
                    });
                }
                Some(_) => continue,
                None => entry_depths[offset] = Some(depth),
            }

            max_depth = max_depth.max(depth);
            let instruction = self.instructions[offset];
            let effect = instruction.stack_effect();
            let required = effect.required as usize;
            if depth < required {
                return Err(ChunkError::StackUnderflow {
                    offset,
                    required,
                    available: depth,
                });
            }

            if instruction.is_terminator() {
                let expected = match instruction {
                    Instruction::Throw => 1,
                    Instruction::Return => 1,
                    Instruction::ReturnUndefined => 0,
                    _ => unreachable!(),
                };
                if depth != expected {
                    return Err(ChunkError::InvalidTerminatorStackDepth {
                        offset,
                        expected,
                        actual: depth,
                    });
                }
                continue;
            }

            let next_depth = depth - effect.pops as usize + effect.pushes as usize;
            max_depth = max_depth.max(next_depth);

            if let Some(target) = instruction.jump_target() {
                if target >= self.instructions.len() {
                    return Err(ChunkError::InvalidJumpTarget { offset, target });
                }
                queue.push_back((target, next_depth));
            }
            if instruction.has_fallthrough() {
                if offset + 1 >= self.instructions.len() {
                    return Err(ChunkError::MissingTerminator);
                }
                queue.push_back((offset + 1, next_depth));
            }
        }

        Ok(StackAnalysis { max_depth })
    }
}
