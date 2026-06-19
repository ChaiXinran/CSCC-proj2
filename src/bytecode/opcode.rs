//! AgentJS virtual machine instructions.

/// One decoded bytecode instruction.
///
/// A decoded enum keeps the initial implementation easy to test. It can later
/// be encoded into compact bytes without changing compiler or VM semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Constant(u16),
    Add,
    Return,
    ReturnUndefined,
}
