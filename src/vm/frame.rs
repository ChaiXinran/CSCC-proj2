//! Function call frames.

/// Execution state for one bytecode function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallFrame {
    pub instruction_pointer: usize,
    pub stack_base: usize,
}

impl CallFrame {
    #[must_use]
    pub const fn new(stack_base: usize) -> Self {
        Self {
            instruction_pointer: 0,
            stack_base,
        }
    }
}
