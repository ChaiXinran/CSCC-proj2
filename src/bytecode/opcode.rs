//! AgentJS virtual machine instructions.

/// One decoded bytecode instruction.
///
/// A decoded enum keeps the initial implementation easy to test. It can later
/// be encoded into compact bytes without changing compiler or VM semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Constant(u16),
    Pop,
    DeclareGlobal(u16),
    LoadGlobal(u16),
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
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    GetProperty(u16),
    Call(u16),
    Return,
    ReturnUndefined,
}
