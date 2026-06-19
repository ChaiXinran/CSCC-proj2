//! AgentJS virtual machine instructions.

/// Number of stack values consumed and produced by one instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackEffect {
    /// Minimum stack depth required before the instruction executes.
    pub required: u32,
    pub pops: u32,
    pub pushes: u32,
}

impl StackEffect {
    #[must_use]
    pub const fn new(pops: u32, pushes: u32) -> Self {
        Self {
            required: pops,
            pops,
            pushes,
        }
    }

    #[must_use]
    pub const fn with_required(required: u32, pops: u32, pushes: u32) -> Self {
        Self {
            required,
            pops,
            pushes,
        }
    }
}

/// One decoded bytecode instruction.
///
/// Constant and name operands are indexes into [`super::Chunk::constants`].
/// Jump operands are absolute instruction offsets inside the same chunk.
/// Keeping instructions decoded makes the first implementation easy to test;
/// compact byte encoding can be added later without changing their semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Constant(u16),
    Pop,

    DeclareGlobal(u16),
    LoadGlobal(u16),
    /// Stores the top value and leaves that value on the stack.
    StoreGlobal(u16),

    UnaryPlus,
    Negate,
    LogicalNot,

    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,

    StrictEqual,
    StrictNotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    /// Observes, but does not remove, the top stack value.
    JumpIfFalse(usize),
    /// Observes, but does not remove, the top stack value.
    JumpIfTrue(usize),

    GetProperty(u16),
    /// Pops the callee and `argument_count` arguments, then pushes the result.
    Call(u16),

    Return,
    ReturnUndefined,
}

impl Instruction {
    /// Returns the instruction's fixed operand-stack contract.
    #[must_use]
    pub const fn stack_effect(self) -> StackEffect {
        match self {
            Self::Constant(_) | Self::LoadGlobal(_) => StackEffect::new(0, 1),
            Self::Pop | Self::DeclareGlobal(_) | Self::Return => StackEffect::new(1, 0),
            Self::StoreGlobal(_)
            | Self::UnaryPlus
            | Self::Negate
            | Self::LogicalNot
            | Self::GetProperty(_) => StackEffect::new(1, 1),
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Remainder
            | Self::StrictEqual
            | Self::StrictNotEqual
            | Self::LessThan
            | Self::LessThanOrEqual
            | Self::GreaterThan
            | Self::GreaterThanOrEqual => StackEffect::new(2, 1),
            Self::JumpIfFalse(_) | Self::JumpIfTrue(_) => StackEffect::with_required(1, 0, 0),
            Self::Call(argument_count) => StackEffect::new(argument_count as u32 + 1, 1),
            Self::ReturnUndefined => StackEffect::new(0, 0),
        }
    }

    #[must_use]
    pub const fn is_terminator(self) -> bool {
        matches!(self, Self::Return | Self::ReturnUndefined)
    }

    #[must_use]
    pub const fn jump_target(self) -> Option<usize> {
        match self {
            Self::JumpIfFalse(target) | Self::JumpIfTrue(target) => Some(target),
            _ => None,
        }
    }
}
