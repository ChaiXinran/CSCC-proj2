//! Stack-based native virtual machine.

mod frame;
mod interpreter;
mod invocation;

pub use frame::{CallFrame, Completion};
pub(crate) use interpreter::evaluate_local_module;
pub use interpreter::{Vm, VmError, VmErrorKind};
pub(crate) use invocation::{
    CallRequest, ConstructRequest, FunctionEnvironmentMode, FunctionInstantiationRequest,
    InvocationOutcome,
};
