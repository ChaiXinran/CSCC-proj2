//! Function bytecode and constant pool.

use std::{collections::VecDeque, fmt};

use super::Instruction;

/// Structured exception-handler category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerKind {
    Catch,
    Finally,
}

/// One protected bytecode range and its handler entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionHandler {
    pub start: usize,
    pub end: usize,
    pub target: usize,
    pub kind: HandlerKind,
    pub stack_depth: u32,
    pub environment_depth: u32,
}

/// Immutable values stored in a bytecode constant pool.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    BigInt(i128),
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
    InvalidHandlerRange {
        index: usize,
        start: usize,
        end: usize,
    },
    InvalidHandlerTarget {
        index: usize,
        target: usize,
    },
    InvalidHandlerStackDepth {
        index: usize,
        depth: u32,
        max_depth: usize,
    },
    EnvironmentUnderflow {
        offset: usize,
    },
    InconsistentEnvironmentDepth {
        offset: usize,
        expected: usize,
        actual: usize,
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
            Self::InvalidHandlerRange { index, start, end } => {
                write!(
                    f,
                    "handler {index} has invalid protected range {start}..{end}"
                )
            }
            Self::InvalidHandlerTarget { index, target } => {
                write!(f, "handler {index} has invalid target {target}")
            }
            Self::InvalidHandlerStackDepth {
                index,
                depth,
                max_depth,
            } => write!(
                f,
                "handler {index} restores stack depth {depth}, above chunk maximum {max_depth}"
            ),
            Self::EnvironmentUnderflow { offset } => {
                write!(
                    f,
                    "instruction at offset {offset} pops the root environment"
                )
            }
            Self::InconsistentEnvironmentDepth {
                offset,
                expected,
                actual,
            } => write!(
                f,
                "control flow reaches offset {offset} with environment depths {expected} and {actual}"
            ),
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
    /// Formal parameter names in declaration order (excludes any rest parameter).
    pub params: Vec<String>,
    /// If the function has a rest parameter `...name`, its binding name.
    pub rest_param: Option<String>,
    /// Bytecode for the function body.
    pub chunk: Chunk,
    pub is_strict: bool,
    pub is_generator: bool,
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
    /// Structured catch/finally entries used by the V5 VM.
    pub handlers: Vec<ExceptionHandler>,
}

/// Stack requirements computed from all reachable bytecode paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StackAnalysis {
    pub max_depth: usize,
}

/// Cache-safe metadata derived from a chunk and all nested function chunks.
///
/// A successful result means the full chunk tree validates structurally and
/// contains only static bytecode data: instructions, literals, function
/// templates, and handler metadata. Runtime heap identities are deliberately
/// absent from this summary so the native script cache can store it safely.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChunkCacheMetadata {
    pub max_stack_depth: usize,
    pub total_instructions: usize,
    pub total_constants: usize,
    pub total_functions: usize,
    pub total_handlers: usize,
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
            | Instruction::JumpIfNotNullish(current)
            | Instruction::JumpIfNotUndefined(current)
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
                | Instruction::GetMethod(index)
                | Instruction::DefineDataProperty(index)
                | Instruction::DefineGetter(index)
                | Instruction::DefineSetter(index)
                | Instruction::DeleteProperty(index)
                | Instruction::CreateMutableBinding(index)
                | Instruction::CreateImmutableBinding(index)
                | Instruction::InitializeBinding(index) => Some(index),
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
                | Instruction::GetMethod(index)
                | Instruction::DefineDataProperty(index)
                | Instruction::DefineGetter(index)
                | Instruction::DefineSetter(index)
                | Instruction::DeleteProperty(index)
                | Instruction::CreateMutableBinding(index)
                | Instruction::CreateImmutableBinding(index)
                | Instruction::InitializeBinding(index) => Some(index),
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

        let analysis = self.analyze_stack()?;
        for (index, handler) in self.handlers.iter().enumerate() {
            if handler.start >= handler.end || handler.end > self.instructions.len() {
                return Err(ChunkError::InvalidHandlerRange {
                    index,
                    start: handler.start,
                    end: handler.end,
                });
            }
            if handler.target >= self.instructions.len() {
                return Err(ChunkError::InvalidHandlerTarget {
                    index,
                    target: handler.target,
                });
            }
            if handler.stack_depth as usize > analysis.max_depth {
                return Err(ChunkError::InvalidHandlerStackDepth {
                    index,
                    depth: handler.stack_depth,
                    max_depth: analysis.max_depth,
                });
            }
        }
        self.analyze_environments()?;
        Ok(())
    }

    /// Returns recursive metadata for native script caching.
    ///
    /// This validates the current chunk and every nested function chunk before
    /// returning. The result contains only cache-safe bytecode facts and no
    /// runtime object, function, or environment identities.
    pub fn cache_metadata(&self) -> Result<ChunkCacheMetadata, ChunkError> {
        self.validate()?;
        let analysis = self.analyze_stack()?;
        let mut metadata = ChunkCacheMetadata {
            max_stack_depth: analysis.max_depth,
            total_instructions: self.instructions.len(),
            total_constants: self.constants.len(),
            total_functions: self.functions.len(),
            total_handlers: self.handlers.len(),
        };

        for template in &self.functions {
            let child = template.chunk.cache_metadata()?;
            metadata.max_stack_depth = metadata.max_stack_depth.max(child.max_stack_depth);
            metadata.total_instructions += child.total_instructions;
            metadata.total_constants += child.total_constants;
            metadata.total_functions += child.total_functions;
            metadata.total_handlers += child.total_handlers;
        }

        Ok(metadata)
    }

    /// Verifies lexical-environment balance at every reachable merge.
    pub fn analyze_environments(&self) -> Result<(), ChunkError> {
        if self.instructions.is_empty() {
            return Err(ChunkError::MissingTerminator);
        }

        let mut entry_depths = vec![None; self.instructions.len()];
        let mut queue = VecDeque::from([(0usize, 0usize)]);
        for handler in &self.handlers {
            if handler.target < self.instructions.len() {
                queue.push_back((handler.target, handler.environment_depth as usize));
            }
        }

        while let Some((offset, depth)) = queue.pop_front() {
            match entry_depths[offset] {
                Some(expected) if expected != depth => {
                    return Err(ChunkError::InconsistentEnvironmentDepth {
                        offset,
                        expected,
                        actual: depth,
                    });
                }
                Some(_) => continue,
                None => entry_depths[offset] = Some(depth),
            }

            let instruction = self.instructions[offset];
            let next_depth = match instruction {
                Instruction::CreateLexicalEnvironment => depth + 1,
                Instruction::PopEnvironment if depth == 0 => {
                    return Err(ChunkError::EnvironmentUnderflow { offset });
                }
                Instruction::PopEnvironment => depth - 1,
                _ => depth,
            };

            if instruction.is_terminator() {
                continue;
            }
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
        for handler in &self.handlers {
            if handler.target < self.instructions.len() {
                queue.push_back((handler.target, handler.stack_depth as usize));
            }
        }
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
