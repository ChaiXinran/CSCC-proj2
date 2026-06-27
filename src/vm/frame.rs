//! Function call frames and control-flow completions.

use crate::runtime::{EnvironmentId, FunctionId, JsValue};

/// Execution state for one bytecode function.
#[derive(Debug, Clone, PartialEq)]
pub struct CallFrame {
    pub function: Option<FunctionId>,
    pub return_ip: usize,
    pub environment: EnvironmentId,
    pub this_value: JsValue,
    pub stack_base: usize,
}

impl CallFrame {
    #[must_use]
    pub const fn new(
        function: Option<FunctionId>,
        return_ip: usize,
        environment: EnvironmentId,
        this_value: JsValue,
        stack_base: usize,
    ) -> Self {
        Self {
            function,
            return_ip,
            environment,
            this_value,
            stack_base,
        }
    }
}

/// Shared V5 completion model for non-local statement results.
#[derive(Debug, Clone, PartialEq)]
pub enum Completion {
    Normal(JsValue),
    Return(JsValue),
    Yield {
        value: JsValue,
        next_ip: usize,
    },
    YieldDelegate {
        iterator: JsValue,
        value: JsValue,
        next_ip: usize,
    },
    Throw(JsValue),
    Break(Option<String>),
    Continue(Option<String>),
}
